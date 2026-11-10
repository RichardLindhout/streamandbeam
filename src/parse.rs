//! The program's own reader for liturgy text, without AI: a line on the church website or
//! what the operator types ("ps 42:5, gebed, joh 3:16-21, HC 57") becomes the fixed item
//! format (`RawItem`), the same Claude fills in. What it does not understand is kept,
//! marked as not recognised with the reason, never dropped.
//!
//! The church's notation: in psalms and gezangen a dash is not a range but "and"
//! ("Ps. 27: 3-7" is verses 3 and 7, "Ps 119: 1-3-5" is 1, 3 and 5); only in readings a
//! dash is a range ("Joh. 3:16-21"), like "t/m" ("vers 17 t/m 19").

use crate::books::{BOOKS, BibleRef};
use crate::claude::RawItem;
use crate::show::Item;
use regex::Regex;
use std::sync::LazyLock;

/// Lower case without accents, one kind of dash and single spaces: "Samuël  –  1" →
/// "samuel - 1".
pub fn fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        let c = match c {
            'á' | 'à' | 'â' | 'ä' | 'Á' | 'À' | 'Â' | 'Ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' | 'Í' | 'Ì' | 'Î' | 'Ï' => 'i',
            'ó' | 'ò' | 'ô' | 'ö' | 'Ó' | 'Ò' | 'Ô' | 'Ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' | 'Ú' | 'Ù' | 'Û' | 'Ü' => 'u',
            'ç' | 'Ç' => 'c',
            'ñ' | 'Ñ' => 'n',
            '–' | '—' | '‐' | '‑' | '−' => '-',
            '‘' | '’' | '´' | '`' => '\'',
            '“' | '”' | '„' => '"',
            c if c.is_whitespace() => ' ',
            c => c,
        };
        out.extend(c.to_lowercase());
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Only the letters and digits of `fold(s)`: "Geb.d.H" → "gebdh", "L v Maria" → "lvmaria".
pub fn compact(s: &str) -> String {
    fold(s).chars().filter(|c| c.is_alphanumeric()).collect()
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(u32),
    Word(String),
    Sym(char),
}

impl Tok {
    fn word(&self) -> &str {
        match self {
            Tok::Word(w) => w,
            _ => "",
        }
    }

    fn num(&self) -> Option<u32> {
        match self {
            Tok::Num(n) => Some(*n),
            _ => None,
        }
    }

    fn text(&self) -> String {
        match self {
            Tok::Num(n) => n.to_string(),
            Tok::Word(w) => w.clone(),
            Tok::Sym(c) => c.to_string(),
        }
    }
}

/// "t/m", "t.e.m.", "tot en met" and "tot" all mean a range; "1e Samuël" is "1 Samuël".
static RANGE_WORDS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\bt\s*/\s*m\b|\bt\.e\.m\b\.?|\btot en met\b|\btot\b").unwrap());
static ORDINAL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b([1-3])\s*(?:e|ste|de)\s+([a-z])").unwrap());
/// Remarks between brackets: "Ps. 26: 8 (1) en 12", "Ps. 75: 1 (1e tafel)".
static BRACKETS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\([^)]*\)?").unwrap());

/// Splits folded text into numbers, words and signs. Dots are dropped (abbreviations);
/// a letter right after a number ("2a", half a verse) is too: the whole verse is shown.
fn tokens(folded: &str) -> Vec<Tok> {
    let s = RANGE_WORDS.replace_all(folded, " tm ");
    let s = ORDINAL.replace_all(&s, "$1 $2");
    let chars: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_ascii_digit() {
            let start = i;
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
            let digits: String = chars[start..i].iter().collect();
            out.push(Tok::Num(digits.parse().unwrap_or(u32::MAX)));
            if i < chars.len() && matches!(chars[i], 'a'..='d') && !chars.get(i + 1).is_some_and(|c| c.is_alphabetic()) {
                i += 1;
            }
        } else if c.is_alphabetic() {
            let start = i;
            while i < chars.len() && chars[i].is_alphabetic() {
                i += 1;
            }
            out.push(Tok::Word(chars[start..i].iter().collect()));
        } else {
            if !c.is_whitespace() && !matches!(c, '.' | '\'' | '"') {
                out.push(Tok::Sym(c));
            }
            i += 1;
        }
    }
    out
}

fn key_of(toks: &[Tok]) -> String {
    toks.iter().filter(|t| !matches!(t, Tok::Sym(_))).map(Tok::text).collect()
}

/// "3, 4 en 5" → "3, 4 en 5"; "3, 7" → "3 en 7": a list as written in Dutch.
pub fn and_list(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} en {last}", rest.join(", ")),
    }
}

/// The gezangen of the 1773 book as the church numbers them (its Excel): 6 and 7 are both
/// the Twaalf Artikelen.
const GEZANGEN: [&str; 12] = [
    "De Tien Geboden",
    "Lofzang van Maria",
    "Lofzang van Zacharias",
    "Lofzang van Simeon",
    "Gebed des HEEREN",
    "Twaalf Artikelen",
    "Twaalf Artikelen",
    "Bedezang voor de predikatie",
    "Morgenzang",
    "Bedezang voor het eten",
    "Dankzang na het eten",
    "Avondzang",
];

/// "Gezang 5 (Gebed des HEEREN)", for messages.
pub fn gezang_name(nr: u32) -> String {
    match GEZANGEN.get((nr as usize).wrapping_sub(1)) {
        Some(name) => format!("Gezang {nr} ({name})"),
        None => format!("Gezang {nr}"),
    }
}

/// Which gezang (1-12) a name means, as the church writes it on the website and in its
/// Excel, often abbreviated: "TGdH", "10 Geb", "LvM", "L v Zach", "Geb.d.H", "Onze Vader",
/// "12 Artikelen", "Morgenzang". `key` is the name in `compact` form. Also used on the
/// program's own gezang names, so the numbers follow from the names.
fn gezang_by_name(key: &str) -> Option<u32> {
    const SHORT: [(&str, u32); 17] = [
        ("tgdh", 1),
        ("10geb", 1),
        ("tiengeb", 1),
        ("lvm", 2),
        ("lvmaria", 2),
        ("lvz", 3),
        ("lvzach", 3),
        ("lvzacharias", 3),
        ("lvs", 4),
        ("lvsim", 4),
        ("lvsimeon", 4),
        ("gebdh", 5),
        ("gebdesh", 5),
        ("gebdesheeren", 5),
        ("12art", 6),
        ("12artikelen", 6),
        ("geloofsartikelen", 6),
    ];
    if let Some((_, n)) = SHORT.iter().find(|(k, _)| *k == key) {
        return Some(*n);
    }
    let sung = key.contains("zang");
    let words: [(&str, bool, u32); 13] = [
        ("geboden", true, 1),
        ("maria", sung, 2),
        ("zacharias", true, 3),
        ("simeon", true, 4),
        ("gebeddes", true, 5),
        ("onzevader", true, 5),
        ("artikelen", true, 6),
        ("predi", sung, 8),
        ("morgenzang", true, 9),
        ("voorheteten", sung || key.contains("gebed"), 10),
        ("naheteten", sung || key.contains("dank"), 11),
        ("avondzang", true, 12),
        ("avondgezang", true, 12),
    ];
    words.iter().find(|(w, ok, _)| *ok && key.contains(w)).map(|(_, _, n)| *n)
}

