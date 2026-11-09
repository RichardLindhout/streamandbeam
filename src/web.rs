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
        // The operator console: a React app built from ui/ into web/app/.
        .route("/", get(|| asset("app/index.html")))
        .route("/app/{*file}", get(|Path(file): Path<String>| asset(format!("app/{file}"))))
        .route("/beam", get(|| asset("beam.html")))
        .route("/{file}", get(|Path(file): Path<String>| asset(file)))
        .route("/fonts/{file}", get(font))
        .route("/api/state", get(state))
        .route("/api/info", get(info))
        .route("/api/fetch", post(fetch))
        .route("/api/service", post(save_service))
        .route("/api/website", post(website_choice))
        .route("/api/tonen", post(show_changes))
        .route("/api/new-service", post(new_service))
        .route("/api/delete-service", post(delete_service))
        .route("/api/quick", post(quick))
        .route("/api/select", post(select))
        .route("/api/go", post(go))
        .route("/events", get(events))
        .route("/api/stream", get(stream_status).post(stream_mode))
        .route("/api/stream/later", post(stream_later))
        .route("/api/settings", get(get_settings).post(save_settings))
        .route("/preview.jpg", get(preview))
        .route("/api/youtube", get(youtube_status))
        .route("/api/youtube/logout", post(youtube_logout))
        .route("/youtube/login", get(youtube_login))
        .route("/youtube/callback", get(youtube_callback))
        .route("/api/listen", get(listen_status))
        .route("/api/listen/accept", post(listen_accept))
        .route("/api/board-override", post(board_override))
        .route("/preview.mjpg", get(preview_mjpeg))
        .route("/api/thumb", get(thumb))
        .route("/api/youtube-check", get(youtube_check))
        .route("/api/scherm", post(save_layout))
        .route("/api/scherm-weg", post(delete_layout))
        .route("/api/scherm-tonen", post(show_layout))
        .route("/api/scherm-voorbeeld", post(layout_preview))
        .route("/api/afbeeldingen", get(images))
        .route("/api/afbeelding", post(upload_image).layer(axum::extract::DefaultBodyLimit::max(12 * 1024 * 1024)))
        .route("/afbeelding/{file}", get(image))
        .route("/qr.png", get(qr_png))
        .route("/bord-achtergrond", get(board_background))
        .route(
            "/api/bord-achtergrond",
            post(set_board_background)
                .delete(reset_board_background)
                .layer(axum::extract::DefaultBodyLimit::max(12 * 1024 * 1024)),
        )
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
        .route("/kijk.css", get(|| asset("kijk.css")))
        .route("/hout.jpg", get(|| asset("hout.jpg")))
        .route("/bord-achtergrond", get(board_background))
        .route("/afbeelding/{file}", get(image))
        .route("/qr.png", get(qr_png))
        .route("/fonts/{file}", get(font))
        .route("/events", get(events))
        .route("/api/state", get(state))
        .route("/api/stream", get(public_stream_status))
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

fn is_live(app: &App) -> bool {
    app.director.engine.status().mode == Mode::Live
}

fn conflict(msg: &str) -> Response {
    (StatusCode::CONFLICT, msg.to_string()).into_response()
}

/// `?bevestigd=1`: the operator confirmed an action that changes what viewers see.
#[derive(Deserialize, Default)]
struct Confirmed {
    #[serde(default)]
    bevestigd: Option<String>,
}

impl Confirmed {
    fn yes(&self) -> bool {
        matches!(self.bevestigd.as_deref(), Some("1" | "true" | "ja"))
    }
}

/// While live, an action that changes what viewers see first needs a confirmation: the
/// answer is 409 with the question (e.g. "Een andere dienst kiezen?") and the header
/// `X-Bevestigen`, and the console asks and sends it again with `?bevestigd=1`.
fn needs_confirmation(app: &App, confirmed: bool, what: &str) -> Option<Response> {
    (is_live(app) && !confirmed).then(|| (StatusCode::CONFLICT, [("X-Bevestigen", "1")], what.to_string()).into_response())
}

async fn fetch(State(app): State<App>, Query(c): Query<Confirmed>) -> Response {
    if let Some(r) = needs_confirmation(&app, c.yes(), "Diensten opnieuw van de website ophalen?") {
        return r;
    }
    let live = || is_live(&app);
    match app.show.fetch(live).await {
        Ok(()) => Json(app.show.view()).into_response(),
        Err(e) => (StatusCode::BAD_GATEWAY, format!("{e:#}")).into_response(),
    }
}

#[derive(Deserialize, Default)]
struct SaveQuery {
    /// "1": also put the change on the screens now (else they keep what they show).
    #[serde(default)]
    tonen: Option<String>,
}

