//! The services, their order, and what the screens show right now.

use crate::books::{BibleRef, psalm_ref_string};
use crate::extract::{Extracted, ExtractedService};
use crate::layout::Layout;
use crate::roosters::Roosters;
use crate::songs::{Library, Song};
use crate::texts::{Verse, read_json, write_json};
use chrono::{Datelike, Local, NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tokio::sync::{Mutex, watch};

/// One part of a service, in the order the predikant uses. All references are numbers
/// (crate::parse maps what is typed or on the website onto them), so nothing is parsed later.
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
    /// For a song not in the psalm book ("lied", "nazang" without a psalm): its title in the
    /// song library ("" when it is not in there), and whether the program has its words.
    /// Worked out again at every change (see `annotate`), not read back.
    #[serde(skip_deserializing)]
    pub song: String,
    #[serde(skip_deserializing, rename = "songText")]
    pub song_text: bool,
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
            "nazang" => self.song_title(),
            "belijdenis" => crate::confessions::short(&self.doc, self.chapter, &self.articles),
            "formulier" => crate::confessions::form(&self.doc).map(|f| f.kort.clone()).unwrap_or_default(),
            _ => String::new(),
        };
    }

    /// The title of a song that is not in the psalm book. A song after the service has it in
    /// its text when the label only says when it is sung ("Na de dienst").
    pub(crate) fn song_title(&self) -> String {
        let label = self.label.trim();
        let first = self.text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or_default();
        let only_when = label.is_empty() || label.to_lowercase().contains("na de dienst") || label.eq_ignore_ascii_case("nazang");
        if only_when && !first.is_empty() { first.to_string() } else { label.to_string() }
    }
}

impl Item {
    /// As written on the psalm board: "Ps 74:2,20", "Openb.1:9-26", "Zondag 21".
    pub fn short(&self) -> String {
        let verses = self.verses.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
        match self.kind.as_str() {
            "psalm" | "slotzang" | "nazang" if crate::texts::is_gezang(self.psalm) => format!("{}:{verses}", crate::books::psalm_name(self.psalm)),
            "psalm" | "slotzang" | "nazang" if self.psalm > 0 => format!("Ps {}:{verses}", self.psalm),
            "lezing" => self.passage.map(|p| p.short()).unwrap_or_default(),
            "catechismus" => {
                let zondag = if self.zondag > 0 { self.zondag } else { crate::catechism::for_item(self).first().map_or(0, |q| q.zondag) };
                if zondag > 0 { format!("Zondag {zondag}") } else { String::new() }
            }
            "lied" => self.label.clone(),
            "nazang" => self.song_title(),
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
    /// What to check in the website's liturgy: parts the program could not place, and
    /// where the AI double check reads it differently.
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
    /// From the church's schedules, worked out again at every change (see `annotate`): the
    /// special day ("Heilig Avondmaal") and who operates the stream at this service.
    #[serde(skip_deserializing)]
    pub special_day: String,
    #[serde(skip_deserializing)]
    pub operator: String,
    /// The song of the special day, as the church's Excel has it ("U zij de Glorie" at Easter);
    /// empty on other days. Also worked out at every change.
    #[serde(skip_deserializing)]
    pub feast_song: String,
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
/// theme), to notice when it changes. Not the collecte text: that comes from the
/// collecterooster or the operator.
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
/// `new.website_sig` may already be set: then it is what the program itself read, so a
/// part read by the AI double check does not count as a change of the website.
fn merge_fetched(old: Option<&Service>, mut new: Service, protect: bool) -> Service {
    if new.website_sig.is_empty() {
        new.website_sig = website_signature(&new);
    }
    let Some(old) = old else { return new };
    if new.next.is_empty() {
        new.next = old.next.clone();
    }
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
    /// A quiet note about the last reading: how the AI double check went, or that it is off.
    pub fetch_note: String,
    /// The screens still show the slide as it was before the service was changed (see `hold`).
    pub held: bool,
    /// While held: the new version of the board on screen, for the console to edit.
    pub fresh_board: Option<Slide>,
    /// The screens the church made itself, for the boards and the stream (see `layout`).
    pub layouts: Vec<Layout>,
    /// A screen put on the boards by hand (its id; "" = none), until the operator moves on.
    pub board_layout: String,
    /// A screen put into the stream by hand, likewise.
    pub stream_layout: String,
    /// After a change to the service, what the screens showed stays (slide index, stream
    /// slide, board slide) until the operator moves on or chooses to show the change.
    #[serde(skip)]
    hold: Option<(usize, Slide, Slide)>,
    /// The boards show the full text of the slotzang verses (a setting, see `set_slotzang_text`).
    #[serde(skip)]
    slotzang_text: bool,
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
    check: std::sync::Mutex<AiCheck>,
    /// The church's schedules (collectes, voorzang, who operates); see `roosters`.
    roosters: RwLock<Arc<Roosters>>,
    /// The songs that are not in the psalm book (see `crate::songs`).
    songs: RwLock<Arc<Library>>,
}

/// The last page read and what the AI made of it, so the AI reads each version of the
/// page once.
#[derive(Default)]
struct AiCheck {
    /// Fingerprint of the page's text.
    page: u64,
    html: String,
    own: Extracted,
    /// Claude's reading of `page`, when done.
    ai: Option<(u64, Extracted)>,
    busy: bool,
}

fn fingerprint(text: &str) -> u64 {
    text.bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3))
}

impl Show {
    pub async fn new(path: PathBuf, liturgy_url: String) -> Arc<Show> {
        let saved: Saved = read_json(&path).unwrap_or_default();
        let dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();
        let roosters = crate::roosters::load(&dir);
        let songs = Library::load(&dir);
        let view = View {
            services: saved.services,
            current: saved.current,
            index: saved.index,
            last_fetched: saved.last_fetched,
            ..Default::default()
        };
        let mut view = view;
        migrate(&mut view.services);
        // The collecte comes from the collecterooster now, no longer from the service before.
        for svc in view.services.iter_mut() {
            follow_collecte(svc, &Roosters::default(), &roosters);
        }
        view.board_background = background_version(&dir);
        view.layouts = crate::layout::load(&dir);
        let (tx, _) = watch::channel(view.clone());
        let show = Arc::new(Show {
            liturgy_url,
            path,
            state: Mutex::new(view),
            tx,
            check: Default::default(),
            roosters: RwLock::new(Arc::new(roosters)),
            songs: RwLock::new(Arc::new(songs)),
        });
        let mut st = show.state.lock().await;
        show.changed(&mut st);
        drop(st);
        show
    }

    /// Folder with the saved liturgy, settings and the board background.
    pub fn data_dir(&self) -> PathBuf {
        self.path.parent().map(Path::to_path_buf).unwrap_or_default()
    }

