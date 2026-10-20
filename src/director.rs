//! Follows the operator: for every slide it picks the scene for the YouTube stream and
//! moves the camera, plays the short intros the church's stream has (organ before a
//! psalm, the theme before the sermon), and can start and stop the stream by itself:
//! live with the start screen some minutes before a service, and stopped a while after
//! the end screen (as the church does: blessing, empty pulpit, end screen, stop).

use crate::render::{Body, Photo, Renderer, Scene};
use crate::show::{Service, Show, View, long_date, long_date_no_year};
use crate::stream::{Engine, Mode, Settings, recall_preset};
use crate::texts::{read_json, write_json};
use crate::youtube::{Broadcast, YouTube};
use chrono::{Local, NaiveDate, NaiveDateTime, NaiveTime};
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
    autopilot: Mutex<Autopilot>,
    /// Where `Autopilot::started` is kept, so a restart does not go live again.
    autopilot_path: PathBuf,
}

/// What the automatic start and stop know (times in Unix seconds).
#[derive(Debug, Default)]
struct Autopilot {
    /// The service that already went live (automatically or by hand): never again by itself.
    started: String,
    /// Why the last automatic start failed; empty when it worked.
    start_error: String,
    /// When the end screen (church photo) appeared; 0 = not on the end screen.
    end_card_at: i64,
    /// Extra time before stopping, added with "later".
    extra_secs: u64,
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
struct SavedAutopilot {
    started: String,
}

/// The camera stays on the pulpit this long after "Aftiteling" (the predikant walks away,
/// the organ starts), then the end screen comes.
const PULPIT_HOLD: Duration = Duration::from_secs(20);
/// The end screen changes sentence this often.
const SWAP: Duration = Duration::from_secs(8);
/// After this long on the blessing, the end screen comes by itself.
const ZEGEN_MAX: Duration = Duration::from_secs(600);

fn service_start(svc: &Service) -> Option<NaiveDateTime> {
    NaiveDateTime::parse_from_str(&svc.id, "%Y-%m-%d %H:%M").ok()
}

/// The earliest service with `now` between `lead_minutes` before its start and 30 minutes after.
fn service_in_window(services: &[Service], now: NaiveDateTime, lead_minutes: u64) -> Option<&Service> {
    let lead = chrono::Duration::minutes(lead_minutes as i64);
    services
        .iter()
        .filter_map(|svc| service_start(svc).map(|start| (start, svc)))
        .filter(|(start, _)| now >= *start - lead && now <= *start + chrono::Duration::minutes(30))
        .min_by_key(|(start, _)| *start)
        .map(|(_, svc)| svc)
}

fn unix(t: NaiveDateTime) -> i64 {
    t.and_local_timezone(Local).earliest().map(|t| t.timestamp()).unwrap_or(0)
}

impl Director {
    pub fn new(engine: Arc<Engine>, data_dir: &std::path::Path) -> Arc<Director> {
        let path = data_dir.join("instellingen.json");
        let mut settings: Settings = read_json(&path).unwrap_or_default();
        // Once: the automatic start was off by default before; the church goes live
        // about 15 minutes before the service.
        if settings.settings_version < 2 {
            if settings.auto_start_minutes == 0 {
                settings.auto_start_minutes = 15;
            }
            settings.settings_version = 2;
        }
        // Once: listening along gave too many suggestions; off until switched on again.
        if settings.settings_version < 3 {
            if settings.autopilot == "suggestie" {
                settings.autopilot = "uit".into();
            }
            settings.settings_version = 3;
            if let Err(e) = write_json(&path, &settings) {
                eprintln!("instellingen opslaan mislukt: {e:#}");
            }
        }
        let autopilot_path = data_dir.join("autopilot.json");
        let saved: SavedAutopilot = read_json(&autopilot_path).unwrap_or_default();
        Arc::new(Director {
            engine,
            youtube: YouTube::new(data_dir.join("youtube.json")),
            settings: Mutex::new(settings),
            settings_changed: tokio::sync::Notify::new(),
            path,
            broadcast: Mutex::new(None),
            renderer: Mutex::new(None),
            thumbs: Mutex::new(std::collections::HashMap::new()),
            autopilot: Mutex::new(Autopilot { started: saved.started, ..Default::default() }),
            autopilot_path,
        })
    }

