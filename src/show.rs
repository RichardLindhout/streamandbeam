//! The services, their order, and what the screens show right now.

use crate::books::{BibleRef, psalm_ref_string};
use crate::extract::ExtractedService;
use crate::texts::{Verse, read_json, write_json};
use chrono::{Datelike, Local, NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{Mutex, watch};

/// One part of a service, in the order the predikant uses. All references are numbers
/// (Claude maps what is typed or on the website onto them), so nothing is parsed later.
/// Kinds: welkom, psalm, slotzang, nazang (song after the service), lied (a song not in
/// the psalm book, e.g. on a children's singing day), lezing, catechismus, gebed,
/// collecte, preek, mededeling, einde.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Item {
    pub id: String,
    pub kind: String,
    pub label: String, // shown above the text, e.g. "Voorzang"
    pub psalm: u32,
    pub verses: Vec<u32>,
    pub passage: Option<BibleRef>,
    pub questions: Vec<u32>,
    pub zondag: u32,
    pub text: String, // free text for a mededeling or the collecte purposes
    /// Readable reference, e.g. "Psalm 42 : 5" (made from the numbers above).
    #[serde(rename = "ref")]
    pub reference: String,
    /// What is wrong or uncertain about this item; empty when fine.
    pub problem: String,
    /// The operator's own version of this item's slides, replacing the generated ones
    /// (to fix or change anything by hand). Empty = use the generated slides.
    pub custom: Vec<CustomSlide>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CustomSlide {
    pub header: String,
    pub sub: String,
    /// One line per line of text.
    pub text: String,
    pub footer: String,
}

impl Item {
    pub fn refresh(&mut self) {
        self.reference = match self.kind.as_str() {
            "psalm" | "slotzang" | "nazang" if self.psalm > 0 => psalm_ref_string(self.psalm, &self.verses),
            "lezing" => self.passage.map(|p| p.to_string()).unwrap_or_default(),
            "catechismus" if !self.questions.is_empty() => {
                let qs: Vec<String> = self.questions.iter().map(u32::to_string).collect();
                format!("vraag {}", qs.join(", "))
            }
            "catechismus" => format!("zondag {}", self.zondag),
            "lied" => self.label.clone(),
            _ => String::new(),
        };
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Service {
    pub id: String,
    pub date: String,
    pub label: String,
    pub time: String,
    pub predikant: String,
    /// The sermon text as an item (a reading or catechism questions).
    pub text: Item,
    pub theme: String,
    pub points: Vec<String>,
    /// e.g. "Zondag 25 oktober om 9.30 uur · ds. K. Klopstra", shown at the end.
    pub next: String,
    /// Parts of the website's liturgy Claude could not place.
    pub problems: Vec<String>,
    /// The YouTube broadcast made for this service.
    pub youtube_id: String,
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
    /// Which line of `list` is the part of the service going on now.
    pub active: Option<usize>,
    pub footer: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct Saved {
    services: Vec<Service>,
    current: String,
    index: usize,
    /// Collecte purposes of the last service, used for the next one.
    collecte: String,
    /// Show every text on the boards in church. Off (the default) the boards show only
    /// the liturgy, the theme and the last psalm, as the church wants; the stream always
    /// gets the full texts.
    board_full: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub services: Vec<Service>,
    pub current: String,
    pub index: usize,
    pub black: bool,
    /// The slides: what the stream follows and the operator steps through.
    pub slides: Vec<Slide>,
    /// What the boards in church show for each slide.
    pub boards: Vec<Slide>,
    pub board_full: bool,
    /// Text the operator put on the boards directly, over whatever the service shows.
    pub override_board: Option<Slide>,
    /// Whether that text also goes into the stream.
    pub override_stream: bool,
    #[serde(skip)]
    collecte: String,
}

impl View {
    pub fn service(&self) -> Option<&Service> {
        self.services.iter().find(|s| s.id == self.current)
    }
}

pub struct Show {
    liturgy_url: String,
    path: PathBuf,
    state: Mutex<View>,
    tx: watch::Sender<View>,
}

impl Show {
    pub async fn new(path: PathBuf, liturgy_url: String) -> Arc<Show> {
        let saved: Saved = read_json(&path).unwrap_or_default();
        let view = View {
            services: saved.services,
            current: saved.current,
            index: saved.index,
            collecte: saved.collecte,
            board_full: saved.board_full,
            ..Default::default()
        };
        let (tx, _) = watch::channel(view.clone());
        let show = Arc::new(Show { liturgy_url, path, state: Mutex::new(view), tx });
        let mut st = show.state.lock().await;
        show.changed(&mut st);
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
        for e in ex.diensten {
            let mut svc = Service {
                id: format!("{} {}", e.datum, e.tijd),
                date: e.datum.clone(),
                label: e.dienst.clone(),
                time: e.tijd.clone(),
                text: e.preektekst.clone().into_item(),
                theme: e.thema.clone(),
                points: e.punten.clone(),
                problems: e.niet_herkend.clone(),
                items: default_items(&e, &st.collecte),
                ..Default::default()
            };
            // Keep what the operator already filled in for this service.
            if let Some(old) = st.services.iter().find(|s| s.id == svc.id) {
                svc.predikant = old.predikant.clone();
                svc.youtube_id = old.youtube_id.clone();
            }
            upsert(&mut st.services, svc);
        }
        enrich_from_kerktijden(&mut st.services).await;
        if st.current.is_empty() || !st.services.iter().any(|s| s.id == st.current) {
            st.current = upcoming(&st.services);
            st.index = 0;
        }
        self.changed(&mut st);
        Ok(())
    }

    /// A new, empty service the operator fills in by typing.
    pub async fn new_service(&self, date: String, time: String, label: String) {
        let mut st = self.state.lock().await;
        let mut items = vec![
            Item { kind: "welkom".into(), label: "Welkom".into(), ..Default::default() },
            Item { kind: "einde".into(), label: "Einde".into(), ..Default::default() },
        ];
        for (i, it) in items.iter_mut().enumerate() {
            it.id = format!("i{}", i + 1);
        }
        let svc = Service { id: format!("{date} {time}"), date, time, label, items, ..Default::default() };
        st.current = svc.id.clone();
        st.index = 0;
        upsert(&mut st.services, svc);
        enrich_from_kerktijden(&mut st.services).await;
        self.changed(&mut st);
    }

    pub async fn delete_service(&self, id: &str) {
        let mut st = self.state.lock().await;
        st.services.retain(|s| s.id != id);
        if st.current == id {
            st.current = upcoming(&st.services);
            st.index = 0;
        }
        self.changed(&mut st);
    }

    pub async fn save_service(&self, mut svc: Service) {
        let mut st = self.state.lock().await;
        for it in &mut svc.items {
            it.refresh();
        }
        svc.text.refresh();
        if let Some(c) = svc.items.iter().find(|i| i.kind == "collecte") {
            st.collecte = c.text.clone();
        }
        upsert(&mut st.services, svc);
        self.changed(&mut st);
    }

    pub async fn set_youtube_id(&self, service: &str, broadcast: &str) {
        let mut st = self.state.lock().await;
        if let Some(s) = st.services.iter_mut().find(|s| s.id == service) {
            s.youtube_id = broadcast.to_string();
        }
        self.publish(&st);
    }

    pub async fn select(&self, id: String) {
        let mut st = self.state.lock().await;
        st.current = id;
        st.index = 0;
        st.black = false;
        self.changed(&mut st);
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
    /// Puts the operator's own text on the boards right away (None = back to normal).
    pub async fn set_override(&self, slide: Option<Slide>, stream: bool) {
        let mut st = self.state.lock().await;
        st.override_board = slide.map(|s| Slide { kind: "mededeling".into(), ..s });
        st.override_stream = stream && st.override_board.is_some();
        self.publish(&st);
    }

    pub async fn set_board_full(&self, full: bool) {
        let mut st = self.state.lock().await;
        st.board_full = full;
        self.changed(&mut st);
    }

    fn changed(&self, st: &mut View) {
        st.slides = st.service().map(build_slides).unwrap_or_default();
        st.boards = match st.service() {
            Some(svc) if !st.board_full => board_slides(svc, &st.slides),
            _ => st.slides.clone(),
        };
        st.index = st.index.min(st.slides.len().saturating_sub(1));
        self.publish(st);
    }

    fn publish(&self, st: &View) {
        let saved = Saved {
            services: st.services.clone(),
            current: st.current.clone(),
            index: st.index,
            collecte: st.collecte.clone(),
            board_full: st.board_full,
        };
        if let Err(e) = write_json(&self.path, &saved) {
            eprintln!("opslaan mislukt: {e}");
        }
        self.tx.send_replace(st.clone());
    }
}

/// Fills in the predikant and the next service from kerktijden.nl. Failing is fine:
/// the operator can type them in.
async fn enrich_from_kerktijden(services: &mut [Service]) {
    let gatherings = match crate::kerktijden::gatherings(crate::kerktijden::COMMUNITY_ID, 6).await {
        Ok(g) => g,
        Err(e) => {
            eprintln!("kerktijden.nl: {e:#}");
            return;
        }
    };
    for svc in services.iter_mut() {
        let Ok(start) = NaiveDateTime::parse_from_str(&svc.id, "%Y-%m-%d %H:%M") else { continue };
        if svc.predikant.is_empty()
            && let Some(g) = gatherings.iter().find(|g| g.start == start)
        {
            svc.predikant = g.preacher.clone();
        }
        if let Some(next) = gatherings.iter().filter(|g| g.start > start).min_by_key(|g| g.start) {
            let when = format!("{} om {} uur", long_date_no_year(next.start.date()), next.start.format("%-H.%M"));
            svc.next = if next.preacher.is_empty() { when } else { format!("{when} · {}", next.preacher) };
        }
    }
}

fn upsert(services: &mut Vec<Service>, svc: Service) {
    match services.iter_mut().find(|s| s.id == svc.id) {
        Some(s) => *s = svc,
        None => services.push(svc),
    }
    services.sort_by(|a, b| a.id.cmp(&b.id));
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

/// Puts the website's psalms in the usual HHK order, as seen in the church's stream:
/// psalms, prayer, reading (and catechism), collecte during the psalm before the sermon,
/// sermon, tussenzang, sermon, prayer of thanks, slotzang, end. The predikant's own order
/// can then be made in the operator screen by dragging items around.
fn default_items(e: &ExtractedService, collecte: &str) -> Vec<Item> {
    let parsed: Vec<Item> = e.onderdelen.iter().cloned().map(|r| r.into_item()).collect();
    let psalms: Vec<&Item> = parsed.iter().filter(|i| i.kind == "psalm" || i.kind == "slotzang").collect();
    let reading: Vec<&Item> = parsed.iter().filter(|i| i.kind == "lezing" || i.kind == "catechismus").collect();
    let mut items: Vec<Item> = Vec::new();
    let simple = |kind: &str, label: &str| Item { kind: kind.into(), label: label.into(), ..Default::default() };
    let sung = |it: &Item, kind: &str, label: &str| {
        let mut it = Item { kind: kind.into(), label: label.into(), ..it.clone() };
        it.refresh();
        it
    };
    let reading_part = |items: &mut Vec<Item>| {
        items.push(simple("gebed", "Gebed"));
        items.extend(reading.iter().map(|i| (*i).clone()));
    };
    let n = psalms.len();
    items.push(simple("welkom", "Welkom"));
    for (i, p) in psalms.iter().enumerate() {
        if i == 0 {
            items.push(sung(p, "psalm", "Voorzang"));
            if n < 3 {
                reading_part(&mut items);
                items.push(simple("preek", "Prediking"));
            }
        } else if i + 1 == n {
            items.push(simple("gebed", "Dankgebed"));
            items.push(sung(p, "slotzang", "Slotzang"));
        } else if i + 2 == n {
            items.push(sung(p, "psalm", "Tussenzang"));
            items.push(simple("preek", "Prediking"));
        } else if i + 3 == n {
            reading_part(&mut items);
            items.push(Item { text: collecte.into(), ..simple("collecte", "Collecte") });
            items.push(sung(p, "psalm", "Zingen"));
            items.push(simple("preek", "Prediking"));
        } else {
            items.push(sung(p, "psalm", "Zingen"));
        }
    }
    items.push(simple("einde", "Einde"));
    for (i, it) in items.iter_mut().enumerate() {
        it.id = format!("i{}", i + 1);
    }
    items
}

const DAYS: [&str; 7] = ["maandag", "dinsdag", "woensdag", "donderdag", "vrijdag", "zaterdag", "zondag"];
const MONTHS: [&str; 12] = [
    "januari", "februari", "maart", "april", "mei", "juni", "juli", "augustus", "september", "oktober",
    "november", "december",
];

/// "Zondag 25 oktober"
fn long_date_no_year(d: NaiveDate) -> String {
    let day = DAYS[d.weekday().num_days_from_monday() as usize];
    format!("{}{} {} {}", day[..1].to_uppercase(), &day[1..], d.day(), MONTHS[d.month0() as usize])
}

/// "Zondag 18 oktober 2026"
pub fn long_date(date: &str) -> String {
    match NaiveDate::parse_from_str(date, "%Y-%m-%d") {
        Ok(d) => format!("{} {}", long_date_no_year(d), d.year()),
        Err(_) => date.to_string(),
    }
}

pub fn service_title(svc: &Service) -> String {
    let Ok(d) = NaiveDate::parse_from_str(&svc.date, "%Y-%m-%d") else {
        return format!("{} {}", svc.label, svc.time);
    };
    let mut parts = vec![
        svc.label.clone(),
        format!("{} {} {}", DAYS[d.weekday().num_days_from_monday() as usize], d.day(), MONTHS[d.month0() as usize]),
        format!("{} uur", svc.time),
    ];
    if !svc.predikant.is_empty() {
        parts.push(svc.predikant.clone());
    }
    parts.join(" · ")
}

fn build_slides(svc: &Service) -> Vec<Slide> {
    let mut slides = Vec::new();
    for (idx, it) in svc.items.iter().enumerate() {
        if !it.custom.is_empty() {
            for c in &it.custom {
                slides.push(Slide {
                    item: idx,
                    kind: it.kind.clone(),
                    header: c.header.clone(),
                    sub: c.sub.clone(),
                    lines: c.text.lines().map(String::from).collect(),
                    footer: c.footer.clone(),
                    ..Default::default()
                });
            }
            continue;
        }
        let mut add = |sl: Slide| slides.push(Slide { item: idx, kind: it.kind.clone(), ..sl });
        let problem = |msg: &str| Slide { header: it.label.clone(), sub: msg.to_string(), ..Default::default() };
        match it.kind.as_str() {
            "welkom" => {
                let list = liturgy(svc).into_iter().map(|(_, l)| l).collect();
                add(Slide { header: "Hersteld Hervormde Gemeente Tholen".into(), sub: service_title(svc), list, ..Default::default() });
            }
            "lied" => add(Slide {
                header: it.label.clone(),
                sub: "Lied".into(),
                lines: it.text.lines().map(String::from).collect(),
                ..Default::default()
            }),
            "nazang" if it.psalm == 0 => add(Slide {
                header: it.label.clone(),
                lines: it.text.lines().map(String::from).collect(),
                ..Default::default()
            }),
            "psalm" | "slotzang" | "nazang" => {
                if it.kind == "slotzang" {
                    add(Slide { header: "Slotzang".into(), sub: it.reference.clone(), ..Default::default() });
                }
                let name = crate::texts::psalm(it.psalm).map_or_else(|| format!("Psalm {}", it.psalm), |p| p.naam.clone());
                if it.verses.is_empty() {
                    add(problem(&format!("Geen verzen bij {name}")));
                }
                for v in &it.verses {
                    match crate::texts::psalm_verse(it.psalm, *v) {
                        Ok(lines) => add(Slide { header: format!("{name} : {v}"), sub: it.label.clone(), lines: lines.to_vec(), ..Default::default() }),
                        Err(e) => add(problem(&e.to_string())),
                    }
                }
            }
            "lezing" => match it.passage.map(crate::texts::passage) {
                Some(Ok(verses)) => {
                    let r = it.passage.unwrap();
                    for page in paginate(verses, 650) {
                        add(Slide { header: r.to_string(), sub: it.label.clone(), verses: page, ..Default::default() });
                    }
                }
                Some(Err(e)) => add(problem(&e.to_string())),
                None => add(problem("Geen bijbeltekst opgegeven")),
            },
            "catechismus" => {
                let qas = crate::catechism::for_item(it);
                if qas.is_empty() {
                    add(problem("Onbekende catechismusvraag"));
                }
                for q in qas {
                    add(Slide {
                        header: it.label.clone(),
                        sub: format!("Zondag {}", q.zondag),
                        lines: vec![format!("Vraag {}: {}", q.nummer, q.vraag), format!("Antwoord: {}", q.antwoord)],
                        footer: format!("Zondag {}", q.zondag),
                        ..Default::default()
                    });
                }
            }
            "preek" => {
                let verses = svc.text.passage.and_then(|r| crate::texts::passage(r).ok()).unwrap_or_default();
                let reference = if svc.text.kind == "catechismus" {
                    format!("Heidelbergse Catechismus, {}", svc.text.reference)
                } else {
                    svc.text.reference.clone()
                };
                add(Slide {
                    header: if reference.is_empty() { it.label.clone() } else { format!("Tekst: {reference}") },
                    sub: it.label.clone(),
                    verses,
                    theme: svc.theme.clone(),
                    points: svc.points.clone(),
                    ..Default::default()
                });
            }
            "collecte" => add(Slide {
                header: it.label.clone(),
                lines: it.text.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect(),
                ..Default::default()
            }),
            "gebed" => add(Slide { header: it.label.clone(), ..Default::default() }),
            "mededeling" => add(Slide { header: it.label.clone(), lines: it.text.lines().map(String::from).collect(), ..Default::default() }),
            "einde" => add(Slide {
                header: "Hersteld Hervormde Gemeente Tholen".into(),
                sub: if svc.next.is_empty() { String::new() } else { format!("Volgende dienst: {}", svc.next) },
                footer: "www.hhgtholen.nl".into(),
                ..Default::default()
            }),
            _ => {}
        }
    }
    slides
}

/// The liturgy overview: (item index, line) for every item with a reference.
fn liturgy(svc: &Service) -> Vec<(usize, String)> {
    svc.items
        .iter()
        .enumerate()
        .filter(|(_, o)| !o.reference.is_empty())
        .map(|(i, o)| {
            let line = match o.kind.as_str() {
                "lezing" => format!("Lezen: {}", o.reference),
                "catechismus" => format!("Catechismus: {}", o.reference),
                "nazang" => format!("Na de dienst: {}", o.reference),
                _ => o.reference.clone(),
            };
            (i, line)
        })
        .collect()
}

/// What the boards show, as the church wants it: the liturgy (with the current part
/// marked), the theme during the sermon, and the last psalm and any song after the
/// service. Not every text, so people keep using their own Bible and psalter.
fn board_slides(svc: &Service, slides: &[Slide]) -> Vec<Slide> {
    let list = liturgy(svc);
    let lines: Vec<String> = list.iter().map(|(_, l)| l.clone()).collect();
    slides
        .iter()
        .map(|sl| match sl.kind.as_str() {
            "slotzang" | "nazang" | "einde" => sl.clone(),
            "preek" => Slide { verses: Vec::new(), ..sl.clone() },
            _ => Slide {
                item: sl.item,
                kind: "welkom".into(),
                header: if sl.kind == "welkom" { sl.header.clone() } else { "Liturgie".into() },
                sub: service_title(svc),
                list: lines.clone(),
                active: if sl.kind == "welkom" { None } else { list.iter().position(|(i, _)| *i == sl.item) },
                ..Default::default()
            },
        })
        .collect()
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
