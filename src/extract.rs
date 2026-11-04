//! Reads the liturgy from the church website. The program reads the page itself (no key
//! needed): the day headings, the services, their psalms, readings, text, theme and points.
//! Claude can read the same page as a double check (see crate::compare).

use crate::claude::{RawItem, ask_json, item_schema, reference_lists};
use crate::parse::{Ctx, Reader, fold, theme_point};
use anyhow::Result;
use chrono::{Datelike, Duration, NaiveDate, Weekday};
use regex::Regex;
use serde::Deserialize;
use serde_json::json;
use std::sync::LazyLock;

pub const LITURGY_URL: &str = "https://www.hhgtholen.nl/kerkdiensten";

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Extracted {
    pub diensten: Vec<ExtractedService>,
    /// About the page as a whole (a service without a date, no liturgy found).
    #[serde(skip)]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ExtractedService {
    pub datum: String,  // 2026-10-18
    pub tijd: String,   // 09:30
    pub dienst: String, // Morgendienst
    /// The psalms, readings and catechism in the order the website lists them.
    pub onderdelen: Vec<RawItem>,
    pub preektekst: RawItem,
    pub thema: String,
    pub punten: Vec<String>,
    /// Things on the page that could not be placed, with the reason.
    pub niet_herkend: Vec<String>,
}

/// GET with a few retries.
pub async fn http_get(url: &str) -> Result<String> {
    let mut last_err = None;
    for attempt in 0..3 {
        if attempt > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(800 * attempt)).await;
        }
        let resp = reqwest::Client::new()
            .get(url)
            .header("User-Agent", "Mozilla/5.0 (streamandbeam)")
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .and_then(|r| r.error_for_status());
        match resp {
            Ok(r) => return Ok(r.text().await?),
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.unwrap().into())
}

/// Claude's reading of the page, for the double check.
pub async fn ask_ai(page: &str) -> Result<Extracted> {
    let service = json!({
        "type": "object", "additionalProperties": false,
        "required": ["datum", "tijd", "dienst", "onderdelen", "preektekst", "thema", "punten", "niet_herkend"],
        "properties": {
            "datum": {"type": "string", "description": "YYYY-MM-DD"},
            "tijd": {"type": "string", "description": "HH:MM"},
            "dienst": {"type": "string", "description": "e.g. Morgendienst, Middagdienst"},
            "onderdelen": {"type": "array", "items": item_schema(), "description": "All psalms (voorzang first), readings and catechism, in the order listed"},
            "preektekst": item_schema(),
            "thema": {"type": "string", "description": "Thema title without the numbered points"},
            "punten": {"type": "array", "items": {"type": "string"}, "description": "The numbered thema points, without numbers"},
            "niet_herkend": {"type": "array", "items": {"type": "string"}, "description": "Liturgy lines you could not place, each with the reason"},
        }
    });
    let schema = json!({
        "type": "object", "additionalProperties": false, "required": ["diensten"],
        "properties": {"diensten": {"type": "array", "items": service}}
    });
    let prompt = format!(
        "Below is the HTML of the \"Kerkdiensten\" page of a Dutch Reformed (Hersteld Hervormde) church. It lists the \
         liturgy of upcoming services. Map every service onto the format. The sermon text (\"Tekst:\") goes in preektekst, \
         not in onderdelen. Ignore discussion questions (\"Vragen\").\n\n{}\n\n{}\n\nHTML:\n{page}",
        crate::claude::RULES,
        reference_lists()
    );
    ask_json(&prompt, schema).await
}

static DROP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?is)<script\b.*?</script\s*>|<style\b.*?</style\s*>|<head\b.*?</head\s*>|<!--.*?-->").unwrap());
static BLOCK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)<\s*/?\s*(?:br|p|div|address|h[1-6]|li|ul|ol|tr|td|th|table|section|article|header|footer|blockquote|pre|hr|main|nav)\b[^>]*>").unwrap()
});
static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)<[^>]*>").unwrap());