    /// A small picture of what the stream shows for a slide (the overlay over a grey
    /// stand-in for the camera), for the clickable strip in the operator page.
    pub fn thumbnail(&self, view: &View, index: usize) -> Option<Vec<u8>> {
        let mut v = view.clone();
        v.index = index;
        v.override_board = None;
        let (scene, _, _) = scene_for(&v, Duration::from_secs(3600), &self.settings());
        let zones = self.settings().overlay_zones;
        let key = format!("{scene:?}{zones:?}");
        if let Some(png) = self.thumbs.lock().unwrap().get(&key) {
            return Some(png.clone());
        }
        let renderer = self.renderer.lock().unwrap().clone()?;
        let rgba = {
            let mut r = renderer.lock().unwrap();
            r.set_zones(&zones);
            r.render(&scene)
        };
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
        if mode == Mode::Live && !was_live {
            // Never go live on an old end screen: the service about to start, from the top.
            let view = show.view();
            let on_end = view.slides.get(view.index).is_some_and(|sl| sl.kind == "einde");
            let now = Local::now().naive_local();
            match service_in_window(&view.services, now, 90) {
                Some(svc) if view.current != svc.id || on_end => show.select(svc.id.clone()).await,
                _ if on_end => show.go(Some(0), 0, "").await,
                _ => {}
            }
        }
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
        // After the service the preview stays on (nothing goes to YouTube).
        if was_live && mode == Mode::Stop {
            self.start_preview().await;
        }
        Ok(())
    }

