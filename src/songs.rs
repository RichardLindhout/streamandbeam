//! Songs that are not in the psalm book: a "lied" or a song after the service ("nazang") with
//! a title, as "Een vaste burcht is onze God" on the website or the songs of the special days.
//! Built in (assets/data/liederen.json, made by tools/bouw_liederen.mjs) and the operator's own
//! in <data>/liederen.json, which go before a built-in song with the same title. A built-in song
//! has its words only when they are certainly free (poet and translator died before 1956) and
//! the same in two sources; otherwise only its title and other names, and the operator fills in
//! the words (the panel says "tekst nog invullen").
//!
//! Also the song of each special day, as in the church's Excel (tab Input, J2:K8): Pasen
//! "U zij de Glorie", Kerst "Ere zij God", … (see `crate::roosters::Roosters::feast_song`).

use crate::parse::{compact, sung_verse_list};
use crate::show::Item;
use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::LazyLock;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Song {
    pub titel: String,
    /// Other names it goes by ("Wilhelmus van Nassouwe", "Ein feste Burg").
    pub aliassen: Vec<String>,
    /// Verses as lines; verzen[0] is verse 1. No verses: the words are not in the program (yet);
    /// an empty verse: that one is missing.
    pub verzen: Vec<Vec<String>>,
    /// Where the words come from.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub bron: String,
    /// The song book it is in ("Liedboek", see BUNDELS) and its number there; "" and 0 when
    /// it is in none. Then "LB 23" finds it too.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub bundel: String,
    #[serde(skip_serializing_if = "is_zero")]
    pub nummer: u32,
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

/// A song book. The program knows only its name and how it is written short: the words of
/// these books are protected, so the church fills in what it sings, under its own licence,
/// and they stay on its own computer (<data>/liederen.json).
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Bundel {
    pub naam: &'static str,
    /// As on the boards: "LB 23".
    pub kort: &'static str,
    /// Other ways it is written, folded (see crate::parse::compact).
    #[serde(skip)]
    afkortingen: &'static [&'static str],
}

pub const BUNDELS: [Bundel; 8] = [
    Bundel { naam: "Liedboek", kort: "LB", afkortingen: &["lb", "liedboek", "lvk"] },
    Bundel { naam: "Weerklank", kort: "WK", afkortingen: &["wk", "weerklank"] },
    Bundel { naam: "Op Toonhoogte", kort: "OTH", afkortingen: &["oth", "optoonhoogte"] },
    Bundel { naam: "Opwekking", kort: "Opw", afkortingen: &["opw", "opwekking"] },
    Bundel { naam: "Johannes de Heer", kort: "JdH", afkortingen: &["jdh", "johannesdeheer", "johdeheer"] },
    Bundel { naam: "Hemelhoog", kort: "HH", afkortingen: &["hh", "hemelhoog"] },
    Bundel { naam: "Psalmen voor Nu", kort: "PvN", afkortingen: &["pvn", "psalmenvoornu"] },
    Bundel { naam: "Evangelische Liedbundel", kort: "ELB", afkortingen: &["elb", "evangelischeliedbundel"] },
];

/// The song book with this name or short name ("LB", "Liedboek", "Op Toonhoogte").
pub fn bundel(name: &str) -> Option<&'static Bundel> {
    let key = compact(name);
    (!key.is_empty()).then_some(())?;
    BUNDELS.iter().find(|b| compact(b.naam) == key || compact(b.kort) == key || b.afkortingen.contains(&key.as_str()))
}

static BOOK_REF: LazyLock<Regex> = LazyLock::new(|| {
    let mut names: Vec<String> = BUNDELS.iter().flat_map(|b| [b.naam.to_string(), b.kort.to_string()]).collect();
    names.extend(["Op Toonhoogte", "Joh. de Heer", "Johannes de Heer", "Lvk"].map(String::from));
    names.sort_by_key(|n| std::cmp::Reverse(n.len()));
    let alt = names.iter().map(|n| regex::escape(n).replace(' ', r"\s*")).collect::<Vec<_>>().join("|");
    Regex::new(&format!(r"(?i)^({alt})\.?\s*(\d{{1,4}})\b\s*(?:(?::|,|\bvers\b|\bverzen\b|\bvs\b\.?)\s*(.*))?$")).unwrap()
});