/// The text of a page as lines, the way a browser shows it: block elements on their own
/// line, scripts and styles left out, entities decoded (Samu&euml;l → Samuël).
pub fn page_lines(html: &str) -> Vec<String> {
    let text = DROP.replace_all(html, "\n");
    let text = BLOCK.replace_all(&text, "\n");
    let text = TAG.replace_all(&text, "");
    let text = html_escape::decode_html_entities(&text);
    text.lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|l| !l.is_empty())
        .collect()
}

/// The liturgy part of the page in one text, to notice when it changes.
pub fn liturgy_text(html: &str) -> String {
    page_lines(html).join("\n")
}

const MONTHS: [&str; 12] = [
    "januari", "februari", "maart", "april", "mei", "juni", "juli", "augustus", "september", "oktober", "november", "december",
];
static DATE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(\d{1,2})\s+(januari|februari|maart|april|mei|juni|juli|augustus|september|oktober|november|december)\b(?:\s+(\d{4}))?").unwrap()
});
static BARE_DATE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:(?:zon|maan|dins|woens|donder|vrij|zater)dag\s+)?\d{1,2}\s+(?:januari|februari|maart|april|mei|juni|juli|augustus|september|oktober|november|december)(?:\s+\d{4})?\W*$").unwrap()
});
static YEAR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b(20\d\d)\b").unwrap());
static HEADER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s*(?P<label>\p{L}[\p{L}'-]*(?:\s+\p{L}[\p{L}'-]*){0,2}?)\s*(?:om\s+|van\s+)?(?P<h>\d{1,2})[:.](?P<m>\d{2})\s*(?:uur|u)?\s*(?:[-–(]\s*(?P<extra>[^)]{1,40}?)\s*\)?)?\W*$").unwrap()
});
/// Words a service heading ends with: "Morgendienst", "Bidstond", "Biddag", "Oudejaarsavond".
const SERVICE_WORDS: [&str; 9] = ["dienst", "stond", "viering", "samenkomst", "biddag", "dankdag", "avond", "nacht", "vesper"];
/// Where the liturgy ends and the rest of the page begins.
const STOP: [&str; 7] = ["komende diensten", "populaire pagina", "live uitzending", "plaatselijke agenda", "agenda", "uw eerste kerkdienst", "kerkdiensten"];

/// Easter Sunday (Gregorian calendar; the anonymous algorithm of Meeus, Jones and Butcher).
fn easter(year: i32) -> NaiveDate {
    let (a, b, c) = (year % 19, year / 100, year % 100);
    let (d, e) = (b / 4, b % 4);
    let f = (b + 8) / 25;
    let g = (b - f + 1) / 3;
    let h = (19 * a + b - d - g + 15) % 30;
    let (i, k) = (c / 4, c % 4);
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let n = h + l - 7 * m + 114;
    NaiveDate::from_ymd_opt(year, (n / 31) as u32, (n % 31 + 1) as u32).unwrap()
}

fn nth_weekday(year: i32, month: u32, weekday: Weekday, n: u8) -> Option<NaiveDate> {
    NaiveDate::from_weekday_of_month_opt(year, month, weekday, n)
}

/// A feast day without a date ("1e Pinksterdag 2023", "Goede Vrijdag", "Biddag").
fn feast(folded: &str, year: i32) -> Option<NaiveDate> {
    let second = folded.contains("2e ") || folded.contains("tweede");
    let e = easter(year);
    let day = |m: u32, d: u32| NaiveDate::from_ymd_opt(year, m, d);
    if folded.contains("pinkster") {
        Some(e + Duration::days(if second { 50 } else { 49 }))
    } else if folded.contains("hemelvaart") {
        Some(e + Duration::days(39))
    } else if folded.contains("goede vrijdag") {
        Some(e - Duration::days(2))
    } else if folded.contains("witte donderdag") {
        Some(e - Duration::days(3))
    } else if folded.contains("stille zaterdag") {
        Some(e - Duration::days(1))
    } else if folded.contains("pasen") || folded.contains("paas") {
        Some(e + Duration::days(if second { 1 } else { 0 }))
    } else if folded.contains("kerstavond") || folded.contains("kerstnacht") {
        day(12, 24)
    } else if folded.contains("kerst") {
        day(12, if second { 26 } else { 25 })
    } else if folded.contains("oudejaars") || folded.contains("oudjaars") {
        day(12, 31)
    } else if folded.contains("nieuwjaar") {
        day(1, 1)
    } else if folded.contains("biddag") {
        nth_weekday(year, 3, Weekday::Wed, 2)
    } else if folded.contains("dankdag") {
        nth_weekday(year, 11, Weekday::Wed, 1)
    } else {
        None
    }
}

