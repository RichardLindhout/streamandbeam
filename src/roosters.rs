//! The church's schedules, from its Excel workbook (Streamupdate.xlsm): the collectes and
//! special days per date (tab Collecterooster), the voorzang of each service (Voorzang) and
//! who operates the stream (Streamdienst). Read with `streamandbeam importeer-excel <bestand>`
//! or from the settings; the program has a copy built in (assets/data/roosters.json) and a
//! newer import in <data>/roosters.json goes before it. The workbook is only read: no macros.

use crate::excel::{Cell, Sheet, Workbook};
use crate::show::Item;
use anyhow::{Result, bail};
use chrono::{Local, NaiveDate, NaiveTime};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::LazyLock;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Roosters {
    /// The workbook it came from (file name) and when it was read ("2026-11-04 20:30").
    pub bron: String,
    pub ingelezen: String,
    pub collectes: Vec<CollecteDag>,
    pub voorzang: Vec<VoorzangDag>,
    pub bediening: Vec<BedieningDag>,
}

/// One date of the collecterooster.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CollecteDag {
    pub datum: String, // 2026-11-01
    /// The special day ("Heilig Avondmaal", "Biddag", "1e Paasdag"); empty on an ordinary Sunday.
    pub dag: String,
    /// The purposes in order: diaconie, kerkvoogdij, extra. A weekday often has two.
    pub doelen: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VoorzangDag {
    pub datum: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub morgen: Option<Zang>,
    /// The afternoon column, also for an evening service.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub middag: Option<Zang>,
}

/// One voorzang: a psalm or one of the gezangen, with its verses.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Zang {
    /// As written in the workbook, e.g. "Ps. 27: 3-7" or "LvM:1".
    pub tekst: String,
    /// "psalm" or "gezang".
    pub soort: String,
    /// The psalm, or the number of the gezang in the workbook's own list (tab Input).
    pub nummer: u32,
    /// "Psalm 27", "Lofzang van Maria".
    pub naam: String,
    pub verzen: Vec<u32>,
}

/// Who operates the stream on a date (tab Streamdienst).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BedieningDag {
    pub datum: String,
    pub morgen: String,
    pub middag: String,
    /// What else is written beside it, e.g. "Bediener 5 niet".
    pub notities: Vec<String>,
}

/// Morning or afternoon/evening, as the schedules split a day: before 13:00 is morning.
pub fn morning(time: &str) -> bool {
    NaiveTime::parse_from_str(time.trim(), "%H:%M").map_or(true, |t| t < NaiveTime::from_hms_opt(13, 0, 0).unwrap())
}

/// The schedules as the program starts with them: an import in `dir`, else the built-in copy.
pub fn load(dir: &Path) -> Roosters {
    let own = dir.join("roosters.json");
    if own.exists() {
        match crate::texts::read_json(&own) {
            Ok(r) => return r,
            Err(e) => eprintln!("{}: {e:#}; het ingebouwde rooster wordt gebruikt", own.display()),
        }
    }
    builtin()
}

pub fn builtin() -> Roosters {
    serde_json::from_str(include_str!("../assets/data/roosters.json")).expect("roosters.json")
}

impl Roosters {
    pub fn collecte(&self, date: &str) -> Option<&CollecteDag> {
        self.collectes.iter().find(|c| c.datum == date)
    }

    /// The collecte purposes as the collecte slide shows them, one per line:
    /// "1. Diaconie\n2. Kerkvoogdij\n3. Verwarming"; empty when the date is not in the schedule.
    pub fn collecte_text(&self, date: &str) -> String {
        let Some(c) = self.collecte(date) else { return String::new() };
        c.doelen.iter().enumerate().map(|(i, d)| format!("{}. {d}", i + 1)).collect::<Vec<_>>().join("\n")
    }

    pub fn voorzang(&self, date: &str, time: &str) -> Option<&Zang> {
        let day = self.voorzang.iter().find(|v| v.datum == date)?;
        if morning(time) { day.morgen.as_ref() } else { day.middag.as_ref() }
    }

    /// The special day of a service, as its name: "Heilig Avondmaal", and in the afternoon
    /// "Dankzegging Heilig Avondmaal"; "Oudejaarsavond" in the evening. Empty on other days.
    pub fn special_day(&self, date: &str, time: &str) -> String {
        let day = self.collecte(date).map(|c| c.dag.as_str()).unwrap_or_default();
        match day {
            "Heilig Avondmaal" if !morning(time) => "Dankzegging Heilig Avondmaal".into(),
            "Oudejaarsdag" if !morning(time) => "Oudejaarsavond".into(),
            _ => day.to_string(),
        }
    }

    /// Who operates the stream at this service, from the roster; empty when nobody is in it.
    pub fn operator(&self, date: &str, time: &str) -> String {
        let Some(b) = self.bediening.iter().find(|b| b.datum == date) else { return String::new() };
        if morning(time) { b.morgen.clone() } else { b.middag.clone() }
    }

    pub fn summary(&self) -> Summary {
        let span = |dates: Vec<&str>| Span {
            aantal: dates.len(),
            van: dates.iter().min().map(|d| d.to_string()).unwrap_or_default(),
            tot: dates.iter().max().map(|d| d.to_string()).unwrap_or_default(),
        };
        Summary {
            collectes: span(self.collectes.iter().filter(|c| !c.doelen.is_empty()).map(|c| c.datum.as_str()).collect()),
            dagen: span(self.collectes.iter().filter(|c| !c.dag.is_empty()).map(|c| c.datum.as_str()).collect()),
            voorzang: span(self.voorzang.iter().map(|v| v.datum.as_str()).collect()),
            bediening: span(self.bediening.iter().map(|b| b.datum.as_str()).collect()),
        }
    }

    /// Every date from `from` on (for `days` days) that is in one of the schedules.
    pub fn upcoming(&self, from: NaiveDate, days: u64) -> Vec<Upcoming> {
        let (first, last) = (from.to_string(), (from + chrono::Days::new(days)).to_string());
        let mut dates: Vec<&str> = self
            .collectes
            .iter()
            .map(|c| c.datum.as_str())
            .chain(self.voorzang.iter().map(|v| v.datum.as_str()))
            .chain(self.bediening.iter().map(|b| b.datum.as_str()))
            .filter(|d| *d >= first.as_str() && *d <= last.as_str())
            .collect();
        dates.sort();
        dates.dedup();
        dates
            .into_iter()
            .map(|d| {
                let v = self.voorzang.iter().find(|v| v.datum == d);
                let b = self.bediening.iter().find(|b| b.datum == d);
                Upcoming {
                    datum: d.to_string(),
                    dag: self.collecte(d).map(|c| c.dag.clone()).unwrap_or_default(),
                    collectes: self.collecte(d).map(|c| c.doelen.clone()).unwrap_or_default(),
                    voorzang_morgen: v.and_then(|v| v.morgen.as_ref()).map(Zang::display).unwrap_or_default(),
                    voorzang_middag: v.and_then(|v| v.middag.as_ref()).map(Zang::display).unwrap_or_default(),
                    bediening_morgen: b.map(|b| b.morgen.clone()).unwrap_or_default(),
                    bediening_middag: b.map(|b| b.middag.clone()).unwrap_or_default(),
                    notities: b.map(|b| b.notities.clone()).unwrap_or_default(),
                }
            })
            .collect()
    }
}

