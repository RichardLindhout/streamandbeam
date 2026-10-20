//! Dutch Bible book names and how references are written.

use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

/// One book of the Bible, from assets/data/bijbelboeken.json.
#[derive(Debug, Deserialize)]
pub struct Book {
    pub nummer: u32,
    /// Full name, e.g. "1 Korinthe".
    pub naam: String,
    /// As on the psalm board, e.g. "1 Kor." or "Openb.".
    pub afkorting: String,
}

/// The 66 books in canonical order; `nummer` is also the number used in the Bible data.
pub static BOOKS: LazyLock<Vec<Book>> =
    LazyLock::new(|| serde_json::from_str(include_str!("../assets/data/bijbelboeken.json")).expect("bijbelboeken.json"));

fn book(n: u32) -> Option<&'static Book> {
    BOOKS.get((n as usize).wrapping_sub(1))
}

pub fn book_name(n: u32) -> &'static str {
    book(n).map_or("?", |b| b.naam.as_str())
}

impl BibleRef {
    /// As written on the psalm board: "Openb.1:9-26", "Job 3:1".
    pub fn short(&self) -> String {
        let abbr = book(self.book).map_or("?", |b| b.afkorting.as_str());
        let sep = if abbr.ends_with('.') { "" } else { " " };
        if self.from <= 1 && self.to >= 999 {
            format!("{abbr}{sep}{}", self.chapter)
        } else if self.to <= self.from {
            format!("{abbr}{sep}{}:{}", self.chapter, self.from)
        } else {
            format!("{abbr}{sep}{}:{}-{}", self.chapter, self.from, self.to)
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
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

/// "Psalm 42 : 5, 6", or the name of a gezang ("De Tien Geboden : 2").
pub fn psalm_ref_string(n: u32, verses: &[u32]) -> String {
    let name = crate::texts::psalm(n).map_or_else(|| format!("Psalm {n}"), |p| p.naam.clone());
    let vs: Vec<String> = verses.iter().map(u32::to_string).collect();
    format!("{name} : {}", vs.join(", "))
}