/// The date in a day heading: "Liturgie zondag 1 november 2026", "Liturgie 1e Pinksterdag
/// 2023", "Zondag 8 november". Without a year: the year that puts it nearest to `today`.
fn heading_date(folded: &str, today: NaiveDate) -> Option<NaiveDate> {
    let years = |explicit: Option<i32>| explicit.map_or_else(|| vec![today.year() - 1, today.year(), today.year() + 1], |y| vec![y]);
    let nearest = |dates: Vec<NaiveDate>| dates.into_iter().min_by_key(|d| (*d - today).num_days().abs());
    if let Some(c) = DATE.captures(folded) {
        let day: u32 = c[1].parse().ok()?;
        let month = MONTHS.iter().position(|m| *m == &c[2])? as u32 + 1;
        let year = c.get(3).and_then(|y| y.as_str().parse().ok());
        return nearest(years(year).into_iter().filter_map(|y| NaiveDate::from_ymd_opt(y, month, day)).collect());
    }
    let year = YEAR.captures(folded).and_then(|c| c[1].parse().ok());
    nearest(years(year).into_iter().filter_map(|y| feast(folded, y)).collect())
}

/// A day heading, with its date (None when the heading has none).
fn day_heading(line: &str, today: NaiveDate) -> Option<Option<NaiveDate>> {
    let f = fold(line);
    let heading = (f.starts_with("liturgie") && f.len() < 100) || BARE_DATE.is_match(&f);
    heading.then(|| heading_date(&f, today))
}

