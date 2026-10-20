//! HTTP server: the operator page, the screen page for the TVs, and live updates (SSE).

use crate::director::Director;
use crate::show::{Service, Show};
use crate::stream::{Mode, Settings};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response, Sse, sse::Event},
    routing::{get, post},
};
use futures::Stream;
use rust_embed::Embed;
use serde::Deserialize;
use std::{convert::Infallible, sync::Arc, time::Duration};
use tokio_stream::{StreamExt, wrappers::WatchStream};

#[derive(Embed)]
#[folder = "web/"]
struct Assets;

/// The fonts of the board and the stream, served to the TVs (no internet needed).
#[derive(Embed)]
#[folder = "assets/fonts/"]
struct Fonts;

async fn font(Path(file): Path<String>) -> Response {
    match Fonts::get(&file) {
        Some(f) => ([(header::CONTENT_TYPE, "font/ttf"), (header::CACHE_CONTROL, "max-age=31536000")], f.data).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

#[derive(Clone)]
struct App {
    /// How many boards (TV pages) are connected right now.
    boards: Arc<std::sync::atomic::AtomicUsize>,
    show: Arc<Show>,
    director: Arc<Director>,
    listener: Arc<crate::listen::Listener>,
    check: Arc<crate::ytcheck::YouTubeCheck>,
    screen_url: String,
    port: u16,
}

pub fn router(
    show: Arc<Show>,
    director: Arc<Director>,
    listener: Arc<crate::listen::Listener>,
    check: Arc<crate::ytcheck::YouTubeCheck>,
    screen_url: String,
    port: u16,
) -> Router {
    Router::new()
        .route("/", get(|| asset("index.html")))
        .route("/beam", get(|| asset("beam.html")))
        .route("/{file}", get(|Path(file): Path<String>| asset(file)))
        .route("/fonts/{file}", get(font))
        .route("/api/state", get(state))
        .route("/api/info", get(info))
        .route("/api/fetch", post(fetch))
        .route("/api/service", post(save_service))
        .route("/api/new-service", post(new_service))
        .route("/api/delete-service", post(delete_service))
        .route("/api/quick", post(quick))
        .route("/api/select", post(select))
        .route("/api/go", post(go))
        .route("/events", get(events))
        .route("/api/stream", get(stream_status).post(stream_mode))
        .route("/api/settings", get(get_settings).post(save_settings))
        .route("/preview.jpg", get(preview))
        .route("/api/youtube", get(youtube_status))
        .route("/api/youtube/logout", post(youtube_logout))
        .route("/youtube/login", get(youtube_login))
        .route("/youtube/callback", get(youtube_callback))
        .route("/api/listen", get(listen_status))
        .route("/api/listen/accept", post(listen_accept))
        .route("/api/board-override", post(board_override))
        .route("/vendor/{file}", get(|Path(file): Path<String>| asset(format!("vendor/{file}"))))
        .route("/preview.mjpg", get(preview_mjpeg))
        .route("/api/thumb", get(thumb))
        .route("/api/youtube-check", get(youtube_check))
        .with_state(App { boards: Arc::default(), show, director, listener, check, screen_url, port })
}

/// The read-only side for the internet (via a Cloudflare tunnel): the watch page, the board,
/// the stream preview and status. No buttons, no settings, nothing that changes anything.
pub fn public_router(
    show: Arc<Show>,
    director: Arc<Director>,
    listener: Arc<crate::listen::Listener>,
    check: Arc<crate::ytcheck::YouTubeCheck>,
    screen_url: String,
    port: u16,
) -> Router {
    Router::new()
        .route("/", get(|| asset("kijk.html")))
        .route("/beam", get(|| asset("beam.html")))
        .route("/slide.js", get(|| asset("slide.js")))
        .route("/slide.css", get(|| asset("slide.css")))
        .route("/app.css", get(|| asset("app.css")))
        .route("/hout.jpg", get(|| asset("hout.jpg")))
        .route("/fonts/{file}", get(font))
        .route("/events", get(events))
        .route("/api/state", get(state))
        .route("/api/stream", get(stream_status))
        .route("/api/youtube", get(youtube_status))
        .route("/api/listen", get(listen_status))
        .route("/preview.jpg", get(preview))
        .route("/preview.mjpg", get(preview_mjpeg))
        .route("/api/youtube-check", get(youtube_check))
        .with_state(App { boards: Arc::default(), show, director, listener, check, screen_url, port })
}

async fn asset(path: impl AsRef<str>) -> Response {
    let path = path.as_ref();
    match Assets::get(path) {
        Some(file) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            // Always the latest version: no copies in the browser or at Cloudflare, so a
            // change to the boards shows up on every screen right away.
            ([(header::CONTENT_TYPE, mime.as_ref()), (header::CACHE_CONTROL, "no-cache")], file.data).into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn state(State(app): State<App>) -> impl IntoResponse {
    Json(app.show.view())
}

async fn info(State(app): State<App>) -> impl IntoResponse {
    Json(serde_json::json!({ "screenUrl": app.screen_url }))
}

async fn fetch(State(app): State<App>) -> Response {
    match app.show.fetch().await {
        Ok(()) => Json(app.show.view()).into_response(),
        Err(e) => (StatusCode::BAD_GATEWAY, format!("{e:#}")).into_response(),
    }
}

async fn save_service(State(app): State<App>, Json(svc): Json<Service>) -> impl IntoResponse {
    app.show.save_service(svc).await;
    Json(app.show.view())
}

#[derive(Deserialize)]
struct NewService {
    date: String,
    time: String,
    label: String,
}

async fn new_service(State(app): State<App>, Json(n): Json<NewService>) -> impl IntoResponse {
    app.show.new_service(n.date, n.time, n.label).await;
    Json(app.show.view())
}

async fn delete_service(State(app): State<App>, Query(q): Query<SelectQuery>) -> StatusCode {
    app.show.delete_service(&q.id).await;
    StatusCode::NO_CONTENT
}

#[derive(Deserialize)]
struct QuickRequest {
    text: String,
}

/// Typed liturgy ("ps 42:5, gebed, joh 3:16") → items, mapped by Claude.
async fn quick(Json(q): Json<QuickRequest>) -> Response {
    match crate::claude::parse_typed(&q.text).await {
        Ok(items) => Json(items).into_response(),
        Err(e) => (StatusCode::BAD_GATEWAY, format!("{e:#}")).into_response(),
    }
}

#[derive(Deserialize)]
struct SelectQuery {
    id: String,
}

async fn select(State(app): State<App>, Query(q): Query<SelectQuery>) -> StatusCode {
    app.show.select(q.id).await;
    StatusCode::NO_CONTENT
}

#[derive(Deserialize)]
struct GoRequest {
    index: Option<usize>,
    #[serde(default)]
    delta: i64,
    #[serde(default)]
    kind: String,
}

async fn go(State(app): State<App>, Json(req): Json<GoRequest>) -> StatusCode {
    app.show.go(req.index, req.delta, &req.kind).await;
    StatusCode::NO_CONTENT
}

#[derive(Deserialize, Default)]
struct EventsQuery {
    /// "bord" for a TV page, so the operator sees how many boards are connected.
    #[serde(default)]
    rol: String,
}

/// Counts a connected board for as long as its connection lives.
struct BoardGuard(Arc<std::sync::atomic::AtomicUsize>);

impl Drop for BoardGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    }
}

/// Every screen and operator page listens here and gets the full state on each change.
async fn events(State(app): State<App>, Query(q): Query<EventsQuery>) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let guard = (q.rol == "bord").then(|| {
        app.boards.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        BoardGuard(app.boards.clone())
    });
    let stream = WatchStream::new(app.show.subscribe()).map(move |view| {
        let _keep = &guard;
        Ok(Event::default().data(serde_json::to_string(&view).unwrap_or_default()))
    });
    Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::new().interval(Duration::from_secs(15)))
}

