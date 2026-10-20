//! Follows the operator: for every slide it picks the scene for the YouTube stream and
//! moves the camera, plays the short intros the church's stream has (organ before a
//! psalm, the theme before the sermon), and can start and stop the stream by itself.

use crate::render::{Body, Photo, Renderer, Scene};
use crate::show::{Show, View, long_date};
use crate::stream::{Engine, Mode, Settings, recall_preset};
use crate::texts::{read_json, write_json};
use crate::youtube::{Broadcast, YouTube};
use chrono::{Local, NaiveDateTime};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub struct Director {
    pub engine: Arc<Engine>,
    pub youtube: YouTube,
    settings: Mutex<Settings>,
    settings_changed: tokio::sync::Notify,
    path: PathBuf,
    /// The YouTube broadcast of the current live stream.
    broadcast: Mutex<Option<Broadcast>>,
    /// Draws the stream scenes; shared with the thumbnails of the operator page.
    renderer: Mutex<Option<Arc<Mutex<Renderer>>>>,
    /// Thumbnails already made, by scene.
    thumbs: Mutex<std::collections::HashMap<String, Vec<u8>>>,
}

impl Director {
    pub fn new(engine: Arc<Engine>, data_dir: &std::path::Path) -> Arc<Director> {
        let path = data_dir.join("instellingen.json");
        let settings = read_json(&path).unwrap_or_default();
        Arc::new(Director {
            engine,
            youtube: YouTube::new(data_dir.join("youtube.json")),
            settings: Mutex::new(settings),
            settings_changed: tokio::sync::Notify::new(),
            path,
            broadcast: Mutex::new(None),
            renderer: Mutex::new(None),
            thumbs: Mutex::new(std::collections::HashMap::new()),
        })
    }

