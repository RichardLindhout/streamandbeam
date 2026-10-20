//! The services, their order, and what the screens show right now.

use crate::books::{BibleRef, psalm_ref_string};
use crate::extract::ExtractedService;
use crate::texts::{Verse, read_json, write_json};
use chrono::{Datelike, Local, NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{Mutex, watch};

/// One part of a service, in the order the predikant uses. All references are numbers
/// (Claude maps what is typed or on the website onto them), so nothing is parsed later.
/// Kinds: welkom, psalm, slotzang, nazang (song after the service), lied (a song not in
/// the psalm book, e.g. on a children's singing day), lezing, catechismus, gebed,
/// collecte, preek, belijdenis (NGB, DL, a creed), formulier (baptism, Lord's Supper, …),
/// mededeling (also a named part without text: levensloop, condoleance), zegen (the
/// blessing: the camera on the predikant), einde (the end screen, "Aftiteling").
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
    /// For a belijdenis: ngb, dl, apostolicum, nicea, athanasius; for a formulier its id.
    pub doc: String,
    /// NGB articles, or DL articles within `chapter`.
    pub articles: Vec<u32>,
    pub chapter: u32,
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
            "belijdenis" => crate::confessions::short(&self.doc, self.chapter, &self.articles),
            "formulier" => crate::confessions::form(&self.doc).map(|f| f.kort.clone()).unwrap_or_default(),
            _ => String::new(),
        };
    }
}

impl Item {
    /// As written on the psalm board: "Ps 74:2,20", "Openb.1:9-26", "Zondag 21".
    pub fn short(&self) -> String {
        let verses = self.verses.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
        match self.kind.as_str() {
            "psalm" | "slotzang" | "nazang" if self.psalm > 0 && self.psalm <= 150 => format!("Ps {}:{verses}", self.psalm),
            "psalm" | "slotzang" | "nazang" if self.psalm > 150 => {
                let name = crate::texts::psalm(self.psalm).map(|p| p.naam.clone()).unwrap_or_default();
                format!("{name}:{verses}")
            }
            "lezing" => self.passage.map(|p| p.short()).unwrap_or_default(),
            "catechismus" => {
                let zondag = if self.zondag > 0 { self.zondag } else { crate::catechism::for_item(self).first().map_or(0, |q| q.zondag) };
                if zondag > 0 { format!("Zondag {zondag}") } else { String::new() }
            }
            "lied" | "nazang" => self.label.clone(),
            "belijdenis" => crate::confessions::short(&self.doc, self.chapter, &self.articles),
            _ => String::new(),
        }
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
    /// The operator's own text for the boards; what is not set is made automatically.
    pub board_custom: BoardCustom,
    /// The operator changed this service: fetching the website no longer replaces it.
    pub edited: bool,
    /// What the website gave the last time (see `website_signature`).
    pub website_sig: String,
    /// The website has a different version than when this service was edited; it is kept in
    /// `website` until the operator takes it over or ignores it.
    pub website_changed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub website: Option<WebsiteVersion>,
}

/// The website's version of an edited service, waiting for the operator.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WebsiteVersion {
    pub label: String,
    pub text: Item,
    pub theme: String,
    pub points: Vec<String>,
    pub problems: Vec<String>,
    pub items: Vec<Item>,
    /// The liturgy in one line, to show the operator what changed.
    pub summary: String,
}

impl WebsiteVersion {
    fn of(svc: &Service) -> WebsiteVersion {
        WebsiteVersion {
            label: svc.label.clone(),
            text: svc.text.clone(),
            theme: svc.theme.clone(),
            points: svc.points.clone(),
            problems: svc.problems.clone(),
            items: svc.items.clone(),
            summary: liturgy(svc).into_iter().map(|(_, l)| l).collect::<Vec<_>>().join(" · "),
        }
    }
}

/// A fingerprint of what the website says about a service (parts, references, text and
/// theme), to notice when it changes. Not the collecte text: that comes from the operator.
fn website_signature(svc: &Service) -> String {
    let mut text = format!("{}|{}|{}|{}", svc.label, svc.text.reference, svc.theme, svc.points.join("/"));
    for it in &svc.items {
        text.push_str(&format!("|{}:{}:{}", it.kind, it.label, it.reference));
    }
    // FNV-1a: the same on every computer and every version of the program.
    let hash = text.bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3));
    format!("{hash:016x}")
}

