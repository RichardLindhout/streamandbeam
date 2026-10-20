//! Dutch Bible book names and parsing of psalm and Bible references.

use regex::Regex;
use std::sync::LazyLock;

/// Statenvertaling book names in canonical order; index+1 is the number used by getbible.net.
pub const BOOK_NAMES: [&str; 66] = [
    "Genesis", "Exodus", "Leviticus", "Numeri", "Deuteronomium", "Jozua", "Richteren", "Ruth",
    "1 Samuël", "2 Samuël", "1 Koningen", "2 Koningen", "1 Kronieken", "2 Kronieken", "Ezra",
    "Nehemia", "Esther", "Job", "Psalmen", "Spreuken", "Prediker", "Hooglied", "Jesaja", "Jeremia",
    "Klaagliederen", "Ezechiël", "Daniël", "Hosea", "Joël", "Amos", "Obadja", "Jona", "Micha",
    "Nahum", "Habakuk", "Sefanja", "Haggaï", "Zacharia", "Maleachi", "Mattheüs", "Markus", "Lukas",
    "Johannes", "Handelingen", "Romeinen", "1 Korinthe", "2 Korinthe", "Galaten", "Efeze",
    "Filippenzen", "Kolossenzen", "1 Thessalonicenzen", "2 Thessalonicenzen", "1 Timotheüs",
    "2 Timotheüs", "Titus", "Filemon", "Hebreeën", "Jakobus", "1 Petrus", "2 Petrus", "1 Johannes",
    "2 Johannes", "3 Johannes", "Judas", "Openbaring",
];

pub fn book_name(n: u32) -> &'static str {
    BOOK_NAMES.get((n as usize).wrapping_sub(1)).copied().unwrap_or("?")
}

/// Lowercase without accents, dots and spaces, so "1 Kor." and "1korinthe" compare.
fn normalize(s: &str) -> String {
    s.trim()
        .to_lowercase()
        .chars()
        .filter(|c| *c != '.' && *c != ' ')
        .map(|c| match c {
            'ë' | 'é' => 'e',
            'ï' => 'i',
            'ü' => 'u',
            c => c,
        })
        .collect()
}

/// Resolves a (possibly abbreviated) Dutch book name to its number.
pub fn find_book(s: &str) -> u32 {
    let key = normalize(s);
    if key.is_empty() {
        return 0;
    }
    let aliases = [
        ("ps", 19), ("psalm", 19), ("mat", 40), ("matt", 40), ("mark", 41), ("mar", 41),
        ("luc", 42), ("lucas", 42), ("joh", 43), ("hand", 44), ("rom", 45), ("ef", 49),
        ("fil", 50), ("kol", 51), ("hebr", 58), ("jak", 59), ("op", 66), ("openb", 66),
        ("pred", 21), ("spr", 20), ("jes", 23), ("jer", 24), ("ez", 26), ("dan", 27), ("gen", 1),
        ("ex", 2), ("lev", 3), ("num", 4), ("deut", 5), ("richt", 7),
    ];
    if let Some((_, n)) = aliases.iter().find(|(a, _)| *a == key) {
        return *n;
    }
    BOOK_NAMES
        .iter()
        .position(|name| normalize(name).starts_with(&key))
        .map_or(0, |i| i as u32 + 1)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BibleRef {
    pub book: u32,
    pub chapter: u32,
    pub from: u32,
    pub to: u32,
}

impl std::fmt::Display for BibleRef {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let name = book_name(self.book);
        if self.from <= 1 && self.to >= 999 {
            write!(f, "{name} {}", self.chapter)
        } else if self.to <= self.from {
            write!(f, "{name} {}:{}", self.chapter, self.from)
        } else {
            write!(f, "{name} {}:{}-{}", self.chapter, self.from, self.to)
        }
    }
}

static REF_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*(.+?)\s+(\d+)(?:\s*(?::|vers)\s*(\d+)(?:\s*(?:-|t/m)\s*(\d+))?)?\s*$").unwrap()
});

/// Parses "Joh. 5: 16-29", "1 Korinthe 6 vers 15 t/m 20" or a whole chapter "Psalm 23".
pub fn parse_bible_ref(s: &str) -> Option<BibleRef> {
    let m = REF_RE.captures(s)?;
    let book = find_book(&m[1]);
    if book == 0 {
        return None;
    }
    let chapter = m[2].parse().ok()?;
    let Some(from) = m.get(3) else {
        return Some(BibleRef { book, chapter, from: 1, to: 999 });
    };
    let from: u32 = from.as_str().parse().ok()?;
    let to = m.get(4).and_then(|t| t.as_str().parse().ok()).unwrap_or(from);
    Some(BibleRef { book, chapter, from, to })
}

static PSALM_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s*(?:ps(?:alm)?\.?\s*)?(\d+)\s*[:\s]\s*([\d\s,\-en]+)$").unwrap()
});

/// Parses "38: 15", "Ps. 86: 5-6" or "73: 12, 14 en 15" into psalm and verses.
pub fn parse_psalm_ref(s: &str) -> Option<(u32, Vec<u32>)> {
    let m = PSALM_RE.captures(s)?;
    let psalm: u32 = m[1].parse().ok()?;
    let mut verses = Vec::new();
    for part in m[2].replace("en", ",").split([',', ' ']).filter(|p| !p.is_empty()) {
        let (a, b) = part.split_once('-').unwrap_or((part, part));
        if let (Ok(from), Ok(to)) = (a.parse::<u32>(), b.parse::<u32>()) {
            verses.extend(from..=to);
        }
    }
    (psalm > 0 && !verses.is_empty()).then_some((psalm, verses))
}

pub fn psalm_ref_string(n: u32, verses: &[u32]) -> String {
    let vs: Vec<String> = verses.iter().map(u32::to_string).collect();
    format!("Psalm {n} : {}", vs.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_refs() {
        assert_eq!(parse_psalm_ref("Ps. 86: 5-6"), Some((86, vec![5, 6])));
        assert_eq!(parse_psalm_ref("42:5"), Some((42, vec![5])));
        assert_eq!(parse_psalm_ref("73: 12, 14 en 15"), Some((73, vec![12, 14, 15])));
        let r = parse_bible_ref("1 Korinthe 6 vers 15 t/m 20").unwrap();
        assert_eq!(r, BibleRef { book: 46, chapter: 6, from: 15, to: 20 });
        assert_eq!(parse_bible_ref("Joh. 5: 16-29").unwrap().to_string(), "Johannes 5:16-29");
        assert_eq!(parse_bible_ref("Psalm 23").unwrap().to_string(), "Psalmen 23");
        assert!(parse_bible_ref("Heidelbergse Catechismus, vraag en antwoord 57").is_none());
    }
}