/// The stream as it leaves the computer, as moving pictures (MJPEG) for an <img>.
async fn preview_mjpeg(State(app): State<App>) -> Response {
    let stream = futures::stream::unfold((app, Vec::new()), |(app, last)| async move {
        loop {
            tokio::time::sleep(Duration::from_millis(120)).await;
            let jpeg = app.director.engine.preview_jpeg();
            if !jpeg.is_empty() && jpeg != last {
                let mut part = format!("--frame\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n", jpeg.len()).into_bytes();
                part.extend_from_slice(&jpeg);
                part.extend_from_slice(b"\r\n");
                return Some((Ok::<_, Infallible>(axum::body::Bytes::from(part)), (app, jpeg)));
            }
        }
    });
    (
        [(header::CONTENT_TYPE, "multipart/x-mixed-replace; boundary=frame"), (header::CACHE_CONTROL, "no-cache")],
        axum::body::Body::from_stream(stream),
    )
        .into_response()
}

#[derive(Deserialize)]
struct ThumbQuery {
    index: usize,
}

/// What the stream shows for one slide, as a small picture.
async fn thumb(State(app): State<App>, Query(q): Query<ThumbQuery>) -> Response {
    let view = app.show.view();
    let director = app.director.clone();
    match tokio::task::spawn_blocking(move || director.thumbnail(&view, q.index)).await.ok().flatten() {
        Some(png) => ([(header::CONTENT_TYPE, "image/png"), (header::CACHE_CONTROL, "no-cache")], png).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn stream_status(State(app): State<App>) -> impl IntoResponse {
    let mut v = serde_json::to_value(app.director.engine.status()).unwrap_or_default();
    v["boards"] = app.boards.load(std::sync::atomic::Ordering::Relaxed).into();
    Json(v)
}

#[derive(Deserialize)]
struct ModeRequest {
    mode: Mode,
}

async fn stream_mode(State(app): State<App>, Json(req): Json<ModeRequest>) -> Response {
    match app.director.set_mode(&app.show, req.mode).await {
        Ok(()) => Json(app.director.engine.status()).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, format!("{e:#}")).into_response(),
    }
}

/// Google only sends the login back to this computer itself (localhost).
fn redirect_uri(app: &App) -> String {
    format!("http://localhost:{}/youtube/callback", app.port)
}

async fn youtube_status(State(app): State<App>) -> impl IntoResponse {
    let s = app.director.settings();
    let broadcast = app.director.broadcast();
    let status = match &broadcast {
        Some(b) => app.director.youtube.status(&s.youtube_client_id, &s.youtube_client_secret, &b.id).await.unwrap_or_default(),
        None => String::new(),
    };
    Json(serde_json::json!({
        "configured": !s.youtube_client_id.is_empty(),
        "channel": app.director.youtube.channel(),
        "broadcast": broadcast,
        "status": status,
    }))
}

async fn youtube_logout(State(app): State<App>) -> StatusCode {
    app.director.youtube.logout();
    StatusCode::NO_CONTENT
}

async fn youtube_login(State(app): State<App>) -> Response {
    let s = app.director.settings();
    if s.youtube_client_id.is_empty() {
        return (StatusCode::BAD_REQUEST, "Vul eerst de Google client-id en -secret in bij de instellingen.").into_response();
    }
    axum::response::Redirect::to(&crate::youtube::YouTube::login_url(&s.youtube_client_id, &redirect_uri(&app))).into_response()
}

#[derive(Deserialize)]
struct Callback {
    code: Option<String>,
    error: Option<String>,
}

async fn youtube_callback(State(app): State<App>, Query(c): Query<Callback>) -> Response {
    let s = app.director.settings();
    let page = |msg: String| axum::response::Html(format!("<!doctype html><meta charset=utf-8><body style=\"font:18px sans-serif;padding:40px\">{msg}<p><a href=\"/\">Terug naar de bediening</a></p>"));
    let Some(code) = c.code else {
        return page(format!("Inloggen geannuleerd ({}).", c.error.unwrap_or_default())).into_response();
    };
    match app.director.youtube.finish_login(&s.youtube_client_id, &s.youtube_client_secret, &redirect_uri(&app), &code).await {
        Ok(channel) => page(format!("Ingelogd bij YouTube als <b>{}</b>.", html_escape::encode_text(&channel))).into_response(),
        Err(e) => page(format!("Inloggen mislukt: {}", html_escape::encode_text(&format!("{e:#}")))).into_response(),
    }
}

async fn get_settings(State(app): State<App>) -> impl IntoResponse {
    Json(app.director.settings())
}

async fn save_settings(State(app): State<App>, Json(s): Json<Settings>) -> Response {
    match app.director.save_settings(s) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn preview(State(app): State<App>) -> Response {
    let jpeg = app.director.engine.preview_jpeg();
    if jpeg.is_empty() {
        return StatusCode::NO_CONTENT.into_response();
    }
    ([(header::CONTENT_TYPE, "image/jpeg"), (header::CACHE_CONTROL, "no-store")], jpeg).into_response()
}

async fn listen_status(State(app): State<App>) -> impl IntoResponse {
    Json(app.listener.status())
}

/// The operator follows the suggestion.
async fn listen_accept(State(app): State<App>) -> StatusCode {
    if let Some(s) = app.listener.status().suggestion {
        app.show.go(Some(s.slide), 0, "").await;
        app.listener.clear_suggestion();
    }
    StatusCode::NO_CONTENT
}

#[derive(Deserialize)]
struct Override {
    #[serde(default)]
    title: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    stream: bool,
    #[serde(default)]
    clear: bool,
}

/// Puts the operator's own text on the boards right away, or takes it off again.
async fn board_override(State(app): State<App>, Json(o): Json<Override>) -> StatusCode {
    let slide = (!o.clear).then(|| crate::show::Slide {
        header: o.title,
        lines: o.text.lines().map(String::from).collect(),
        ..Default::default()
    });
    app.show.set_override(slide, o.stream).await;
    StatusCode::NO_CONTENT
}

/// What viewers get on YouTube: live or not, and whether there is sound.
async fn youtube_check(State(app): State<App>) -> impl IntoResponse {
    let mut v = serde_json::to_value(app.check.status()).unwrap_or_default();
    v["channel"] = app.director.settings().youtube_channel.into();
    v["streaming"] = (app.director.engine.status().mode == crate::stream::Mode::Live).into();
    Json(v)
}