/// How many dates each schedule has, and from when to when.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Summary {
    pub collectes: Span,
    /// The special days (from the collecterooster).
    pub dagen: Span,
    pub voorzang: Span,
    pub bediening: Span,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Span {
    pub aantal: usize,
    pub van: String,
    pub tot: String,
}

/// One date in the list of what is coming, for the settings.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Upcoming {
    pub datum: String,
    pub dag: String,
    pub collectes: Vec<String>,
    pub voorzang_morgen: String,
    pub voorzang_middag: String,
    pub bediening_morgen: String,
    pub bediening_middag: String,
    pub notities: Vec<String>,
}

impl Zang {
    /// "Psalm 27 : 3, 7" or "Lofzang van Maria : 1".
    pub fn display(&self) -> String {
        let vs: Vec<String> = self.verzen.iter().map(u32::to_string).collect();
        if vs.is_empty() { self.naam.clone() } else { format!("{} : {}", self.naam, vs.join(", ")) }
    }

    /// The psalm number the program uses for it (151 and up for the gezangen it has the text of).
    fn program_number(&self) -> Option<u32> {
        if self.soort == "psalm" {
            return Some(self.nummer);
        }
        let key = gezang_key(&self.naam)?;
        crate::texts::gezangen().into_iter().find(|(_, name)| gezang_key(name) == Some(key)).map(|(n, _)| n)
    }

    /// The voorzang as a part of a new service.
    pub fn item(&self) -> Item {
        let mut it = match self.program_number() {
            Some(n) => Item { kind: "psalm".into(), label: "Voorzang".into(), psalm: n, verses: self.verzen.clone(), problem: psalm_problem(n, &self.verzen), ..Default::default() },
            // A gezang whose text the program does not have: its name, the text typed by hand.
            None => Item {
                kind: "lied".into(),
                label: self.display(),
                problem: "de tekst van dit gezang zit niet in het programma; vul hem in bij Tekst aanpassen".into(),
                ..Default::default()
            },
        };
        it.refresh();
        it
    }

    /// When the website's voorzang is not the one in the schedule: what differs, in Dutch.
    pub fn differs(&self, site: &Item) -> Option<String> {
        let sung = matches!(site.kind.as_str(), "psalm" | "slotzang" | "nazang") && site.psalm > 0;
        let sorted = |v: &[u32]| {
            let mut v = v.to_vec();
            v.sort_unstable();
            v.dedup();
            v
        };
        let same = match (self.program_number(), sung) {
            (Some(n), true) => n == site.psalm && sorted(&self.verzen) == sorted(&site.verses),
            // A gezang without text in the program: the same when the website names the same one.
            (None, false) if site.kind == "lied" || site.kind == "nazang" => {
                let theirs = gezang_key(&site.label).or_else(|| gezang_key(&site.text));
                theirs.is_none() || theirs == gezang_key(&self.naam)
            }
            (None, false) => true,
            _ => false,
        };
        let theirs = if site.reference.is_empty() { site.label.clone() } else { site.reference.clone() };
        (!same).then(|| format!("Voorzang op de website ({theirs}) is anders dan in het voorzangrooster ({})", self.display()))
    }
}

/// What is wrong with a psalm and its verses, as the program checks typed psalms; empty when fine.
fn psalm_problem(n: u32, verses: &[u32]) -> String {
    let Some(p) = crate::texts::psalm(n) else { return format!("psalm {n} bestaat niet") };
    let mut problems = Vec::new();
    if verses.is_empty() {
        problems.push("geen verzen opgegeven".to_string());
    }
    for v in verses {
        if *v == 0 || *v as usize > p.verzen.len() {
            problems.push(format!("{} heeft maar {} verzen (niet {v})", p.naam, p.verzen.len()));
        }
    }
    problems.join("; ")
}

/// The gezangen of the psalm book ("Enige Gezangen"): key, name, and the number in the
/// workbook's list when the workbook has no list of its own (tab Input; 6 and 7 are both
/// the Twaalf Artikelen).
const GEZANGEN: [(&str, &str, &[u32]); 11] = [
    ("geboden", "Tien Geboden des HEEREN", &[1]),
    ("maria", "Lofzang van Maria", &[2]),
    ("zacharias", "Lofzang van Zacharias", &[3]),
    ("simeon", "Lofzang van Simeon", &[4]),
    ("gebed", "Gebed des HEEREN", &[5]),
    ("artikelen", "Twaalf Artikelen", &[6, 7]),
    ("predikatie", "Bedezang voor de predikatie", &[8]),
    ("morgenzang", "Morgenzang", &[9]),
    ("eten", "Bedezang voor het eten", &[10]),
    ("dankzang", "Dankzang na het eten", &[11]),
    ("avondzang", "Avondzang", &[12]),
];

/// Which gezang a name or abbreviation means: "LvM", "L v Maria", "Lofzang van Maria",
/// "TGdH", "10 Geb", "Geb.d.H", "Avondzang".
fn gezang_key(name: &str) -> Option<&'static str> {
    let s: String = name.to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect();
    let key = if s.contains("predikatie") || s.contains("predicatie") {
        "predikatie"
    } else if s.contains("dankzang") {
        "dankzang"
    } else if s.contains("eten") && s.contains("bedezang") {
        "eten"
    } else if s.contains("geboden") || s.starts_with("10geb") || s.starts_with("tiengeb") || s.starts_with("tgdh") {
        "geboden"
    } else if s.contains("maria") || s == "lvm" {
        "maria"
    } else if s.contains("zach") || s == "lvz" {
        "zacharias"
    } else if s.contains("simeon") || s.starts_with("lvsim") || s == "lvs" {
        "simeon"
    } else if s.contains("gebeddes") || s.starts_with("gebdh") || s.contains("onzevader") {
        "gebed"
    } else if s.contains("artikelen") {
        "artikelen"
    } else if s.contains("morgenzang") {
        "morgenzang"
    } else if s.contains("avondzang") {
        "avondzang"
    } else {
        return None;
    };
    Some(key)
}

fn gezang_name(key: &str) -> &'static str {
    GEZANGEN.iter().find(|(k, _, _)| *k == key).map_or("Gezang", |(_, name, _)| name)
}