async fn save_service(State(app): State<App>, Query(q): Query<SaveQuery>, Query(c): Query<Confirmed>, Json(svc): Json<Service>) -> Response {
    let show_now = q.tonen.as_deref() == Some("1");
    if show_now
        && svc.id == app.show.view().current
        && let Some(r) = needs_confirmation(&app, c.yes(), "De wijziging nu op het scherm zetten?")
    {
        return r;
    }
    app.show.save_service(svc, show_now).await;
    Json(app.show.view()).into_response()
}

/// Puts the changed version of what is on screen on the screens.
async fn show_changes(State(app): State<App>, Query(c): Query<Confirmed>) -> Response {
    if let Some(r) = needs_confirmation(&app, c.yes(), "De wijziging nu op het scherm zetten?") {
        return r;
    }
    app.show.show_changes().await;
    StatusCode::NO_CONTENT.into_response()
}

#[derive(Deserialize)]
struct WebsiteChoice {
    id: String,
    /// true = take over the website's version, false = ignore it.
    take: bool,
}

/// The website changed an edited service: take its version over or ignore it.
async fn website_choice(State(app): State<App>, Json(c): Json<WebsiteChoice>) -> StatusCode {
    app.show.website_choice(&c.id, c.take).await;
    StatusCode::NO_CONTENT
}

#[derive(Deserialize)]
struct NewService {
    date: String,
    time: String,
    label: String,
}

async fn new_service(State(app): State<App>, Query(c): Query<Confirmed>, Json(n): Json<NewService>) -> Response {
    let id = format!("{} {}", n.date, n.time);
    if app.show.view().services.iter().any(|s| s.id == id) {
        return conflict("Er is al een dienst op dat moment");
    }
    if let Some(r) = needs_confirmation(&app, c.yes(), "Een nieuwe dienst maken en op het scherm zetten?") {
        return r;
    }
    app.show.new_service(n.date, n.time, n.label).await;
    Json(app.show.view()).into_response()
}

async fn delete_service(State(app): State<App>, Query(q): Query<SelectQuery>, Query(c): Query<Confirmed>) -> Response {
    if app.show.view().current == q.id
        && let Some(r) = needs_confirmation(&app, c.yes(), "De dienst die nu live is verwijderen?")
    {
        return r;
    }
    app.show.delete_service(&q.id).await;
    StatusCode::NO_CONTENT.into_response()
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

async fn select(State(app): State<App>, Query(q): Query<SelectQuery>, Query(c): Query<Confirmed>) -> Response {
    if q.id != app.show.view().current
        && let Some(r) = needs_confirmation(&app, c.yes(), "Een andere dienst kiezen?")
    {
        return r;
    }
    app.show.select(q.id).await;
    StatusCode::NO_CONTENT.into_response()
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
    // When the stream starts and stops by itself.
    if let (Some(obj), serde_json::Value::Object(auto)) =
        (v.as_object_mut(), app.director.autopilot_status(&app.show.view(), &app.director.settings()))
    {
        obj.extend(auto);
    }
    Json(v)
}

/// For the watch page on the internet: only whether it runs and the sound, never error
/// texts (they can contain the stream key).
async fn public_stream_status(State(app): State<App>) -> impl IntoResponse {
    let st = app.director.engine.status();
    Json(serde_json::json!({ "mode": st.mode, "audioDb": st.audio_db, "audioWarning": st.audio_warning }))
}

/// Five more minutes on the end screen before the stream stops by itself.
async fn stream_later(State(app): State<App>) -> Response {
    match app.director.later() {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => conflict(&format!("{e:#}")),
    }
}

#[derive(Deserialize)]
struct ModeRequest {
    mode: Mode,
    /// Stopping while live was confirmed (also possible as `?bevestigd=1`).
    #[serde(default)]
    bevestigd: bool,
}

async fn stream_mode(State(app): State<App>, Query(c): Query<Confirmed>, Json(req): Json<ModeRequest>) -> Response {
    let what = match req.mode {
        Mode::Voorbeeld => "Uitzending stoppen en alleen voorbeeld tonen?",
        Mode::Opnemen => "Uitzending stoppen en alleen opnemen?",
        // Stopping is confirmed in the console itself (and the testdienst stops directly).
        Mode::Stop | Mode::Live => "",
    };
    if !what.is_empty()
        && let Some(r) = needs_confirmation(&app, c.yes() || req.bevestigd, what)
    {
        return r;
    }
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

/// The background of the boards: the church's own picture, or the standard wood.
async fn board_background(State(app): State<App>) -> Response {
    if let Some(file) = app.show.board_background() {
        let ext = file.extension().and_then(|e| e.to_str()).unwrap_or_default().to_string();
        let mime = crate::show::BACKGROUND_TYPES.iter().find(|(e, _)| *e == ext).map(|(_, m)| *m).unwrap_or("image/jpeg");
        if let Ok(data) = tokio::fs::read(&file).await {
            return ([(header::CONTENT_TYPE, mime), (header::CACHE_CONTROL, "no-cache")], data).into_response();
        }
    }
    asset("hout.jpg").await
}

async fn remove_board_background(app: &App) {
    for (ext, _) in crate::show::BACKGROUND_TYPES {
        let _ = tokio::fs::remove_file(app.show.data_dir().join(format!("bord-achtergrond.{ext}"))).await;
    }
}

/// A new background picture for the boards (the raw image as the request body).
async fn set_board_background(State(app): State<App>, headers: axum::http::HeaderMap, body: axum::body::Bytes) -> Response {
    let mime = headers.get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or_default();
    let Some((ext, _)) = crate::show::BACKGROUND_TYPES.iter().find(|(_, m)| mime.starts_with(m)) else {
        return (StatusCode::UNSUPPORTED_MEDIA_TYPE, "Kies een JPG-, PNG- of WebP-afbeelding.").into_response();
    };
    if body.is_empty() {
        return (StatusCode::BAD_REQUEST, "Lege afbeelding.").into_response();
    }
    remove_board_background(&app).await;
    if let Err(e) = tokio::fs::write(app.show.data_dir().join(format!("bord-achtergrond.{ext}")), &body).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, format!("Opslaan mislukt: {e}")).into_response();
    }
    app.show.board_background_changed().await;
    StatusCode::NO_CONTENT.into_response()
}