    /// Camera, overlay and sound in the console, nothing sent anywhere. Failing is fine:
    /// the status light shows it.
    async fn start_preview(&self) {
        let (engine, settings) = (self.engine.clone(), self.settings());
        match tokio::task::spawn_blocking(move || engine.set_mode(Mode::Voorbeeld, &settings)).await {
            Ok(Err(e)) => eprintln!("voorbeeld starten mislukt: {e:#}"),
            Err(e) => eprintln!("voorbeeld starten mislukt: {e}"),
            Ok(Ok(())) => {}
        }
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
        // At the start the preview is on, so the console shows the camera and the sound.
        if self.engine.status().mode == Mode::Stop {
            self.start_preview().await;
        }
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
            // When the end screen comes (after the pulpit), from the moment "Aftiteling" was
            // chosen; leaving it (← Terug) cancels the automatic stop at once.
            {
                let on_end = view.slides.get(view.index).is_some_and(|sl| sl.kind == "einde");
                let mut ap = self.autopilot.lock().unwrap();
                if !on_end {
                    ap.end_card_at = 0;
                    ap.extra_secs = 0;
                } else if ap.end_card_at == 0 {
                    let since = on_screen.2.elapsed().as_secs() as i64;
                    ap.end_card_at = Local::now().timestamp() - since + PULPIT_HOLD.as_secs() as i64;
                }
            }
            let (scene, position, until) = scene_for(&view, on_screen.2.elapsed(), &settings);
            let drawn = (scene.clone(), settings.overlay_zones.clone());
            if last_scene.as_ref() != Some(&drawn) {
                last_scene = Some(drawn);
                let r = renderer.clone();
                let zones = settings.overlay_zones.clone();
                let render = move || {
                    let mut r = r.lock().unwrap();
                    r.set_zones(&zones);
                    r.render(&scene)
                };
                if let Ok(pixels) = tokio::task::spawn_blocking(render).await {
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

    fn set_started(&self, id: &str) {
        let mut ap = self.autopilot.lock().unwrap();
        if ap.started == id {
            return;
        }
        ap.started = id.to_string();
        if let Err(e) = write_json(&self.autopilot_path, &SavedAutopilot { started: id.to_string() }) {
            eprintln!("autopilot opslaan mislukt: {e:#}");
        }
    }

    /// For the operator page: when the stream starts and stops by itself (Unix seconds,
    /// 0 = not planned), and why an automatic start failed.
    pub fn autopilot_status(&self, view: &View, s: &Settings) -> serde_json::Value {
        let ap = self.autopilot.lock().unwrap();
        let now = Local::now().naive_local();
        let lead = chrono::Duration::minutes(s.auto_start_minutes as i64);
        let next = if s.auto_start_minutes == 0 {
            None
        } else {
            service_in_window(&view.services, now, s.auto_start_minutes).filter(|svc| svc.id != ap.started).or_else(|| {
                view.services
                    .iter()
                    .filter(|svc| service_start(svc).is_some_and(|start| start - lead > now))
                    .min_by_key(|svc| service_start(svc))
            })
        };
        let live = self.engine.status().mode == Mode::Live;
        let stop_at = if ap.end_card_at > 0 && s.auto_stop_seconds > 0 && live {
            ap.end_card_at + (s.auto_stop_seconds + ap.extra_secs) as i64
        } else {
            0
        };
        serde_json::json!({
            "autoStartAt": next.and_then(service_start).map(|start| unix(start - lead)).unwrap_or(0),
            "autoStartService": next.map(|svc| svc.id.clone()).unwrap_or_default(),
            "autoStartError": ap.start_error,
            "autoStopAt": stop_at,
            "endCardAt": ap.end_card_at,
        })
    }

    /// Five more minutes before the automatic stop.
    pub fn later(&self) -> anyhow::Result<()> {
        let mut ap = self.autopilot.lock().unwrap();
        if ap.end_card_at == 0 {
            anyhow::bail!("niet op de aftiteling");
        }
        ap.extra_secs += 300;
        Ok(())
    }

    /// Goes live before a service (with the start screen) and stops a while after the end
    /// screen, when switched on.
    async fn autopilot(self: Arc<Self>, show: Arc<Show>) {
        // The blessing slide on screen and since when.
        let mut zegen_since: Option<(usize, Instant)> = None;
        loop {
            tokio::time::sleep(Duration::from_secs(10)).await;
            let s = self.settings();
            let view = show.view();
            let mode = self.engine.status().mode;
            let now = Local::now().naive_local();
            let kind = view.slides.get(view.index).map(|sl| sl.kind.clone()).unwrap_or_default();

            let due = service_in_window(&view.services, now, s.auto_start_minutes).cloned();
            // A failed automatic start is only news until the stream is live or the service is past.
            if mode == Mode::Live || due.is_none() {
                self.autopilot.lock().unwrap().start_error.clear();
            }
            let started = self.autopilot.lock().unwrap().started.clone();
            if s.auto_start_minutes > 0
                && matches!(mode, Mode::Stop | Mode::Voorbeeld)
                && let Some(svc) = due.as_ref().filter(|svc| svc.id != started)
            {
                println!("automatisch live voor {}", svc.id);
                self.set_started(&svc.id);
                if view.current != svc.id {
                    show.select(svc.id.clone()).await;
                }
                let result = self.set_mode(&show, Mode::Live).await;
                if let Err(e) = &result {
                    eprintln!("automatisch live gaan mislukt: {e:#}");
                }
                self.autopilot.lock().unwrap().start_error = result.err().map(|e| format!("{e:#}")).unwrap_or_default();
                continue;
            }
            // Live by hand counts too: after a deliberate stop it does not go live again.
            if mode == Mode::Live
                && let Some(svc) = &due
            {
                self.set_started(&svc.id);
            }

            // Nobody pressed on after the blessing: the end screen comes by itself.
            zegen_since = if kind == "zegen" { zegen_since.filter(|(i, _)| *i == view.index).or(Some((view.index, Instant::now()))) } else { None };
            if mode == Mode::Live
                && zegen_since.is_some_and(|(_, t)| t.elapsed() >= ZEGEN_MAX)
                && view.slides.get(view.index + 1).is_some_and(|sl| sl.kind == "einde")
            {
                println!("automatisch naar de aftiteling na de zegen");
                show.go(Some(view.index + 1), 0, "").await;
                zegen_since = None;
                continue;
            }

            let stop_at = {
                let ap = self.autopilot.lock().unwrap();
                (ap.end_card_at > 0).then(|| ap.end_card_at + (s.auto_stop_seconds + ap.extra_secs) as i64)
            };
            if s.auto_stop_seconds > 0 && mode == Mode::Live && stop_at.is_some_and(|t| Local::now().timestamp() >= t) {
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
        // The church's start screen: the outside of the church, the date and the service.
        "welkom" => {
            let left = svc.map(|s| long_date(&s.date)).unwrap_or_default();
            let right = svc.map(|s| s.label.clone()).unwrap_or_default();
            (Scene::Frame { photo: Photo::Outside, left, right }, Position::Overzicht, None)
        }
        "zegen" => (Scene::Camera, Position::Preekstoel, None),
        // First the empty pulpit while the organ starts, then the end screen: the inside of
        // the church with, in turn, the next service and the website.
        "einde" if shown < PULPIT_HOLD => (Scene::Camera, Position::Preekstoel, Some(PULPIT_HOLD)),
        "einde" => {
            let mut sentences: Vec<String> = next_sentence(view).into_iter().collect();
            sentences.push("Voor meer informatie over onze gemeente kijk op www.hhgtholen.nl".into());
            let n = (shown - PULPIT_HOLD).as_secs() / SWAP.as_secs();
            let left = sentences[n as usize % sentences.len()].clone();
            let until = (sentences.len() > 1).then(|| PULPIT_HOLD + SWAP * (n as u32 + 1));
            (Scene::Frame { photo: Photo::Inside, left, right: String::new() }, Position::Overzicht, until)
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

/// The sentence about the next service on the end screen, as the church writes it:
/// "Vanmiddag is er een dienst om 16.00 uur." or "Zondag om 9.30 uur is er weer een dienst.".
fn next_sentence(view: &View) -> Option<String> {
    let svc = view.service()?;
    let today = NaiveDate::parse_from_str(&svc.date, "%Y-%m-%d").ok().map(long_date_no_year).unwrap_or_default();
    // svc.next is "Zondag 25 oktober om 9.30 uur · ds. K. Klopstra".
    let (date, time) = if svc.next.is_empty() {
        let next = view.services.iter().filter(|s| s.id > svc.id).min_by(|a, b| a.id.cmp(&b.id))?;
        let date = long_date_no_year(NaiveDate::parse_from_str(&next.date, "%Y-%m-%d").ok()?);
        let time = NaiveTime::parse_from_str(&next.time, "%H:%M").ok()?.format("%-H.%M").to_string();
        (date, time)
    } else {
        let when = svc.next.split(" · ").next().unwrap_or_default();
        let (date, time) = when.split_once(" om ")?;
        (date.to_string(), time.trim_end_matches(" uur").trim().to_string())
    };
    if date == today {
        let hour: u32 = time.split('.').next().and_then(|h| h.parse().ok()).unwrap_or(0);
        let part = if hour >= 18 { "Vanavond" } else { "Vanmiddag" };
        Some(format!("{part} is er een dienst om {time} uur."))
    } else {
        let day = date.split_whitespace().next()?;
        Some(format!("{day} om {time} uur is er weer een dienst."))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(services: Vec<Service>, current: &str) -> View {
        let mut v = View::default();
        v.services = services;
        v.current = current.to_string();
        v
    }

    fn svc(date: &str, time: &str, next: &str) -> Service {
        Service { id: format!("{date} {time}"), date: date.into(), time: time.into(), next: next.into(), ..Default::default() }
    }

    #[test]
    fn same_day() {
        let v = view(vec![svc("2026-10-18", "09:30", "Zondag 18 oktober om 16.00 uur · ds. A. den Hartog")], "2026-10-18 09:30");
        assert_eq!(next_sentence(&v).as_deref(), Some("Vanmiddag is er een dienst om 16.00 uur."));
        let v = view(vec![svc("2026-10-18", "09:30", "Zondag 18 oktober om 18.30 uur")], "2026-10-18 09:30");
        assert_eq!(next_sentence(&v).as_deref(), Some("Vanavond is er een dienst om 18.30 uur."));
    }

    #[test]
    fn next_sunday() {
        let v = view(vec![svc("2026-10-18", "16:00", "Zondag 25 oktober om 9.30 uur · ds. K. Klopstra")], "2026-10-18 16:00");
        assert_eq!(next_sentence(&v).as_deref(), Some("Zondag om 9.30 uur is er weer een dienst."));
        // Without kerktijden.nl: the next service in the list.
        let v = view(vec![svc("2026-10-18", "16:00", ""), svc("2026-10-25", "09:30", "")], "2026-10-18 16:00");
        assert_eq!(next_sentence(&v).as_deref(), Some("Zondag om 9.30 uur is er weer een dienst."));
        let v = view(vec![svc("2026-10-18", "09:30", ""), svc("2026-10-18", "16:00", "")], "2026-10-18 09:30");
        assert_eq!(next_sentence(&v).as_deref(), Some("Vanmiddag is er een dienst om 16.00 uur."));
    }

    #[test]
    fn empty() {
        let v = view(vec![svc("2026-10-18", "16:00", "")], "2026-10-18 16:00");
        assert_eq!(next_sentence(&v), None);
        assert_eq!(next_sentence(&view(vec![], "")), None);
    }
}