/// The workbook's numbering of the gezangen (tab Input: number, name), by key.
struct Numbering(Vec<(u32, &'static str)>);

impl Numbering {
    fn of(wb: &Workbook) -> Numbering {
        let mut list = Vec::new();
        if let Some(sheet) = wb.sheet("Input") {
            for row in 1..=sheet.last_row() {
                let (n, name) = (sheet.text(row, 1), sheet.text(row, 2));
                if let (Ok(n), Some(key)) = (n.parse::<u32>(), gezang_key(&name)) {
                    list.push((n, key));
                }
            }
        }
        if list.is_empty() {
            list = GEZANGEN.iter().flat_map(|(k, _, ns)| ns.iter().map(move |n| (*n, *k))).collect();
        }
        Numbering(list)
    }

    fn key(&self, n: u32) -> Option<&'static str> {
        self.0.iter().find(|(m, _)| *m == n).map(|(_, k)| *k)
    }

    fn number(&self, key: &str) -> u32 {
        self.0.iter().find(|(_, k)| *k == key).map_or(0, |(n, _)| *n)
    }

    fn zang(&self, key: &'static str, verzen: Vec<u32>) -> Parsed {
        Parsed { soort: "gezang", nummer: self.number(key), naam: gezang_name(key).to_string(), verzen }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Parsed {
    soort: &'static str,
    nummer: u32,
    naam: String,
    verzen: Vec<u32>,
}

static PSALM_HEAD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(?:ps\.?|psalm)\s*(\d{1,3})$").unwrap());
static GEZANG_HEAD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^gez(?:ang)?\.?\s*(\d{1,2})$").unwrap());
static NOTE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\([^)]*\)").unwrap());
static RANGE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(\d+)\s*t/?m\s*(\d+)$").unwrap());

/// The verses of a psalm or gezang as the church writes them. A dash is not a range here:
/// "3-7" is verse 3 and verse 7, "1-3-5" is 1, 3 and 5 (only "t/m" is a range). Words that are
/// not verses (as "Vz") are left out and returned on their own.
fn sung_verses(s: &str) -> (Vec<u32>, Vec<String>) {
    let (mut verses, mut other) = (Vec::new(), Vec::new());
    let s = s.replace(" en ", ",").replace('&', ",");
    for part in s.split([',', ';']).map(str::trim).filter(|p| !p.is_empty()) {
        if let Some(m) = RANGE.captures(part) {
            let (a, b): (u32, u32) = (m[1].parse().unwrap_or(0), m[2].parse().unwrap_or(0));
            if a > 0 && b >= a && b - a < 200 {
                verses.extend(a..=b);
                continue;
            }
        }
        for token in part.split(|c: char| c == '-' || c.is_whitespace()).filter(|t| !t.is_empty()) {
            match token.parse::<u32>() {
                Ok(v) if v > 0 => verses.push(v),
                _ if token.eq_ignore_ascii_case("en") => {}
                _ => other.push(token.to_string()),
            }
        }
    }
    (verses, other)
}

/// "Ps. 26: 8 (1) en 12", "139: 9-14", "Gez 2: 3", "LvM:1", "Morgenzang : 3, 4 en 5 (2)".
/// `kind` is what the workbook's own column says ("Psalm" or "Gezang"), for "1:9" without a name.
fn parse_text(text: &str, kind: &str, numbering: &Numbering) -> Option<(Parsed, Vec<String>)> {
    // What is between brackets is about how it is sung, not which verses.
    let text = NOTE.replace_all(text, " ");
    let (head, tail) = text.split_once(':').unwrap_or((&text, ""));
    let head = head.trim();
    let (verzen, other) = sung_verses(tail);
    let parsed = if let Some(m) = PSALM_HEAD.captures(head) {
        let n: u32 = m[1].parse().ok()?;
        Parsed { soort: "psalm", nummer: n, naam: format!("Psalm {n}"), verzen }
    } else if let Some(m) = GEZANG_HEAD.captures(head) {
        numbering.zang(numbering.key(m[1].parse().ok()?)?, verzen)
    } else if let Ok(n) = head.parse::<u32>() {
        if kind.eq_ignore_ascii_case("gezang") {
            numbering.zang(numbering.key(n)?, verzen)
        } else {
            Parsed { soort: "psalm", nummer: n, naam: format!("Psalm {n}"), verzen }
        }
    } else {
        numbering.zang(gezang_key(head)?, verzen)
    };
    (parsed.nummer > 0 || parsed.soort == "gezang").then_some((parsed, other))
}

/// The voorzang from the workbook's own columns: "Psalm"/"Gezang", the number (or for newer
/// rows an abbreviation like "LvM"), and up to five verses.
fn parse_columns(sheet: &Sheet, row: u32, col: u32, numbering: &Numbering) -> Option<Parsed> {
    let kind = sheet.text(row, col + 1);
    let number = sheet.text(row, col + 2);
    let verzen: Vec<u32> = (col + 3..col + 8).filter_map(|c| sheet.text(row, c).parse().ok()).filter(|v| *v > 0).collect();
    if number.is_empty() {
        return None;
    }
    match number.parse::<u32>() {
        Ok(n) if kind.eq_ignore_ascii_case("gezang") => Some(numbering.zang(numbering.key(n)?, verzen)),
        Ok(n) if n > 0 => Some(Parsed { soort: "psalm", nummer: n, naam: format!("Psalm {n}"), verzen }),
        Ok(_) => None,
        Err(_) => Some(numbering.zang(gezang_key(&number)?, verzen)),
    }
}

/// One voorzang cell with the columns beside it. The written text counts: the columns were
/// made from it and sometimes cut a verse short ("34: 11" became verse 1). When Excel turned
/// the text into a time ("146:3" as 146 hours), the columns count.
fn zang_at(sheet: &Sheet, row: u32, col: u32, numbering: &Numbering, warn: &mut Vec<String>, when: &str) -> Option<Zang> {
    let columns = parse_columns(sheet, row, col, numbering);
    let kind = sheet.text(row, col + 1);
    let (tekst, chosen) = match sheet.cell(row, col) {
        Some(Cell::Text(t)) => {
            let t = t.trim().to_string();
            match parse_text(&t, &kind, numbering) {
                Some((p, other)) if !p.verzen.is_empty() => {
                    if !other.is_empty() {
                        warn.push(format!("Voorzang {when}: \"{}\" overgeslagen in \"{t}\"", other.join(" ")));
                    }
                    (t, Some(p))
                }
                _ => {
                    if columns.is_none() {
                        warn.push(format!("Voorzang {when}: \"{t}\" niet begrepen"));
                    }
                    (t, columns)
                }
            }
        }
        Some(Cell::Number(n)) if n.fract() != 0.0 => {
            let minutes = (n * 24.0 * 60.0).round() as u64;
            let shown = format!("{}:{:02}", minutes / 60, minutes % 60);
            let p = columns.or_else(|| parse_text(&shown, &kind, numbering).map(|(p, _)| p));
            if let Some(p) = &p {
                warn.push(format!("Voorzang {when}: in de cel staat een tijd ({shown}); gebruikt: {}", zang(String::new(), p.clone()).display()));
            }
            (shown, p)
        }
        Some(Cell::Number(n)) => (format!("{n}"), columns),
        None => (String::new(), columns),
    };
    chosen.map(|p| zang(tekst, p))
}

fn zang(tekst: String, p: Parsed) -> Zang {
    Zang { tekst, soort: p.soort.into(), nummer: p.nummer, naam: p.naam, verzen: p.verzen }
}

static SUPERSCRIPT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\u{00B9}\u{00B2}\u{00B3}\u{2070}\u{2074}-\u{2079}]").unwrap());
static WEEKDAY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\((?:ma|maa|di|din|wo|woe|do|don|vr|vrij|za|zat|zo|zon)\.?\)").unwrap());
static MONTH_HEADING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^\d+\s+zondagen").unwrap());

/// A text from the schedule without the footnote marks (¹²³⁴) and extra spaces.
fn clean(s: &str) -> String {
    SUPERSCRIPT.replace_all(s, "").split_whitespace().collect::<Vec<_>>().join(" ")
}

/// "Biddag² (woe)" → "Biddag", "Eerste Kerstdag (don)" → "Eerste Kerstdag".
fn clean_day(s: &str) -> String {
    clean(&WEEKDAY.replace_all(s, ""))
}

fn date_of(wb: &Workbook, sheet: &Sheet, row: u32, col: u32) -> Option<String> {
    match sheet.cell(row, col)? {
        Cell::Number(n) => wb.date(*n).map(|d| d.to_string()),
        Cell::Text(_) => None,
    }
}

/// Tab Collecterooster: the table under "Datum"; per date the special day and three collectes.
/// The month headings in it are skipped, and it ends at the notes and bank accounts under it.
fn collectes(wb: &Workbook, sheet: &Sheet) -> Vec<CollecteDag> {
    let Some((head, date_col)) = sheet.find("Datum") else { return Vec::new() };
    let day_col = sheet.find_in_row(head, "Omschrijving").unwrap_or(date_col + 1);
    let mut list: Vec<CollecteDag> = Vec::new();
    for row in head + 1..=sheet.last_row() {
        let date = match sheet.cell(row, date_col) {
            Some(Cell::Text(_)) => break,
            Some(Cell::Number(_)) => date_of(wb, sheet, row, date_col),
            None => None,
        };
        let Some(datum) = date else { continue };
        let doelen: Vec<String> = (1..=3).map(|k| clean(&sheet.text(row, day_col + k))).filter(|d| !d.is_empty()).collect();
        if doelen.first().is_some_and(|d| MONTH_HEADING.is_match(d)) {
            continue;
        }
        let dag = clean_day(&sheet.text(row, day_col));
        // The first row of a date counts, as in the workbook's own lookups.
        if (doelen.is_empty() && dag.is_empty()) || list.iter().any(|c| c.datum == datum) {
            continue;
        }
        list.push(CollecteDag { datum, dag, doelen });
    }
    list.sort_by(|a, b| a.datum.cmp(&b.datum));
    list
}

/// Tab Voorzang: per date the morning ("Ochtend") and afternoon ("Middag") voorzang.
fn voorzang(wb: &Workbook, sheet: &Sheet, numbering: &Numbering, warn: &mut Vec<String>) -> Vec<VoorzangDag> {
    let Some((head, morning_col)) = sheet.find("Ochtend") else { return Vec::new() };
    let afternoon_col = sheet.find_in_row(head, "Middag");
    let date_col = sheet.find_in_row(head, "Dag").or_else(|| sheet.find_in_row(head, "Datum")).unwrap_or(1);
    let mut list: Vec<VoorzangDag> = Vec::new();
    for row in head + 1..=sheet.last_row() {
        let Some(datum) = date_of(wb, sheet, row, date_col) else { continue };
        let when = |part: &str| format!("{} ({part})", crate::show::day_month_year(&datum));
        let morgen = zang_at(sheet, row, morning_col, numbering, warn, &when("morgen"));
        let middag = afternoon_col.and_then(|c| zang_at(sheet, row, c, numbering, warn, &when("middag")));
        match list.iter_mut().find(|v| v.datum == datum) {
            // A date twice: the first row counts, as in the workbook's own lookups.
            Some(v) => {
                v.morgen = v.morgen.take().or(morgen);
                v.middag = v.middag.take().or(middag);
            }
            None if morgen.is_some() || middag.is_some() => list.push(VoorzangDag { datum, morgen, middag }),
            None => {}
        }
    }
    list.sort_by(|a, b| a.datum.cmp(&b.datum));
    list
}

/// Tab Streamdienst: per date who operates in the morning and the afternoon, and the notes
/// beside it. A name followed by a number is a count of turns, not a note.
fn bediening(wb: &Workbook, sheet: &Sheet) -> Vec<BedieningDag> {
    let Some((head, morning_col)) = sheet.find("Ochtend") else { return Vec::new() };
    let Some(afternoon_col) = sheet.find_in_row(head, "Middag") else { return Vec::new() };
    let date_col = sheet.find_in_row(head, "Datum").unwrap_or(1);
    let notes_from = morning_col.max(afternoon_col) + 1;
    let mut list: Vec<BedieningDag> = Vec::new();
    for row in head + 1..=sheet.last_row() {
        let Some(datum) = date_of(wb, sheet, row, date_col) else { continue };
        let (morgen, middag) = (clean(&sheet.text(row, morning_col)), clean(&sheet.text(row, afternoon_col)));
        let notities: Vec<String> = sheet
            .row(row)
            .filter(|(c, cell)| *c >= notes_from && matches!(cell, Cell::Text(_)) && !matches!(sheet.cell(row, c + 1), Some(Cell::Number(_))))
            .map(|(c, _)| clean(&sheet.text(row, c)))
            .filter(|t| !t.is_empty())
            .collect();
        if morgen.is_empty() && middag.is_empty() && notities.is_empty() {
            continue;
        }
        match list.iter_mut().find(|b| b.datum == datum) {
            Some(b) => {
                if b.morgen.is_empty() {
                    b.morgen = morgen;
                }
                if b.middag.is_empty() {
                    b.middag = middag;
                }
                for n in notities {
                    if !b.notities.contains(&n) {
                        b.notities.push(n);
                    }
                }
            }
            None => list.push(BedieningDag { datum, morgen, middag, notities }),
        }
    }
    list.sort_by(|a, b| a.datum.cmp(&b.datum));
    list
}

/// What an import found. A tab that is not in the workbook leaves that schedule as it was.
#[derive(Debug, Default)]
pub struct Import {
    pub collectes: Option<Vec<CollecteDag>>,
    pub voorzang: Option<Vec<VoorzangDag>>,
    pub bediening: Option<Vec<BedieningDag>>,
    pub waarschuwingen: Vec<String>,
}

/// Reads the schedules from the bytes of the church's workbook.
pub fn import(bytes: &[u8]) -> Result<Import> {
    let wb = crate::excel::read(bytes)?;
    let numbering = Numbering::of(&wb);
    let mut imp = Import::default();
    if let Some(s) = wb.sheet("Collecterooster") {
        imp.collectes = Some(collectes(&wb, s));
    }
    if let Some(s) = wb.sheet("Voorzang") {
        let mut warn = Vec::new();
        imp.voorzang = Some(voorzang(&wb, s, &numbering, &mut warn));
        imp.waarschuwingen.extend(warn);
    }
    if let Some(s) = wb.sheet("Streamdienst") {
        imp.bediening = Some(bediening(&wb, s));
    }
    if imp.collectes.is_none() && imp.voorzang.is_none() && imp.bediening.is_none() {
        bail!("Geen tabblad Collecterooster, Voorzang of Streamdienst in dit Excel-bestand");
    }
    Ok(imp)
}

/// What was imported, for the operator.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Report {
    pub bron: String,
    pub samenvatting: Summary,
    /// Schedules that were not in the workbook and stay as they were.
    pub behouden: Vec<String>,
    pub waarschuwingen: Vec<String>,
}