/// Back to the standard wood.
async fn reset_board_background(State(app): State<App>) -> StatusCode {
    remove_board_background(&app).await;
    app.show.board_background_changed().await;
    StatusCode::NO_CONTENT
}

/// Adds or changes one of the church's own screens (see `layout`).
async fn save_layout(State(app): State<App>, Query(c): Query<Confirmed>, Json(layout): Json<crate::layout::Layout>) -> Response {
    // Only a screen that is in use changes what people see.
    let view = app.show.view();
    let in_use = !layout.moment.is_empty() || [&view.board_layout, &view.stream_layout].contains(&&layout.id);
    if in_use && let Some(r) = needs_confirmation(&app, c.yes(), "Dit scherm wijzigen?") {
        return r;
    }
    match app.show.save_layout(layout).await {
        Ok(saved) => Json(saved).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Opslaan mislukt: {e:#}")).into_response(),
    }
}

async fn delete_layout(State(app): State<App>, Query(q): Query<SelectQuery>, Query(c): Query<Confirmed>) -> Response {
    if let Some(r) = needs_confirmation(&app, c.yes(), "Dit scherm verwijderen?") {
        return r;
    }
    match app.show.delete_layout(&q.id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Verwijderen mislukt: {e:#}")).into_response(),
    }
}

#[derive(Deserialize)]
struct ShowLayout {
    id: String,
    /// true = put it up, false = take it off.
    aan: bool,
}

/// Puts one of the church's own screens up by hand (boards or stream), or takes it off.
async fn show_layout(State(app): State<App>, Query(c): Query<Confirmed>, Json(req): Json<ShowLayout>) -> Response {
    if req.aan && let Some(r) = needs_confirmation(&app, c.yes(), "Dit scherm nu tonen?") {
        return r;
    }
    if app.show.show_layout(&req.id, req.aan).await {
        StatusCode::NO_CONTENT.into_response()
    } else {
        (StatusCode::NOT_FOUND, "Dit scherm bestaat niet (meer).").into_response()
    }
}

#[derive(Deserialize)]
struct LayoutPreview {
    layout: crate::layout::Layout,
    #[serde(default)]
    fields: std::collections::BTreeMap<String, String>,
}

/// A screen for the stream as the program itself draws it, with example texts (PNG).
async fn layout_preview(State(app): State<App>, Json(req): Json<LayoutPreview>) -> Response {
    let director = app.director.clone();
    let draw = move || director.layout_preview(req.layout, req.fields.into_iter().collect());
    match tokio::task::spawn_blocking(draw).await.ok().flatten() {
        Some(png) => ([(header::CONTENT_TYPE, "image/png"), (header::CACHE_CONTROL, "no-store")], png).into_response(),
        None => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}

/// Pictures built into the program, usable in the church's own screens.
const BUILTIN_IMAGES: [(&str, &[u8]); 3] = [
    ("buiten.jpg", include_bytes!("../assets/images/buiten.jpg")),
    ("interieur.jpg", include_bytes!("../assets/images/interieur.jpg")),
    ("logo-wit.png", include_bytes!("../assets/images/logo-wit.png")),
];

fn image_dir(app: &App) -> std::path::PathBuf {
    app.show.data_dir().join("afbeeldingen")
}

/// The names of the pictures: the church's own first, then the built-in ones.
async fn images(State(app): State<App>) -> impl IntoResponse {
    let mut names: Vec<String> = Vec::new();
    if let Ok(mut dir) = tokio::fs::read_dir(image_dir(&app)).await {
        while let Ok(Some(entry)) = dir.next_entry().await {
            let name = entry.file_name().to_string_lossy().to_string();
            if crate::layout::safe_name(&name).as_deref() == Some(name.as_str()) {
                names.push(name);
            }
        }
    }
    names.sort_by_key(|n| n.to_lowercase());
    let builtin: Vec<String> = BUILTIN_IMAGES.iter().map(|(n, _)| n.to_string()).filter(|n| !names.contains(n)).collect();
    names.extend(builtin);
    Json(names)
}

async fn image(State(app): State<App>, Path(file): Path<String>) -> Response {
    let Some(name) = crate::layout::safe_name(&file) else { return StatusCode::NOT_FOUND.into_response() };
    let mime = mime_guess::from_path(&name).first_or_octet_stream();
    let data = match tokio::fs::read(image_dir(&app).join(&name)).await {
        Ok(data) => data,
        Err(_) => match BUILTIN_IMAGES.iter().find(|(n, _)| *n == name) {
            Some((_, data)) => data.to_vec(),
            None => return StatusCode::NOT_FOUND.into_response(),
        },
    };
    ([(header::CONTENT_TYPE, mime.as_ref()), (header::CACHE_CONTROL, "no-cache")], data).into_response()
}

#[derive(Deserialize)]
struct ImageName {
    #[serde(default)]
    naam: String,
}

/// A new picture for the church's own screens (the raw image as the request body). The
/// stream draws JPG and PNG.
async fn upload_image(State(app): State<App>, Query(q): Query<ImageName>, headers: axum::http::HeaderMap, body: axum::body::Bytes) -> Response {
    let mime = headers.get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or_default();
    let ext = if mime.starts_with("image/png") {
        "png"
    } else if mime.starts_with("image/jpeg") {
        "jpg"
    } else {
        return (StatusCode::UNSUPPORTED_MEDIA_TYPE, "Kies een JPG- of PNG-afbeelding.").into_response();
    };
    if body.is_empty() || image::load_from_memory(&body).is_err() {
        return (StatusCode::BAD_REQUEST, "Dit bestand is geen leesbare afbeelding.").into_response();
    }
    let stem = q.naam.rsplit_once('.').map(|(s, _)| s.to_string()).unwrap_or(q.naam.clone());
    let Some(name) = crate::layout::safe_name(&format!("{}.{ext}", if stem.trim().is_empty() { "afbeelding" } else { stem.trim() })) else {
        return (StatusCode::BAD_REQUEST, "Geef de afbeelding een naam.").into_response();
    };
    let dir = image_dir(&app);
    if let Err(e) = tokio::fs::create_dir_all(&dir).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, format!("Opslaan mislukt: {e}")).into_response();
    }
    if let Err(e) = tokio::fs::write(dir.join(&name), &body).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, format!("Opslaan mislukt: {e}")).into_response();
    }
    Json(serde_json::json!({ "name": name })).into_response()
}

/// The QR code of the settings as a picture, for the boards and the editor.
async fn qr_png(State(app): State<App>) -> Response {
    let link = app.director.settings().qr_link;
    let Ok(code) = qrcode::QrCode::new(if link.is_empty() { "https://www.hhgtholen.nl" } else { link.as_str() }) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let (n, quiet, cell) = (code.width() as u32, 2u32, 10u32);
    let dark = code.to_colors();
    let side = (n + 2 * quiet) * cell;
    let img = image::GrayImage::from_fn(side, side, |x, y| {
        let (cx, cy) = ((x / cell) as i64 - quiet as i64, (y / cell) as i64 - quiet as i64);
        let on = cx >= 0 && cy >= 0 && cx < n as i64 && cy < n as i64 && dark[(cy * n as i64 + cx) as usize] == qrcode::Color::Dark;
        image::Luma([if on { 16 } else { 255 }])
    });
    let mut png = Vec::new();
    if image::DynamicImage::ImageLuma8(img).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    ([(header::CONTENT_TYPE, "image/png"), (header::CACHE_CONTROL, "no-cache")], png).into_response()
}