/// More ways to write the books of the Bible, besides the program's own names and
/// abbreviations: other spellings (NBV, without accents) and short forms.
const MORE_BOOK_NAMES: [(u32, &str); 66] = [
    (1, "gen"),
    (2, "exod"),
    (3, "lev"),
    (4, "num"),
    (5, "deut,dt"),
    (6, "joz"),
    (7, "richt,rechters"),
    (8, "rut"),
    (9, "1 sam,i samuel"),
    (10, "2 sam,ii samuel"),
    (11, "1 kon,i koningen"),
    (12, "2 kon,ii koningen"),
    (13, "1 kron,1 kr"),
    (14, "2 kron,2 kr"),
    (15, "ezr"),
    (16, "neh,nehemia"),
    (17, "ester,esth"),
    (18, "job"),
    (19, "psalm,ps,psalmen"),
    (20, "spr"),
    (21, "pred"),
    (22, "hoogl"),
    (23, "jes"),
    (24, "jer"),
    (25, "klaagl,kl"),
    (26, "ez,ezech,ezechiel"),
    (27, "dan,daniel"),
    (28, "hos"),
    (29, "joel"),
    (30, "am"),
    (31, "ob"),
    (32, "jon"),
    (33, "mich"),
    (34, "nah"),
    (35, "hab"),
    (36, "sef,zef,zefanja"),
    (37, "hag,haggai"),
    (38, "zach"),
    (39, "mal"),
    (40, "matth,matt,mat,mattheus,matteus,mt"),
    (41, "mark,marc,marcus,mk"),
    (42, "luk,lucas,lc,lk"),
    (43, "joh,jh"),
    (44, "hand"),
    (45, "rom"),
    (46, "1 kor,1 korintiers,1 korinthiers,1 cor"),
    (47, "2 kor,2 korintiers,2 korinthiers,2 cor"),
    (48, "gal"),
    (49, "ef,efez,efeziers"),
    (50, "fil,filip,filippensen"),
    (51, "kol,kolossensen"),
    (52, "1 thess,1 tess,1 tessalonicenzen,1 th"),
    (53, "2 thess,2 tess,2 tessalonicenzen,2 th"),
    (54, "1 tim,1 timoteus"),
    (55, "2 tim,2 timoteus"),
    (56, "tit"),
    (57, "filem,flm"),
    (58, "hebr,heb,hebreen"),
    (59, "jak,jac"),
    (60, "1 petr,1 pet,1 pe"),
    (61, "2 petr,2 pet,2 pe"),
    (62, "1 joh"),
    (63, "2 joh"),
    (64, "3 joh"),
    (65, "jud"),
    (66, "openb,op,opb,apoc,apocalyps"),
];

/// Every way to write a book, as tokens ("1 Kor." → [1, kor]); longest first.
static BOOK_NAMES: LazyLock<Vec<(Vec<Tok>, u32)>> = LazyLock::new(|| {
    let mut names: Vec<(Vec<Tok>, u32)> = Vec::new();
    let mut add = |name: &str, n: u32| {
        let toks = tokens(&fold(name));
        if !toks.is_empty() && !names.iter().any(|(t, _)| *t == toks) {
            names.push((toks, n));
        }
    };
    for b in BOOKS.iter() {
        add(&b.naam, b.nummer);
        add(&b.afkorting, b.nummer);
    }
    for (n, more) in MORE_BOOK_NAMES {
        for name in more.split(',') {
            add(name, n);
        }
    }
    names.sort_by_key(|(t, _)| std::cmp::Reverse(t.len()));
    names
});

/// The book written at `toks[i..]` when a chapter number follows: (book, tokens used).
fn book_at(toks: &[Tok], i: usize) -> Option<(u32, usize)> {
    BOOK_NAMES.iter().find_map(|(name, n)| {
        let end = i + name.len();
        (toks.get(i..end) == Some(name.as_slice()) && matches!(toks.get(end), Some(Tok::Num(_)))).then_some((*n, name.len()))
    })
}

/// The last verse of a chapter (999 when the program does not have it).
pub fn last_verse(book: u32, chapter: u32) -> u32 {
    crate::texts::passage(BibleRef { book, chapter, from: 1, to: 999 })
        .ok()
        .and_then(|v| v.last().map(|v| v.number))
        .unwrap_or(999)
}

/// The verses in "1, 6", "3-7" (3 and 7) or "1 t/m 3", as `sung_verses` reads them; for a song.
pub fn sung_verse_list(text: &str) -> Vec<u32> {
    sung_verses(&tokens(&fold(text))).0
}

/// Whether a dash between verses is a range ("3-7" is 3 to 7), as most churches write it;
/// off (the default) it is "and", as HHG Tholen writes it. Set from the church's settings
/// (Kerk::streepje_tot_en_met).
static DASH_IS_RANGE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_dash_is_range(on: bool) {
    DASH_IS_RANGE.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// "vers 3, 7 en 9-11" after a psalm or gezang number: a dash is "and" or a range, as the
/// church set it (see DASH_IS_RANGE); "t/m" is always a range. Returns the verses and the
/// words it did not understand.
fn sung_verses(toks: &[Tok]) -> (Vec<u32>, Vec<String>) {
    sung_verses_as(toks, DASH_IS_RANGE.load(std::sync::atomic::Ordering::Relaxed))
}

fn sung_verses_as(toks: &[Tok], dash_is_range: bool) -> (Vec<u32>, Vec<String>) {
    let mut verses: Vec<u32> = Vec::new();
    let mut unknown = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        match &toks[i] {
            Tok::Num(n) => verses.push(*n),
            Tok::Sym('-') if dash_is_range && matches!(toks.get(i + 1), Some(Tok::Num(_))) && !verses.is_empty() => {
                if let (Some(&from), Some(Tok::Num(to))) = (verses.last(), toks.get(i + 1))
                    && *to > from
                    && to - from < 200
                {
                    verses.extend(from + 1..=*to);
                    i += 1;
                }
            }
            Tok::Word(w) if w == "tm" => {
                if let (Some(&from), Some(Tok::Num(to))) = (verses.last(), toks.get(i + 1))
                    && *to > from
                    && to - from < 200
                {
                    verses.extend(from + 1..=*to);
                    i += 1;
                } else {
                    unknown.push("t/m".to_string());
                }
            }
            Tok::Word(w) if matches!(w.as_str(), "en" | "vers" | "verzen" | "vs" | "v" | "couplet" | "coupletten") => {}
            Tok::Sym(',' | '-' | '&' | '+' | ';' | ':' | '/') => {}
            t => unknown.push(t.text()),
        }
        i += 1;
    }
    (verses, unknown)
}

/// Where a part stands: typed by the operator, a line in the website's list of psalms,
/// after "Schriftlezing:", or after "Tekst:".
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Ctx {
    Typed,
    Songs,
    Readings,
    Text,
}

/// What a label before a part says ("Voorzang: Ps 42:1", "Schriftlezing: Joh 3").
#[derive(Debug, Clone, Copy, PartialEq)]
enum Label {
    Song(&'static str, &'static str),
    Lied,
    Reading,
    Text,
    Announcement,
    Collecte,
}

/// Labels: (folded word, needs a colon, meaning).
const LABELS: [(&str, bool, Label); 22] = [
    ("voorzang", false, Label::Song("psalm", "Voorzang")),
    ("zingen", false, Label::Song("psalm", "Zingen")),
    ("tussenzang", false, Label::Song("psalm", "Tussenzang")),
    ("slotzang", false, Label::Song("slotzang", "Slotzang")),
    ("slotpsalm", false, Label::Song("slotzang", "Slotzang")),
    ("nazang", false, Label::Song("nazang", "Na de dienst")),
    ("na de dienst", false, Label::Song("nazang", "Na de dienst")),
    ("lied na de dienst", false, Label::Song("nazang", "Na de dienst")),
    ("lied", true, Label::Lied),
    ("kinderlied", true, Label::Lied),
    ("schriftlezingen", false, Label::Reading),
    ("schriftlezing", false, Label::Reading),
    ("bijbellezing", false, Label::Reading),
    ("schriftgedeelte", false, Label::Reading),
    ("lezingen", true, Label::Reading),
    ("lezing", false, Label::Reading),
    ("lezen", true, Label::Reading),
    ("preektekst", false, Label::Text),
    ("tekst", false, Label::Text),
    ("mededelingen", true, Label::Announcement),
    ("mededeling", true, Label::Announcement),
    ("collecte", true, Label::Collecte),
];