impl Import {
    /// The new schedules: what the workbook has, the rest from `old`. `today` is for the
    /// warning that a schedule has run out.
    pub fn apply(self, old: &Roosters, bron: &str, today: NaiveDate) -> (Roosters, Report) {
        let mut behouden = Vec::new();
        let mut keep = |name: &str, found: bool| {
            if !found {
                behouden.push(name.to_string());
            }
        };
        keep("collectes", self.collectes.is_some());
        keep("voorzang", self.voorzang.is_some());
        keep("bediening", self.bediening.is_some());
        let new = Roosters {
            bron: bron.to_string(),
            ingelezen: Local::now().format("%Y-%m-%d %H:%M").to_string(),
            collectes: self.collectes.unwrap_or_else(|| old.collectes.clone()),
            voorzang: self.voorzang.unwrap_or_else(|| old.voorzang.clone()),
            bediening: self.bediening.unwrap_or_else(|| old.bediening.clone()),
        };
        let samenvatting = new.summary();
        let mut waarschuwingen = Vec::new();
        let soon = (today + chrono::Days::new(28)).to_string();
        for (name, span) in [("collecterooster", &samenvatting.collectes), ("voorzangrooster", &samenvatting.voorzang), ("bedieningsrooster", &samenvatting.bediening)] {
            if span.aantal == 0 {
                waarschuwingen.push(format!("Het {name} is leeg"));
            } else if span.tot < soon {
                waarschuwingen.push(format!("Het {name} loopt maar tot {}", crate::show::day_month_year(&span.tot)));
            }
        }
        waarschuwingen.extend(self.waarschuwingen);
        let report = Report { bron: bron.to_string(), samenvatting, behouden, waarschuwingen };
        (new, report)
    }
}

