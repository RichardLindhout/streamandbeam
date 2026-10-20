//! Follows the operator: for every slide it picks the scene for the YouTube stream and
//! moves the camera to the matching position.

use crate::render::{Renderer, Scene};
use crate::show::{Show, View};
use crate::stream::{Engine, Settings, recall_preset};
use crate::texts::{read_json, write_json};
use chrono::{Datelike, NaiveDate};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub struct Director {
    pub engine: Arc<Engine>,
    settings: Mutex<Settings>,
    settings_changed: tokio::sync::Notify,
    path: PathBuf,
}

impl Director {
    pub fn new(engine: Arc<Engine>, path: PathBuf) -> Arc<Director> {
        let settings = read_json(&path).unwrap_or_default();
        Arc::new(Director { engine, settings: Mutex::new(settings), settings_changed: tokio::sync::Notify::new(), path })
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
    pub async fn run(self: Arc<Self>, show: Arc<Show>, photo: Option<Vec<u8>>) {
        let renderer = Arc::new(Mutex::new(Renderer::new(photo.as_deref())));
        let mut rx = show.subscribe();
        let mut last_scene = None;
        // Camera and preset last sent, so a newly entered camera IP also gets the position.
        let mut last_sent = (String::new(), 0);
        loop {
            let view = rx.borrow_and_update().clone();
            let (scene, position) = scene_for(&view);
            if last_scene.as_ref() != Some(&scene) {
                last_scene = Some(scene.clone());
                let r = renderer.clone();
                if let Ok(pixels) = tokio::task::spawn_blocking(move || r.lock().unwrap().render(&scene)).await {
                    self.engine.set_overlay(pixels);
                }
            }
            let settings = self.settings();
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
            tokio::select! {
                changed = rx.changed() => if changed.is_err() { return },
                _ = self.settings_changed.notified() => {}
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

/// The stream scene and camera position for what the screens show now.
fn scene_for(view: &View) -> (Scene, Position) {
    let Some(slide) = view.slides.get(view.index) else {
        return (Scene::Camera, Position::Overzicht);
    };
    let panel = |paragraphs: Vec<String>| Scene::Panel { title: slide.header.clone(), paragraphs };
    match slide.kind.as_str() {
        "welkom" => {
            let svc = view.services.iter().find(|s| s.id == view.current);
            let title = svc.map(|s| long_date(&s.date)).unwrap_or_default();
            let sub = svc.map(|s| s.label.clone()).unwrap_or_default();
            (Scene::Holding { title, sub }, Position::Overzicht)
        }
        "psalm" | "slotzang" if slide.lines.is_empty() => {
            (Scene::Bar { text: format!("{}: {}", slide.header, slide.sub) }, Position::Orgel)
        }
        "psalm" | "slotzang" => (panel(slide.lines.clone()), Position::Orgel),
        "lezing" => {
            let verses = slide.verses.iter().map(|v| format!("{}  {}", v.number, v.text)).collect();
            (panel(verses), Position::Preekstoel)
        }
        "gebed" => (Scene::Bar { text: "Gebed".into() }, Position::Doopvont),
        "mededeling" => (panel(slide.lines.clone()), Position::Preekstoel),
        // During the sermon viewers see the predikant.
        _ => (Scene::Camera, Position::Preekstoel),
    }
}

fn long_date(date: &str) -> String {
    let Ok(d) = NaiveDate::parse_from_str(date, "%Y-%m-%d") else { return date.to_string() };
    const DAYS: [&str; 7] = ["Maandag", "Dinsdag", "Woensdag", "Donderdag", "Vrijdag", "Zaterdag", "Zondag"];
    const MONTHS: [&str; 12] = [
        "januari", "februari", "maart", "april", "mei", "juni", "juli", "augustus", "september",
        "oktober", "november", "december",
    ];
    format!("{} {} {} {}", DAYS[d.weekday().num_days_from_monday() as usize], d.day(), MONTHS[d.month0() as usize], d.year())
}
