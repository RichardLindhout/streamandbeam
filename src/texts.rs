//! The texts, built into the program once and checked when they were made (see tools/):
//! the psalms in the 1773 rhyming as the church sings them, the Statenvertaling and the
//! Heidelberg Catechism. Nothing is fetched during a service.

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::LazyLock;

#[derive(Debug, Clone, Deserialize)]
pub struct Psalm {
    /// "Psalm 42", or the name of one of the gezangen ("De Tien Geboden").
    pub naam: String,
    /// Verses as lines; verzen[0] is verse 1.
    pub verzen: Vec<Vec<String>>,
}

/// Psalms 1-150 and the "enige gezangen" as 151-160.
static PSALMS: LazyLock<BTreeMap<u32, Psalm>> = LazyLock::new(|| {
    let raw: BTreeMap<String, Psalm> = serde_json::from_str(include_str!("../assets/data/psalmen.json")).expect("psalmen.json");
    raw.into_iter().map(|(k, v)| (k.parse().unwrap(), v)).collect()
});

#[derive(Debug, Deserialize)]
struct Book {
    hoofdstukken: Vec<Vec<String>>,
}

static BIBLE: LazyLock<Vec<Book>> =
    LazyLock::new(|| serde_json::from_str(include_str!("../assets/data/statenvertaling.json")).expect("statenvertaling.json"));

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Verse {
    #[serde(rename = "verse")]
    pub number: u32,
    pub text: String,
}

pub fn psalm(n: u32) -> Option<&'static Psalm> {
    PSALMS.get(&n)
}

/// The names of the gezangen (151-160), for Claude to map "Tien Geboden" etc.
pub fn gezangen() -> Vec<(u32, String)> {
    PSALMS.range(151..).map(|(n, p)| (*n, p.naam.clone())).collect()
}

pub fn psalm_verse(n: u32, v: u32) -> Result<&'static [String]> {
    let Some(p) = PSALMS.get(&n) else { bail!("psalm {n} bestaat niet") };
    match p.verzen.get((v as usize).wrapping_sub(1)) {
        Some(lines) => Ok(lines),
        None => bail!("{} heeft geen vers {v}", p.naam),
    }
}

/// Verses from..=to of a chapter.
pub fn passage(r: crate::books::BibleRef) -> Result<Vec<Verse>> {
    let chapter = BIBLE
        .get((r.book as usize).wrapping_sub(1))
        .and_then(|b| b.hoofdstukken.get((r.chapter as usize).wrapping_sub(1)));
    let Some(chapter) = chapter else { bail!("{r} bestaat niet") };
    let verses: Vec<Verse> = chapter
        .iter()
        .enumerate()
        .map(|(i, t)| Verse { number: i as u32 + 1, text: t.clone() })
        .filter(|v| v.number >= r.from && v.number <= r.to)
        .collect();
    if verses.is_empty() {
        bail!("{r} bestaat niet");
    }
    Ok(verses)
}

pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    Ok(serde_json::from_slice(&std::fs::read(path)?)?)
}

pub fn write_json<T: Serialize>(path: &Path, v: &T) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, serde_json::to_vec_pretty(v)?)?;
    Ok(())
}
