//! Nepcamera: stands in for the church's equipment so the whole stream can be tested on
//! one computer, automatically:
//!
//! - an RTSP server like the PTZOptics camera (`rtsp://127.0.0.1:8554/1`), playing a
//!   recording of an earlier service (or a test picture),
//! - the same recording's sound as a separate stream (`/geluid`), standing in for the
//!   USB sound input, and a silent one (`/stilte`) to test the "no sound" warning,
//! - the camera's HTTP interface, logging which positions are called (`/log`),
//! - an example church website with a liturgy page (`/kerkdiensten`).

use axum::{Json, Router, extract::RawQuery, routing::get};
use gstreamer as gst;
use gstreamer_rtsp_server::prelude::*;
use gstreamer_rtsp_server::{RTSPMediaFactory, RTSPServer};
use std::sync::{Arc, Mutex};

const KERKDIENSTEN: &str = r#"<!doctype html>
<html lang="nl"><head><meta charset="utf-8"><title>Kerkdiensten - Voorbeeldgemeente</title></head>
<body><main>
<h1>Kerkdiensten</h1>
<p>Op deze pagina kunt u de liturgie voor de aankomende diensten vinden.</p>
<h2>Liturgie zondag 18 oktober 2026</h2>
<h3>Middagdienst 16:00 uur</h3>
<p>Voorzang: Ps. 55: 1<br>Ps. 49: 6<br>Ps. 16: 6<br>Ps. 73: 12-13<br>Ps. 17: 8<br>Ps. 73: 14</p>
<p>Schriftlezing: 1 Korinthe 6 vers 15 t/m 20</p>
<p>Tekst: Heidelbergse Catechismus vraag en antwoord 57</p>
<p>Thema: Dit mijn lichaam<br>1. Nu ik leef<br>2. Als ik sterf<br>3. Als Jezus terugkomt</p>
<p>Vragen<br>Hoe kijk jij naar je lichaam?</p>
<h2>Komende diensten</h2>
</main></body></html>"#;

fn main() -> anyhow::Result<()> {
    let mut file = None;
    let mut rtsp_port = "8554".to_string();
    let mut http_port = 8095u16;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bestand" => file = args.next(),
            "--rtsp-poort" => rtsp_port = args.next().unwrap_or(rtsp_port),
            "--http-poort" => http_port = args.next().and_then(|p| p.parse().ok()).unwrap_or(http_port),
            _ => {
                eprintln!("nepcamera [--bestand dienst.mp4] [--rtsp-poort 8554] [--http-poort 8095]");
                std::process::exit(2);
            }
        }
    }

    gst::init()?;
    let server = RTSPServer::new();
    server.set_service(&rtsp_port);
    let mounts = server.mount_points().unwrap();
    let source = match &file {
        Some(f) => format!("filesrc location=\"{f}\" ! decodebin name=d"),
        None => String::new(),
    };
    let video = match &file {
        Some(_) => format!("{source} d. ! queue ! videoconvert ! videoscale ! video/x-raw,width=1920,height=1080"),
        None => "videotestsrc is-live=true pattern=ball ! video/x-raw,width=1920,height=1080,framerate=25/1".into(),
    };
    let audio = match &file {
        Some(_) => format!("{source} d. ! queue ! audioconvert ! audioresample"),
        None => "audiotestsrc is-live=true wave=sine volume=0.2".into(),
    };
    let mount = |path: &str, launch: String| {
        let factory = RTSPMediaFactory::new();
        factory.set_launch(&format!("( {launch} )"));
        mounts.add_factory(path, factory);
    };
    // A file source has no audio/video branch to drop, so each mount uses only one and
    // sends the other to a fakesink.
    let (video_only, audio_only) = match &file {
        Some(_) => (
            format!("{video} ! x264enc tune=zerolatency speed-preset=ultrafast key-int-max=25 ! rtph264pay name=pay0 pt=96 d. ! queue ! fakesink sync=true"),
            format!("{audio} ! opusenc ! rtpopuspay name=pay0 pt=97 d. ! queue ! fakesink sync=true"),
        ),
        None => (
            format!("{video} ! x264enc tune=zerolatency speed-preset=ultrafast key-int-max=25 ! rtph264pay name=pay0 pt=96"),
            format!("{audio} ! opusenc ! rtpopuspay name=pay0 pt=97"),
        ),
    };
    mount("/1", video_only);
    mount("/geluid", audio_only);
    mount("/stilte", "audiotestsrc is-live=true wave=silence ! opusenc ! rtpopuspay name=pay0 pt=97".into());
    server.attach(None)?;

    let calls: Arc<Mutex<Vec<u32>>> = Arc::default();
    let log = calls.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            let app = Router::new()
                .route(
                    "/cgi-bin/ptzctrl.cgi",
                    get(move |RawQuery(q): RawQuery| async move {
                        // ptzcmd&poscall&N
                        let q = q.unwrap_or_default();
                        if let Some(n) = q.strip_prefix("ptzcmd&poscall&").and_then(|n| n.parse().ok()) {
                            println!("camera → positie {n}");
                            calls.lock().unwrap().push(n);
                        }
                        "OK"
                    }),
                )
                .route("/log", get(move || async move { Json(log.lock().unwrap().clone()) }))
                .route("/kerkdiensten", get(|| async { axum::response::Html(KERKDIENSTEN) }));
            let listener = tokio::net::TcpListener::bind(("127.0.0.1", http_port)).await.unwrap();
            axum::serve(listener, app).await.unwrap();
        });
    });

    println!("Nepcamera klaar.");
    println!("  Camerabeeld:  rtsp://127.0.0.1:{rtsp_port}/1   ({})", file.as_deref().unwrap_or("testbeeld"));
    println!("  Geluid:       rtsp://127.0.0.1:{rtsp_port}/geluid   (stil: /stilte)");
    println!("  Camera-IP:    127.0.0.1:{http_port}   (opgeroepen posities: http://127.0.0.1:{http_port}/log)");
    println!("  Website:      http://127.0.0.1:{http_port}/kerkdiensten");
    gst::glib::MainLoop::new(None, false).run();
    Ok(())
}