    pub fn roosters(&self) -> Arc<Roosters> {
        self.roosters.read().unwrap().clone()
    }

    /// New schedules from the church's workbook: kept in the data folder (before the built-in
    /// ones from then on), and the collecte of every service follows them, unless the operator
    /// wrote one of his own.
    pub async fn set_roosters(&self, new: Roosters) -> anyhow::Result<()> {
        write_json(&self.data_dir().join("roosters.json"), &new)?;
        let new = Arc::new(new);
        let old = std::mem::replace(&mut *self.roosters.write().unwrap(), new.clone());
        let mut st = self.state.lock().await;
        for svc in st.services.iter_mut() {
            follow_collecte(svc, &old, &new);
        }
        self.changed(&mut st);
        Ok(())
    }

    pub fn songs(&self) -> Arc<Library> {
        self.songs.read().unwrap().clone()
    }

    /// The operator's own songs: kept in the data folder, and the slides follow at once.
    pub async fn set_songs(&self, new: Library) -> anyhow::Result<()> {
        new.save(&self.data_dir())?;
        *self.songs.write().unwrap() = Arc::new(new);
        let mut st = self.state.lock().await;
        self.changed(&mut st);
        Ok(())
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
    /// service on screen is not replaced either. The program reads the page itself; the AI
    /// double check (`ai_check`) comes after, when there is a key.
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
        let html = crate::extract::http_get(&self.liturgy_url).await?;
        let own = crate::extract::read_page(&html, Local::now().date_naive());
        let page = fingerprint(&crate::extract::liturgy_text(&html));
        // The AI's reading of this same page from before, if there is one.
        let (checked, note) = {
            let mut c = self.check.lock().unwrap();
            c.page = page;
            c.html = html;
            c.own = own.clone();
            match &c.ai {
                Some((p, ai)) if *p == page => Some(apply_ai_check(&own, ai)),
                _ => None,
            }
            .map_or((own.clone(), None), |(ex, note)| (ex, Some(note)))
        };
        let roosters = self.roosters();
        let mut st = self.state.lock().await;
        let live = live();
        apply_fetched(&mut st, &own, &checked, &roosters, live);
        enrich_from_kerktijden(&mut st.services).await;
        if st.current.is_empty() || !st.services.iter().any(|s| s.id == st.current) {
            st.current = upcoming(&st.services);
            st.index = 0;
        }
        st.fetch_note = match note {
            Some(note) => note,
            None if !crate::claude::has_key() => "AI-controle staat uit (geen ANTHROPIC_API_KEY)".into(),
            None => "AI-controle bezig…".into(),
        };
        if !own.notes.is_empty() {
            st.fetch_note = format!("{} · {}", own.notes.join(" · "), st.fetch_note);
        }
        self.changed(&mut st);
        Ok(())
    }

    /// The AI double check of the page read last: Claude reads it too, and every difference
    /// becomes a warning with the service. Once per version of the page; without a key, or
    /// when Claude fails, only a quiet note.
    pub async fn ai_check(&self, live: impl Fn() -> bool) {
        if !crate::claude::has_key() {
            return;
        }
        loop {
            let (page, html, own) = {
                let mut c = self.check.lock().unwrap();
                let done = c.ai.as_ref().is_some_and(|(p, _)| *p == c.page);
                if c.busy || done || c.html.is_empty() {
                    return;
                }
                c.busy = true;
                (c.page, c.html.clone(), c.own.clone())
            };
            let result = crate::extract::ask_ai(&html).await;
            let current = {
                let mut c = self.check.lock().unwrap();
                c.busy = false;
                if let Ok(ai) = &result {
                    c.ai = Some((page, ai.clone()));
                }
                c.page == page
            };
            // The page changed meanwhile: check the new one.
            if !current {
                continue;
            }
            let roosters = self.roosters();
            let mut st = self.state.lock().await;
            match result {
                Ok(ai) => {
                    let (checked, note) = apply_ai_check(&own, &ai);
                    apply_fetched(&mut st, &own, &checked, &roosters, live());
                    st.fetch_note = note;
                }
                Err(e) => {
                    eprintln!("AI-controle: {e:#}");
                    st.fetch_note = "AI-controle overgeslagen (AI niet bereikbaar)".into();
                }
            }
            self.changed(&mut st);
            return;
        }
    }

    /// A new service the operator fills in by typing (with the voorzang from the schedule
    /// already in it), or a copy of the liturgy of the service `copy_from`.
    pub async fn new_service(&self, date: String, time: String, label: String, copy_from: Option<String>) {
        let roosters = self.roosters();
        let mut st = self.state.lock().await;
        let source = copy_from.and_then(|id| st.services.iter().find(|s| s.id == id).cloned());
        let svc = match source {
            Some(src) => copied_service(&src, date, time, label, &roosters),
            None => empty_service(date, time, label, &roosters),
        };
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
        let roosters = self.roosters();
        let mut st = self.state.lock().await;
        // A collecte just added (typed) gets the purposes of that date from the collecterooster.
        let known: Vec<String> = st.services.iter().find(|s| s.id == svc.id).map(|s| s.items.iter().map(|i| i.id.clone()).collect()).unwrap_or_default();
        let collecte = roosters.collecte_text(&svc.date);
        for it in svc.items.iter_mut().filter(|i| i.kind == "collecte" && i.text.trim().is_empty() && !known.contains(&i.id)) {
            it.text = collecte.clone();
        }
        for it in &mut svc.items {
            it.refresh();
        }
        svc.text.refresh();
        svc.edited = true;
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
            // The collecte as the operator has it stays (the website has none of its own).
            let own = svc.items.iter().find(|i| i.kind == "collecte" && !i.text.trim().is_empty()).map(|i| i.text.clone());
            svc.label = w.label;
            svc.text = w.text;
            svc.theme = w.theme;
            svc.points = w.points;
            svc.problems = w.problems;
            svc.items = w.items;
            svc.edited = false;
            if let Some(own) = own {
                for it in svc.items.iter_mut().filter(|i| i.kind == "collecte") {
                    it.text = own.clone();
                }
            }
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
            let songs = self.songs();
            st.slides = st.service().map(|s| build_slides(s, &songs)).unwrap_or_default();
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
            // Moving on takes a screen that was put up by hand off again.
            st.board_layout.clear();
            st.stream_layout.clear();
            // Moving on shows the service as it is now.
            if st.hold.take().is_some() {
                self.changed(&mut st);
                return;
            }
        }
        self.publish(&st);
    }

    /// Puts the operator's own text on the boards right away (None = back to normal).
    pub async fn set_override(&self, slide: Option<Slide>, stream: bool) {
        let mut st = self.state.lock().await;
        st.override_board = slide.map(|s| Slide { kind: "vrij".into(), ..s });
        st.override_stream = stream && st.override_board.is_some();
        self.publish(&st);
    }