/// A service just read from the website, merged with what is already there: the operator's
/// own details are kept, and an edited (or protected) service is not replaced; then the
/// website's version waits beside it when it differs from last time.
fn merge_fetched(old: Option<&Service>, mut new: Service, protect: bool) -> Service {
    new.website_sig = website_signature(&new);
    let Some(old) = old else { return new };
    new.predikant = old.predikant.clone();
    new.youtube_id = old.youtube_id.clone();
    new.board_custom = old.board_custom.clone();
    if !(old.edited || protect) {
        return new;
    }
    let mut kept = old.clone();
    if old.website_sig != new.website_sig {
        kept.website = Some(WebsiteVersion::of(&new));
        kept.website_changed = true;
        kept.website_sig = new.website_sig.clone();
    }
    kept
}

/// The operator's own version of a church board. None = automatic.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BoardCustom {
    /// The psalm board, one line per ledge ("" = an empty ledge).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub liturgie: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thema: Option<BoardTheme>,
    /// What the slotzang board shows, e.g. "Ps 134:3".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slotzang: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BoardTheme {
    /// The sermon text, e.g. "Joh.3:16".
    pub tekst: String,
    pub thema: String,
    pub punten: Vec<String>,
}

/// What a screen shows.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
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
    last_fetched: i64,
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
    /// What the boards in church show for each slide: the liturgy, the theme and the
    /// last psalm, as the church wants (the stream gets the full texts).
    pub boards: Vec<Slide>,
    /// Text the operator put on the boards directly, over whatever the service shows.
    pub override_board: Option<Slide>,
    /// Whether that text also goes into the stream.
    pub override_stream: bool,
    /// Version of the board background (file time in ms); 0 = the standard wood.
    pub board_background: u64,
    /// When the website was last read successfully (Unix seconds; 0 = never).
    pub last_fetched: i64,
    /// Why the last reading of the website failed; empty when it worked.
    pub fetch_error: String,
    /// The screens still show the slide as it was before the service was changed (see `hold`).
    pub held: bool,
    /// While held: the new version of the board on screen, for the console to edit.
    pub fresh_board: Option<Slide>,
    /// After a change to the service, what the screens showed stays (slide index, stream
    /// slide, board slide) until the operator moves on or chooses to show the change.
    #[serde(skip)]
    hold: Option<(usize, Slide, Slide)>,
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
            last_fetched: saved.last_fetched,
            ..Default::default()
        };
        let mut view = view;
        migrate(&mut view.services);
        view.board_background = background_version(path.parent().unwrap_or(Path::new(".")));
        let (tx, _) = watch::channel(view.clone());
        let show = Arc::new(Show { liturgy_url, path, state: Mutex::new(view), tx });
        let mut st = show.state.lock().await;
        show.changed(&mut st);
        drop(st);
        show
    }

    /// Folder with the saved liturgy, settings and the board background.
    pub fn data_dir(&self) -> PathBuf {
        self.path.parent().map(Path::to_path_buf).unwrap_or_default()
    }

    /// The church's own background picture for the boards, if one was chosen.
    pub fn board_background(&self) -> Option<PathBuf> {
        background_file(&self.data_dir())
    }

    /// Tells every screen that the board background changed, so they load it again.
    pub async fn board_background_changed(&self) {
        let mut st = self.state.lock().await;
        st.board_background = background_version(&self.data_dir());
        self.publish(&st);
    }

    pub fn subscribe(&self) -> watch::Receiver<View> {
        self.tx.subscribe()
    }

    pub fn view(&self) -> View {
        self.tx.borrow().clone()
    }

    /// Reads the liturgy from the website and adds/replaces the services; remembers when
    /// that last worked and why it failed. `live` says whether the stream is live: then the
    /// service on screen is not replaced either.
    pub async fn fetch(&self, live: impl Fn() -> bool) -> anyhow::Result<()> {
        let result = self.fetch_services(live).await;
        let mut st = self.state.lock().await;
        match &result {
            Ok(()) => {
                st.last_fetched = Local::now().timestamp();
                st.fetch_error.clear();
            }
            Err(e) => st.fetch_error = format!("{e:#}"),
        }
        self.publish(&st);
        result
    }

    async fn fetch_services(&self, live: impl Fn() -> bool) -> anyhow::Result<()> {
        let ex = crate::extract::extract_liturgy(&self.liturgy_url).await?;
        let mut st = self.state.lock().await;
        let live = live();
        for e in ex.diensten {
            let svc = Service {
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
            // The service on screen is not replaced while live or once it has begun.
            let protect = st.current == svc.id && (live || st.index > 0);
            let svc = merge_fetched(st.services.iter().find(|s| s.id == svc.id), svc, protect);
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
            Item { kind: "welkom".into(), label: "Startscherm".into(), ..Default::default() },
            Item { kind: "zegen".into(), label: "Zegen".into(), ..Default::default() },
            Item { kind: "einde".into(), label: "Aftiteling".into(), ..Default::default() },
        ];
        for (i, it) in items.iter_mut().enumerate() {
            it.id = format!("i{}", i + 1);
        }
        let svc = Service { id: format!("{date} {time}"), date, time, label, items, ..Default::default() };
        st.current = svc.id.clone();
        st.index = 0;
        st.hold = None;
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
            st.hold = None;
        }
        self.changed(&mut st);
    }

    /// The operator's version of a service; from now on the website does not replace it.
    /// The screens keep what they show unless `show_now`.
    pub async fn save_service(&self, mut svc: Service, show_now: bool) {
        let mut st = self.state.lock().await;
        for it in &mut svc.items {
            it.refresh();
        }
        svc.text.refresh();
        svc.edited = true;
        if let Some(c) = svc.items.iter().find(|i| i.kind == "collecte") {
            st.collecte = c.text.clone();
        }
        self.store(&mut st, svc, show_now);
    }

    /// Puts the changed version of what is on screen on the screens.
    pub async fn show_changes(&self) {
        let mut st = self.state.lock().await;
        if st.hold.take().is_some() {
            self.changed(&mut st);
        }
    }

    /// The website's newer version of an edited service: take it over, or ignore it.
    pub async fn website_choice(&self, id: &str, take: bool) {
        let mut st = self.state.lock().await;
        let Some(mut svc) = st.services.iter().find(|s| s.id == id).cloned() else { return };
        if let (true, Some(w)) = (take, svc.website.take()) {
            svc.label = w.label;
            svc.text = w.text;
            svc.theme = w.theme;
            svc.points = w.points;
            svc.problems = w.problems;
            svc.items = w.items;
            svc.edited = false;
        }
        svc.website = None;
        svc.website_changed = false;
        self.store(&mut st, svc, false);
    }

    /// Stores a service. Changing the service on screen keeps the same part (and slide in
    /// it) on screen, even when parts are added or removed before it.
    /// What the screens show stays the same (also the boards, whose liturgy can change by an
    /// edit elsewhere in the service) until the operator moves on, unless `show_now`.
    fn store(&self, st: &mut View, svc: Service, show_now: bool) {
        let keep = if st.current == svc.id { on_screen(st) } else { None };
        let before = if st.current == svc.id && !show_now {
            st.slides.get(st.index).cloned().zip(Some(st.boards.get(st.index).cloned().unwrap_or_default()))
        } else {
            None
        };
        if show_now {
            st.hold = None;
        }
        upsert(&mut st.services, svc);
        if let Some((id, offset)) = keep {
            st.slides = st.service().map(build_slides).unwrap_or_default();
            let item = st.service().and_then(|s| s.items.iter().position(|i| i.id == id));
            if let Some(item) = item {
                let mine: Vec<usize> = st.slides.iter().enumerate().filter(|(_, s)| s.item == item).map(|(i, _)| i).collect();
                if let Some(&i) = mine.get(offset.min(mine.len().saturating_sub(1))) {
                    st.index = i;
                }
            }
        }
        if let Some((slide, board)) = before {
            st.hold = Some((st.index, slide, board));
        }
        self.changed(st);
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
        st.hold = None;
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
            // Moving on shows the service as it is now.
            if st.hold.take().is_some() {
                self.changed(&mut st);
                return;
            }
        }
        self.publish(&st);
    }

    /// Rebuilds the slides after the services changed.
    /// Puts the operator's own text on the boards right away (None = back to normal).
    pub async fn set_override(&self, slide: Option<Slide>, stream: bool) {
        let mut st = self.state.lock().await;
        st.override_board = slide.map(|s| Slide { kind: "vrij".into(), ..s });
        st.override_stream = stream && st.override_board.is_some();
        self.publish(&st);
    }

    fn changed(&self, st: &mut View) {
        st.slides = st.service().map(build_slides).unwrap_or_default();
        st.boards = st.service().map(|svc| board_slides(svc, &st.slides)).unwrap_or_default();
        st.index = st.index.min(st.slides.len().saturating_sub(1));
        apply_hold(st);
        self.publish(st);
    }

    fn publish(&self, st: &View) {
        let saved = Saved {
            services: st.services.clone(),
            current: st.current.clone(),
            index: st.index,
            collecte: st.collecte.clone(),
            last_fetched: st.last_fetched,
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

/// Keeps the held slide and board on the screens (see `View::hold`), as long as the index is
/// the same and they differ from the new version; otherwise the hold ends.
fn apply_hold(st: &mut View) {
    let i = st.index;
    st.fresh_board = None;
    let Some((at, slide, board)) = st.hold.as_mut() else {
        st.held = false;
        return;
    };
    let ok = *at == i && i < st.slides.len() && i < st.boards.len();
    if ok {
        // The part may have moved in the list; the screens show the same text.
        slide.item = st.slides[i].item;
        board.item = st.boards[i].item;
    }
    if !ok || (st.slides[i] == *slide && st.boards[i] == *board) {
        st.hold = None;
        st.held = false;
        return;
    }
    st.slides[i] = slide.clone();
    st.fresh_board = Some(std::mem::replace(&mut st.boards[i], board.clone()));
    st.held = true;
}

/// The part on screen (its id, never empty) and which of its slides.
fn on_screen(st: &View) -> Option<(String, usize)> {
    let slide = st.slides.get(st.index)?;
    let id = st.service()?.items.get(slide.item)?.id.clone();
    let first = st.slides.iter().position(|s| s.item == slide.item)?;
    (!id.is_empty()).then_some((id, st.index - first))
}

/// Brings services saved by an older version up to date, once at startup: the new names
/// "Startscherm" and "Aftiteling", and a "Zegen" part before the end screen.
fn migrate(services: &mut [Service]) {
    for svc in services.iter_mut() {
        for it in &mut svc.items {
            if it.kind == "welkom" && it.label == "Welkom" {
                it.label = "Startscherm".into();
            }
            if it.kind == "einde" && it.label == "Einde" {
                it.label = "Aftiteling".into();
            }
        }
        // A blessing typed earlier was a mededeling without text: now its own kind.
        for it in &mut svc.items {
            if it.kind == "mededeling" && it.label.trim().eq_ignore_ascii_case("zegen") && it.text.trim().is_empty() && it.custom.is_empty() {
                it.kind = "zegen".into();
            }
        }
        let has_zegen = svc.items.iter().any(|i| i.kind == "zegen");
        let ends = svc.items.last().is_some_and(|i| i.kind == "einde");
        let sung_at_end = svc.items.iter().any(|i| i.kind == "slotzang" || i.kind == "nazang");
        if !has_zegen && ends && sung_at_end {
            let max = svc.items.iter().filter_map(|i| i.id.strip_prefix('i')?.parse::<u32>().ok()).max().unwrap_or(0);
            let zegen = Item { id: format!("i{}", max + 1), kind: "zegen".into(), label: "Zegen".into(), ..Default::default() };
            // Before the end screen, and before a song after the service (that comes after the blessing).
            let mut at = svc.items.len() - 1;
            while at > 0 && svc.items[at - 1].kind == "nazang" {
                at -= 1;
            }
            svc.items.insert(at, zegen);
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
    items.push(simple("welkom", "Startscherm"));
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
    items.push(simple("zegen", "Zegen"));
    items.push(simple("einde", "Aftiteling"));
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
pub(crate) fn long_date_no_year(d: NaiveDate) -> String {
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
            "zegen" => add(Slide { header: if it.label.is_empty() { "Zegen".into() } else { it.label.clone() }, ..Default::default() }),
            "mededeling" => add(Slide { header: it.label.clone(), lines: it.text.lines().map(String::from).collect(), ..Default::default() }),
            "belijdenis" => {
                let pieces = crate::confessions::confession(&it.doc, it.chapter, &it.articles);
                if pieces.is_empty() {
                    add(problem("Deze tekst is (nog) niet beschikbaar"));
                }
                for p in pieces {
                    for page in paginate_paragraphs(&p.paragraphs, 650) {
                        add(Slide { header: p.header.clone(), sub: p.sub.clone(), lines: page, ..Default::default() });
                    }
                }
            }
            "formulier" => match crate::confessions::form(&it.doc) {
                Some(f) => {
                    for part in &f.delen {
                        for page in paginate_paragraphs(&part.tekst, 650) {
                            add(Slide { header: f.kort.clone(), sub: part.kop.clone(), lines: page, ..Default::default() });
                        }
                    }
                }
                None => add(problem("Dit formulier is (nog) niet beschikbaar")),
            },
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

/// The liturgy overview, as on the boards before the sermon: the psalms, the reading,
/// the sermon text and the catechism. (item index, line) per line.
fn liturgy(svc: &Service) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut text_done = false;
    for (i, o) in svc.items.iter().enumerate() {
        let line = match o.kind.as_str() {
            "lezing" => format!("Lezen: {}", o.reference),
            "catechismus" => format!("HC {}", o.reference),
            "nazang" => format!("Na de dienst: {}", o.reference),
            // The text of the sermon, once, where the sermon starts.
            "preek" if !text_done && !svc.text.reference.is_empty() => {
                text_done = true;
                if svc.text.kind == "catechismus" {
                    format!("Tekst: HC {}", svc.text.reference)
                } else {
                    format!("Tekst: {}", svc.text.reference)
                }
            }
            _ => o.reference.clone(),
        };
        if !line.is_empty() && !line.ends_with(": ") {
            out.push((i, line));
        }
    }
    out
}

/// The psalm board as it hangs in church: one line per ledge — the psalms and the
/// reading in order, an empty ledge after the reading, and the catechism Sunday at the
/// bottom ("Zondag 21"). No headings, no texts.
fn board_rows(svc: &Service) -> Vec<String> {
    let mut rows: Vec<String> = Vec::new();
    let mut zondag = String::new();
    let last_reading = svc.items.iter().rposition(|i| i.kind == "lezing");
    for (i, it) in svc.items.iter().enumerate() {
        match it.kind.as_str() {
            "catechismus" => zondag = it.short(),
            "psalm" | "slotzang" | "nazang" | "lezing" | "lied" | "belijdenis" => {
                let line = it.short();
                if !line.is_empty() {
                    rows.push(line);
                }
                if Some(i) == last_reading && svc.items[i + 1..].iter().any(|o| matches!(o.kind.as_str(), "psalm" | "slotzang")) {
                    rows.push(String::new());
                }
            }
            _ => {}
        }
    }
    if !zondag.is_empty() {
        rows.extend([String::new(), String::new(), zondag]);
    }
    rows
}

/// What the boards show, as the church wants it: the psalm board, the theme during the
/// sermon, and the last psalm (and any song after the service). Not every text, so
/// people keep using their own Bible and psalter.
fn board_slides(svc: &Service, slides: &[Slide]) -> Vec<Slide> {
    let custom = &svc.board_custom;
    let rows = custom.liturgie.clone().unwrap_or_else(|| board_rows(svc));
    let mut out: Vec<Slide> = Vec::with_capacity(slides.len());
    for sl in slides {
        let board = match sl.kind.as_str() {
            // The last psalm and the song after the service: only which one, in large letters.
            "slotzang" | "nazang" => Slide {
                item: sl.item,
                kind: "slotzang".into(),
                header: if sl.kind == "nazang" { "Na de dienst".into() } else { "Slotzang".into() },
                sub: match &custom.slotzang {
                    Some(own) if sl.kind == "slotzang" => own.clone(),
                    _ => svc.items.get(sl.item).map(|i| i.short()).unwrap_or_default(),
                },
                ..Default::default()
            },
            // During the sermon: the text and the theme with its points.
            "preek" => match &custom.thema {
                Some(own) => Slide { item: sl.item, kind: "thema".into(), sub: own.tekst.clone(), theme: own.thema.clone(), points: own.punten.clone(), ..Default::default() },
                None => Slide {
                    item: sl.item,
                    kind: "thema".into(),
                    sub: if svc.text.kind == "catechismus" { svc.text.short() } else { svc.text.passage.map(|p| p.short()).unwrap_or_default() },
                    theme: sl.theme.clone(),
                    points: sl.points.clone(),
                    ..Default::default()
                },
            },
            // The blessing: the boards keep what they showed (the slotzang stays up).
            "zegen" if !out.is_empty() => Slide { item: sl.item, ..out[out.len() - 1].clone() },
            _ => Slide { item: sl.item, kind: "bord".into(), list: rows.clone(), ..Default::default() },
        };
        out.push(board);
    }
    out
}

/// Splits paragraphs into slides of roughly max_chars characters (a long paragraph stays
/// whole on its own slide; the text sizes itself to fit).
fn paginate_paragraphs(paragraphs: &[String], max_chars: usize) -> Vec<Vec<String>> {
    let mut pages: Vec<Vec<String>> = Vec::new();
    let mut size = 0;
    for p in paragraphs {
        match pages.last_mut() {
            Some(page) if size + p.len() <= max_chars => {
                size += p.len();
                page.push(p.clone());
            }
            _ => {
                size = p.len();
                pages.push(vec![p.clone()]);
            }
        }
    }
    pages
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

/// File types allowed for the board background.
pub const BACKGROUND_TYPES: [(&str, &str); 3] = [("jpg", "image/jpeg"), ("png", "image/png"), ("webp", "image/webp")];

fn background_file(dir: &Path) -> Option<PathBuf> {
    BACKGROUND_TYPES.iter().map(|(ext, _)| dir.join(format!("bord-achtergrond.{ext}"))).find(|p| p.exists())
}

fn background_version(dir: &Path) -> u64 {
    background_file(dir)
        .and_then(|p| std::fs::metadata(p).ok()?.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, kind: &str, label: &str) -> Item {
        Item { id: id.into(), kind: kind.into(), label: label.into(), ..Default::default() }
    }

    fn kinds(svc: &Service) -> Vec<String> {
        svc.items.iter().map(|i| format!("{}:{}", i.kind, i.label)).collect()
    }

    #[test]
    fn migrate_adds_blessing_before_song_after_service() {
        let mut services = vec![Service {
            items: vec![item("i1", "welkom", "Welkom"), item("i2", "slotzang", "Slotzang"), item("i3", "nazang", "Na de dienst"), item("i4", "einde", "Einde")],
            ..Default::default()
        }];
        migrate(&mut services);
        assert_eq!(kinds(&services[0]), ["welkom:Startscherm", "slotzang:Slotzang", "zegen:Zegen", "nazang:Na de dienst", "einde:Aftiteling"]);
        assert_eq!(services[0].items[2].id, "i5");
        // Once only.
        migrate(&mut services);
        assert_eq!(services[0].items.len(), 5);
    }

    fn psalm(id: &str, label: &str, n: u32, verses: &[u32]) -> Item {
        let mut it = Item { id: id.into(), kind: "psalm".into(), label: label.into(), psalm: n, verses: verses.to_vec(), ..Default::default() };
        it.refresh();
        it
    }

    fn service() -> Service {
        Service {
            id: "2026-10-18 09:30".into(),
            date: "2026-10-18".into(),
            time: "09:30".into(),
            items: vec![item("i1", "welkom", "Startscherm"), psalm("i2", "Voorzang", 1, &[1, 2]), item("i3", "gebed", "Gebed"), psalm("i4", "Zingen", 2, &[1]), item("i5", "einde", "Aftiteling")],
            ..Default::default()
        }
    }

    #[test]
    fn own_boards_replace_the_automatic_ones() {
        let mut svc = service();
        svc.items.insert(4, Item { kind: "preek".into(), label: "Prediking".into(), ..Default::default() });
        svc.items.insert(5, Item { psalm: 3, verses: vec![1], kind: "slotzang".into(), label: "Slotzang".into(), ..Default::default() });
        let slides = build_slides(&svc);
        let auto = board_slides(&svc, &slides);
        assert_eq!(auto[0].list, ["Ps 1:1,2", "Ps 2:1", "Ps 3:1"]);
        svc.board_custom = BoardCustom {
            liturgie: Some(vec!["Ps 84:3".into(), "".into(), "Zondag 5".into()]),
            thema: Some(BoardTheme { tekst: "Joh.3:16".into(), thema: "Gods liefde".into(), punten: vec!["groot".into()] }),
            slotzang: Some("Ps 134:3".into()),
        };
        let own = board_slides(&svc, &slides);
        assert_eq!(own[0].list, ["Ps 84:3", "", "Zondag 5"]);
        let thema = own.iter().find(|b| b.kind == "thema").unwrap();
        assert_eq!((thema.sub.as_str(), thema.theme.as_str(), thema.points.len()), ("Joh.3:16", "Gods liefde", 1));
        assert!(own.iter().any(|b| b.kind == "slotzang" && b.sub == "Ps 134:3"));
    }

    #[test]
    fn fetching_keeps_an_edited_service_and_offers_the_website_version() {
        let site = service();
        // Not edited: replaced, operator details kept.
        let mut old = service();
        old.predikant = "ds. A".into();
        old.items.pop();
        let merged = merge_fetched(Some(&old), site.clone(), false);
        assert_eq!(merged.items.len(), 5);
        assert_eq!(merged.predikant, "ds. A");
        assert!(!merged.website_changed);
        // Edited, website unchanged since: nothing to do.
        let mut edited = merged.clone();
        edited.edited = true;
        edited.items.remove(2);
        let again = merge_fetched(Some(&edited), site.clone(), false);
        assert_eq!(again.items.len(), 4);
        assert!(!again.website_changed && again.website.is_none());
        // Edited, website changed: kept, the website version waits.
        let mut site2 = service();
        site2.items[1] = psalm("i2", "Voorzang", 84, &[3]);
        let changed = merge_fetched(Some(&edited), site2, false);
        assert_eq!(changed.items.len(), 4);
        assert!(changed.website_changed);
        assert!(changed.website.as_ref().unwrap().summary.contains("Psalm 84"));
        // A protected service (on screen) is not replaced either.
        let protected = merge_fetched(Some(&old), site, true);
        assert_eq!(protected.items.len(), 4);
    }

    async fn show_with(svc: Service) -> Arc<Show> {
        let dir = std::env::temp_dir().join(format!("sb-test-{}-{}", std::process::id(), rand_suffix()));
        std::fs::create_dir_all(&dir).unwrap();
        let show = Show::new(dir.join("liturgie.json"), String::new()).await;
        let id = svc.id.clone();
        show.save_service(svc, false).await;
        show.select(id).await;
        show
    }

    fn rand_suffix() -> u128 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    }

    #[tokio::test]
    async fn editing_keeps_the_screens_as_they_are() {
        let show = show_with(service()).await;
        // On "Zingen" (psalm 2, verse 1).
        let at = show.view().slides.iter().position(|s| s.header.contains(": 1") && s.sub == "Zingen").unwrap();
        show.go(Some(at), 0, "").await;
        let screen = show.view().slides[at].clone();
        let board = show.view().boards[at].clone();
        // A part added before it: the same slide stays on screen, at its new place.
        let mut svc = show.view().service().unwrap().clone();
        svc.items.insert(2, psalm("i9", "Zingen", 3, &[1]));
        show.save_service(svc, false).await;
        let v = show.view();
        assert_eq!(v.slides[v.index].header, screen.header);
        assert_eq!(v.slides[v.index].item, 4);
        // The board's liturgy would now have psalm 3 too, but the TVs keep what they showed.
        assert_eq!(v.boards[v.index].list, board.list);
        assert!(v.held);
        assert_eq!(v.fresh_board.as_ref().unwrap().list.len(), board.list.len() + 1);
        // The part on screen changed: still the old text until "show now" or moving on.
        let mut svc = v.service().unwrap().clone();
        svc.items[4] = psalm("i4", "Zingen", 5, &[1]);
        show.save_service(svc, false).await;
        assert_eq!(show.view().slides[show.view().index].header, screen.header);
        show.show_changes().await;
        let v = show.view();
        assert!(!v.held);
        assert!(v.slides[v.index].header.starts_with("Psalm 5"), "{}", v.slides[v.index].header);
        assert_eq!(v.boards[v.index].list.len(), 3);
    }

    #[test]
    fn migrate_turns_typed_blessing_into_its_own_kind() {
        let mut services = vec![Service {
            items: vec![item("i1", "slotzang", "Slotzang"), item("i2", "mededeling", "Zegen"), item("i3", "einde", "Aftiteling")],
            ..Default::default()
        }];
        migrate(&mut services);
        assert_eq!(kinds(&services[0]), ["slotzang:Slotzang", "zegen:Zegen", "einde:Aftiteling"]);
    }
}