impl Report {
    /// The report as lines of text, for the command line.
    pub fn lines(&self) -> Vec<String> {
        let span = |s: &Span| {
            if s.aantal == 0 {
                "geen".to_string()
            } else {
                format!("{} data, {} t/m {}", s.aantal, crate::show::day_month_year(&s.van), crate::show::day_month_year(&s.tot))
            }
        };
        let s = &self.samenvatting;
        let mut lines = vec![
            format!("Roosters uit {}", self.bron),
            format!("  Collectes:        {}", span(&s.collectes)),
            format!("  Bijzondere dagen: {}", span(&s.dagen)),
            format!("  Voorzang:         {}", span(&s.voorzang)),
            format!("  Bediening:        {}", span(&s.bediening)),
        ];
        if !self.behouden.is_empty() {
            lines.push(format!("  Niet in dit bestand, blijft zoals het was: {}", self.behouden.join(", ")));
        }
        lines.extend(self.waarschuwingen.iter().map(|w| format!("  Let op: {w}")));
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn numbering() -> Numbering {
        Numbering(GEZANGEN.iter().flat_map(|(k, _, ns)| ns.iter().map(move |n| (*n, *k))).collect())
    }

    fn parsed(text: &str, kind: &str) -> (String, u32, Vec<u32>) {
        let (p, _) = parse_text(text, kind, &numbering()).unwrap_or_else(|| panic!("{text}"));
        (p.naam, p.nummer, p.verzen)
    }

    #[test]
    fn a_dash_between_sung_verses_is_not_a_range() {
        assert_eq!(parsed("Ps. 27: 3-7", ""), ("Psalm 27".into(), 27, vec![3, 7]));
        assert_eq!(parsed("Ps. 74: 8-11", ""), ("Psalm 74".into(), 74, vec![8, 11]));
        assert_eq!(parsed("Ps. 119: 1-3-5", ""), ("Psalm 119".into(), 119, vec![1, 3, 5]));
        assert_eq!(parsed("139: 9-14", "Psalm"), ("Psalm 139".into(), 139, vec![9, 14]));
        // "t/m" is a range.
        assert_eq!(parsed("Ps. 119: 1 t/m 3", ""), ("Psalm 119".into(), 119, vec![1, 2, 3]));
    }

    #[test]
    fn voorzang_as_the_church_writes_it() {
        assert_eq!(parsed("Ps. 26: 8 (1) en 12", "Psalm"), ("Psalm 26".into(), 26, vec![8, 12]));
        assert_eq!(parsed("Ps. 146: 5 en 6 (1 en 3)", "Psalm"), ("Psalm 146".into(), 146, vec![5, 6]));
        assert_eq!(parsed("Ps.31: 19", ""), ("Psalm 31".into(), 31, vec![19]));
        assert_eq!(parsed("30:1 ", "Psalm"), ("Psalm 30".into(), 30, vec![1]));
        assert_eq!(parsed("Morgenzang : 3, 4 en 5 (2)", "Gezang"), ("Morgenzang".into(), 9, vec![3, 4, 5]));
        assert_eq!(parsed("Geb.d.H: 3 (2) en 6 (3)", "Gezang"), ("Gebed des HEEREN".into(), 5, vec![3, 6]));
        assert_eq!(parsed("Gez 2: 3", "Gezang"), ("Lofzang van Maria".into(), 2, vec![3]));
        assert_eq!(parsed("Gez. 5: 4", ""), ("Gebed des HEEREN".into(), 5, vec![4]));
        assert_eq!(parsed("Gez 12: 7", "Psalm"), ("Avondzang".into(), 12, vec![7]));
        assert_eq!(parsed("L v Zach: 1", "Gezang"), ("Lofzang van Zacharias".into(), 3, vec![1]));
        assert_eq!(parsed("L v Sim: 1", "Gezang"), ("Lofzang van Simeon".into(), 4, vec![1]));
        assert_eq!(parsed("LvM:1", "Gezang"), ("Lofzang van Maria".into(), 2, vec![1]));
        assert_eq!(parsed("LvS:2", "Gezang"), ("Lofzang van Simeon".into(), 4, vec![2]));
        assert_eq!(parsed("TGdH:1", "Gezang"), ("Tien Geboden des HEEREN".into(), 1, vec![1]));
        assert_eq!(parsed("10 Geb: 9", "Gezang"), ("Tien Geboden des HEEREN".into(), 1, vec![9]));
        // A number without a name: the workbook's column says what it is.
        assert_eq!(parsed("1:9", "Gezang"), ("Tien Geboden des HEEREN".into(), 1, vec![9]));
        assert_eq!(parsed("1:4", "Psalm"), ("Psalm 1".into(), 1, vec![4]));
        let (_, other) = parse_text("Ps. 18: Vz en 1", "Psalm", &numbering()).unwrap();
        assert_eq!(other, ["Vz"]);
        assert!(parse_text("Iets anders: 3", "", &numbering()).is_none());
    }

    #[test]
    fn footnote_marks_and_weekdays_go() {
        assert_eq!(clean("Najaarszendingscollecte³"), "Najaarszendingscollecte");
        assert_eq!(clean("Onderhoud orgel "), "Onderhoud orgel");
        assert_eq!(clean_day("Biddag² (woe)"), "Biddag");
        assert_eq!(clean_day("Dankdag² (woe) "), "Dankdag");
        assert_eq!(clean_day("Eerste Kerstdag (don)"), "Eerste Kerstdag");
        assert_eq!(clean_day("Israëlzondag"), "Israëlzondag");
    }

    #[test]
    fn morning_is_before_one() {
        assert!(morning("09:30"));
        assert!(morning("9:30"));
        assert!(morning("12:59"));
        assert!(!morning("13:00"));
        assert!(!morning("16:00"));
        assert!(!morning("18:30"));
    }

    /// A small workbook in the church's layout, read end to end.
    #[test]
    fn imports_a_workbook_in_the_church_layout() {
        let collecte: &[&[(&str, &str)]] = &[
            &[("A1", "COLLECTEROOSTER 2026")],
            &[("A3", "Datum"), ("B3", "Omschrijving"), ("C3", "Diaconie"), ("D3", "Kerkvoogdij"), ("E3", "Extra collecte ¹")],
            &[("C4", "4 zondagen; 1 feestdag")],
            &[("A5", "#46023"), ("B5", "Nieuwjaarsdag (don)"), ("C5", "Diaconie"), ("D5", "Kerkvoogdij"), ("E5", "Opleiding predikanten")],
            &[("A6", "#46078"), ("B6", "Biddag² (woe)"), ("C6", "Diaconie (biddagcollecte)"), ("D6", "Biddagcollecte")],
            &[("A7", "#46113"), ("C7", "4 zondagen; w.v. 1 eerste feestdag")],
            &[("A8", "#46327"), ("C8", "Najaarszendingscollecte³"), ("D8", "Pastoraat & Catechese"), ("E8", "Aflossing Bouwfonds")],
            &[("A9", "Toelichting:")],
            &[("A10", "Evangelisatie"), ("C10", "NL59 RABO 0141 4580 70")],
            &[("A11", "#45658"), ("C11", "Diaconie")],
        ];
        let voorzang: &[&[(&str, &str)]] = &[
            &[("A1", "Dag"), ("B1", "Ochtend"), ("C1", "Lied"), ("D1", "Psalm"), ("E1", "Vers1"), ("J1", "Middag"), ("K1", "Lied"), ("L1", "Psalm"), ("M1", "Vers1")],
            &[("A2", "#46327"), ("B2", "31:19"), ("C2", "Psalm"), ("D2", "#31"), ("E2", "#19"), ("J2", "90:1"), ("K2", "Psalm"), ("L2", "#90"), ("M2", "#1")],
            &[("A3", "#46334"), ("B3", "34: 11"), ("C3", "Psalm"), ("D3", "#34"), ("E3", "#1"), ("J3", "#0.46180555555555558"), ("K3", "Psalm"), ("L3", "#62"), ("M3", "#5")],
            &[("A4", "#46369"), ("B4", "LvM:1"), ("C4", "Gezang"), ("D4", "LvM"), ("E4", "#1"), ("K4", "Psalm"), ("L4", "#48"), ("M4", "#4")],
        ];
        let stream: &[&[(&str, &str)]] = &[
            &[("A2", "Datum"), ("B2", "Omschrijving"), ("C2", "Ochtend"), ("D2", "Middag")],
            &[("A3", "JANUARI 2023")],
            &[("A4", "#45102"), ("C4", "Bediener 3/Bediener 4"), ("D4", "Bediener 1"), ("F4", "Bediener 5 niet")],
            &[("A5", "#45298"), ("C5", "Bediener 10"), ("D5", "Bediener 6"), ("H5", "Bediener 8"), ("I5", "#3")],
            &[("A6", "#45292")],
            &[("A7", "#45292"), ("C7", "Bediener 6")],
        ];
        let input: &[&[(&str, &str)]] = &[&[("A1", "Nummer"), ("B1", "Omschrijving")], &[("A2", "#1"), ("B2", "10 Geboden des HEEREN")], &[("A3", "#2"), ("B3", "Lofzang van Maria")]];
        let bytes = crate::excel::tests::workbook(&[("Collecterooster", collecte), ("Voorzang", voorzang), ("Streamdienst", stream), ("Input", input)]);
        let imp = import(&bytes).unwrap();
        let (r, report) = imp.apply(&Roosters::default(), "test.xlsx", NaiveDate::from_ymd_opt(2026, 11, 4).unwrap());
        // Collectes: no month headings, nothing under the table, footnote marks gone.
        assert_eq!(r.collectes.iter().map(|c| c.datum.as_str()).collect::<Vec<_>>(), ["2026-01-01", "2026-02-25", "2026-11-01"]);
        assert_eq!(r.collecte_text("2026-11-01"), "1. Najaarszendingscollecte\n2. Pastoraat & Catechese\n3. Aflossing Bouwfonds");
        assert_eq!(r.collecte_text("2026-02-25"), "1. Diaconie (biddagcollecte)\n2. Biddagcollecte");
        assert_eq!(r.special_day("2026-02-25", "19:30"), "Biddag");
        assert_eq!(r.special_day("2026-01-01", "10:00"), "Nieuwjaarsdag");
        // Voorzang: the text counts, the columns when Excel made a time of it.
        assert_eq!(r.voorzang("2026-11-08", "09:30").unwrap().display(), "Psalm 34 : 11");
        assert_eq!(r.voorzang("2026-11-08", "16:00").unwrap().display(), "Psalm 62 : 5");
        assert_eq!(r.voorzang("2026-12-13", "09:30").unwrap().display(), "Lofzang van Maria : 1");
        assert_eq!(r.voorzang("2026-12-13", "16:00").unwrap().display(), "Psalm 48 : 4");
        assert!(report.waarschuwingen.iter().any(|w| w.contains("tijd (11:05)")), "{:?}", report.waarschuwingen);
        // Bediening: notes, not the count of turns; a date twice gives one entry.
        assert_eq!(r.bediening.iter().map(|b| b.datum.as_str()).collect::<Vec<_>>(), ["2023-06-25", "2024-01-01", "2024-01-07"]);
        assert_eq!(r.operator("2023-06-25", "09:30"), "Bediener 3/Bediener 4");
        assert_eq!(r.bediening[0].notities, ["Bediener 5 niet"]);
        assert_eq!((r.bediening[2].morgen.as_str(), r.bediening[2].notities.len()), ("Bediener 10", 0));
        assert_eq!(r.operator("2024-01-01", "09:30"), "Bediener 6");
        assert!(report.waarschuwingen.iter().any(|w| w.contains("bedieningsrooster loopt maar tot")));
    }

    #[test]
    fn a_missing_tab_keeps_the_old_schedule() {
        let input: &[&[(&str, &str)]] = &[&[("A1", "Datum"), ("C1", "Ochtend"), ("D1", "Middag")], &[("A2", "#46327"), ("C2", "Bediener 10")]];
        let bytes = crate::excel::tests::workbook(&[("Streamdienst", input)]);
        let old = builtin();
        let (r, report) = import(&bytes).unwrap().apply(&old, "x.xlsx", NaiveDate::from_ymd_opt(2026, 11, 4).unwrap());
        assert_eq!(r.collectes, old.collectes);
        assert_eq!(r.voorzang, old.voorzang);
        assert_eq!(r.bediening.len(), 1);
        assert_eq!(report.behouden, ["collectes", "voorzang"]);
        let none = crate::excel::tests::workbook(&[("Blad1", input)]);
        assert!(import(&none).is_err());
    }

    #[test]
    fn voorzang_item_and_website_check() {
        let z = Zang { tekst: "Ps. 27: 3-7".into(), soort: "psalm".into(), nummer: 27, naam: "Psalm 27".into(), verzen: vec![3, 7] };
        let it = z.item();
        assert_eq!((it.kind.as_str(), it.label.as_str(), it.psalm, it.verses.clone()), ("psalm", "Voorzang", 27, vec![3, 7]));
        assert_eq!(it.reference, "Psalm 27 : 3, 7");
        assert!(it.problem.is_empty(), "{}", it.problem);
        assert!(z.differs(&it).is_none());
        let mut other = it.clone();
        other.verses = vec![3, 4, 5, 6, 7];
        other.refresh();
        let w = z.differs(&other).unwrap();
        assert!(w.contains("Psalm 27 : 3, 4, 5, 6, 7") && w.contains("(Psalm 27 : 3, 7)"), "{w}");
        // A gezang the program has no text of: a song with its name, to type in.
        let g = Zang { tekst: "LvM:1".into(), soort: "gezang".into(), nummer: 2, naam: "Lofzang van Maria".into(), verzen: vec![1] };
        let it = g.item();
        assert_eq!((it.kind.as_str(), it.label.as_str()), ("lied", "Lofzang van Maria : 1"));
        assert!(!it.problem.is_empty());
        let site = Item { kind: "lied".into(), label: "Lofzang van Maria".into(), ..Default::default() };
        assert!(g.differs(&site).is_none());
        assert!(g.differs(&other).is_some());
        let bad = Zang { verzen: vec![40], ..z };
        assert!(bad.item().problem.contains("Psalm 27 heeft maar"));
    }

    /// The built-in schedules, made by `importeer-excel` from Streamupdate.xlsm, checked by hand
    /// against the cells of the workbook (date serial in column A, the texts beside it).
    #[test]
    fn built_in_collecterooster_is_the_workbook() {
        let r = builtin();
        assert_eq!(r.bron, "Streamupdate.xlsm");
        let cases: [(&str, &str, &[&str]); 15] = [
            ("2026-01-01", "Nieuwjaarsdag", &["Diaconie", "Kerkvoogdij", "Opleiding predikanten"]), // 46023, "Nieuwjaarsdag (don)"
            ("2026-01-04", "", &["St. SLRO", "Pastoraat & Catechese", "Aflossing Bouwfonds"]),
            // Biddag on a Wednesday (46078, "Biddag² (woe)"): two collectes.
            ("2026-02-25", "Biddag", &["Diaconie (biddagcollecte)", "Biddagcollecte"]),
            ("2026-03-01", "", &["Voorjaarszendingscollecte", "Nalezing biddagcollecte", "Aflossing Bouwfonds"]), // "Voorjaarszendingscollecte³"
            ("2026-03-15", "Heilig Avondmaal", &["St. For a Change", "Kerkvoogdij", "Onderhoud gebouwen"]),
            ("2026-04-03", "Goede Vrijdag", &["Diaconie", "Pastoraat & Catechese", "Plaatselijk jeugdwerk"]),
            ("2026-04-19", "", &["Diaconie", "Pastoraat & Catechese", "Aflossing Bouwfonds"]),
            ("2026-05-14", "Hemelvaartsdag", &["Diaconie", "Beheer en onderhoud Levensbron", "Aflossing Bouwfonds"]),
            ("2026-05-24", "1e Pinksterdag", &["Zendingscollecte", "Instandhouding predikantsplaats", "Aflossing Bouwfonds"]),
            ("2026-05-31", "", &["Diaconie", "Pastoraat & Catechese", "Onderhoud orgel"]), // "Onderhoud orgel "
            ("2026-10-04", "Israëlzondag", &["Werk onder het Joodse volk in Israël", "Pastoraat & Catechese", "Aflossing Bouwfonds"]),
            ("2026-11-01", "", &["Najaarszendingscollecte", "Pastoraat & Catechese", "Aflossing Bouwfonds"]), // "Najaarszendingscollecte³"
            // Dankdag on a Wednesday (46351, "Dankdag² (woe) ").
            ("2026-11-25", "Dankdag", &["Diaconie (oogstcollecte)", "Oogstcollecte"]),
            ("2026-12-25", "Eerste Kerstdag", &["Evangelisatiewerk", "Kerkvoogdij", "Boekjes zondagschool"]),
            ("2026-12-31", "Oudejaarsdag", &["Diaconie", "Kerkvoogdij", "Opleiding predikanten"]),
        ];
        for (date, day, purposes) in cases {
            let c = r.collecte(date).unwrap_or_else(|| panic!("{date}"));
            assert_eq!((c.dag.as_str(), c.doelen.iter().map(String::as_str).collect::<Vec<_>>()), (day, purposes.to_vec()), "{date}");
        }
        assert_eq!(r.collecte_text("2026-11-25"), "1. Diaconie (oogstcollecte)\n2. Oogstcollecte");
        // 62 dates; no month headings (46113 is "4 zondagen; …"), nothing from under the table
        // (bank accounts, the 2025 rows), no footnote marks.
        assert_eq!(r.collectes.len(), 62);
        assert!(r.collecte("2026-04-01").is_none() && r.collecte("2025-01-01").is_none());
        let all: Vec<&str> = r.collectes.iter().flat_map(|c| c.doelen.iter().chain([&c.dag])).map(String::as_str).collect();
        assert!(all.iter().all(|t| !t.contains("RABO") && !t.contains("t.n.v.") && !t.contains(['¹', '²', '³', '⁴']) && !t.contains("zondagen")), "{all:?}");
        assert_eq!(r.collectes.iter().filter(|c| !c.dag.is_empty()).count(), 17);
    }

    #[test]
    fn built_in_voorzangrooster_is_the_workbook() {
        let r = builtin();
        let show = |date: &str, time: &str| r.voorzang(date, time).map(Zang::display).unwrap_or_default();
        let cases: [(&str, &str, &str); 15] = [
            ("2023-01-01", "Psalm 26 : 8, 12", "Psalm 91 : 5"), // "Ps. 26: 8 (1) en 12"
            ("2023-02-22", "Morgenzang : 3, 4, 5", "Psalm 145 : 5, 6"), // Wednesday; "Morgenzang : 3, 4 en 5 (2)"
            ("2023-09-24", "Psalm 146 : 3", "Psalm 13 : 5"), // Excel made a time of "146:3"
            ("2023-11-12", "Psalm 139 : 9, 14", "Psalm 64 : 10"), // "139: 9-14": verse 9 and 14
            ("2023-11-15", "Psalm 65 : 6", "Psalm 67 : 3"), // Wednesday; the text says 65 (the column 64)
            ("2024-06-09", "Tien Geboden des HEEREN : 9", "Psalm 92 : 1"), // "1:9" with Gezang beside it
            ("2024-11-17", "Psalm 34 : 11", "Psalm 46 : 1"), // the column cut 11 to 1
            ("2025-08-17", "Psalm 81 : 12", "Psalm 72 : 8"), // the date twice: the first row
            ("2026-04-05", "Psalm 18 : 9", "Psalm 62 : 5"), // the afternoon cell is a time (11:05)
            ("2026-05-24", "Psalm 87 : 1, 3", "Psalm 26 : 3"), // "87: 1 en 3" (the columns 1 and 4)
            ("2026-11-01", "Psalm 31 : 19", "Psalm 90 : 1"), // as on the website that Sunday
            ("2026-11-25", "Psalm 147 : 1", "Psalm 136 : 25"), // Dankdag, Wednesday
            ("2026-12-13", "Lofzang van Maria : 1", "Psalm 119 : 3"), // "LvM:1"
            ("2026-12-20", "Lofzang van Simeon : 2", "Lofzang van Zacharias : 1"),
            ("2027-07-18", "Tien Geboden des HEEREN : 1", "Psalm 88 : 1"), // "TGdH:1"
        ];
        for (date, morning, afternoon) in cases {
            assert_eq!((show(date, "09:30"), show(date, "16:00")), (morning.to_string(), afternoon.to_string()), "{date}");
        }
        // Evening services use the afternoon column; the morning of Oudejaarsavond is empty.
        assert_eq!(show("2026-12-31", "19:30"), "Psalm 90 : 6");
        assert_eq!(show("2026-12-31", "09:30"), "");
        let z = r.voorzang("2023-01-01", "09:30").unwrap();
        assert_eq!((z.tekst.as_str(), z.soort.as_str(), z.nummer), ("Ps. 26: 8 (1) en 12", "psalm", 26));
        let g = r.voorzang("2026-12-13", "09:30").unwrap();
        assert_eq!((g.tekst.as_str(), g.soort.as_str(), g.nummer), ("LvM:1", "gezang", 2));
        assert_eq!(r.voorzang.len(), 283);
        assert_eq!((r.voorzang[0].datum.as_str(), r.voorzang.last().unwrap().datum.as_str()), ("2023-01-01", "2027-07-18"));
    }

    #[test]
    fn built_in_bedieningsrooster_is_the_workbook() {
        let r = builtin();
        let cases: [(&str, &str, &str, &[&str]); 12] = [
            ("2023-02-19", "Bediener 10", "Bediener 8", &[]),
            ("2023-02-22", "Bediener 8", "Bediener 10", &[]), // Biddag, Wednesday
            ("2023-03-12", "Bediener 10", "Bediener 3/Bediener 4", &[]),
            ("2023-04-07", "Bediener 5", "", &[]), // Goede Vrijdag
            ("2023-06-25", "Bediener 3/Bediener 4", "Bediener 1", &["Bediener 5 niet"]),
            ("2023-07-30", "Bediener 8", "Bediener 5", &["Bediener 6 niet", "Bediener 11 niet", "Bediener 2 niet"]),
            ("2023-11-15", "Bediener 5", "Bediener 6", &[]), // Dankdag, Wednesday
            ("2024-01-01", "Bediener 6", "", &[]), // the date twice, the first row empty
            ("2024-01-07", "Bediener 3/Bediener 4", "Bediener 8", &[]), // "Bediener 8 3" beside it is a count, no note
            ("2024-07-14", "Bediener 3/Bediener 4", "Bediener 1", &["Bediener 5, Bediener 10", "Bediener 9"]),
            ("2024-09-08", "Bediener 3", "Bediener 1", &[]),
            ("2024-11-24", "Bediener 9", "Bediener 3", &[]),
        ];
        for (date, morning, afternoon, notes) in cases {
            let b = r.bediening.iter().find(|b| b.datum == date).unwrap_or_else(|| panic!("{date}"));
            assert_eq!((b.morgen.as_str(), b.middag.as_str(), b.notities.iter().map(String::as_str).collect::<Vec<_>>()), (morning, afternoon, notes.to_vec()), "{date}");
        }
        assert_eq!(r.operator("2023-06-25", "16:00"), "Bediener 1");
        // Nobody on New Year's Day 2023: not in the roster.
        assert!(r.bediening.iter().all(|b| b.datum != "2023-01-01"));
    }

    /// Importing the workbook again gives the built-in copy (when the workbook is on this
    /// computer: STREAMUPDATE_XLSM, or the Downloads folder).
    #[test]
    fn importing_the_workbook_gives_the_built_in_copy() {
        let path = std::env::var("STREAMUPDATE_XLSM").unwrap_or_else(|_| r"C:\Users\info\Downloads\Streamupdate.xlsm".into());
        let Ok(bytes) = std::fs::read(&path) else {
            eprintln!("{path} niet gevonden; overgeslagen");
            return;
        };
        let (mut r, report) = import(&bytes).unwrap().apply(&Roosters::default(), "Streamupdate.xlsm", NaiveDate::from_ymd_opt(2026, 11, 4).unwrap());
        let builtin = builtin();
        r.ingelezen = builtin.ingelezen.clone();
        assert_eq!(r, builtin);
        assert!(report.behouden.is_empty());
    }

    #[test]
    fn special_days_by_part_of_day() {
        let r = Roosters {
            collectes: vec![
                CollecteDag { datum: "2026-11-29".into(), dag: "Heilig Avondmaal".into(), doelen: vec!["Diaconie".into()] },
                CollecteDag { datum: "2026-12-31".into(), dag: "Oudejaarsdag".into(), doelen: vec![] },
            ],
            ..Default::default()
        };
        assert_eq!(r.special_day("2026-11-29", "09:30"), "Heilig Avondmaal");
        assert_eq!(r.special_day("2026-11-29", "16:00"), "Dankzegging Heilig Avondmaal");
        assert_eq!(r.special_day("2026-12-31", "19:00"), "Oudejaarsavond");
        assert_eq!(r.special_day("2026-11-22", "09:30"), "");
        assert_eq!(r.collecte_text("2026-11-29"), "1. Diaconie");
        assert_eq!(r.collecte_text("2026-11-22"), "");
    }
}