/// Parts of a service that are only a name: (compact text, soort, opschrift).
const SIMPLE: [(&str, &str, &str); 43] = [
    ("gebed", "gebed", "Gebed"),
    ("openingsgebed", "gebed", "Gebed"),
    ("gebedomverlichting", "gebed", "Gebed"),
    ("gebedvoordeprediking", "gebed", "Gebed"),
    ("gebedvoordeschriftlezing", "gebed", "Gebed"),
    ("voorbede", "gebed", "Voorbede"),
    ("slotgebed", "gebed", "Slotgebed"),
    ("dankgebed", "gebed", "Dankgebed"),
    ("dankzegging", "gebed", "Dankgebed"),
    ("dankgebedenvoorbede", "gebed", "Dankgebed"),
    ("collecte", "collecte", "Collecte"),
    ("collectes", "collecte", "Collecte"),
    ("collecten", "collecte", "Collecte"),
    ("inzamelingdergaven", "collecte", "Collecte"),
    ("inzamelingvandegaven", "collecte", "Collecte"),
    ("gaven", "collecte", "Collecte"),
    ("preek", "preek", "Prediking"),
    ("prediking", "preek", "Prediking"),
    ("predicatie", "preek", "Prediking"),
    ("verkondiging", "preek", "Prediking"),
    ("bedieningvanhetwoord", "preek", "Prediking"),
    ("zegen", "zegen", "Zegen"),
    ("zegenbede", "zegen", "Zegen"),
    ("slotzegen", "zegen", "Zegen"),
    ("welkom", "welkom", "Startscherm"),
    ("startscherm", "welkom", "Startscherm"),
    ("einde", "einde", "Aftiteling"),
    ("aftiteling", "einde", "Aftiteling"),
    ("mededeling", "mededeling", "Mededelingen"),
    ("mededelingen", "mededeling", "Mededelingen"),
    ("afkondigingen", "mededeling", "Mededelingen"),
    ("votum", "mededeling", "Votum"),
    ("zegengroet", "mededeling", "Zegengroet"),
    ("votumenzegengroet", "mededeling", "Votum en zegengroet"),
    ("levensloop", "mededeling", "Levensloop"),
    ("inmemoriam", "mededeling", "In memoriam"),
    ("condoleance", "mededeling", "Condoleance"),
    ("woordvandefamilie", "mededeling", "Woord van de familie"),
    ("uitdragen", "mededeling", "Uitdragen"),
    ("afscheid", "mededeling", "Afscheid"),
    ("wet", "lezing", "Wet"),
    ("wetslezing", "lezing", "Wet"),
    ("lezingvandewet", "lezing", "Wet"),
];

fn raw(soort: &str, opschrift: &str, bron: &str) -> RawItem {
    RawItem { soort: soort.into(), opschrift: opschrift.into(), herkend: true, bron: bron.into(), ..Default::default() }
}

/// Something the reader does not understand: kept as a named part, marked with the reason.
fn not_understood(text: &str, reden: String) -> RawItem {
    RawItem { niet_begrepen: true, herkend: false, reden, ..raw("mededeling", text, text) }
}

fn understood_partly(mut item: RawItem, unknown: &[String]) -> RawItem {
    if !unknown.is_empty() {
        item.herkend = false;
        item.niet_begrepen = true;
        item.reden = format!("'{}' niet begrepen", unknown.join(" "));
    }
    item
}

/// Typed text is split at commas, semicolons and new lines, but "ps 42: 1, 3" stays one part.
static CONTINUATION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:(?:en|vers|vs|v)\s*)?\d+[a-d]?(?:\s*(?:-|en|&|\+|t/m|tm)\s*\d+[a-d]?)*$").unwrap());
static POINT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*\d{1,2}\s*[.)]\s*(\S.*)$").unwrap());

/// The numbered point of a theme ("1. Nu al" → "Nu al").
pub fn theme_point(line: &str) -> Option<String> {
    POINT.captures(line).map(|c| c[1].trim().to_string())
}

/// Reads liturgy text. The gezangen are found by name among the ones the program has.
pub struct Reader {
    gezangen: Vec<(u32, String)>,
}

impl Default for Reader {
    fn default() -> Reader {
        Reader { gezangen: crate::texts::gezangen() }
    }
}

impl Reader {
    /// With another list of gezangen (number, name) than the program's own (for tests).
    #[cfg(test)]
    pub fn with_gezangen(gezangen: Vec<(u32, String)>) -> Reader {
        Reader { gezangen }
    }

    /// What the operator typed, part by part ("ps 42:5, gebed, joh 3:16-21, HC 57").
    pub fn typed(&self, input: &str) -> Vec<RawItem> {
        let mut parts: Vec<String> = Vec::new();
        for line in input.lines() {
            let mut first = true;
            for (seg, sep) in split_keep(line) {
                let seg = seg.trim();
                // Only numbers ("6", "3 en 5") are never a part of their own.
                let continues = !first && !parts.is_empty() && CONTINUATION.is_match(fold(&BRACKETS.replace_all(seg, "")).trim());
                if continues {
                    let last = parts.last_mut().unwrap();
                    last.push(sep);
                    last.push(' ');
                    last.push_str(seg);
                } else if !seg.is_empty() {
                    parts.push(seg.to_string());
                }
                first = false;
            }
        }
        parts.iter().flat_map(|p| self.part(p, Ctx::Typed)).collect()
    }

    /// One part: a psalm, reading, catechism, form, a named part, … Several items when it
    /// holds several readings.
    pub fn part(&self, text: &str, ctx: Ctx) -> Vec<RawItem> {
        let text = text.trim().trim_end_matches(['.', ',', ';']).trim();
        if text.is_empty() {
            return Vec::new();
        }
        if let Some(item) = simple(text) {
            return vec![item];
        }
        let (label, rest) = split_label(text);
        let rest = rest.trim();
        if rest.is_empty() {
            return vec![not_understood(text, format!("'{text}' zonder psalm, tekst of naam erbij"))];
        }
        let ctx = match label {
            Some(Label::Reading) => Ctx::Readings,
            Some(Label::Text) => Ctx::Text,
            _ => ctx,
        };
        match label {
            Some(Label::Announcement) => return vec![RawItem { tekst: rest.into(), ..raw("mededeling", "Mededeling", text) }],
            Some(Label::Collecte) => return vec![RawItem { tekst: rest.into(), ..raw("collecte", "Collecte", text) }],
            _ => {}
        }
        if ctx == Ctx::Text {
            return vec![self.sermon_text(rest)];
        }
        if ctx == Ctx::Readings {
            return self.readings(rest);
        }
        // A song from a song book by its number: "LB 23:1, 3", "Opw 733".
        if let Some(song) = crate::songs::book_ref(rest) {
            return vec![raw("lied", &song, text)];
        }
        let folded = fold(&BRACKETS.replace_all(rest, " "));
        let toks = tokens(&folded);
        let mut items = self.recognise(rest, &toks, ctx);
        if items.is_empty() {
            items = vec![match label {
                Some(Label::Lied) => raw("lied", rest, rest),
                Some(Label::Song(..)) => raw("lied", rest, rest),
                _ if ctx == Ctx::Songs => raw("lied", rest, rest),
                _ if folded.starts_with("lied ") => raw("lied", rest.split_once(char::is_whitespace).map_or("", |(_, t)| t).trim(), rest),
                _ => not_understood(
                    rest,
                    "niet herkend; typ bijvoorbeeld 'ps 42:5', 'gez 3:1', 'joh 3:16-21', 'HC 57', 'gebed' of 'lied: …'".into(),
                ),
            }];
        }
        if let Some(Label::Song(soort, opschrift)) = label {
            for it in &mut items {
                if it.soort == "psalm" {
                    it.soort = soort.into();
                    it.opschrift = opschrift.into();
                }
            }
        }
        items
    }

    /// A known kind of part in `toks`, or nothing.
    fn recognise(&self, text: &str, toks: &[Tok], ctx: Ctx) -> Vec<RawItem> {
        let key = key_of(toks);
        // Among the website's psalms only with its name ("HC zondag 22"), not "zondag 22".
        if let Some(item) = catechism(text, toks, ctx != Ctx::Songs) {
            return vec![item];
        }
        if let Some(item) = form(text, &key, toks) {
            return vec![item];
        }
        if let Some(item) = confession(text, &key, toks) {
            return vec![item];
        }
        // "Psalm 116 vers 1 t/m 9" is a reading: sung psalms are never written with t/m.
        let psalm_reading = matches!(toks.first(), Some(Tok::Word(w)) if w == "psalm" || w == "psalmen")
            && toks.iter().any(|t| t.word() == "tm");
        if !psalm_reading && let Some(item) = self.song(text, toks) {
            return vec![item];
        }
        if book_at(toks, 0).is_some() {
            return self.readings(text);
        }
        Vec::new()
    }