/// Typed or on the website, "LB 23:1, 3", "Opw 733" or "Weerklank 412 vers 2": the song as the
/// program writes it ("LB 23 : 1, 3"). None when it is not a song book with a number.
pub fn book_ref(text: &str) -> Option<String> {
    let m = BOOK_REF.captures(text.trim())?;
    let b = bundel(&m[1])?;
    let verses = m.get(3).map(|v| sung_verse_list(v.as_str())).unwrap_or_default();
    if m.get(3).is_some_and(|v| !v.as_str().trim().is_empty()) && verses.is_empty() {
        return None;
    }
    let mut out = format!("{} {}", b.kort, &m[2]);
    if !verses.is_empty() {
        out.push_str(&format!(" : {}", verses.iter().map(u32::to_string).collect::<Vec<_>>().join(", ")));
    }
    Some(out)
}

impl Song {
    /// Whether the program has words of this song.
    pub fn has_words(&self) -> bool {
        self.verzen.iter().any(|v| v.iter().any(|l| !l.trim().is_empty()))
    }

    /// Its title, other names, and its place in its song book in every way that is written.
    fn names(&self) -> Vec<String> {
        let mut out: Vec<String> = std::iter::once(&self.titel).chain(self.aliassen.iter()).cloned().collect();
        if let Some(b) = bundel(&self.bundel).filter(|_| self.nummer > 0) {
            out.extend([b.naam, b.kort].into_iter().chain(b.afkortingen.iter().copied()).map(|n| format!("{n} {}", self.nummer)));
        }
        out
    }

    /// How it is called on the screens: "LB 23" for a song from a song book, else its title.
    pub fn display(&self) -> String {
        match bundel(&self.bundel).filter(|_| self.nummer > 0) {
            Some(b) => format!("{} {}", b.kort, self.nummer),
            None => self.titel.clone(),
        }
    }

    /// Only the parts that are filled in: no empty names, lines or trailing empty verses.
    fn cleaned(mut self) -> Song {
        self.titel = self.titel.trim().to_string();
        // The song book by its own name; a song without a title is called by its place in it.
        self.bundel = bundel(&self.bundel).map(|b| b.naam.to_string()).unwrap_or_default();
        if self.bundel.is_empty() {
            self.nummer = 0;
        }
        if self.titel.is_empty() && self.nummer > 0 {
            self.titel = self.display();
        }
        self.aliassen = self.aliassen.iter().map(|a| a.trim().to_string()).filter(|a| !a.is_empty()).collect();
        self.verzen = self.verzen.into_iter().map(|v| v.into_iter().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect()).collect();
        while self.verzen.last().is_some_and(Vec::is_empty) {
            self.verzen.pop();
        }
        self
    }
}

/// The song of a special day, as the church's Excel has it: "Pasen" → "U zij de Glorie".
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FeestLied {
    pub dag: String,
    pub lied: String,
}

#[derive(Deserialize)]
struct BuiltIn {
    liederen: Vec<Song>,
    feestdagen: Vec<FeestLied>,
}

static BUILTIN: LazyLock<BuiltIn> = LazyLock::new(|| serde_json::from_str(include_str!("../assets/data/liederen.json")).expect("liederen.json"));

pub fn builtin() -> &'static [Song] {
    &BUILTIN.liederen
}

/// The songs of the special days from the Excel (tab Input, J2:K8), as built in.
pub fn builtin_feast_songs() -> &'static [FeestLied] {
    &BUILTIN.feestdagen
}

/// Whether `day` (a special day from the schedules: "1e Paasdag", "Eerste Kerstdag",
/// "Hemelvaartsdag") is the day the Excel calls `name` ("Pasen", "Kerst", "Hemelvaart").
pub fn same_day(day: &str, name: &str) -> bool {
    let (day, name) = (compact(day), compact(name));
    if day.is_empty() || name.is_empty() {
        return false;
    }
    // The Excel's names, and the stem in the names of the days themselves.
    let stem = match name.as_str() {
        "pasen" => "paas",
        "pinksteren" => "pinkster",
        "kerst" | "kerstmis" | "kerstfeest" => "kerst",
        "hemelvaart" => "hemelvaart",
        _ => name.as_str(),
    };
    day.contains(stem) || day == name
}