/// A service heading: "Morgendienst 09:30 uur" → ("Morgendienst", "09:30").
fn service_header(line: &str) -> Option<(String, String)> {
    let c = HEADER.captures(line)?;
    let label = c["label"].trim();
    let folded = fold(label);
    if !SERVICE_WORDS.iter().any(|w| folded.ends_with(w)) {
        return None;
    }
    let (h, m): (u32, u32) = (c["h"].parse().ok()?, c["m"].parse().ok()?);
    if h > 23 || m > 59 {
        return None;
    }
    let mut label = label.to_string();
    if let Some(extra) = c.name("extra").map(|e| e.as_str().trim()).filter(|e| !e.is_empty()) {
        label = format!("{label} ({extra})");
    }
    Some((label, format!("{h:02}:{m:02}")))
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Section {
    Songs,
    Readings,
    Text,
    Theme,
    Questions,
}

/// A label at the start of a line on the website: "Voorzang:", "Schriftlezing:", "Tekst:",
/// "Thema:", "Vragen".
fn section_label(line: &str) -> Option<(Section, &str)> {
    let f = fold(line);
    let after_colon = || line.split_once(':').map_or("", |(_, r)| r).trim();
    let labelled = |words: &[&str]| words.iter().any(|w| f.strip_prefix(w).is_some_and(|r| r.trim_start().starts_with(':')));
    if labelled(&["voorzang"]) {
        return Some((Section::Songs, line.trim()));
    }
    if labelled(&["schriftlezingen", "schriftlezing", "lezingen", "lezing", "bijbellezing", "schriftgedeelte"]) {
        return Some((Section::Readings, after_colon()));
    }
    if labelled(&["preektekst", "tekst"]) {
        return Some((Section::Text, after_colon()));
    }
    if labelled(&["thema"]) || f == "thema" {
        return Some((Section::Theme, after_colon()));
    }
    let questions = ["vragen", "gespreksvragen", "vragen bij de preek", "vragen voor de kring", "vragen voor thuis", "vragen naar aanleiding"];
    if f.len() < 40 && questions.iter().any(|q| f == *q || f.starts_with(&format!("{q} ")) || f.starts_with(&format!("{q}:"))) {
        return Some((Section::Questions, ""));
    }
    None
}

/// One service while its lines are read.
struct Building {
    date: Option<NaiveDate>,
    heading: String,
    svc: ExtractedService,
    section: Section,
    notes: Vec<String>,
}

impl Building {
    fn finish(self, out: &mut Extracted) {
        let Some(date) = self.date else {
            out.notes.push(format!("{} {} overgeslagen: geen datum gevonden in '{}'", self.svc.dienst, self.svc.tijd, self.heading));
            return;
        };
        let mut svc = self.svc;
        svc.datum = date.format("%Y-%m-%d").to_string();
        // Everything that is not right: not understood, or not in the program's texts.
        let mut problems = self.notes;
        for raw in svc.onderdelen.iter().chain(std::iter::once(&svc.preektekst)) {
            let item = raw.clone().into_item();
            if !item.problem.is_empty() {
                problems.push(format!("{}: {}", raw.bron, item.problem));
            }
        }
        svc.niet_herkend = problems;
        out.diensten.push(svc);
    }
}

/// The program's own reading of the hhgtholen.nl "Kerkdiensten" page: per service the
/// psalms (the first after "Voorzang:" is the voorzang), the readings, the sermon text,
/// the theme and its points. Discussion questions are skipped; lines that fit nowhere are
/// listed as not placed. `today` decides the year of a date written without one.
pub fn read_page(html: &str, today: NaiveDate) -> Extracted {
    read_lines(&page_lines(html), &Reader::default(), today)
}

fn read_lines(lines: &[String], reader: &Reader, today: NaiveDate) -> Extracted {
    let mut out = Extracted::default();
    let mut date: Option<NaiveDate> = None;
    let mut heading = String::new();
    let mut started = false;
    let mut cur: Option<Building> = None;
    for line in lines {
        let f = fold(line);
        if started && STOP.iter().any(|s| f == *s || f.starts_with(&format!("{s} "))) {
            break;
        }
        if let Some(d) = day_heading(line, today) {
            if let Some(b) = cur.take() {
                b.finish(&mut out);
            }
            started = true;
            date = d;
            heading = line.clone();
            continue;
        }
        if let Some((label, time)) = service_header(line) {
            if let Some(b) = cur.take() {
                b.finish(&mut out);
            }
            started = true;
            let svc = ExtractedService { tijd: time, dienst: label, ..Default::default() };
            cur = Some(Building { date, heading: heading.clone(), svc, section: Section::Songs, notes: Vec::new() });
            continue;
        }
        let Some(b) = cur.as_mut() else { continue };
        if let Some((section, rest)) = section_label(line) {
            b.section = section;
            match section {
                Section::Songs => {
                    for mut it in reader.part(rest, Ctx::Songs) {
                        if !b.svc.onderdelen.iter().any(|o| is_sung(o)) && is_sung(&it) {
                            it.opschrift = "Voorzang".into();
                        }
                        b.svc.onderdelen.push(it);
                    }
                }
                Section::Readings => b.svc.onderdelen.extend(reader.readings(rest)),
                Section::Text if !rest.is_empty() => b.svc.preektekst = reader.sermon_text(rest),
                Section::Theme => b.svc.thema = rest.to_string(),
                _ => {}
            }
            continue;
        }
        match b.section {
            Section::Questions => {}
            Section::Theme if theme_point(line).is_some() => b.svc.punten.push(theme_point(line).unwrap()),
            // "Thema:" on a line of its own, the theme on the next.
            Section::Theme if b.svc.thema.is_empty() && b.svc.punten.is_empty() && !line.starts_with('(') => b.svc.thema = line.clone(),
            Section::Songs => {
                for mut it in reader.part(line, Ctx::Songs) {
                    if !b.svc.onderdelen.iter().any(|o| is_sung(o)) && is_sung(&it) && it.opschrift == "Zingen" {
                        it.opschrift = "Voorzang".into();
                    }
                    b.svc.onderdelen.push(it);
                }
            }
            // After the readings: another reading, or a psalm listed late.
            _ => {
                let items = reader.part(line, Ctx::Typed);
                if items.iter().all(|i| !i.niet_begrepen && matches!(i.soort.as_str(), "psalm" | "lezing" | "catechismus" | "belijdenis" | "formulier")) {
                    b.svc.onderdelen.extend(items);
                } else {
                    b.notes.push(format!("Niet in de liturgie gezet: {line}"));
                }
            }
        }
    }
    if let Some(b) = cur.take() {
        b.finish(&mut out);
    }
    if out.diensten.is_empty() && out.notes.is_empty() {
        out.notes.push("Geen liturgie gevonden op de website".into());
    }
    out
}

fn is_sung(it: &RawItem) -> bool {
    matches!(it.soort.as_str(), "psalm" | "slotzang") && it.psalm > 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(file: &str, today: &str) -> Extracted {
        let html = std::fs::read_to_string(format!("{}/fixtures/{file}", env!("CARGO_MANIFEST_DIR"))).unwrap();
        read_page(&html, NaiveDate::parse_from_str(today, "%Y-%m-%d").unwrap())
    }

    /// The parts as short text: "psalm 31 [19]", "lezing 9 13:1-999", "lied Een vaste …".
    fn parts(items: &[RawItem]) -> Vec<String> {
        items.iter().map(desc).collect()
    }

    fn desc(r: &RawItem) -> String {
        match r.soort.as_str() {
            "psalm" | "slotzang" | "nazang" => format!("{} {} {:?}", r.soort, r.psalm, r.verzen),
            "lezing" => format!("lezing {} {}:{}-{}", r.boek, r.hoofdstuk, r.van, r.tot),
            "catechismus" => format!("catechismus {:?} zondag {}", r.vragen, r.zondag),
            "" => String::new(),
            other => format!("{other} {}", r.opschrift),
        }
    }

    #[test]
    fn reads_the_church_website() {
        let ex = read("kerkdiensten-2026-11-01.html", "2026-11-04");
        assert_eq!(ex.diensten.len(), 2, "{:?}", ex.notes);
        let m = &ex.diensten[0];
        assert_eq!((m.datum.as_str(), m.tijd.as_str(), m.dienst.as_str()), ("2026-11-01", "09:30", "Morgendienst"));
        assert_eq!(parts(&m.onderdelen), [
            "psalm 31 [19]",
            "psalm 74 [8, 11]",
            "psalm 77 [7]",
            "psalm 51 [6, 7]",
            "psalm 27 [7]",
            "psalm 130 [3, 4]",
            "lied Een vaste burcht is onze God",
            "lezing 9 13:1-999",
        ]);
        assert_eq!(m.onderdelen[0].opschrift, "Voorzang");
        assert_eq!(m.onderdelen[1].opschrift, "Zingen");
        assert_eq!(desc(&m.preektekst), "lezing 9 13:11-11");
        assert_eq!((m.thema.as_str(), m.punten.len()), ("", 0));
        assert!(m.niet_herkend.is_empty(), "{:?}", m.niet_herkend);

        let a = &ex.diensten[1];
        assert_eq!((a.datum.as_str(), a.tijd.as_str(), a.dienst.as_str()), ("2026-11-01", "16:00", "Middagdienst"));
        assert_eq!(parts(&a.onderdelen), [
            "psalm 90 [1]",
            "psalm 147 [1]",
            "psalm 32 [6]",
            "psalm 27 [3, 7]",
            "psalm 42 [1, 5]",
            "psalm 43 [4]",
            "lezing 35 3:17-19",
            "lezing 60 1:3-9",
            "lezing 66 7:9-12",
        ]);
        assert_eq!(desc(&a.preektekst), "catechismus [58] zondag 0");
        assert_eq!(a.thema, "Verrast door vreugde");
        assert_eq!(a.punten, ["Nu al", "Straks volkomen"]);
        assert!(a.niet_herkend.is_empty(), "{:?}", a.niet_herkend);
        assert!(ex.notes.is_empty());
    }

    #[test]
    fn feast_day_heading_without_a_date_and_three_services() {
        // Pinksteren 2023: "Liturgie 1e Pinksterdag 2023", "Gez. 5", an empty "Tekst:",
        // a theme with only points, "&" between readings.
        let ex = read("kerkdiensten-2023-05-28.html", "2023-06-02");
        let ids: Vec<String> = ex.diensten.iter().map(|s| format!("{} {} {}", s.datum, s.tijd, s.dienst)).collect();
        assert_eq!(ids, ["2023-05-28 09:30 Morgendienst", "2023-05-28 16:00 Middagdienst", "2023-05-29 09:30 Morgendienst"]);
        let m = &ex.diensten[0];
        assert_eq!(parts(&m.onderdelen)[..2], ["psalm 133 [1, 3]".to_string(), "psalm 119 [3, 86]".to_string()]);
        // Gez. 5: the Gebed des HEEREN, as the program numbers it (150 + 5) when found by name.
        let gez = &m.onderdelen[5];
        assert_eq!((gez.soort.as_str(), gez.verzen.clone()), ("psalm", vec![3, 9]));
        assert!(gez.psalm > 150, "{gez:?}");
        assert_eq!(desc(&m.preektekst), "lezing 44 2:14-21");
        assert_eq!(m.thema, "");
        assert_eq!(m.punten, ["De uitstorting van de Heilige geest (14-18)", "Het laatste oordeel (19-20)", "Aanroepen en zalig worden (21)"]);
        let a = &ex.diensten[1];
        assert_eq!(parts(&a.onderdelen)[0], "psalm 24 [5]");
        assert_eq!(parts(&a.onderdelen)[3], "psalm 118 [11, 12, 13]");
        assert_eq!(a.thema, "Jezus stort de Heilige Geest uit");
        let p = &ex.diensten[2];
        assert_eq!(parts(&p.onderdelen)[5..], ["lezing 23 63:7-11".to_string(), "lezing 49 4:17-32".to_string()]);
        assert_eq!(p.preektekst.soort, "");
        assert_eq!((p.thema.as_str(), p.punten.len()), ("", 0));
    }

    #[test]
    fn catechism_sunday_as_text_and_questions_skipped() {
        // April 2024: "Ps. 118; 14", "Tekst: Zondag 4 Heidelbergse Catechismus", questions
        // that mention "Zondag 4" and "vraag en antwoord 10".
        let ex = read("kerkdiensten-2024-04-07.html", "2024-04-12");
        assert_eq!(ex.diensten.len(), 2);
        let m = &ex.diensten[0];
        assert_eq!(parts(&m.onderdelen).last().unwrap(), "lezing 43 20:24-31");
        assert_eq!(parts(&m.onderdelen)[5], "psalm 118 [14]");
        assert_eq!(m.preektekst.soort, "");
        let a = &ex.diensten[1];
        assert_eq!(a.onderdelen.len(), 8);
        assert_eq!(parts(&a.onderdelen)[6..], ["lezing 34 1:1-7".to_string(), "lezing 42 23:39-43".to_string()]);
        assert_eq!(desc(&a.preektekst), "catechismus [] zondag 4");
        assert_eq!(a.thema, "Drie gevaarlijke zoethoudertjes");
        assert_eq!(a.punten.len(), 3);
        assert!(a.niet_herkend.is_empty(), "{:?}", a.niet_herkend);
    }

    #[test]
    fn lords_supper_with_tables_and_readings_joined_by_en() {
        // March 2025: "- 3e lijdenszondag" after the date, "Ps. 75: 1 (1e tafel)", nine
        // psalms, "1 Korinthe 11 vers 23 t/m 26 en Jesaja 62 vers 6 en 7".
        let ex = read("kerkdiensten-2025-03-16.html", "2025-03-16");
        assert_eq!(ex.diensten.iter().map(|s| s.datum.as_str()).collect::<Vec<_>>(), ["2025-03-16", "2025-03-16"]);
        let m = &ex.diensten[0];
        assert_eq!(m.onderdelen.iter().filter(|o| o.soort == "psalm").count(), 9);
        assert_eq!(parts(&m.onderdelen)[6], "psalm 75 [1]");
        assert_eq!(parts(&m.onderdelen)[9..], ["lezing 46 11:23-26".to_string(), "lezing 23 62:6-7".to_string()]);
        assert_eq!(m.thema, "Verkondigt de dood van de Heere!");
        let a = &ex.diensten[1];
        assert_eq!(parts(&a.onderdelen)[8..], ["lezing 43 6:48-56".to_string(), "lezing 46 11:23-26".to_string()]);
        assert_eq!(a.thema, "... todat Hij komt");
        assert_eq!(a.punten, ["De voorsmaak", "De zekerheid"]);
    }

    #[test]
    fn reading_over_two_chapters() {
        // October 2025: "Kolossensen 1: 21 - 2: 23", "Ps.42: 1-3-5", the text in italics.
        let ex = read("kerkdiensten-2025-10-12.html", "2025-10-14");
        let m = &ex.diensten[0];
        assert_eq!(parts(&m.onderdelen)[3], "psalm 42 [1, 3, 5]");
        assert_eq!(parts(&m.onderdelen)[6..], ["lezing 51 1:21-29".to_string(), "lezing 51 2:1-23".to_string()]);
        assert_eq!(desc(&m.preektekst), "lezing 51 1:23-23");
        assert_eq!(m.punten.len(), 3);
        let a = &ex.diensten[1];
        assert_eq!(parts(&a.onderdelen).last().unwrap(), "lezing 62 4:1-10");
        assert_eq!(desc(&a.preektekst), "lezing 62 4:1-10");
        assert_eq!(a.thema, "Hij was een mens als iedereen");
    }

    #[test]
    fn example_website_of_the_testdienst() {
        // As nepcamera serves it (src/bin/nepcamera.rs).
        let html = "<h1>Kerkdiensten</h1><p>Op deze pagina kunt u de liturgie voor de aankomende diensten vinden.</p>\
            <h2>Liturgie zondag 18 oktober 2026</h2><h3>Middagdienst 16:00 uur</h3>\
            <p>Voorzang: Ps. 55: 1<br>Ps. 49: 6<br>Ps. 16: 6<br>Ps. 73: 12-13<br>Ps. 17: 8<br>Ps. 73: 14</p>\
            <p>Schriftlezing: 1 Korinthe 6 vers 15 t/m 20</p><p>Tekst: Heidelbergse Catechismus vraag en antwoord 57</p>\
            <p>Thema: Dit mijn lichaam<br>1. Nu ik leef<br>2. Als ik sterf<br>3. Als Jezus terugkomt</p>\
            <p>Vragen<br>Hoe kijk jij naar je lichaam?</p><h2>Komende diensten</h2>";
        let ex = read_page(html, NaiveDate::from_ymd_opt(2026, 10, 14).unwrap());
        assert_eq!(ex.diensten.len(), 1);
        let s = &ex.diensten[0];
        assert_eq!(parts(&s.onderdelen), [
            "psalm 55 [1]",
            "psalm 49 [6]",
            "psalm 16 [6]",
            "psalm 73 [12, 13]",
            "psalm 17 [8]",
            "psalm 73 [14]",
            "lezing 46 6:15-20",
        ]);
        assert_eq!(desc(&s.preektekst), "catechismus [57] zondag 0");
        assert_eq!((s.thema.as_str(), s.punten.len()), ("Dit mijn lichaam", 3));
        assert!(s.niet_herkend.is_empty(), "{:?}", s.niet_herkend);
    }

    #[test]
    fn other_shapes_of_the_page() {
        let lines = |s: &str| s.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect::<Vec<_>>();
        let reader = Reader::default();
        let today = NaiveDate::from_ymd_opt(2026, 11, 4).unwrap();
        // One service, an evening service, no theme, a year-less date, "Ps 42 : 1, 2",
        // an extra blank line, and a remark that fits nowhere.
        let page = lines(
            "Kerkdiensten
             Op deze pagina kunt u de liturgie vinden.
             Liturgie Dankdag 4 november
             Avonddienst 19:30 uur

             Voorzang: Ps 42 : 1, 2
             Ps 65: 1-6
             Gez. 9: 5-6
             Schriftlezing: Psalm 65
             Tekst: Psalm 65 vers 12
             (Na de dienst koffie)
             Komende diensten
             Morgendienst 09:30 uur",
        );
        let ex = read_lines(&page, &reader, today);
        assert_eq!(ex.diensten.len(), 1);
        let s = &ex.diensten[0];
        assert_eq!((s.datum.as_str(), s.tijd.as_str(), s.dienst.as_str()), ("2026-11-04", "19:30", "Avonddienst"));
        assert_eq!(parts(&s.onderdelen)[..2], ["psalm 42 [1, 2]".to_string(), "psalm 65 [1, 6]".to_string()]);
        assert_eq!(parts(&s.onderdelen)[3], "lezing 19 65:1-999");
        assert_eq!(desc(&s.preektekst), "lezing 19 65:12-12");
        assert_eq!(s.thema, "");
        assert_eq!(s.niet_herkend.iter().filter(|p| p.contains("Na de dienst koffie")).count(), 1, "{:?}", s.niet_herkend);
        // A feast day: Goede Vrijdag 2027 is 26 March.
        let page = lines("Liturgie Goede Vrijdag\nAvonddienst 19.30 uur\nVoorzang: Ps. 22: 1");
        let ex = read_lines(&page, &reader, NaiveDate::from_ymd_opt(2027, 3, 20).unwrap());
        assert_eq!(ex.diensten[0].datum, "2027-03-26");
        assert_eq!(ex.diensten[0].tijd, "19:30");
        // No date at all: not guessed, but said.
        let page = lines("Liturgie bijzondere dienst\nMorgendienst 10:00\nVoorzang: Ps. 1: 1");
        let ex = read_lines(&page, &reader, today);
        assert!(ex.diensten.is_empty());
        assert!(ex.notes[0].contains("geen datum"), "{:?}", ex.notes);
        // An empty page.
        assert_eq!(read_lines(&[], &reader, today).notes, ["Geen liturgie gevonden op de website"]);
    }

    #[test]
    fn dates_of_feasts() {
        assert_eq!(easter(2023), NaiveDate::from_ymd_opt(2023, 4, 9).unwrap());
        assert_eq!(easter(2026), NaiveDate::from_ymd_opt(2026, 4, 5).unwrap());
        let today = NaiveDate::from_ymd_opt(2026, 11, 4).unwrap();
        let d = |s: &str| heading_date(&fold(s), today).map(|d| d.to_string());
        assert_eq!(d("Liturgie 1e Pinksterdag 2023").as_deref(), Some("2023-05-28"));
        assert_eq!(d("Liturgie 2e Pinksterdag 2023").as_deref(), Some("2023-05-29"));
        assert_eq!(d("Liturgie Hemelvaartsdag 2026").as_deref(), Some("2026-05-14"));
        assert_eq!(d("Liturgie 1e Kerstdag").as_deref(), Some("2026-12-25"));
        assert_eq!(d("Liturgie Oudejaarsavond").as_deref(), Some("2026-12-31"));
        assert_eq!(d("Liturgie zondag 1 november 2026").as_deref(), Some("2026-11-01"));
        assert_eq!(d("Liturgie zondag 16 maart 2025 - 3e lijdenszondag").as_deref(), Some("2025-03-16"));
        // Without a year, early January is next year.
        assert_eq!(heading_date("zondag 3 januari", NaiveDate::from_ymd_opt(2026, 12, 28).unwrap()).map(|d| d.to_string()).as_deref(), Some("2027-01-03"));
        assert_eq!(service_header("Middagdienst  15:30 uur").unwrap(), ("Middagdienst".to_string(), "15:30".to_string()));
        assert_eq!(service_header("Morgendienst 9.30 uur").unwrap().1, "09:30");
        assert_eq!(service_header("Morgendienst 09:30 uur (Heilig Avondmaal)").unwrap().0, "Morgendienst (Heilig Avondmaal)");
        assert!(service_header("Wat gebeurde er op de avond om 19:00 uur?").is_none());
        assert!(service_header("Ps. 42: 1").is_none());
    }
}