    /// A psalm or gezang: "Ps.31: 19", "Psalm 42 vers 1 en 3", "Gez. 5: 7-8", "LvM:1",
    /// "Morgenzang : 3, 4 en 5".
    fn song(&self, text: &str, toks: &[Tok]) -> Option<RawItem> {
        let bron = text.trim();
        let first = toks.first()?.word();
        if matches!(first, "ps" | "psalm") {
            let n = toks.get(1)?.num()?;
            let (verses, unknown) = sung_verses(&toks[2..]);
            let item = RawItem { psalm: n, verzen: verses, ..raw("psalm", "Zingen", bron) };
            return Some(understood_partly(item, &unknown));
        }
        if matches!(first, "gez" | "gezang" | "gezangen") {
            let nr = toks.get(1)?.num()?;
            let (verses, unknown) = sung_verses(&toks[2..]);
            return Some(understood_partly(self.gezang(nr, verses, bron), &unknown));
        }
        // By name: the name before the colon, before "vers", or before the numbers at the end
        // ("Morgenzang 3"; "10 Geb" keeps its number).
        let split = toks.iter().position(|t| matches!(t, Tok::Sym(':' | ';')) || matches!(t.word(), "vers" | "verzen" | "vs")).unwrap_or_else(|| {
            let mut at = toks.len();
            while at > 0 && (toks[at - 1].num().is_some() || toks[at - 1].word() == "en" || matches!(toks[at - 1], Tok::Sym(_))) {
                at -= 1;
            }
            at
        });
        let nr = gezang_by_name(&key_of(&toks[..split]))?;
        let (verses, unknown) = sung_verses(&toks[split..]);
        Some(understood_partly(self.gezang(nr, verses, bron), &unknown))
    }

    /// Gezang `nr` (1-12, the church's numbering) as an item: the program's own number for
    /// it, found by name; flagged when the program does not have it.
    fn gezang(&self, nr: u32, verses: Vec<u32>, bron: &str) -> RawItem {
        let item = RawItem { verzen: verses, ..raw("psalm", "Zingen", bron) };
        if !(1..=12).contains(&nr) {
            return RawItem { herkend: false, reden: format!("gezang {nr} bestaat niet (1 t/m 12)"), ..item };
        }
        // 150 + nr is how the program numbers them; the check of the texts
        // (RawItem::into_item) then says it is missing.
        RawItem { psalm: self.gezang_psalm(nr).unwrap_or(150 + nr), ..item }
    }

    /// The program's number for gezang `nr` of the church's list, found by name among the
    /// gezangen the program has (6 and 7, both the Twaalf Artikelen, in order); else 150 + nr
    /// if the program has that one.
    fn gezang_psalm(&self, nr: u32) -> Option<u32> {
        let (base, index) = if nr == 7 { (6, 1) } else { (nr, 0) };
        let same: Vec<u32> = self.gezangen.iter().filter(|(_, name)| gezang_by_name(&compact(name)) == Some(base)).map(|(n, _)| *n).collect();
        same.get(index)
            .copied()
            .or_else(|| self.gezangen.iter().any(|(n, _)| *n == 150 + nr).then_some(150 + nr))
            .or_else(|| same.first().copied())
    }

    /// Readings: "1 Samuël 13", "Joh. 3:16-21", "Habakuk 3 vers 17 t/m 19, 1 Petrus 1 vers
    /// 3 t/m 9", "Kolossensen 1: 21 - 2: 23", "Johannes 8 vers 25 t/m 51 en 58 en 59". One
    /// item per piece of a chapter.
    pub fn readings(&self, text: &str) -> Vec<RawItem> {
        let mut out = Vec::new();
        for chunk in reading_chunks(text) {
            match passages(&chunk) {
                Ok(refs) => out.extend(refs.into_iter().map(|r| reading_item(r, &chunk))),
                Err((refs, why)) if refs.is_empty() => out.push(not_understood(&chunk, why)),
                Err((refs, why)) => {
                    out.extend(refs.into_iter().map(|r| RawItem { herkend: false, niet_begrepen: true, reden: why.clone(), ..reading_item(r, &chunk) }))
                }
            }
        }
        out
    }

    /// The text of the sermon: one reading or catechism questions ("1 Samuël 13 vers 11",
    /// "Catechismus vraag en antwoord 58", "Zondag 4 Heidelbergse Catechismus").
    pub fn sermon_text(&self, text: &str) -> RawItem {
        let text = text.trim();
        let toks = tokens(&fold(&BRACKETS.replace_all(text, " ")));
        if let Some(mut item) = catechism(text, &toks, true) {
            item.opschrift = "Heidelbergse Catechismus".into();
            return item;
        }
        let items = self.readings(text);
        let Some(first) = items.first() else {
            return not_understood(text, "tekst niet herkend".into());
        };
        let mut item = RawItem { opschrift: "Tekst".into(), bron: text.into(), ..first.clone() };
        if items.len() > 1 {
            // One passage for the sermon: from the first to the last verse, to check.
            let same = items.iter().all(|i| i.boek == first.boek && i.hoofdstuk == first.hoofdstuk && i.soort == "lezing");
            if same {
                item.tot = items.iter().map(|i| i.tot).max().unwrap_or(item.tot);
            }
            let shown = BibleRef { book: item.boek, chapter: item.hoofdstuk, from: item.van, to: item.tot };
            item.herkend = false;
            item.reden = format!("de tekst heeft meer delen; nu {shown}");
        }
        item
    }
}

/// Splits at commas and semicolons, keeping which one it was.
fn split_keep(line: &str) -> Vec<(&str, char)> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut sep = ',';
    for (i, c) in line.char_indices() {
        if c == ',' || c == ';' {
            out.push((&line[start..i], sep));
            sep = c;
            start = i + c.len_utf8();
        }
    }
    out.push((&line[start..], sep));
    out
}

/// The label before a part and the rest: "Voorzang: Ps 42:1" → (Voorzang, "Ps 42:1").
fn split_label(text: &str) -> (Option<Label>, &str) {
    let folded = fold(text);
    for (word, colon, label) in LABELS {
        let Some(after) = folded.strip_prefix(word) else { continue };
        let colon_follows = after.trim_start().starts_with(':');
        let space_follows = after.starts_with(' ');
        if colon_follows || (!colon && space_follows) {
            // The same number of words in the original text.
            let words = word.split(' ').count();
            let mut rest = text.trim_start();
            for _ in 0..words {
                rest = rest.trim_start();
                rest = rest.find(|c: char| c.is_whitespace() || c == ':').map_or("", |i| &rest[i..]);
            }
            let rest = rest.trim_start().trim_start_matches(':');
            return (Some(label), rest);
        }
    }
    (None, text)
}

/// A part that is only a name ("gebed", "Dankgebed", "collecte", "votum", "zegen").
fn simple(text: &str) -> Option<RawItem> {
    let key = compact(text);
    let (_, soort, opschrift) = SIMPLE.iter().find(|(k, _, _)| *k == key)?;
    let item = raw(soort, opschrift, text);
    if *opschrift == "Wet" {
        // The Law is read from Exodus 20.
        return Some(RawItem { boek: 2, hoofdstuk: 20, van: 1, tot: 17, ..item });
    }
    Some(item)
}