/// The operator's songs and the built-in ones.
#[derive(Debug, Clone, Default)]
pub struct Library {
    own: Vec<Song>,
}

/// One song as the panel lists it: built in or the operator's own (also when it replaces a
/// built-in one), and whether the program has its words.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Listed {
    #[serde(flatten)]
    pub song: Song,
    pub eigen: bool,
    pub ingebouwd: bool,
    pub tekst: bool,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct OwnFile {
    liederen: Vec<Song>,
}

impl Library {
    /// The songs with the operator's own from `dir` (<data>/liederen.json).
    pub fn load(dir: &Path) -> Library {
        let path = dir.join("liederen.json");
        if !path.exists() {
            return Library::default();
        }
        match crate::texts::read_json::<OwnFile>(&path) {
            Ok(f) => Library::with_own(f.liederen),
            Err(e) => {
                eprintln!("{}: {e:#}; alleen de ingebouwde liederen", path.display());
                Library::default()
            }
        }
    }

    /// With these songs of the operator's own (songs without a title are left out).
    pub fn with_own(own: Vec<Song>) -> Library {
        Library { own: own.into_iter().map(Song::cleaned).filter(|s| !s.titel.is_empty()).collect() }
    }

    pub fn own(&self) -> &[Song] {
        &self.own
    }

    pub fn save(&self, dir: &Path) -> Result<()> {
        crate::texts::write_json(&dir.join("liederen.json"), &OwnFile { liederen: self.own.clone() })
    }

    /// Every song: the operator's own, then the built-in ones they do not replace, by title.
    pub fn all(&self) -> Vec<Listed> {
        let replaced = |b: &Song| self.own.iter().any(|o| compact(&o.titel) == compact(&b.titel));
        let mut out: Vec<Listed> = self
            .own
            .iter()
            .map(|s| Listed { song: s.clone(), eigen: true, ingebouwd: builtin().iter().any(|b| compact(&b.titel) == compact(&s.titel)), tekst: s.has_words() })
            .collect();
        out.extend(builtin().iter().filter(|b| !replaced(b)).map(|s| Listed { song: s.clone(), eigen: false, ingebouwd: true, tekst: s.has_words() }));
        out.sort_by_key(|l| l.song.titel.to_lowercase());
        out
    }

    /// The song with this title or other name: the operator's own first.
    pub fn find(&self, title: &str) -> Option<&Song> {
        let key = compact(title);
        if key.is_empty() {
            return None;
        }
        let by_name = |s: &&Song| s.names().iter().any(|n| compact(n) == key);
        self.own.iter().find(by_name).or_else(|| builtin().iter().find(by_name))
    }

    /// The song of a part of the service that is not a psalm ("lied", or "nazang" without a
    /// psalm), with the verses to sing: the part's own verses, else those after its title
    /// ("Wilhelmus : 1, 6"; a dash is "and", as in the psalms), else all.
    pub fn resolve(&self, it: &Item) -> Option<(&Song, Vec<u32>)> {
        if !(it.kind == "lied" || (it.kind == "nazang" && it.psalm == 0)) {
            return None;
        }
        let title = it.song_title();
        let (song, written) = match self.find(&title) {
            Some(s) => (s, Vec::new()),
            None => {
                let (title, verses) = split_verses(&title)?;
                (self.find(&title)?, verses)
            }
        };
        let verses = if !it.verses.is_empty() {
            it.verses.clone()
        } else if !written.is_empty() {
            written
        } else {
            (1..=song.verzen.len() as u32).collect()
        };
        Some((song, verses))
    }
}

static VERSES_AFTER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(.*?)\s*(?::|,|\b(?:vers|verzen|vs|couplet|coupletten|cpl)\b\.?)\s*((?:\d+\s*(?:,|;|-|–|&|\ben\b|t/m|tm)?\s*)+)$").unwrap()
});