    /// Adds or changes one of the church's own screens; gives it back with its id.
    pub async fn save_layout(&self, layout: Layout) -> anyhow::Result<Layout> {
        let mut st = self.state.lock().await;
        let mut all = st.layouts.clone();
        let saved = crate::layout::upsert(&mut all, layout);
        crate::layout::save(&self.data_dir(), &all)?;
        st.layouts = all;
        self.publish(&st);
        Ok(saved)
    }

    pub async fn delete_layout(&self, id: &str) -> anyhow::Result<()> {
        let mut st = self.state.lock().await;
        let all: Vec<Layout> = st.layouts.iter().filter(|l| l.id != id).cloned().collect();
        crate::layout::save(&self.data_dir(), &all)?;
        st.layouts = all;
        if st.board_layout == id {
            st.board_layout.clear();
        }
        if st.stream_layout == id {
            st.stream_layout.clear();
        }
        self.publish(&st);
        Ok(())
    }

    /// Puts a screen up by hand (on the boards or in the stream, whichever it is for), or
    /// takes it off again. False when the screen does not exist.
    pub async fn show_layout(&self, id: &str, on: bool) -> bool {
        let mut st = self.state.lock().await;
        let Some(target) = st.layouts.iter().find(|l| l.id == id).map(|l| l.target.clone()) else { return false };
        let slot = if target == crate::layout::BOARD { &mut st.board_layout } else { &mut st.stream_layout };
        if on {
            *slot = id.to_string();
        } else if *slot == id {
            slot.clear();
        }
        if on && target == crate::layout::BOARD {
            st.black = false;
        }
        self.publish(&st);
        true
    }

    /// The setting "Slotzang: volledige tekst op de borden": the boards then show the slotzang
    /// verses with their text, as the church's PowerPoint did, instead of only which psalm.
    pub async fn set_slotzang_text(&self, on: bool) {
        let mut st = self.state.lock().await;
        if st.slotzang_text != on {
            st.slotzang_text = on;
            self.changed(&mut st);
        }
    }

    /// Rebuilds the slides after the services changed.
    fn changed(&self, st: &mut View) {
        let songs = self.songs();
        annotate(&mut st.services, &self.roosters(), &songs);
        st.slides = st.service().map(|s| build_slides(s, &songs)).unwrap_or_default();
        st.boards = st.service().map(|svc| board_slides(svc, &st.slides, st.slotzang_text)).unwrap_or_default();
        st.index = st.index.min(st.slides.len().saturating_sub(1));
        apply_hold(st);
        self.publish(st);
    }

    fn publish(&self, st: &View) {
        let saved = Saved {
            services: st.services.clone(),
            current: st.current.clone(),
            index: st.index,
            last_fetched: st.last_fetched,
        };
        if let Err(e) = write_json(&self.path, &saved) {
            eprintln!("opslaan mislukt: {e}");
        }
        self.tx.send_replace(st.clone());
    }
}

/// The program's reading with the AI's differences added, and the quiet note about it.
fn apply_ai_check(own: &Extracted, ai: &Extracted) -> (Extracted, String) {
    let mut checked = own.clone();
    let (n, notes) = crate::compare::double_check(&mut checked, ai);
    let mut note = match n {
        0 => "AI-controle: geen verschillen".to_string(),
        1 => "AI-controle: 1 verschil, zie nakijken".to_string(),
        n => format!("AI-controle: {n} verschillen, zie nakijken"),
    };
    for extra in notes {
        note.push_str(" · ");
        note.push_str(&extra);
    }
    (checked, note)
}