/// "HC 57", "HC 57, 58", "Catechismus vraag en antwoord 58", "Zondag 4 Heidelbergse
/// Catechismus"; with `bare` also "zondag 22" and "vraag 57".
fn catechism(text: &str, toks: &[Tok], bare: bool) -> Option<RawItem> {
    let words: Vec<&str> = toks.iter().map(Tok::word).filter(|w| !w.is_empty()).collect();
    let named = words.iter().any(|w| matches!(*w, "catechismus" | "heidelbergse" | "heidelberger" | "heidelbergsche"))
        || words.first() == Some(&"hc");
    let first_num = matches!(toks.get(1), Some(Tok::Num(_)));
    let starts = bare && matches!(words.first(), Some(&("zondag" | "zo" | "vraag" | "vr" | "vragen"))) && first_num;
    let qa = toks.windows(3).any(|w| matches!(w[0].word(), "vraag" | "vr" | "v") && w[1].word() == "en" && matches!(w[2].word(), "antwoord" | "antw" | "a"));
    if !(named || starts || qa) {
        return None;
    }
    let mut zondag = 0;
    let mut questions: Vec<u32> = Vec::new();
    let mut unknown: Vec<String> = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        match &toks[i] {
            Tok::Word(w) if matches!(w.as_str(), "zondag" | "zo" | "zd") && toks.get(i + 1).and_then(Tok::num).is_some() => {
                zondag = toks[i + 1].num().unwrap_or(0);
                i += 1;
            }
            Tok::Num(n) => questions.push(*n),
            Tok::Word(w) if w == "tm" => {
                if let (Some(&from), Some(to)) = (questions.last(), toks.get(i + 1).and_then(Tok::num))
                    && to > from
                    && to - from < 129
                {
                    questions.extend(from + 1..=to);
                    i += 1;
                }
            }
            Tok::Sym('-') => {
                // A range of questions ("vraag 1-3"), as in a reading.
                if let (Some(&from), Some(to)) = (questions.last(), toks.get(i + 1).and_then(Tok::num))
                    && to > from
                    && to - from < 129
                {
                    questions.extend(from + 1..=to);
                    i += 1;
                }
            }
            Tok::Word(w)
                if matches!(
                    w.as_str(),
                    "hc" | "heidelbergse" | "heidelberger" | "heidelbergsche" | "catechismus" | "vraag" | "vragen" | "vr" | "v" | "en"
                        | "antwoord" | "antwoorden" | "antw" | "a" | "de" | "van" | "uit" | "het" | "zondag" | "zo"
                ) => {}
            Tok::Sym(',' | '&' | ':' | ';' | '/' | '+') => {}
            t => unknown.push(t.text()),
        }
        i += 1;
    }
    let item = RawItem { vragen: questions.clone(), zondag, ..raw("catechismus", "Heidelbergse Catechismus", text) };
    if questions.is_empty() && zondag == 0 {
        return Some(RawItem { herkend: false, reden: "welke vraag of zondag van de catechismus?".into(), ..item });
    }
    Some(understood_partly(item, &unknown))
}

/// The roman numbers of the Dordtse Leerregels' chapters (III/IV is one).
fn roman(w: &str) -> Option<u32> {
    match w {
        "i" => Some(1),
        "ii" => Some(2),
        "iii" => Some(3),
        "iv" => Some(4),
        "v" => Some(5),
        _ => None,
    }
}

