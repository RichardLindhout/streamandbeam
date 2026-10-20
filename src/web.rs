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

#[derive(Clone)]
struct App {
    show: Arc<Show>,
    director: Arc<Director>,
    screen_url: String,
}

pub fn router(show: Arc<Show>, director: Arc<Director>, screen_url: String) -> Router {
    Router::new()
        .route("/", get(|| asset("index.html")))
        .route("/beam", get(|| asset("beam.html")))
        .route("/{file}", get(|Path(file): Path<String>| asset(file)))
        .route("/api/state", get(state))
        .route("/api/info", get(info))
        .route("/api/fetch", post(fetch))
        .route("/api/service", post(save_service))
        .route("/api/select", post(select))
        .route("/api/go", post(go))
        .route("/events", get(events))
        .route("/api/stream", get(stream_status).post(stream_mode))
        .route("/api/settings", get(get_settings).post(save_settings))
        .route("/preview.jpg", get(preview))
        .with_state(App { show, director, screen_url })
}

async fn asset(path: impl AsRef<str>) -> Response {
    let path = path.as_ref();
    match Assets::get(path) {
        Some(file) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            ([(header::CONTENT_TYPE, mime.as_ref())], file.data).into_response()
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

/// Every screen and operator page listens here and gets the full state on each change.
async fn events(State(app): State<App>) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = WatchStream::new(app.show.subscribe())
        .map(|view| Ok(Event::default().data(serde_json::to_string(&view).unwrap_or_default())));
    Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::new().interval(Duration::from_secs(15)))
}

async fn stream_status(State(app): State<App>) -> impl IntoResponse {
    Json(app.director.engine.status())
}

#[derive(Deserialize)]
struct ModeRequest {
    mode: Mode,
}

async fn stream_mode(State(app): State<App>, Json(req): Json<ModeRequest>) -> Response {
    let engine = app.director.engine.clone();
    let settings = app.director.settings();
    // Stopping waits for the recording to be closed, so keep it off the async threads.
    match tokio::task::spawn_blocking(move || engine.set_mode(req.mode, &settings)).await {
        Ok(Ok(())) => Json(app.director.engine.status()).into_response(),
        Ok(Err(e)) => (StatusCode::BAD_REQUEST, format!("{e:#}")).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
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