/// Puts the services read from the website in the list. `own` is the program's own
/// reading, `checked` the same with the AI double check (same services, same order): its
/// fingerprint comes from `own`, so the AI never makes the website look changed. The
/// collecte and the voorzang check come from the schedules (see `website_service`).
fn apply_fetched(st: &mut View, own: &Extracted, checked: &Extracted, roosters: &Roosters, live: bool) {
    for (o, e) in own.diensten.iter().zip(&checked.diensten) {
        let sig = website_signature(&website_service(o, roosters));
        let svc = Service { website_sig: sig, ..website_service(e, roosters) };
        // The service on screen is not replaced while live or once it has begun.
        let protect = st.current == svc.id && (live || st.index > 0);
        let svc = merge_fetched(st.services.iter().find(|s| s.id == svc.id), svc, protect);
        upsert(&mut st.services, svc);
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

/// A service as the website gives it, with the collecte from the collecterooster; when the
/// website's voorzang is not the one in the voorzangrooster, that is among its problems.
fn website_service(e: &ExtractedService, roosters: &Roosters) -> Service {
    let mut problems = e.niet_herkend.clone();
    if let Some(z) = roosters.voorzang(&e.datum, &e.tijd)
        && let Some(site) = website_voorzang(e)
        && let Some(w) = z.differs(&site)
    {
        problems.push(w);
    }
    let mut items = default_items(e, &roosters.collecte_text(&e.datum));
    add_feast_song(&mut items, &roosters.feast_song(&e.datum, &e.tijd));
    number_items(&mut items);
    Service {
        id: format!("{} {}", e.datum, e.tijd),
        date: e.datum.clone(),
        label: e.dienst.clone(),
        time: e.tijd.clone(),
        text: e.preektekst.clone().into_item(),
        theme: e.thema.clone(),
        points: e.punten.clone(),
        problems,
        items,
        ..Default::default()
    }
}

/// The website's voorzang: the part called so, else the first song.
fn website_voorzang(e: &ExtractedService) -> Option<Item> {
    let parsed: Vec<Item> = e.onderdelen.iter().cloned().map(|r| r.into_item()).collect();
    let sung = |i: &&Item| matches!(i.kind.as_str(), "psalm" | "slotzang" | "lied");
    parsed.iter().find(|i| i.label.to_lowercase().contains("voorzang")).or_else(|| parsed.iter().find(sung)).cloned()
}

/// A new service to fill in: start screen, the voorzang from the voorzangrooster (morning or
/// afternoon by its time), blessing and end screen.
fn empty_service(date: String, time: String, label: String, roosters: &Roosters) -> Service {
    let mut items = vec![Item { kind: "welkom".into(), label: "Startscherm".into(), ..Default::default() }];
    items.extend(roosters.voorzang(&date, &time).map(|z| z.item()));
    items.push(Item { kind: "zegen".into(), label: "Zegen".into(), ..Default::default() });
    items.push(Item { kind: "einde".into(), label: "Aftiteling".into(), ..Default::default() });
    add_feast_song(&mut items, &roosters.feast_song(&date, &time));
    number_items(&mut items);
    Service { id: format!("{date} {time}"), date, time, label, items, ..Default::default() }
}

/// The liturgy of `src` (parts, text, theme and points, own boards) at another moment, e.g.
/// Sunday's afternoon service again on a Wednesday to try things out. The collecte is the one
/// of the new date when the collecterooster has it.
fn copied_service(src: &Service, date: String, time: String, label: String, roosters: &Roosters) -> Service {
    let mut items = src.items.clone();
    let collecte = roosters.collecte_text(&date);
    if !collecte.is_empty() {
        for it in items.iter_mut().filter(|i| i.kind == "collecte") {
            it.text = collecte.clone();
        }
    }
    Service {
        id: format!("{date} {time}"),
        date,
        time,
        label,
        text: src.text.clone(),
        theme: src.theme.clone(),
        points: src.points.clone(),
        board_custom: src.board_custom.clone(),
        items,
        // Made by the operator: the website does not replace it.
        edited: true,
        ..Default::default()
    }
}

/// The collecte of a service after the collecterooster changed (`old` → `new`): the new
/// purposes, unless the operator changed the service and wrote a collecte of his own (one that
/// is not empty and not the old schedule's).
fn follow_collecte(svc: &mut Service, old: &Roosters, new: &Roosters) {
    let (was, now) = (old.collecte_text(&svc.date), new.collecte_text(&svc.date));
    let edited = svc.edited;
    if now.is_empty() {
        return;
    }
    for it in svc.items.iter_mut().filter(|i| i.kind == "collecte") {
        if !edited || it.text.trim().is_empty() || it.text == was {
            it.text = now.clone();
        }
    }
}

/// Puts the special day and who operates the stream, from the schedules, beside each service.
fn annotate(services: &mut [Service], roosters: &Roosters, songs: &Library) {
    for svc in services {
        svc.special_day = roosters.special_day(&svc.date, &svc.time);
        svc.operator = roosters.operator(&svc.date, &svc.time);
        svc.feast_song = roosters.feast_song(&svc.date, &svc.time);
        for it in &mut svc.items {
            let found = songs.resolve(it).map(|(s, _)| s);
            it.song = found.map(|s| s.titel.clone()).unwrap_or_default();
            it.song_text = found.is_some_and(Song::has_words);
        }
    }
}

/// On a special day with a song of its own (the Excel's "Speciale dagen": "U zij de Glorie" at
/// Easter) that song comes after the service, unless the service already has a song there.
fn add_feast_song(items: &mut Vec<Item>, song: &str) {
    if song.is_empty() || items.iter().any(|i| i.kind == "nazang" || i.kind == "lied") {
        return;
    }
    let mut it = Item { kind: "nazang".into(), label: "Na de dienst".into(), text: song.into(), ..Default::default() };
    it.refresh();
    let at = items.iter().position(|i| i.kind == "einde").unwrap_or(items.len());
    items.insert(at, it);
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
/// Other parts on the website (a hymn, a form, a creed) stay after the psalm they follow
/// there; a song after the service ("Na de dienst") comes after the blessing.
fn default_items(e: &ExtractedService, collecte: &str) -> Vec<Item> {
    let parsed: Vec<Item> = e.onderdelen.iter().cloned().map(|r| r.into_item()).collect();
    let is_psalm = |i: &Item| i.kind == "psalm" || i.kind == "slotzang";
    let is_reading = |i: &Item| i.kind == "lezing" || i.kind == "catechismus";
    let psalms: Vec<&Item> = parsed.iter().filter(|i| is_psalm(i)).collect();
    let reading: Vec<&Item> = parsed.iter().filter(|i| is_reading(i)).collect();
    // after[k]: the other parts that follow the k-th psalm (after[0]: before the first).
    let mut after: Vec<Vec<Item>> = vec![Vec::new(); psalms.len() + 1];
    let mut at_end: Vec<Item> = Vec::new();
    let mut seen = 0;
    for it in &parsed {
        if is_psalm(it) {
            seen += 1;
        } else if it.kind == "nazang" {
            at_end.push(it.clone());
        } else if !is_reading(it) {
            after[seen].push(it.clone());
        }
    }
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
    items.extend(after[0].iter().cloned());
    for (i, p) in psalms.iter().enumerate() {
        let extra = &after[i + 1];
        if i == 0 {
            items.push(sung(p, "psalm", "Voorzang"));
            items.extend(extra.iter().cloned());
            if n < 3 {
                reading_part(&mut items);
                items.push(simple("preek", "Prediking"));
            }
        } else if i + 1 == n {
            items.push(simple("gebed", "Dankgebed"));
            items.push(sung(p, "slotzang", "Slotzang"));
            items.extend(extra.iter().cloned());
        } else if i + 2 == n {
            items.push(sung(p, "psalm", "Tussenzang"));
            items.extend(extra.iter().cloned());
            items.push(simple("preek", "Prediking"));
        } else if i + 3 == n {
            reading_part(&mut items);
            items.push(Item { text: collecte.into(), ..simple("collecte", "Collecte") });
            items.push(sung(p, "psalm", "Zingen"));
            items.extend(extra.iter().cloned());
            items.push(simple("preek", "Prediking"));
        } else {
            items.push(sung(p, "psalm", "Zingen"));
            items.extend(extra.iter().cloned());
        }
    }
    // No psalms on the website: the reading and the sermon all the same.
    if n == 0 && !reading.is_empty() {
        reading_part(&mut items);
        items.push(simple("preek", "Prediking"));
    }
    items.push(simple("zegen", "Zegen"));
    items.extend(at_end);
    items.push(simple("einde", "Aftiteling"));
    number_items(&mut items);
    items
}

/// The ids of a new service's parts: "i1", "i2", …
fn number_items(items: &mut [Item]) {
    for (i, it) in items.iter_mut().enumerate() {
        it.id = format!("i{}", i + 1);
    }
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

/// "18 oktober 2026"
pub(crate) fn day_month_year(date: &str) -> String {
    match NaiveDate::parse_from_str(date, "%Y-%m-%d") {
        Ok(d) => format!("{} {} {}", d.day(), MONTHS[d.month0() as usize], d.year()),
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

fn build_slides(svc: &Service, songs: &Library) -> Vec<Slide> {
    let mut slides = Vec::new();
    for (idx, it) in svc.items.iter().enumerate() {
        // A song from the song library with its words: verse by verse, like a psalm.
        if it.custom.is_empty()
            && let Some((song, verses)) = songs.resolve(it)
            && song.has_words()
        {
            let sub = if it.kind == "lied" { "Lied".to_string() } else { it.label.clone() };
            for v in verses {
                let header = if song.verzen.len() > 1 { format!("{} : {v}", song.titel) } else { song.titel.clone() };
                let lines = song.verzen.get((v as usize).wrapping_sub(1)).filter(|l| !l.is_empty());
                slides.push(Slide {
                    item: idx,
                    kind: it.kind.clone(),
                    header: header.clone(),
                    sub: if lines.is_some() { sub.clone() } else { format!("{header}: tekst nog invullen") },
                    lines: lines.cloned().unwrap_or_default(),
                    ..Default::default()
                });
            }
            continue;
        }
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
/// reading in order, an empty ledge after the reading, the song after the service as a
/// group of its own under "Na de dienst:" (as on the church's PowerPoint), and the catechism
/// Sunday at the bottom ("Zondag 21"). No other headings, no texts.
fn board_rows(svc: &Service) -> Vec<String> {
    let mut rows: Vec<String> = Vec::new();
    let mut zondag = String::new();
    let mut after_service = false;
    let last_reading = svc.items.iter().rposition(|i| i.kind == "lezing");
    for (i, it) in svc.items.iter().enumerate() {
        match it.kind.as_str() {
            "catechismus" => zondag = it.short(),
            "nazang" => {
                let line = it.short();
                if line.is_empty() {
                    continue;
                }
                if !after_service {
                    after_service = true;
                    if rows.last().is_some_and(|r| !r.is_empty()) {
                        rows.push(String::new());
                    }
                    rows.push("Na de dienst:".into());
                }
                rows.push(line);
            }
            "psalm" | "slotzang" | "lezing" | "lied" | "belijdenis" => {
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
/// people keep using their own Bible and psalter; only with `slotzang_text` (a setting)
/// the verses of the slotzang and of the song after the service come with their text.
fn board_slides(svc: &Service, slides: &[Slide], slotzang_text: bool) -> Vec<Slide> {
    let custom = &svc.board_custom;
    let rows = custom.liturgie.clone().unwrap_or_else(|| board_rows(svc));
    let mut out: Vec<Slide> = Vec::with_capacity(slides.len());
    for sl in slides {
        let board = match sl.kind.as_str() {
            // A verse with its text, like a psalm in the stream. A song after the service that is
            // only a title (one line) keeps the large announcement below.
            "slotzang" | "nazang"
                if slotzang_text
                    && !sl.lines.is_empty()
                    && (sl.lines.len() > 1 || svc.items.get(sl.item).is_some_and(|i| i.psalm > 0)) =>
            {
                Slide { kind: "slotzang".into(), ..sl.clone() }
            }
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
            // During the sermon: the text and the theme with its points, and under them what is
            // sung after the sermon ("Na de preek: Psalm 42 : 1, 5"; the footer).
            "preek" => {
                let footer = after_sermon(svc, sl.item);
                match &custom.thema {
                    Some(own) => Slide { item: sl.item, kind: "thema".into(), sub: own.tekst.clone(), theme: own.thema.clone(), points: own.punten.clone(), footer, ..Default::default() },
                    None => Slide {
                        item: sl.item,
                        kind: "thema".into(),
                        sub: if svc.text.kind == "catechismus" { svc.text.short() } else { svc.text.passage.map(|p| p.short()).unwrap_or_default() },
                        theme: sl.theme.clone(),
                        points: sl.points.clone(),
                        footer,
                        ..Default::default()
                    },
                }
            }
            // The blessing: the boards keep what they showed (the slotzang stays up).
            "zegen" if !out.is_empty() => Slide { item: sl.item, ..out[out.len() - 1].clone() },
            _ => Slide { item: sl.item, kind: "bord".into(), list: rows.clone(), ..Default::default() },
        };
        out.push(board);
    }
    out
}

/// What is sung right after the sermon at item `at`, for the theme board: the first song
/// after it, also when the prayer of thanks comes first (then the slotzang), as the church's
/// PowerPoint had it ("Na de preek: Psalm 42 : 1, 5"). Empty when nothing is sung before the
/// blessing.
fn after_sermon(svc: &Service, at: usize) -> String {
    for it in svc.items.iter().skip(at + 1) {
        match it.kind.as_str() {
            "psalm" | "slotzang" if it.psalm > 0 => return psalm_ref_string(it.psalm, &it.verses),
            "lied" => return it.label.clone(),
            "zegen" | "einde" | "nazang" => break,
            _ => {}
        }
    }
    String::new()
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
pub(crate) mod tests {
    use super::*;

    fn item(id: &str, kind: &str, label: &str) -> Item {
        Item { id: id.into(), kind: kind.into(), label: label.into(), ..Default::default() }
    }

    fn reading(id: &str, book: u32, chapter: u32, from: u32, to: u32) -> Item {
        let mut it = Item { passage: Some(BibleRef { book, chapter, from, to }), ..item(id, "lezing", "Schriftlezing") };
        it.refresh();
        it
    }

    /// The middagdienst of 1 November 2026 (hhgtholen.nl), in the order the program makes:
    /// "Ps. 27: 3-7" and "Ps. 42: 1-5" are verses 3 and 7, 1 and 5.
    pub(crate) fn middagdienst() -> Service {
        let mut text = Item { kind: "catechismus".into(), questions: vec![58], ..Default::default() };
        text.refresh();
        let mut slotzang = psalm("i15", "Slotzang", 43, &[4]);
        slotzang.kind = "slotzang".into();
        Service {
            id: "2026-11-01 16:00".into(),
            date: "2026-11-01".into(),
            time: "16:00".into(),
            label: "Middagdienst".into(),
            text,
            theme: "Verrast door vreugde".into(),
            points: vec!["Nu al".into(), "Straks volkomen".into()],
            items: vec![
                item("i1", "welkom", "Startscherm"),
                psalm("i2", "Voorzang", 90, &[1]),
                psalm("i3", "Zingen", 147, &[1]),
                psalm("i4", "Zingen", 32, &[6]),
                item("i5", "gebed", "Gebed"),
                reading("i6", 35, 3, 17, 19),
                reading("i7", 60, 1, 3, 9),
                reading("i8", 66, 7, 9, 12),
                item("i9", "collecte", "Collecte"),
                psalm("i10", "Zingen", 27, &[3, 7]),
                item("i11", "preek", "Prediking"),
                psalm("i12", "Tussenzang", 42, &[1, 5]),
                item("i13", "preek", "Prediking"),
                item("i14", "gebed", "Dankgebed"),
                slotzang,
                item("i16", "zegen", "Zegen"),
                item("i17", "einde", "Aftiteling"),
            ],
            ..Default::default()
        }
    }

    #[test]
    fn theme_board_names_the_psalm_after_the_sermon() {
        let svc = middagdienst();
        let boards = board_slides(&svc, &build_slides(&svc, &Library::default()), false);
        let themes: Vec<&Slide> = boards.iter().filter(|b| b.kind == "thema").collect();
        assert_eq!(themes.len(), 2);
        assert_eq!((themes[0].sub.as_str(), themes[0].theme.as_str()), ("Zondag 22", "Verrast door vreugde"));
        // The first part: the tussenzang; the second, with the prayer of thanks first: the slotzang.
        assert_eq!(themes[0].footer, "Psalm 42 : 1, 5");
        assert_eq!(themes[1].footer, "Psalm 43 : 4");
        // Nothing sung after the sermon: no line.
        let mut short = middagdienst();
        short.items.retain(|i| !matches!(i.id.as_str(), "i12" | "i13" | "i14" | "i15"));
        let boards = board_slides(&short, &build_slides(&short, &Library::default()), false);
        assert_eq!(boards.iter().find(|b| b.kind == "thema").unwrap().footer, "");
    }

    #[test]
    fn psalm_board_puts_the_song_after_the_service_apart() {
        let mut svc = middagdienst();
        let mut song = Item { text: "U zij de glorie".into(), ..item("i18", "nazang", "Na de dienst") };
        song.refresh();
        assert_eq!(song.reference, "U zij de glorie");
        svc.items.insert(16, song);
        assert_eq!(
            board_rows(&svc),
            ["Ps 90:1", "Ps 147:1", "Ps 32:6", "Hab.3:17-19", "1 Petr.1:3-9", "Openb.7:9-12", "", "Ps 27:3,7", "Ps 42:1,5", "Ps 43:4", "", "Na de dienst:", "U zij de glorie"]
        );
        let slides = build_slides(&svc, &Library::default());
        assert!(slides[0].list.iter().any(|l| l == "Na de dienst: U zij de glorie"), "{:?}", slides[0].list);
        let boards = board_slides(&svc, &slides, false);
        let last = boards.iter().rev().find(|b| b.kind == "slotzang").unwrap();
        assert_eq!((last.header.as_str(), last.sub.as_str()), ("Na de dienst", "U zij de glorie"));
        // A title as the label stays the title.
        let named = Item { kind: "nazang".into(), label: "Ere zij God".into(), ..Default::default() };
        assert_eq!(named.short(), "Ere zij God");
    }

    #[test]
    fn slotzang_with_its_text_on_the_boards_when_set() {
        let svc = middagdienst();
        let slides = build_slides(&svc, &Library::default());
        let slotzang = |boards: Vec<Slide>| -> Vec<Slide> { boards.into_iter().zip(&slides).filter(|(_, s)| s.kind == "slotzang").map(|(b, _)| b).collect() };
        // Off (the default): only which psalm, also during the verse.
        let off = slotzang(board_slides(&svc, &slides, false));
        assert_eq!(off.len(), 2);
        assert!(off.iter().all(|b| b.lines.is_empty() && b.sub == "Ps 43:4"));
        // On: the announcement, then the verse with its text, as the church's PowerPoint had it.
        let on = slotzang(board_slides(&svc, &slides, true));
        assert!(on[0].lines.is_empty() && on[0].sub == "Ps 43:4");
        assert_eq!((on[1].kind.as_str(), on[1].header.as_str()), ("slotzang", "Psalm 43 : 4"));
        assert_eq!(on[1].lines, crate::texts::psalm_verse(43, 4).unwrap());
    }

    #[tokio::test]
    async fn the_slotzang_setting_changes_the_boards_at_once() {
        let show = show_with(middagdienst()).await;
        let verse = show.view().slides.iter().position(|s| s.kind == "slotzang" && !s.lines.is_empty()).unwrap();
        assert!(show.view().boards[verse].lines.is_empty());
        show.set_slotzang_text(true).await;
        assert!(!show.view().boards[verse].lines.is_empty());
        show.set_slotzang_text(false).await;
        assert!(show.view().boards[verse].lines.is_empty());
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
        let slides = build_slides(&svc, &Library::default());
        let auto = board_slides(&svc, &slides, false);
        assert_eq!(auto[0].list, ["Ps 1:1,2", "Ps 2:1", "Ps 3:1"]);
        svc.board_custom = BoardCustom {
            liturgie: Some(vec!["Ps 84:3".into(), "".into(), "Zondag 5".into()]),
            thema: Some(BoardTheme { tekst: "Joh.3:16".into(), thema: "Gods liefde".into(), punten: vec!["groot".into()] }),
            slotzang: Some("Ps 134:3".into()),
        };
        let own = board_slides(&svc, &slides, false);
        assert_eq!(own[0].list, ["Ps 84:3", "", "Zondag 5"]);
        let thema = own.iter().find(|b| b.kind == "thema").unwrap();
        assert_eq!((thema.sub.as_str(), thema.theme.as_str(), thema.points.len()), ("Joh.3:16", "Gods liefde", 1));
        // The psalm after the sermon still comes from the order.
        assert_eq!(thema.footer, "Psalm 3 : 1");
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

    fn raw_psalm(label: &str, psalm: u32, verses: &[u32]) -> crate::claude::RawItem {
        crate::claude::RawItem { soort: "psalm".into(), opschrift: label.into(), psalm, verzen: verses.to_vec(), herkend: true, ..Default::default() }
    }

    /// Sunday 1 November 2026 as on the website: "Voorzang: Ps.31: 19", "Ps. 74: 8-11", …
    fn website_morning(voorzang: &[u32]) -> ExtractedService {
        ExtractedService {
            datum: "2026-11-01".into(),
            tijd: "09:30".into(),
            dienst: "Morgendienst".into(),
            onderdelen: vec![
                raw_psalm("Voorzang", 31, voorzang),
                raw_psalm("Zingen", 74, &[8, 11]),
                raw_psalm("Zingen", 77, &[7]),
                raw_psalm("Zingen", 51, &[6, 7]),
                raw_psalm("Zingen", 27, &[7]),
                raw_psalm("Zingen", 130, &[3, 4]),
            ],
            preektekst: raw_psalm("Tekst", 0, &[]),
            thema: String::new(),
            punten: vec![],
            niet_herkend: vec![],
        }
    }

    #[test]
    fn website_service_gets_the_collecte_and_the_voorzang_is_checked() {
        let r = crate::roosters::builtin();
        let svc = website_service(&website_morning(&[19]), &r);
        let collecte = svc.items.iter().find(|i| i.kind == "collecte").unwrap();
        assert_eq!(collecte.text, "1. Najaarszendingscollecte\n2. Pastoraat & Catechese\n3. Aflossing Bouwfonds");
        assert!(svc.problems.is_empty(), "{:?}", svc.problems);
        // Another voorzang than in the voorzangrooster: a warning with the service.
        let svc = website_service(&website_morning(&[18]), &r);
        assert_eq!(svc.problems, ["Voorzang op de website (Psalm 31 : 18) is anders dan in het voorzangrooster (Psalm 31 : 19)"]);
        // A date the collecterooster does not have: no collecte text, not the one of the service before.
        let mut later = website_morning(&[19]);
        later.datum = "2027-03-07".into();
        assert_eq!(website_service(&later, &r).items.iter().find(|i| i.kind == "collecte").unwrap().text, "");
    }

    #[test]
    fn empty_service_has_the_voorzang_from_the_schedule() {
        let r = crate::roosters::builtin();
        let morning = empty_service("2026-11-08".into(), "09:30".into(), "Morgendienst".into(), &r);
        assert_eq!(kinds(&morning), ["welkom:Startscherm", "psalm:Voorzang", "zegen:Zegen", "einde:Aftiteling"]);
        assert_eq!(morning.items[1].reference, "Psalm 34 : 11");
        assert_eq!(morning.items.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(), ["i1", "i2", "i3", "i4"]);
        let afternoon = empty_service("2026-11-08".into(), "16:00".into(), "Middagdienst".into(), &r);
        assert_eq!(afternoon.items[1].reference, "Psalm 43 : 3");
        // A gezang: sung like a psalm now that the program has the gezangen.
        let advent = empty_service("2026-12-13".into(), "09:30".into(), "Morgendienst".into(), &r);
        assert_eq!(kinds(&advent)[1], "psalm:Voorzang");
        assert_eq!(advent.items[1].reference, "De Lofzang van Maria : 1");
        // Nothing in the schedule: only the fixed parts.
        assert_eq!(empty_service("2027-09-01".into(), "19:30".into(), "Bidstond".into(), &r).items.len(), 3);
    }

    #[test]
    fn copied_service_keeps_the_liturgy_with_the_collecte_of_its_own_date() {
        let r = crate::roosters::builtin();
        let mut src = service();
        src.theme = "Verrast door vreugde".into();
        src.points = vec!["Nu al".into(), "Straks volkomen".into()];
        src.predikant = "ds. A".into();
        src.youtube_id = "abc".into();
        src.items.insert(4, Item { id: "i6".into(), kind: "collecte".into(), label: "Collecte".into(), text: "Eigen tekst".into(), ..Default::default() });
        let copy = copied_service(&src, "2026-11-25".into(), "19:30".into(), "Test".into(), &r);
        assert_eq!((copy.id.as_str(), copy.label.as_str(), copy.theme.as_str(), copy.points.len()), ("2026-11-25 19:30", "Test", "Verrast door vreugde", 2));
        assert_eq!(kinds(&copy), kinds(&src));
        assert_eq!(copy.items[4].text, "1. Diaconie (oogstcollecte)\n2. Oogstcollecte");
        assert!(copy.edited && copy.predikant.is_empty() && copy.youtube_id.is_empty());
        // A date without a collecte in the schedule: the copied text stays.
        let copy = copied_service(&src, "2027-09-01".into(), "19:30".into(), "Test".into(), &r);
        assert_eq!(copy.items[4].text, "Eigen tekst");
    }

    #[test]
    fn the_operators_own_collecte_wins_over_the_schedule() {
        let old = crate::roosters::Roosters::default();
        let new = crate::roosters::builtin();
        let with = |text: &str, edited: bool| Service {
            date: "2026-11-01".into(),
            edited,
            items: vec![Item { kind: "collecte".into(), text: text.into(), ..Default::default() }],
            ..Default::default()
        };
        let schedule = new.collecte_text("2026-11-01");
        for (text, edited, after) in [("", true, schedule.as_str()), ("Eigen doel", true, "Eigen doel"), ("1. Vorige week", false, schedule.as_str())] {
            let mut svc = with(text, edited);
            follow_collecte(&mut svc, &old, &new);
            assert_eq!(svc.items[0].text, after, "{text}");
        }
        // The old schedule's text follows the new schedule; the operator's own does not.
        let mut svc = with(&schedule, true);
        let changed = Roosters { collectes: vec![crate::roosters::CollecteDag { datum: "2026-11-01".into(), dag: String::new(), doelen: vec!["Nieuw".into()] }], ..Default::default() };
        follow_collecte(&mut svc, &new, &changed);
        assert_eq!(svc.items[0].text, "1. Nieuw");
    }

    #[tokio::test]
    async fn a_typed_collecte_gets_the_schedule_and_the_operator_can_clear_it() {
        let mut svc = service();
        svc.id = "2026-11-08 09:30".into();
        svc.date = "2026-11-08".into();
        let show = show_with(svc).await;
        let mut svc = show.view().service().unwrap().clone();
        svc.items.insert(3, Item { id: "i77".into(), kind: "collecte".into(), label: "Collecte".into(), ..Default::default() });
        show.save_service(svc, false).await;
        let mut svc = show.view().service().unwrap().clone();
        assert_eq!(svc.items[3].text, "1. Diaconie\n2. Instandhouding predikantsplaats\n3. Plaatselijk jeugdwerk");
        svc.items[3].text = String::new();
        show.save_service(svc, false).await;
        assert_eq!(show.view().service().unwrap().items[3].text, "");
    }

    #[tokio::test]
    async fn taking_the_website_version_keeps_the_own_collecte() {
        let mut svc = service();
        svc.items.insert(3, Item { id: "i9".into(), kind: "collecte".into(), label: "Collecte".into(), text: "Eigen doel".into(), ..Default::default() });
        let mut site = service();
        site.items.insert(3, Item { id: "i9".into(), kind: "collecte".into(), label: "Collecte".into(), text: "1. Rooster".into(), ..Default::default() });
        svc.website = Some(WebsiteVersion::of(&site));
        svc.website_changed = true;
        let show = show_with(svc).await;
        show.website_choice("2026-10-18 09:30", true).await;
        let v = show.view();
        let svc = v.service().unwrap();
        assert!(!svc.website_changed && svc.website.is_none());
        assert_eq!(svc.items[3].text, "Eigen doel");
    }

    #[tokio::test]
    async fn services_show_the_special_day_and_who_operates() {
        let mut svc = service();
        (svc.id, svc.date, svc.time) = ("2026-12-20 16:00".into(), "2026-12-20".into(), "16:00".into());
        let show = show_with(svc).await;
        let v = show.view();
        assert_eq!(v.service().unwrap().special_day, "Dankzegging Heilig Avondmaal");
        assert_eq!(v.service().unwrap().operator, "");
        let mut svc = service();
        (svc.id, svc.date, svc.time) = ("2024-07-14 09:30".into(), "2024-07-14".into(), "09:30".into());
        show.save_service(svc, false).await;
        let v = show.view();
        assert_eq!(v.services.iter().find(|s| s.id == "2024-07-14 09:30").unwrap().operator, "Bediener 3/Bediener 4");
    }

    #[tokio::test]
    async fn any_service_can_be_chosen_by_hand() {
        let show = show_with(service()).await;
        let mut other = service();
        (other.id, other.date, other.time) = ("2026-11-04 19:30".into(), "2026-11-04".into(), "19:30".into());
        show.save_service(other, false).await;
        let mut old = service();
        (old.id, old.date) = ("2025-01-05 09:30".into(), "2025-01-05".into());
        show.save_service(old, false).await;
        for id in ["2026-11-04 19:30", "2025-01-05 09:30", "2026-10-18 09:30"] {
            show.select(id.into()).await;
            let v = show.view();
            assert_eq!(v.current, id);
            assert_eq!((v.index, v.slides.is_empty()), (0, false));
        }
    }

    #[tokio::test]
    async fn at_start_the_collecte_comes_from_the_schedule() {
        let dir = std::env::temp_dir().join(format!("sb-test-{}-{}", std::process::id(), rand_suffix()));
        std::fs::create_dir_all(&dir).unwrap();
        // Saved by an older version: the collecte of the service before, copied.
        let mut svc = service();
        (svc.id, svc.date) = ("2026-11-01 09:30".into(), "2026-11-01".into());
        svc.items.insert(3, Item { id: "i9".into(), kind: "collecte".into(), label: "Collecte".into(), text: "1. Vorige week".into(), ..Default::default() });
        let mut edited = svc.clone();
        (edited.id, edited.time, edited.edited) = ("2026-11-01 16:00".into(), "16:00".into(), true);
        edited.items[3].text = "Eigen doel".into();
        let saved = serde_json::json!({ "services": [svc, edited], "collecte": "1. Vorige week" });
        std::fs::write(dir.join("liturgie.json"), serde_json::to_vec(&saved).unwrap()).unwrap();
        let show = Show::new(dir.join("liturgie.json"), String::new()).await;
        let v = show.view();
        assert_eq!(v.services[0].items[3].text, "1. Najaarszendingscollecte\n2. Pastoraat & Catechese\n3. Aflossing Bouwfonds");
        assert_eq!(v.services[1].items[3].text, "Eigen doel");
    }

    #[test]
    fn website_service_in_the_usual_order_with_its_hymn() {
        let html = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/kerkdiensten-2026-11-01.html")).unwrap();
        let ex = crate::extract::read_page(&html, NaiveDate::from_ymd_opt(2026, 11, 4).unwrap());
        let r = crate::roosters::builtin();
        let morning = website_service(&ex.diensten[0], &r);
        assert_eq!(kinds(&morning), [
            "welkom:Startscherm",
            "psalm:Voorzang",
            "psalm:Zingen",
            "psalm:Zingen",
            "gebed:Gebed",
            "lezing:Schriftlezing",
            "collecte:Collecte",
            "psalm:Zingen",
            "preek:Prediking",
            "psalm:Tussenzang",
            "preek:Prediking",
            "gebed:Dankgebed",
            "slotzang:Slotzang",
            "lied:Een vaste burcht is onze God",
            "zegen:Zegen",
            "einde:Aftiteling",
        ]);
        assert_eq!(morning.items[12].reference, "Psalm 130 : 3, 4");
        assert_eq!(morning.text.reference, "1 Samuël 13:11");
        // The program's own reading also gets the collecte from the collecterooster, and its
        // voorzang is the one in the voorzangrooster (Ps. 31: 19), so nothing to check.
        assert_eq!(morning.items[6].text, "1. Najaarszendingscollecte\n2. Pastoraat & Catechese\n3. Aflossing Bouwfonds");
        assert!(morning.problems.iter().all(|p| !p.contains("voorzangrooster")), "{:?}", morning.problems);
        let afternoon = website_service(&ex.diensten[1], &r);
        assert_eq!(afternoon.items.iter().filter(|i| i.kind == "lezing").count(), 3);
        assert_eq!((afternoon.text.kind.as_str(), afternoon.text.reference.as_str()), ("catechismus", "vraag 58"));
        assert_eq!(afternoon.items[1].reference, "Psalm 90 : 1");
        assert!(afternoon.items.iter().any(|i| i.reference == "Psalm 27 : 3, 7"));
        assert!(afternoon.problems.iter().all(|p| !p.contains("voorzangrooster")), "{:?}", afternoon.problems);
        // Another voorzang on the website than in the voorzangrooster: a warning.
        let mut other = ex.diensten[1].clone();
        other.onderdelen[0].verzen = vec![2];
        assert!(website_service(&other, &r).problems.iter().any(|p| p.contains("voorzangrooster (Psalm 90 : 1)")));
    }

    #[test]
    fn ai_double_check_never_makes_the_website_look_changed() {
        use crate::claude::RawItem;
        let psalm = |n: u32, v: &[u32], bron: &str| RawItem { soort: "psalm".into(), psalm: n, verzen: v.to_vec(), herkend: true, bron: bron.into(), ..Default::default() };
        let unknown = RawItem { soort: "mededeling".into(), opschrift: "Ps. 18: Vz".into(), bron: "Ps. 18: Vz".into(), niet_begrepen: true, ..Default::default() };
        let svc = ExtractedService {
            datum: "2026-11-08".into(),
            tijd: "09:30".into(),
            dienst: "Morgendienst".into(),
            onderdelen: vec![psalm(90, &[1], "Ps. 90: 1"), unknown, psalm(27, &[3, 7], "Ps. 27: 3-7")],
            ..Default::default()
        };
        let own = Extracted { diensten: vec![svc], ..Default::default() };
        let mut ai = own.clone();
        ai.diensten[0].onderdelen[1] = psalm(18, &[1], "");
        ai.diensten[0].onderdelen[2].verzen = vec![3, 4, 5, 6, 7];
        let (checked, note) = apply_ai_check(&own, &ai);
        assert_eq!(note, "AI-controle: 2 verschillen, zie nakijken");
        // Not edited: the checked version, with the AI's psalm marked and the warning.
        let mut st = View::default();
        let r = Roosters::default();
        apply_fetched(&mut st, &own, &own, &r, false);
        let sig = st.services[0].website_sig.clone();
        apply_fetched(&mut st, &own, &checked, &r, false);
        let s = &st.services[0];
        assert_eq!(s.website_sig, sig);
        assert!(s.items.iter().any(|i| i.psalm == 18 && i.problem.contains("door AI gelezen")));
        assert!(s.problems.iter().any(|p| p.starts_with("AI leest 'Ps. 27: 3-7' anders")), "{:?}", s.problems);
        // Edited by the operator: kept, and no "website changed" because of the AI.
        st.services[0].edited = true;
        apply_fetched(&mut st, &own, &own, &r, false);
        apply_fetched(&mut st, &own, &checked, &r, false);
        assert!(!st.services[0].website_changed);
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