/// "Wilhelmus : 1, 6" or "Wilhelmus vers 1 en 6" → ("Wilhelmus", [1, 6]).
fn split_verses(text: &str) -> Option<(String, Vec<u32>)> {
    let m = VERSES_AFTER.captures(text.trim())?;
    let verses = sung_verse_list(&m[2]);
    (!verses.is_empty() && !m[1].trim().is_empty()).then(|| (m[1].trim().to_string(), verses))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lied(title: &str) -> Item {
        Item { kind: "lied".into(), label: title.into(), ..Default::default() }
    }

    #[test]
    fn built_in_songs_and_their_words() {
        let lib = Library::default();
        let w = lib.find("Het Wilhelmus").unwrap();
        assert_eq!(w.titel, "Wilhelmus");
        assert_eq!(w.verzen.len(), 15);
        assert_eq!(w.verzen[0][0], "Wilhelmus van Nassouwe");
        // Couplet 6, sung after the first, is there; couplets the sources do not agree on are empty.
        assert_eq!(w.verzen[5][0], "Mijn schild ende betrouwen");
        assert!(w.verzen[3].is_empty());
        // Not certainly free: title only.
        for t in ["Een vaste burcht is onze God", "Lam Gods", "Kroon Hem met gouden kroon", "U zij de Glorie", "Heer ik hoor van rijke zegen", "Ere zij God"] {
            let s = lib.find(t).unwrap_or_else(|| panic!("{t}"));
            assert!(!s.has_words(), "{t}");
        }
        assert!(lib.find("Hoe groot zijt Gij").is_none());
    }

    #[test]
    fn every_feast_song_is_in_the_library() {
        let lib = Library::default();
        let days: Vec<&str> = builtin_feast_songs().iter().map(|f| f.dag.as_str()).collect();
        assert_eq!(days, ["Goede vrijdag", "Hemelvaart", "Pasen", "Pinksteren", "Kerst", "Koningsdag"]);
        for f in builtin_feast_songs() {
            assert!(lib.find(&f.lied).is_some(), "{}", f.lied);
        }
    }

    #[test]
    fn special_days_by_their_names_in_the_excel() {
        assert!(same_day("1e Paasdag", "Pasen"));
        assert!(same_day("2e Paasdag", "Pasen"));
        assert!(same_day("Eerste Kerstdag", "Kerst"));
        assert!(same_day("Tweede Kerstdag", "Kerst"));
        assert!(same_day("Hemelvaartsdag", "Hemelvaart"));
        assert!(same_day("1e Pinksterdag", "Pinksteren"));
        assert!(same_day("Goede Vrijdag", "Goede vrijdag"));
        assert!(!same_day("Heilig Avondmaal", "Pasen"));
        assert!(!same_day("", "Pasen"));
    }

    #[test]
    fn own_songs_go_before_the_built_in_ones() {
        let own = Song { titel: " Ere zij God ".into(), aliassen: vec!["Gloria".into(), " ".into()], verzen: vec![vec!["Ere zij God".into(), "".into()], vec![]], ..Default::default() };
        let new = Song { titel: "Hoe groot zijt Gij".into(), ..Default::default() };
        let lib = Library::with_own(vec![own, new, Song::default()]);
        assert_eq!(lib.own().len(), 2);
        let e = lib.find("gloria").unwrap();
        assert_eq!((e.titel.as_str(), e.verzen.clone()), ("Ere zij God", vec![vec!["Ere zij God".to_string()]]));
        let all = lib.all();
        assert_eq!(all.iter().filter(|l| compact(&l.song.titel) == "erezijgod").count(), 1);
        let ere = all.iter().find(|l| l.song.titel == "Ere zij God").unwrap();
        assert!(ere.eigen && ere.ingebouwd && ere.tekst);
        let hoe = all.iter().find(|l| l.song.titel == "Hoe groot zijt Gij").unwrap();
        assert!(hoe.eigen && !hoe.ingebouwd && !hoe.tekst);
        assert!(all.iter().any(|l| l.song.titel == "Wilhelmus" && !l.eigen));
    }

    #[test]
    fn saved_and_loaded_from_the_data_folder() {
        let dir = std::env::temp_dir().join(format!("sb-liederen-{}", std::process::id()));
        let lib = Library::with_own(vec![Song { titel: "Hoe groot zijt Gij".into(), verzen: vec![vec!["O Heer, mijn God".into()]], ..Default::default() }]);
        lib.save(&dir).unwrap();
        assert_eq!(Library::load(&dir).own(), lib.own());
        let _ = std::fs::remove_dir_all(&dir);
        assert!(Library::load(&dir).own().is_empty());
    }

    #[test]
    fn song_books_as_they_are_written() {
        assert_eq!(book_ref("LB 23:1, 3").as_deref(), Some("LB 23 : 1, 3"));
        assert_eq!(book_ref("lb23").as_deref(), Some("LB 23"));
        assert_eq!(book_ref("Liedboek 23 vers 1-3").as_deref(), Some("LB 23 : 1, 3"));
        assert_eq!(book_ref("Opw. 733").as_deref(), Some("Opw 733"));
        assert_eq!(book_ref("Op Toonhoogte 12: 2").as_deref(), Some("OTH 12 : 2"));
        assert_eq!(book_ref("Joh. de Heer 81").as_deref(), Some("JdH 81"));
        assert_eq!(book_ref("Weerklank 412 vers 2 t/m 4").as_deref(), Some("WK 412 : 2, 3, 4"));
        // Not a song book with a number.
        for t in ["LB", "Liedboek", "Ps 42:5", "Lied 23", "Johannes 3:16", "LB 23: en dan"] {
            assert_eq!(book_ref(t), None, "{t}");
        }
    }

    #[test]
    fn a_song_from_a_song_book_is_found_by_its_number() {
        let lb = Song {
            titel: "Heer, U bent mijn leven".into(),
            bundel: "lb".into(),
            nummer: 23,
            verzen: vec![vec!["Heer, U bent mijn leven,".into()], vec!["Ik weet dat U daar bent,".into()]],
            ..Default::default()
        };
        let untitled = Song { bundel: "Opwekking".into(), nummer: 733, ..Default::default() };
        let lib = Library::with_own(vec![lb, untitled, Song { bundel: "Onbekend".into(), nummer: 5, titel: "Los".into(), ..Default::default() }]);
        let s = lib.find("Liedboek 23").unwrap();
        assert_eq!((s.bundel.as_str(), s.display().as_str()), ("Liedboek", "LB 23"));
        assert_eq!(lib.find("lb23").unwrap().titel, "Heer, U bent mijn leven");
        assert_eq!(lib.find("Heer, U bent mijn leven").unwrap().nummer, 23);
        // Without a title it is called by its place in the book.
        assert_eq!(lib.find("Opw 733").unwrap().titel, "Opw 733");
        // An unknown book is no book.
        let los = lib.find("Los").unwrap();
        assert_eq!((los.bundel.as_str(), los.nummer, los.display().as_str()), ("", 0, "Los"));
        // As the program writes a typed "LB 23:2".
        let (s, v) = lib.resolve(&lied(&book_ref("LB 23:2").unwrap())).unwrap();
        assert_eq!((s.titel.as_str(), v), ("Heer, U bent mijn leven", vec![2]));
        // A number the church has not filled in.
        assert!(lib.resolve(&lied("LB 24 : 1")).is_none());
    }

    #[test]
    fn a_part_finds_its_song_and_verses() {
        let lib = Library::default();
        let (s, v) = lib.resolve(&lied("Wilhelmus")).unwrap();
        assert_eq!((s.titel.as_str(), v.len()), ("Wilhelmus", 15));
        let (_, v) = lib.resolve(&lied("Wilhelmus : 1, 6")).unwrap();
        assert_eq!(v, [1, 6]);
        // As in the psalms a dash is "and", not a range.
        let (_, v) = lib.resolve(&lied("Wilhelmus vers 1-6")).unwrap();
        assert_eq!(v, [1, 6]);
        let (_, v) = lib.resolve(&Item { verses: vec![6], ..lied("Wilhelmus") }).unwrap();
        assert_eq!(v, [6]);
        // A song after the service has its title in its text.
        let na = Item { kind: "nazang".into(), label: "Na de dienst".into(), text: "U zij de glorie".into(), ..Default::default() };
        assert_eq!(lib.resolve(&na).unwrap().0.titel, "U zij de glorie");
        assert!(lib.resolve(&Item { psalm: 134, ..na }).is_none());
        assert!(lib.resolve(&lied("Hoe groot zijt Gij")).is_none());
        assert!(lib.resolve(&Item { kind: "psalm".into(), label: "Wilhelmus".into(), ..Default::default() }).is_none());
    }
}
