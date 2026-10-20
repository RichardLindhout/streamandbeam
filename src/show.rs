//! The services, their order, and what the screens show right now.

use crate::books::{BibleRef, parse_bible_ref, parse_psalm_ref, psalm_ref_string};
use crate::extract::{ExtractedPsalm, ExtractedService};
use crate::texts::{Texts, Verse, read_json, write_json};
use chrono::{Datelike, Local, NaiveDate};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{Mutex, watch};

/// One part of a service, in the order the predikant uses.
/// Kinds: welkom, psalm, slotzang, lezing, gebed, preek, mededeling.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Item {
    pub id: String,
    pub kind: String,
    pub label: String, // shown above the text, e.g. "Voorzang"
    #[serde(rename = "ref")]
    pub reference: String, // "38: 15" for psalms, "Johannes 5:16-29" for readings
    pub text: String, // free text for a mededeling
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Service {
    pub id: String,
    pub date: String,
    pub label: String,
    pub time: String,
    pub text_ref: String, // sermon text, e.g. "Johannes 5:25"
    pub theme: String,
    pub points: Vec<String>,
    pub items: Vec<Item>,
}

/// What a screen shows.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Slide {
    pub item: usize,
    pub kind: String,
    pub header: String,
    pub sub: String,
    pub lines: Vec<String>,
    pub verses: Vec<Verse>,
    pub theme: String,
    pub points: Vec<String>,
    pub list: Vec<String>, // liturgy overview on the welcome slide
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct Saved {
    services: Vec<Service>,
    current: String,
    index: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct View {
    pub services: Vec<Service>,
    pub current: String,
    pub index: usize,
    pub black: bool,
    pub slides: Vec<Slide>,
}

pub struct Show {
    texts: Arc<Texts>,
    liturgy_url: String,
    path: PathBuf,
    state: Mutex<View>,
    tx: watch::Sender<View>,
}

impl Show {
    pub async fn new(path: PathBuf, texts: Arc<Texts>, liturgy_url: String) -> Arc<Show> {
        let saved: Saved = read_json(&path).unwrap_or_default();
        let view = View { services: saved.services, current: saved.current, index: saved.index, ..Default::default() };
        let (tx, _) = watch::channel(view.clone());
        let show = Arc::new(Show { texts, liturgy_url, path, state: Mutex::new(view), tx });
        let mut st = show.state.lock().await;
        show.changed(&mut st).await;
        drop(st);
        show
    }

    pub fn subscribe(&self) -> watch::Receiver<View> {
        self.tx.subscribe()
    }

    pub fn view(&self) -> View {
        self.tx.borrow().clone()
    }

    /// Reads the liturgy from the website and adds/replaces the services.
    pub async fn fetch(&self) -> anyhow::Result<()> {
        let ex = crate::extract::extract_liturgy(&self.liturgy_url).await?;
        let mut st = self.state.lock().await;
        for e in ex.services {
            let text_ref = if e.text.book > 0 {
                BibleRef { book: e.text.book, chapter: e.text.chapter, from: e.text.from, to: e.text.to }.to_string()
            } else if e.text.catechism_question > 0 {
                format!("Heidelbergse Catechismus, vraag en antwoord {}", e.text.catechism_question)
            } else {
                e.text.display.clone()
            };
            let svc = Service {
                id: format!("{} {}", e.date, e.time),
                date: e.date.clone(),
                label: e.label.clone(),
                time: e.time.clone(),
                text_ref,
                theme: e.theme.clone(),
                points: e.points.clone(),
                items: default_items(&e),
            };
            upsert(&mut st.services, svc);
        }
        if st.current.is_empty() || !st.services.iter().any(|s| s.id == st.current) {
            st.current = upcoming(&st.services);
            st.index = 0;
        }
        self.changed(&mut st).await;
        Ok(())
    }

    pub async fn save_service(&self, svc: Service) {
        let mut st = self.state.lock().await;
        upsert(&mut st.services, svc);
        self.changed(&mut st).await;
    }

    pub async fn select(&self, id: String) {
        let mut st = self.state.lock().await;
        st.current = id;
        st.index = 0;
        st.black = false;
        self.changed(&mut st).await;
    }

    /// Moves the screens: to an absolute slide, by a delta, or to the next slide of a kind.
    pub async fn go(&self, index: Option<usize>, delta: i64, kind: &str) {
        let mut st = self.state.lock().await;
        if kind == "zwart" {
            st.black = !st.black;
        } else {
            let target = match index {
                Some(i) => i as i64,
                None if !kind.is_empty() => next_of_kind(&st, kind) as i64,
                None => st.index as i64 + delta,
            };
            st.index = target.clamp(0, st.slides.len().saturating_sub(1) as i64) as usize;
            st.black = false;
        }
        self.publish(&st);
    }

    /// Rebuilds the slides after the services changed.
    async fn changed(&self, st: &mut View) {
        st.slides = match st.services.iter().find(|s| s.id == st.current) {
            Some(svc) => build_slides(&self.texts, svc).await,
            None => Vec::new(),
        };
        st.index = st.index.min(st.slides.len().saturating_sub(1));
        self.publish(st);
    }

    fn publish(&self, st: &View) {
        let saved = Saved { services: st.services.clone(), current: st.current.clone(), index: st.index };
        if let Err(e) = write_json(&self.path, &saved) {
            eprintln!("opslaan mislukt: {e}");
        }
        self.tx.send_replace(st.clone());
    }
}

fn upsert(services: &mut Vec<Service>, svc: Service) {
    match services.iter_mut().find(|s| s.id == svc.id) {
        Some(s) => *s = svc,
        None => services.push(svc),
    }
}

/// The first service that has not finished yet (services take about two hours).
fn upcoming(services: &[Service]) -> String {
    let now = (Local::now() - chrono::Duration::hours(2)).format("%Y-%m-%d %H:%M").to_string();
    services
        .iter()
        .map(|s| &s.id)
        .filter(|id| **id >= now)
        .min()
        .or(services.last().map(|s| &s.id))
        .cloned()
        .unwrap_or_default()
}

fn next_of_kind(st: &View, kind: &str) -> usize {
    let cur = st.slides.get(st.index).map(|s| s.item as i64).unwrap_or(-1);
    let mut matching = st.slides.iter().enumerate().filter(|(_, s)| s.kind == kind);
    let first = matching.clone().next().map(|(i, _)| i);
    matching.find(|(_, s)| s.item as i64 > cur).map(|(i, _)| i).or(first).unwrap_or(st.index)
}

/// Puts the website's psalms in the usual HHK order. The predikant's own order can then
/// be made in the operator screen by dragging items around.
fn default_items(e: &ExtractedService) -> Vec<Item> {
    let mut items: Vec<Item> = Vec::new();
    let mut add = |kind: &str, label: &str, reference: String| {
        items.push(Item {
            id: format!("i{}", items.len() + 1),
            kind: kind.into(),
            label: label.into(),
            reference,
            text: String::new(),
        })
    };
    let psalm = |p: &ExtractedPsalm| {
        let vs: Vec<String> = p.verses.iter().map(u32::to_string).collect();
        format!("{}: {}", p.psalm, vs.join(", "))
    };
    let readings: Vec<String> = e
        .readings
        .iter()
        .map(|r| BibleRef { book: r.book, chapter: r.chapter, from: r.from, to: r.to }.to_string())
        .collect();
    let n = e.psalms.len();
    add("welkom", "Welkom", String::new());
    for (i, p) in e.psalms.iter().enumerate() {
        if i == 0 {
            add("psalm", "Voorzang", psalm(p));
        } else if i + 1 == n {
            if n < 3 {
                add("gebed", "Gebed", String::new());
                for r in &readings {
                    add("lezing", "Schriftlezing", r.clone());
                }
                add("preek", "Prediking", String::new());
            }
            add("gebed", "Dankgebed", String::new());
            add("slotzang", "Slotzang", psalm(p));
        } else if i + 2 == n {
            add("psalm", "Tussenzang", psalm(p));
            add("preek", "Prediking", String::new());
        } else if i + 3 == n {
            add("gebed", "Gebed", String::new());
            for r in &readings {
                add("lezing", "Schriftlezing", r.clone());
            }
            add("psalm", "Zingen", psalm(p));
            add("preek", "Prediking", String::new());
        } else {
            add("psalm", "Zingen", psalm(p));
        }
    }
    items
}

pub fn service_title(svc: &Service) -> String {
    let Ok(d) = NaiveDate::parse_from_str(&svc.date, "%Y-%m-%d") else {
        return format!("{} {}", svc.label, svc.time);
    };
    const DAYS: [&str; 7] = ["maandag", "dinsdag", "woensdag", "donderdag", "vrijdag", "zaterdag", "zondag"];
    const MONTHS: [&str; 12] = [
        "januari", "februari", "maart", "april", "mei", "juni", "juli", "augustus", "september",
        "oktober", "november", "december",
    ];
    format!(
        "{} · {} {} {} · {} uur",
        svc.label,
        DAYS[d.weekday().num_days_from_monday() as usize],
        d.day(),
        MONTHS[d.month0() as usize],
        svc.time
    )
}

async fn build_slides(texts: &Texts, svc: &Service) -> Vec<Slide> {
    // Load all psalms in parallel; the first time each one is fetched from the internet.
    let numbers: Vec<u32> = svc
        .items
        .iter()
        .filter(|it| it.kind == "psalm" || it.kind == "slotzang")
        .filter_map(|it| parse_psalm_ref(&it.reference).map(|(n, _)| n))
        .collect();
    futures::future::join_all(numbers.iter().map(|n| texts.psalm(*n))).await;

    let mut slides = Vec::new();
    for (idx, it) in svc.items.iter().enumerate() {
        let mut add = |sl: Slide| slides.push(Slide { item: idx, kind: it.kind.clone(), ..sl });
        match it.kind.as_str() {
            "welkom" => {
                let list = svc
                    .items
                    .iter()
                    .filter_map(|o| match o.kind.as_str() {
                        "psalm" | "slotzang" => parse_psalm_ref(&o.reference).map(|(n, vs)| psalm_ref_string(n, &vs)),
                        "lezing" => parse_bible_ref(&o.reference).map(|r| format!("Lezen: {r}")),
                        _ => None,
                    })
                    .collect();
                add(Slide { header: "Hersteld Hervormde Gemeente Tholen".into(), sub: service_title(svc), list, ..Default::default() });
            }
            "psalm" | "slotzang" => {
                let Some((n, verses)) = parse_psalm_ref(&it.reference) else {
                    add(Slide { header: it.label.clone(), sub: format!("Onbekende psalm: {}", it.reference), ..Default::default() });
                    continue;
                };
                let psalm = texts.psalm(n).await;
                if it.kind == "slotzang" {
                    add(Slide { header: "Slotzang".into(), sub: psalm_ref_string(n, &verses), ..Default::default() });
                }
                for v in verses {
                    let lines = match &psalm {
                        Err(e) => vec![format!("(tekst niet geladen: {e})")],
                        Ok(p) => match p.verses.get((v as usize).wrapping_sub(1)) {
                            Some(l) => l.clone(),
                            None => vec![format!("(psalm {n} heeft geen vers {v})")],
                        },
                    };
                    add(Slide { header: format!("Psalm {n} : {v}"), sub: it.label.clone(), lines, ..Default::default() });
                }
            }
            "lezing" => {
                let Some(r) = parse_bible_ref(&it.reference) else {
                    add(Slide { header: it.label.clone(), sub: format!("Onbekende tekst: {}", it.reference), ..Default::default() });
                    continue;
                };
                match texts.passage(r).await {
                    Ok(verses) if !verses.is_empty() => {
                        for page in paginate(verses, 650) {
                            add(Slide { header: r.to_string(), sub: it.label.clone(), verses: page, ..Default::default() });
                        }
                    }
                    _ => add(Slide { header: r.to_string(), sub: it.label.clone(), lines: vec!["(tekst niet geladen)".into()], ..Default::default() }),
                }
            }
            "preek" => {
                let verses = match parse_bible_ref(&svc.text_ref) {
                    Some(r) => texts.passage(r).await.unwrap_or_default(),
                    None => Vec::new(),
                };
                add(Slide {
                    header: format!("Tekst: {}", svc.text_ref),
                    sub: it.label.clone(),
                    verses,
                    theme: svc.theme.clone(),
                    points: svc.points.clone(),
                    ..Default::default()
                });
            }
            "gebed" => add(Slide { header: it.label.clone(), ..Default::default() }),
            "mededeling" => add(Slide { header: it.label.clone(), lines: it.text.lines().map(String::from).collect(), ..Default::default() }),
            _ => {}
        }
    }
    slides
}

/// Splits a reading into slides of roughly max_chars characters.
fn paginate(verses: Vec<Verse>, max_chars: usize) -> Vec<Vec<Verse>> {
    let mut pages: Vec<Vec<Verse>> = Vec::new();
    let mut size = 0;
    for v in verses {
        match pages.last_mut() {
            Some(page) if size + v.text.len() <= max_chars => {
                size += v.text.len();
                page.push(v);
            }
            _ => {
                size = v.text.len();
                pages.push(vec![v]);
            }
        }
    }
    pages
}