    /// A small picture of what the stream shows for a slide (the overlay over a grey
    /// stand-in for the camera), for the clickable strip in the operator page.
    pub fn thumbnail(&self, view: &View, index: usize) -> Option<Vec<u8>> {
        let mut v = view.clone();
        v.index = index;
        v.override_board = None;
        let (scene, _, _) = scene_for(&v, Duration::from_secs(3600), &self.settings());
        let key = format!("{scene:?}");
        if let Some(png) = self.thumbs.lock().unwrap().get(&key) {
            return Some(png.clone());
        }
        let renderer = self.renderer.lock().unwrap().clone()?;
        let rgba = renderer.lock().unwrap().render(&scene);
        let overlay = image::RgbaImage::from_raw(crate::render::WIDTH, crate::render::HEIGHT, rgba)?;
        let mut img = image::RgbaImage::from_fn(crate::render::WIDTH, crate::render::HEIGHT, |_, y| {
            let g = 70 + (y * 40 / crate::render::HEIGHT) as u8;
            image::Rgba([g, g + 4, g + 8, 255])
        });
        image::imageops::overlay(&mut img, &overlay, 0, 0);
        let small = image::imageops::resize(&img, 384, 216, image::imageops::FilterType::Triangle);
        let mut png = Vec::new();
        small.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).ok()?;
        let mut cache = self.thumbs.lock().unwrap();
        if cache.len() > 300 {
            cache.clear();
        }
        cache.insert(key, png.clone());
        Some(png)
    }

    pub fn broadcast(&self) -> Option<Broadcast> {
        self.broadcast.lock().unwrap().clone()
    }

    /// Switches the stream mode. Going live while logged in to YouTube first creates the
    /// broadcast for the current service (title and description from the liturgy); without
    /// a login the stream key from the settings is used.
    pub async fn set_mode(&self, show: &Show, mode: Mode) -> anyhow::Result<()> {
        let mut settings = self.settings();
        let was_live = self.engine.status().mode == Mode::Live;
        if mode == Mode::Live && self.youtube.channel().is_some() {
            let view = show.view();
            let svc = view.service().cloned().ok_or_else(|| anyhow::anyhow!("kies eerst een dienst"))?;
            let privacy = if settings.youtube_test { "private" } else { settings.youtube_privacy.as_str() };
            let b = self
                .youtube
                .broadcast_for(&settings.youtube_client_id, &settings.youtube_client_secret, &svc, privacy, &svc.youtube_id, settings.youtube_test)
                .await?;
            println!("YouTube-uitzending: {} ({})", b.title, b.watch_url);
            show.set_youtube_id(&svc.id, &b.id).await;
            settings.rtmp_url = b.rtmp_url.clone();
            settings.stream_key = b.stream_key.clone();
            *self.broadcast.lock().unwrap() = Some(b);
        }
        let engine = self.engine.clone();
        tokio::task::spawn_blocking(move || engine.set_mode(mode, &settings)).await??;
        if was_live && mode != Mode::Live {
            let b = self.broadcast.lock().unwrap().take();
            if let Some(b) = b {
                let s = self.settings();
                if let Err(e) = self.youtube.complete(&s.youtube_client_id, &s.youtube_client_secret, &b.id).await {
                    eprintln!("YouTube-uitzending afsluiten: {e:#}");
                }
            }
        }
        Ok(())
    }

    pub fn settings(&self) -> Settings {
        self.settings.lock().unwrap().clone()
    }

    pub fn save_settings(&self, s: Settings) -> anyhow::Result<()> {
        write_json(&self.path, &s)?;
        *self.settings.lock().unwrap() = s;
        self.settings_changed.notify_one();
        Ok(())
    }

    /// Runs for the lifetime of the program.
    pub async fn run(self: Arc<Self>, show: Arc<Show>) {
        tokio::spawn(self.clone().autopilot(show.clone()));
        let qr_link = self.settings().qr_link;
        let renderer = Arc::new(Mutex::new(Renderer::with_builtin_images(&qr_link)));
        *self.renderer.lock().unwrap() = Some(renderer.clone());
        let mut rx = show.subscribe();
        let mut last_scene = None;
        // Camera and preset last sent, so a newly entered camera IP also gets the position.
        let mut last_sent = (String::new(), 0);
        // The slide on screen and since when, for the timed intros.
        let mut on_screen = (usize::MAX, String::new(), Instant::now());
        loop {
            let view = rx.borrow_and_update().clone();
            let settings = self.settings();
            let key = view.slides.get(view.index).map(|s| s.header.clone()).unwrap_or_default();
            if on_screen.0 != view.index || on_screen.1 != key {
                on_screen = (view.index, key, Instant::now());
            }
            let (scene, position, until) = scene_for(&view, on_screen.2.elapsed(), &settings);
            if last_scene.as_ref() != Some(&scene) {
                last_scene = Some(scene.clone());
                let r = renderer.clone();
                if let Ok(pixels) = tokio::task::spawn_blocking(move || r.lock().unwrap().render(&scene)).await {
                    self.engine.set_overlay(pixels);
                }
            }
            let preset = match position {
                Position::Overzicht => settings.presets.overzicht,
                Position::Orgel => settings.presets.orgel,
                Position::Preekstoel => settings.presets.preekstoel,
                Position::Doopvont => settings.presets.doopvont,
            };
            if !settings.camera_ip.is_empty() && (settings.camera_ip.clone(), preset) != last_sent {
                last_sent = (settings.camera_ip.clone(), preset);
                tokio::spawn(async move { recall_preset(&settings.camera_ip, preset).await });
            }
            // Wake up when the slide changes, the settings change, or a timed intro ends.
            let wait = until.map(|u| u.saturating_sub(on_screen.2.elapsed())).unwrap_or(Duration::from_secs(3600));
            tokio::select! {
                changed = rx.changed() => if changed.is_err() { return },
                _ = self.settings_changed.notified() => {}
                _ = tokio::time::sleep(wait) => {}
            }
        }
    }

    /// Goes live before a service and stops after the end screen, when switched on.
    async fn autopilot(self: Arc<Self>, show: Arc<Show>) {
        let mut started: Option<String> = None;
        let mut end_since: Option<Instant> = None;
        loop {
            tokio::time::sleep(Duration::from_secs(10)).await;
            let s = self.settings();
            let view = show.view();
            let mode = self.engine.status().mode;
            let on_end = view.slides.get(view.index).is_some_and(|sl| sl.kind == "einde");
            end_since = if on_end { end_since.or(Some(Instant::now())) } else { None };

            if s.auto_start_minutes > 0 && mode == Mode::Stop {
                let now = Local::now().naive_local();
                let lead = chrono::Duration::minutes(s.auto_start_minutes as i64);
                let due = view.services.iter().find(|svc| {
                    let Ok(start) = NaiveDateTime::parse_from_str(&svc.id, "%Y-%m-%d %H:%M") else { return false };
                    now >= start - lead && now <= start + chrono::Duration::minutes(30) && started.as_deref() != Some(&svc.id)
                });
                if let Some(svc) = due {
                    println!("automatisch live voor {}", svc.id);
                    started = Some(svc.id.clone());
                    show.select(svc.id.clone()).await;
                    if let Err(e) = self.set_mode(&show, Mode::Live).await {
                        eprintln!("automatisch live gaan mislukt: {e:#}");
                    }
                }
            }
            if s.auto_stop_minutes > 0
                && mode == Mode::Live
                && end_since.is_some_and(|t| t.elapsed() >= Duration::from_secs(s.auto_stop_minutes * 60))
            {
                println!("automatisch gestopt na het eindscherm");
                if let Err(e) = self.set_mode(&show, Mode::Stop).await {
                    eprintln!("automatisch stoppen mislukt: {e:#}");
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Position {
    Overzicht,
    Orgel,
    Preekstoel,
    Doopvont,
}

/// The stream scene and camera position for what the screens show now, and until when
/// that holds (for timed intros), measured from when the slide appeared.
fn scene_for(view: &View, shown: Duration, s: &Settings) -> (Scene, Position, Option<Duration>) {
    // The operator's own text, when it should go into the stream too.
    if let Some(o) = view.override_board.as_ref().filter(|_| view.override_stream) {
        let scene = Scene::Panel { label: String::new(), title: o.header.clone(), paragraphs: o.lines.clone(), style: Body::Prose, footer: String::new(), qr: false };
        return (scene, Position::Preekstoel, None);
    }
    let Some(slide) = view.slides.get(view.index) else {
        return (Scene::Camera, Position::Overzicht, None);
    };
    let svc = view.service();
    let panel = |label: &str, title: &str, paragraphs: Vec<String>, style: Body, footer: &str, qr: bool| Scene::Panel {
        label: label.to_string(),
        title: title.to_string(),
        paragraphs,
        style,
        footer: footer.to_string(),
        qr,
    };
    let bar = |label: &str, text: &str| Scene::Bar { label: label.to_string(), text: text.to_string() };
    let intro = Duration::from_secs(s.intro_seconds);
    let theme = Duration::from_secs(s.theme_seconds);
    match slide.kind.as_str() {
        "welkom" => {
            let title = svc.map(|s| long_date(&s.date)).unwrap_or_default();
            let sub = svc
                .map(|s| [s.label.as_str(), s.predikant.as_str()].iter().filter(|x| !x.is_empty()).copied().collect::<Vec<_>>().join(" · "))
                .unwrap_or_default();
            (Scene::Holding { photo: Photo::Inside, label: "Welkom".into(), title, sub, qr: true }, Position::Overzicht, None)
        }
        "einde" => {
            // svc.next is "Zondag 25 oktober om 9.30 uur · ds. K. Klopstra".
            let next = svc.map(|s| s.next.clone()).unwrap_or_default();
            let (title, sub) = next.split_once(" · ").map(|(a, b)| (a.to_string(), b.to_string())).unwrap_or((next.clone(), String::new()));
            let scene = if title.is_empty() {
                Scene::Holding { photo: Photo::Outside, label: String::new(), title: "Hersteld Hervormde Gemeente Tholen".into(), sub: "www.hhgtholen.nl".into(), qr: true }
            } else {
                Scene::Holding { photo: Photo::Outside, label: "Volgende dienst".into(), title, sub, qr: true }
            };
            (scene, Position::Overzicht, None)
        }
        // The predikant announces the psalm: the camera stays on him, the psalm below him.
        "psalm" | "slotzang" | "nazang" if slide.lines.is_empty() => (bar(&slide.header, &slide.sub), Position::Preekstoel, None),
        "psalm" | "slotzang" | "nazang" => {
            // The first verse of a psalm starts with the announcement: the predikant in view
            // with the psalm below him; then the verse, with the organ in the opening.
            let prev = view.index.checked_sub(1).and_then(|i| view.slides.get(i));
            let first_verse = prev.is_none_or(|p| p.item != slide.item || p.lines.is_empty());
            if first_verse && shown < intro {
                (bar(&slide.sub, &slide.header), Position::Preekstoel, Some(intro))
            } else {
                (panel(&slide.sub, &slide.header, slide.lines.clone(), Body::Lines, "", false), Position::Orgel, None)
            }
        }
        "lezing" => {
            let verses = if slide.verses.is_empty() {
                slide.lines.clone()
            } else {
                slide.verses.iter().map(|v| format!("{} {}", v.number, v.text)).collect()
            };
            let style = if slide.verses.is_empty() { Body::Prose } else { Body::Verses };
            (panel(&slide.sub, &slide.header, verses, style, "", false), Position::Preekstoel, None)
        }
        "catechismus" => {
            // lines: "Vraag 57: Wat troost …", "Antwoord: Dat niet alleen …"
            let (title, question) = slide.lines.first().and_then(|q| q.split_once(": ")).unwrap_or(("", ""));
            let mut paragraphs = vec![question.to_string()];
            paragraphs.extend(slide.lines.iter().skip(1).map(|l| l.trim_start_matches("Antwoord: ").to_string()));
            let title = if title.is_empty() { slide.header.as_str() } else { title };
            (panel(&slide.header, title, paragraphs, Body::Question, &slide.footer, false), Position::Preekstoel, None)
        }
        "collecte" => (panel("", &slide.header, slide.lines.clone(), Body::Lines, "", true), Position::Orgel, None),
        "gebed" => (bar("", &slide.header), Position::Doopvont, None),
        // A named part without text (levensloop, zegen, …): only its name on a card.
        "mededeling" if slide.lines.iter().all(|l| l.trim().is_empty()) => (bar("", &slide.header), Position::Preekstoel, None),
        "mededeling" => (panel("", &slide.header, slide.lines.clone(), Body::Prose, "", false), Position::Preekstoel, None),
        "belijdenis" | "formulier" => (panel(&slide.sub, &slide.header, slide.lines.clone(), Body::Prose, "", false), Position::Preekstoel, None),
        "lied" if slide.lines.is_empty() => (bar("Lied", &slide.header), Position::Orgel, None),
        "lied" => (panel("Lied", &slide.header, slide.lines.clone(), Body::Lines, "", false), Position::Orgel, None),
        "preek" if !slide.theme.is_empty() && shown < theme => {
            let points = slide.points.iter().enumerate().map(|(i, p)| format!("{}. {p}", i + 1)).collect();
            (panel("Thema", &slide.theme, points, Body::Lines, "", false), Position::Preekstoel, Some(theme))
        }
        // During the sermon viewers see the predikant.
        _ => (Scene::Camera, Position::Preekstoel, None),
    }
}