/// Numbers, with a dash or t/m as a range (articles are read in order): "14-16" → 14, 15, 16.
fn numbers_with_ranges(toks: &[Tok]) -> Vec<u32> {
    let mut out: Vec<u32> = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        match &toks[i] {
            Tok::Num(n) => out.push(*n),
            t if matches!(t, Tok::Sym('-')) || t.word() == "tm" => {
                if let (Some(&from), Some(to)) = (out.last(), toks.get(i + 1).and_then(Tok::num))
                    && to > from
                    && to - from < 100
                {
                    out.extend(from + 1..=to);
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    out
}

/// NGB, DL and the creeds: "NGB art. 14", "Nederlandse Geloofsbelijdenis artikel 14",
/// "DL I, 1-3", "Dordtse Leerregels hoofdstuk 1 artikel 1", "Apostolische
/// Geloofsbelijdenis", "Twaalf artikelen gelezen".
fn confession(text: &str, key: &str, toks: &[Tok]) -> Option<RawItem> {
    let words: Vec<&str> = toks.iter().map(Tok::word).collect();
    let has_num = toks.iter().any(|t| t.num().is_some());
    let item = |doc: &str| {
        let label = if doc == "dl" { "Dordtse Leerregels" } else { "Geloofsbelijdenis" };
        RawItem { document: doc.into(), ..raw("belijdenis", label, text) }
    };
    if key.contains("ngb") || key.contains("nederlandsegeloofsbelijdenis") || (key.contains("geloofsbelijdenis") && has_num) {
        let at = toks.iter().position(|t| matches!(t.word(), "art" | "artikel" | "artikelen")).map_or(0, |i| i + 1);
        let start = if at == 0 { toks.iter().position(|t| t.num().is_some()).unwrap_or(toks.len()) } else { at };
        return Some(RawItem { artikelen: numbers_with_ranges(&toks[start..]), ..item("ngb") });
    }
    if words.first() == Some(&"dl") || key.contains("leerregels") {
        // The chapter: a roman number, "hoofdstuk 1", or the number before a colon or comma.
        let mut chapter = 0;
        let mut rest = toks.len();
        for (i, t) in toks.iter().enumerate() {
            if let Some(r) = roman(t.word())
                && i > 0
            {
                chapter = r;
                rest = i + 1;
                // "III/IV" is one chapter in the program (3).
                if toks.get(i + 1) == Some(&Tok::Sym('/')) && toks.get(i + 2).and_then(|t| roman(t.word())).is_some() {
                    rest = i + 3;
                }
                break;
            }
            if matches!(t.word(), "hoofdstuk" | "hfst" | "h" | "hfdst")
                && let Some(n) = toks.get(i + 1).and_then(Tok::num)
            {
                chapter = n;
                rest = i + 2;
                break;
            }
            if let Some(n) = t.num() {
                chapter = n;
                rest = i + 1;
                break;
            }
        }
        if chapter == 4 {
            chapter = 3;
        }
        let artikelen = numbers_with_ranges(toks.get(rest..).unwrap_or_default());
        let it = RawItem { hoofdstuk: chapter, artikelen, ..item("dl") };
        if chapter == 0 {
            return Some(RawItem { herkend: false, reden: "welk hoofdstuk van de Dordtse Leerregels?".into(), ..it });
        }
        return Some(it);
    }
    let read = words.iter().any(|w| matches!(*w, "lezen" | "gelezen" | "voorlezen" | "lezing"));
    if key.contains("apostolische") || key.contains("apostolicum") || (key.contains("artikelen") && read) {
        return Some(item("apostolicum"));
    }
    if key.contains("nicea") || key.contains("niceense") || key.contains("nicaenum") {
        return Some(item("nicea"));
    }
    if key.contains("athanasius") || key.contains("athanasiaanse") {
        return Some(item("athanasius"));
    }
    if key == "geloofsbelijdenis" {
        return Some(RawItem { herkend: false, reden: "aangenomen: de Apostolische Geloofsbelijdenis".into(), ..item("apostolicum") });
    }
    None
}

/// The liturgical forms: "Doopformulier", "Bediening van de Heilige Doop",
/// "Avondmaalsformulier", "Openbare belijdenis", "Bevestiging ambtsdragers", "Huwelijk".
fn form(text: &str, key: &str, toks: &[Tok]) -> Option<RawItem> {
    let words: Vec<&str> = toks.iter().map(Tok::word).collect();
    let doc = if key.contains("doop") && !key.contains("doopvont") {
        if key.contains("volwassen") { "doop-volwassenen" } else { "doop-kinderen" }
    } else if key.contains("avondmaal") {
        "heilig-avondmaal"
    } else if key.contains("openbarebelijdenis") || key.contains("openbaregeloofsbelijdenis") || key.contains("belijdenisafleggen") || key.contains("belijdenisdoen")
        || (key.contains("formulier") && key.contains("belijdenis"))
    {
        "belijdenis"
    } else if key.contains("huwelijk") {
        "huwelijk"
    } else if key.contains("bevestiging") && (key.contains("predikant") || key.contains("dienaar") || key.contains("intrede")) {
        "bevestiging-predikant"
    } else if key.contains("bevestiging") && (key.contains("ouderling") || key.contains("diaken") || key.contains("ambtsdrager")) {
        "bevestiging-ouderling-diaken"
    } else if words.contains(&"ban") || key.contains("afsnijding") || key.contains("christelijkeban") {
        "christelijke-ban"
    } else if key.contains("wederopneming") {
        "wederopneming-in-de-gemeente"
    } else {
        return None;
    };
    let label = crate::confessions::form(doc).map(|f| f.kort.clone()).unwrap_or_else(|| "Formulier".into());
    Some(RawItem { document: doc.into(), ..raw("formulier", &label, text) })
}

/// Splits a line of readings where a new book starts: "Habakuk 3 vers 17 t/m 19, 1 Petrus
/// 1 vers 3 t/m 9" and "Jesaja 63 vers 7 t/m 11 & Efeze 4" (but "vers 6 en 7" stays).
fn reading_chunks(text: &str) -> Vec<String> {
    static SEP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\s*(?:[,;&+]|\ben\b)\s*").unwrap());
    let mut chunks = Vec::new();
    let mut start = 0;
    for m in SEP.find_iter(text) {
        let next = &text[m.end()..];
        if book_at(&tokens(&fold(next)), 0).is_some() {
            chunks.push(text[start..m.start()].trim().to_string());
            start = m.end();
        }
    }
    chunks.push(text[start..].trim().to_string());
    chunks.retain(|c| !c.is_empty());
    chunks
}

/// The passages of one reading ("Johannes 8 vers 25 t/m 51 en 58 en 59" → 8:25-51 and
/// 8:58-59). A dash and "t/m" are a range here. Err: what it could not read, with what it
/// could.
fn passages(chunk: &str) -> Result<Vec<BibleRef>, (Vec<BibleRef>, String)> {
    let toks = tokens(&fold(&BRACKETS.replace_all(chunk, " ")));
    let Some((book, used)) = book_at(&toks, 0) else {
        return Err((Vec::new(), "bijbelboek niet herkend".into()));
    };
    let chapter = toks[used].num().unwrap_or(0);
    let mut i = used + 1;
    // Pieces as (chapter, from, to-chapter, to).
    let mut pieces: Vec<(u32, u32, u32, u32)> = Vec::new();
    let mut unknown: Vec<String> = Vec::new();
    let num = |i: usize| toks.get(i).and_then(Tok::num);
    while i < toks.len() {
        match &toks[i] {
            Tok::Num(a) => {
                let a = *a;
                let ranged = matches!(toks.get(i + 1), Some(Tok::Sym('-'))) || toks.get(i + 1).is_some_and(|t| t.word() == "tm");
                match (ranged, num(i + 2)) {
                    // "1: 21 - 2: 23": to another chapter.
                    (true, Some(b)) if toks.get(i + 3) == Some(&Tok::Sym(':')) && num(i + 4).is_some() => {
                        pieces.push((chapter, a, b, num(i + 4).unwrap()));
                        i += 4;
                    }
                    (true, Some(b)) => {
                        pieces.push((chapter, a, chapter, b));
                        i += 2;
                    }
                    _ => pieces.push((chapter, a, chapter, a)),
                }
            }
            Tok::Sym(':' | ',' | ';' | '&' | '+') => {}
            Tok::Word(w) if matches!(w.as_str(), "vers" | "verzen" | "vs" | "v" | "en" | "m" | "n" | "mn") => {}
            t => unknown.push(t.text()),
        }
        i += 1;
    }
    let mut refs: Vec<BibleRef> = Vec::new();
    if pieces.is_empty() {
        refs.push(BibleRef { book, chapter, from: 1, to: 999 });
    }
    for (c1, from, c2, to) in pieces {
        if c2 > c1 && c2 - c1 <= 10 {
            refs.push(BibleRef { book, chapter: c1, from, to: last_verse(book, c1) });
            for c in c1 + 1..c2 {
                refs.push(BibleRef { book, chapter: c, from: 1, to: 999 });
            }
            refs.push(BibleRef { book, chapter: c2, from: 1, to });
        } else {
            refs.push(BibleRef { book, chapter: c1, from, to: to.max(from) });
        }
    }
    // Verses that follow on each other are one passage: "vers 6 en 7" is 6-7.
    let mut merged: Vec<BibleRef> = Vec::new();
    for r in refs {
        match merged.last_mut() {
            Some(p) if p.book == r.book && p.chapter == r.chapter && r.from == p.to + 1 && p.to < 999 => p.to = r.to,
            _ => merged.push(r),
        }
    }
    if unknown.is_empty() { Ok(merged) } else { Err((merged, format!("'{}' niet begrepen", unknown.join(" ")))) }
}

fn reading_item(r: BibleRef, bron: &str) -> RawItem {
    RawItem { boek: r.book, hoofdstuk: r.chapter, van: r.from, tot: r.to, ..raw("lezing", "Schriftlezing", bron) }
}

/// What the operator typed, as items. The program reads it itself; parts it does not
/// understand go to Claude when there is a key (and are then marked as read by AI), else
/// they stay marked as not recognised.
pub async fn typed_items(input: &str) -> Vec<Item> {
    let mut raws = Reader::default().typed(input);
    if crate::claude::has_key() {
        let unknown: Vec<usize> = (0..raws.len()).filter(|&i| raws[i].niet_begrepen).collect();
        let asks = unknown.iter().map(|&i| {
            let text = raws[i].bron.clone();
            async move { tokio::time::timeout(std::time::Duration::from_secs(60), crate::claude::ask_typed(&text)).await }
        });
        let answers = futures::future::join_all(asks).await;
        // From the back, so the places of the others stay the same.
        for (&i, answer) in unknown.iter().zip(answers).rev() {
            match answer {
                Ok(Ok(items)) if !items.is_empty() => {
                    let bron = raws[i].bron.clone();
                    raws.splice(i..=i, items.into_iter().map(|r| from_ai(r, &bron)));
                }
                Ok(Ok(_)) => {}
                Ok(Err(e)) => eprintln!("AI bij '{}': {e:#}", raws[i].bron),
                Err(_) => eprintln!("AI bij '{}': geen antwoord binnen een minuut", raws[i].bron),
            }
        }
    }
    raws.into_iter().map(RawItem::into_item).collect()
}

/// Claude's reading of something the program did not understand: used, but marked.
pub fn from_ai(r: RawItem, bron: &str) -> RawItem {
    let reden = if r.herkend || r.reden.is_empty() {
        format!("door AI gelezen uit '{bron}'; nakijken")
    } else {
        format!("door AI gelezen uit '{bron}': {}", r.reden)
    };
    RawItem { herkend: false, reden, bron: bron.into(), niet_begrepen: false, ..r }
}

#[cfg(test)]
mod tests {

    #[test]
    fn a_dash_is_and_or_a_range_as_the_church_writes_it() {
        let toks = |t: &str| tokens(&fold(t));
        assert_eq!(sung_verses_as(&toks("3-7"), false).0, [3, 7]);
        assert_eq!(sung_verses_as(&toks("3-7"), true).0, [3, 4, 5, 6, 7]);
        assert_eq!(sung_verses_as(&toks("1, 3-5 en 8"), true).0, [1, 3, 4, 5, 8]);
        // "t/m" is a range either way.
        assert_eq!(sung_verses_as(&toks("2 t/m 4"), false).0, [2, 3, 4]);
        // A dash backwards stays two verses.
        assert_eq!(sung_verses_as(&toks("7-3"), true).0, [7, 3]);
    }

    use super::*;

    /// Gezang names as another edition might write them, so the tests do not depend on the
    /// program having the gezangen.
    fn reader() -> Reader {
        let names = [
            "De Tien Geboden des HEEREN",
            "De Lofzang van Maria",
            "De Lofzang van Zacharias",
            "De Lofzang van Simeon",
            "Het Gebed des HEEREN",
            "De Twaalf Artikelen",
            "De Twaalf Artikelen",
            "Bedezang voor de predikatie",
            "Morgenzang",
            "Bedezang voor het eten",
            "Dankzang na het eten",
            "Avondzang",
        ];
        Reader::with_gezangen(names.iter().enumerate().map(|(i, n)| (151 + i as u32, n.to_string())).collect())
    }

    fn one(text: &str) -> RawItem {
        let items = reader().typed(text);
        assert_eq!(items.len(), 1, "{text}: {items:?}");
        items.into_iter().next().unwrap()
    }

    fn psalm(text: &str) -> (u32, Vec<u32>) {
        let it = one(text);
        assert!(it.herkend, "{text}: {}", it.reden);
        assert!(matches!(it.soort.as_str(), "psalm" | "slotzang" | "nazang"), "{text}: {}", it.soort);
        (it.psalm, it.verzen)
    }

    fn reading(text: &str) -> Vec<(u32, u32, u32, u32)> {
        reader().typed(text).iter().inspect(|i| assert_eq!(i.soort, "lezing", "{text}")).map(|i| (i.boek, i.hoofdstuk, i.van, i.tot)).collect()
    }

    #[test]
    fn psalms_with_the_church_dash() {
        assert_eq!(psalm("Ps.31: 19"), (31, vec![19]));
        assert_eq!(psalm("Ps. 74: 8-11"), (74, vec![8, 11]));
        assert_eq!(psalm("Ps. 27: 3-7"), (27, vec![3, 7]));
        assert_eq!(psalm("Psalm 42 vers 1 en 3"), (42, vec![1, 3]));
        assert_eq!(psalm("ps 42:5"), (42, vec![5]));
        assert_eq!(psalm("Ps 119: 1-3-5"), (119, vec![1, 3, 5]));
        assert_eq!(psalm("Ps 42 : 1, 2"), (42, vec![1, 2]));
        assert_eq!(psalm("Ps. 118; 14"), (118, vec![14]));
        assert_eq!(psalm("Ps. 26: 8 (1) en 12"), (26, vec![8, 12]));
        assert_eq!(psalm("Ps. 75: 1 (1e tafel)"), (75, vec![1]));
        assert_eq!(psalm("ps 84 : 3,5"), (84, vec![3, 5]));
        // Only t/m is a range.
        assert_eq!(psalm("ps 27: 3 t/m 5"), (27, vec![3, 4, 5]));
        let s = one("Slotzang: Ps 134:3");
        assert_eq!((s.soort.as_str(), s.opschrift.as_str(), s.psalm), ("slotzang", "Slotzang", 134));
        let v = one("Voorzang: Ps.31: 19");
        assert_eq!((v.opschrift.as_str(), v.psalm), ("Voorzang", 31));
        assert_eq!(one("Na de dienst: Ps 134:3").soort, "nazang");
    }

    #[test]
    fn psalm_not_fully_understood_is_marked() {
        let it = one("Ps. 18: Vz en 1");
        assert_eq!((it.psalm, it.verzen.clone()), (18, vec![1]));
        assert!(!it.herkend && it.niet_begrepen, "{it:?}");
        assert!(it.reden.contains("vz"));
    }

    #[test]
    fn gezangen_by_number_and_name() {
        let g = |t: &str| psalm(t);
        assert_eq!(g("Gez 3"), (153, vec![]));
        assert_eq!(g("Gez. 5: 7-8"), (155, vec![7, 8]));
        assert_eq!(g("gez 4: 2"), (154, vec![2]));
        assert_eq!(g("Gezang 12: 7"), (162, vec![7]));
        assert_eq!(g("Gez. 9: 5-6"), (159, vec![5, 6]));
        assert_eq!(g("Morgenzang:1"), (159, vec![1]));
        assert_eq!(g("Morgenzang : 3, 4 en 5 (2)"), (159, vec![3, 4, 5]));
        assert_eq!(g("Avondzang:7"), (162, vec![7]));
        assert_eq!(g("TGdH:1"), (151, vec![1]));
        assert_eq!(g("Tien Geboden: 2"), (151, vec![2]));
        assert_eq!(g("10 Geb: 9"), (151, vec![9]));
        assert_eq!(g("LvM:7"), (152, vec![7]));
        assert_eq!(g("L v Maria: 3"), (152, vec![3]));
        assert_eq!(g("Lofzang van Maria: 1-3"), (152, vec![1, 3]));
        assert_eq!(g("LvZ:4"), (153, vec![4]));
        assert_eq!(g("L v Zach: 1"), (153, vec![1]));
        assert_eq!(g("Zacharias: 2"), (153, vec![2]));
        assert_eq!(g("Lofzang van Zacharias: 2"), (153, vec![2]));
        assert_eq!(g("Simeon: 1"), (154, vec![1]));
        assert_eq!(g("LvS:2"), (154, vec![2]));
        assert_eq!(g("L v Sim: 1"), (154, vec![1]));
        assert_eq!(g("Geb.d.H: 3 (2) en 6 (3)"), (155, vec![3, 6]));
        assert_eq!(g("Gebed des Heeren: 1"), (155, vec![1]));
        assert_eq!(g("Onze Vader: 2"), (155, vec![2]));
        assert_eq!(g("12 Artikelen: 1-2"), (156, vec![1, 2]));
        assert_eq!(g("Geloofsartikelen: 3"), (156, vec![3]));
        assert_eq!(g("Gez 7: 1"), (157, vec![1]));
    }

    #[test]
    fn gezangen_follow_the_names_not_the_numbers() {
        // Another order in the program: found by name.
        let r = Reader::with_gezangen(vec![(151, "Morgenzang".into()), (152, "Het Gebed des HEEREN".into())]);
        assert_eq!(r.typed("Gez 9: 1")[0].psalm, 151);
        assert_eq!(r.typed("Geb.d.H: 2")[0].psalm, 152);
        // Not in the program: the usual number, and the check says it is missing.
        let none = Reader::with_gezangen(Vec::new());
        let it = none.typed("LvM: 3").remove(0);
        assert_eq!((it.psalm, it.herkend), (152, true));
        if crate::texts::psalm(152).is_none() {
            let item = it.into_item();
            assert!(item.problem.contains("Gezang 2 (Lofzang van Maria)"), "{}", item.problem);
            assert_eq!(item.reference, "Gezang 2 : 3");
        }
    }

    #[test]
    fn readings() {
        assert_eq!(reading("1 Samuël 13"), [(9, 13, 1, 999)]);
        assert_eq!(reading("1 Samu\u{eb}l 13"), [(9, 13, 1, 999)]);
        assert_eq!(reading("1 Sam. 13:11"), [(9, 13, 11, 11)]);
        assert_eq!(reading("1e Samuel 13 vers 11"), [(9, 13, 11, 11)]);
        assert_eq!(reading("Joh. 3:16-21"), [(43, 3, 16, 21)]);
        assert_eq!(reading("joh 3:16-21"), [(43, 3, 16, 21)]);
        assert_eq!(reading("Lezing: Habakuk 3 vers 17 t/m 19"), [(35, 3, 17, 19)]);
        assert_eq!(reading("Schriftlezing: Habakuk 3 vers 17 t/m 19, 1 Petrus 1 vers 3 t/m 9, Openbaring 7 vers 9 t/m 12"), [
            (35, 3, 17, 19),
            (60, 1, 3, 9),
            (66, 7, 9, 12)
        ]);
        assert_eq!(reading("Schriftlezing: Jesaja 63 vers 7 t/m 11 & Efeze 4 vers 17 t/m 32"), [(23, 63, 7, 11), (49, 4, 17, 32)]);
        assert_eq!(reading("Schriftlezing: 1 Korinthe 11 vers 23 t/m 26 en Jesaja 62 vers 6 en 7"), [(46, 11, 23, 26), (23, 62, 6, 7)]);
        assert_eq!(reading("Schriftlezing: Johannes 6 vers 48 t/m 56 en 1 Korinthe 11 vers 23 t/m 26"), [(43, 6, 48, 56), (46, 11, 23, 26)]);
        assert_eq!(reading("Lezing: Johannes 8 vers 25 t/m 51 en 58 en 59"), [(43, 8, 25, 51), (43, 8, 58, 59)]);
        assert_eq!(reading("Lezing: Kolossensen 1: 21 - 2: 23"), [(51, 1, 21, 29), (51, 2, 1, 23)]);
        assert_eq!(reading("Lezing: 1 Johannes 4: 1-10"), [(62, 4, 1, 10)]);
        assert_eq!(reading("Lezing: Psalm 116"), [(19, 116, 1, 999)]);
        assert_eq!(reading("Psalm 116 vers 1 t/m 9"), [(19, 116, 1, 9)]);
        assert_eq!(reading("Lezing: Hooglied 1 vers 2a"), [(22, 1, 2, 2)]);
        assert_eq!(reading("Matth. 5:1-12"), [(40, 5, 1, 12)]);
        assert_eq!(reading("Openb. 21"), [(66, 21, 1, 999)]);
        assert_eq!(reading("hand 2:1-13"), [(44, 2, 1, 13)]);
        assert_eq!(reading("2 Kor 5:17"), [(47, 5, 17, 17)]);
        // Typed: commas between verses stay with the reading.
        assert_eq!(reading("joh 3: 16, 18"), [(43, 3, 16, 16), (43, 3, 18, 18)]);
    }

    #[test]
    fn catechism_and_confessions() {
        let hc = one("HC 57");
        assert_eq!((hc.soort.as_str(), hc.vragen.clone()), ("catechismus", vec![57]));
        assert_eq!(one("HC 57, 58").vragen, vec![57, 58]);
        assert_eq!(one("Catechismus vraag en antwoord 58").vragen, vec![58]);
        assert_eq!(one("vraag en antwoord 57").vragen, vec![57]);
        let z = one("zondag 22");
        assert_eq!((z.soort.as_str(), z.zondag, z.vragen.len()), ("catechismus", 22, 0));
        assert_eq!(one("Heidelbergse Catechismus zondag 22").zondag, 22);
        let ngb = one("NGB art. 14");
        assert_eq!((ngb.soort.as_str(), ngb.document.as_str(), ngb.artikelen.clone()), ("belijdenis", "ngb", vec![14]));
        assert_eq!(one("Nederlandse Geloofsbelijdenis artikel 14-16").artikelen, vec![14, 15, 16]);
        let dl = one("DL I, 1-3");
        assert_eq!((dl.document.as_str(), dl.hoofdstuk, dl.artikelen.clone()), ("dl", 1, vec![1, 2, 3]));
        let dl = one("Dordtse Leerregels hoofdstuk 2 artikel 3");
        assert_eq!((dl.hoofdstuk, dl.artikelen.clone()), (2, vec![3]));
        assert_eq!(one("DL III/IV, 1").hoofdstuk, 3);
        assert_eq!(one("Apostolische Geloofsbelijdenis").document, "apostolicum");
        assert_eq!(one("Geloofsbelijdenis van Nicea").document, "nicea");
        let g = one("geloofsbelijdenis");
        assert_eq!((g.document.as_str(), g.herkend), ("apostolicum", false));
        for (text, doc) in [
            ("Doopformulier", "doop-kinderen"),
            ("Bediening van de Heilige Doop", "doop-kinderen"),
            ("Doop volwassenen", "doop-volwassenen"),
            ("Avondmaalsformulier", "heilig-avondmaal"),
            ("Openbare geloofsbelijdenis", "belijdenis"),
            ("Bevestiging ouderlingen en diakenen", "bevestiging-ouderling-diaken"),
            ("Bevestiging predikant", "bevestiging-predikant"),
            ("Huwelijksbevestiging", "huwelijk"),
        ] {
            let f = one(text);
            assert_eq!((f.soort.as_str(), f.document.as_str()), ("formulier", doc), "{text}");
        }
    }

    #[test]
    fn simple_parts_and_songs() {
        let kinds: Vec<(String, String)> = reader()
            .typed("gebed, Dankgebed, collecte, preek, prediking, zegen, welkom, einde, votum, mededelingen, levensloop")
            .into_iter()
            .map(|i| (i.soort, i.opschrift))
            .collect();
        let want = [
            ("gebed", "Gebed"),
            ("gebed", "Dankgebed"),
            ("collecte", "Collecte"),
            ("preek", "Prediking"),
            ("preek", "Prediking"),
            ("zegen", "Zegen"),
            ("welkom", "Startscherm"),
            ("einde", "Aftiteling"),
            ("mededeling", "Votum"),
            ("mededeling", "Mededelingen"),
            ("mededeling", "Levensloop"),
        ];
        assert_eq!(kinds, want.map(|(a, b)| (a.to_string(), b.to_string())));
        let m = one("mededeling: Na de dienst koffie");
        assert_eq!((m.soort.as_str(), m.tekst.as_str()), ("mededeling", "Na de dienst koffie"));
        // A song book by its number.
        let l = one("LB 23:1-3");
        assert_eq!((l.soort.as_str(), l.opschrift.as_str(), l.herkend), ("lied", "LB 23 : 1, 3", true));
        assert_eq!(one("opwekking 733").opschrift, "Opw 733");
        let l = one("lied: Hoe groot zijt Gij");
        assert_eq!((l.soort.as_str(), l.opschrift.as_str()), ("lied", "Hoe groot zijt Gij"));
        // On the website a line among the psalms is a song.
        let l = reader().part("Een vaste burcht is onze God", Ctx::Songs).remove(0);
        assert_eq!((l.soort.as_str(), l.opschrift.as_str(), l.herkend), ("lied", "Een vaste burcht is onze God", true));
        let w = one("wet");
        assert_eq!((w.soort.as_str(), w.boek, w.hoofdstuk, w.van, w.tot), ("lezing", 2, 20, 1, 17));
    }

    #[test]
    fn nothing_is_dropped() {
        let items = reader().typed("ps 42:5, Een vaste burcht is onze God, gebed, xyz 12, joh 3:16");
        assert_eq!(items.len(), 5);
        let unknown: Vec<&RawItem> = items.iter().filter(|i| i.niet_begrepen).collect();
        assert_eq!(unknown.len(), 2, "{items:?}");
        assert!(unknown.iter().all(|i| !i.herkend && i.reden.contains("niet herkend")));
        assert_eq!(unknown[0].opschrift, "Een vaste burcht is onze God");
        assert_eq!(unknown[1].bron, "xyz 12");
        // The check of the numbers stays: psalm 42 has 7 verses.
        let item = one("ps 42: 12").into_item();
        assert!(item.problem.contains("7 verzen"), "{}", item.problem);
    }

    #[test]
    fn sermon_texts() {
        let r = reader();
        let t = r.sermon_text("1 Samu\u{eb}l 13 vers 11");
        assert_eq!((t.soort.as_str(), t.boek, t.hoofdstuk, t.van, t.tot, t.herkend), ("lezing", 9, 13, 11, 11, true));
        let c = r.sermon_text("Catechismus vraag en antwoord 58");
        assert_eq!((c.soort.as_str(), c.vragen.clone()), ("catechismus", vec![58]));
        let z = r.sermon_text("Zondag 4 Heidelbergse Catechismus");
        assert_eq!((z.soort.as_str(), z.zondag), ("catechismus", 4));
        // Several parts: one passage from the first to the last verse, to check.
        let m = r.sermon_text("Handelingen 27 vers 10, 21 t/m 26 en 31");
        assert_eq!((m.boek, m.hoofdstuk, m.van, m.tot, m.herkend), (44, 27, 10, 31, false));
        let m = r.sermon_text("Johannes 8 m.n. vers 31-32 en 36");
        assert_eq!((m.boek, m.hoofdstuk, m.van, m.tot), (43, 8, 31, 36));
        // Typed in the sermon field.
        let t = one("Tekst: Psalm 116 vers 7 t/m 9");
        assert_eq!((t.soort.as_str(), t.boek, t.van, t.tot), ("lezing", 19, 7, 9));
    }

    #[test]
    fn typed_parts_are_split_at_commas_but_not_inside_references() {
        let items = reader().typed("ps 42:5, 6, gebed\njoh 3:16-21; HC 57, collecte");
        let kinds: Vec<&str> = items.iter().map(|i| i.soort.as_str()).collect();
        assert_eq!(kinds, ["psalm", "gebed", "lezing", "catechismus", "collecte"]);
        assert_eq!(items[0].verzen, vec![5, 6]);
        assert_eq!(and_list(&["3".into(), "7".into()]), "3 en 7");
    }
}
