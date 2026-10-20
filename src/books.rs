//! Dutch Bible book names and how references are written.

use serde::{Deserialize, Serialize};

/// Statenvertaling book names in canonical order; index+1 is the book number.
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
