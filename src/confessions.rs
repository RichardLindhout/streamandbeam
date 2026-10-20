//! The confessions and liturgical forms read in services: the Nederlandse
//! Geloofsbelijdenis, the Dordtse Leerregels, the three ecumenical creeds and the classic
//! forms (baptism, Lord's Supper, marriage, …). Built once and checked against a second
//! source (see tools/ and assets/data/BRON.txt).

use rust_embed::Embed;
use serde::Deserialize;
use std::sync::LazyLock;

#[derive(Embed)]
#[folder = "assets/data/"]
struct Data;

fn load<T: for<'de> Deserialize<'de> + Default>(file: &str) -> T {
    Data::get(file).and_then(|f| serde_json::from_slice(&f.data).ok()).unwrap_or_default()
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Confessions {
    pub ngb: Vec<Article>,
    pub dl: Vec<DlChapter>,
    pub apostolicum: Creed,
    pub nicea: Creed,
    pub athanasius: Creed,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Article {
    pub artikel: u32,
    pub titel: String,
    pub tekst: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct DlChapter {
    pub hoofdstuk: u32,
    pub titel: String,
    pub artikelen: Vec<Numbered>,
    pub verwerpingen: Vec<Numbered>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Numbered {
    pub nummer: u32,
    pub tekst: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Creed {
    pub naam: String,
    pub tekst: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Form {
    pub id: String,
    pub naam: String,
    /// Short name as in a liturgy, e.g. "Doopformulier".
    pub kort: String,
    pub delen: Vec<Part>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Part {
    pub kop: String,
    pub tekst: Vec<String>,
}

pub static CONFESSIONS: LazyLock<Confessions> = LazyLock::new(|| load("belijdenissen.json"));
pub static FORMS: LazyLock<Vec<Form>> = LazyLock::new(|| load("formulieren.json"));

/// One readable piece: a heading and its paragraphs.
pub struct Piece {
    pub header: String,
    pub sub: String,
    pub paragraphs: Vec<String>,
}

const ROMAN: [&str; 6] = ["", "I", "II", "III/IV", "III/IV", "V"];

/// The pieces of a confession reading. `doc`: ngb, dl, apostolicum, nicea, athanasius.
/// For the DL, `chapter` is the hoofdstuk; articles empty = the whole document/chapter.
pub fn confession(doc: &str, chapter: u32, articles: &[u32]) -> Vec<Piece> {
    let c = &*CONFESSIONS;
    let wanted = |n: u32| articles.is_empty() || articles.contains(&n);
    match doc {
        "ngb" => c
            .ngb
            .iter()
            .filter(|a| wanted(a.artikel))
            .map(|a| Piece { header: format!("NGB artikel {}", a.artikel), sub: a.titel.clone(), paragraphs: a.tekst.clone() })
            .collect(),
        "dl" => c
            .dl
            .iter()
            .filter(|h| chapter == 0 || h.hoofdstuk == chapter || (chapter == 4 && h.hoofdstuk == 3))
            .flat_map(|h| {
                let roman = ROMAN.get(h.hoofdstuk as usize).copied().unwrap_or("?");
                h.artikelen.iter().filter(|a| wanted(a.nummer)).map(move |a| Piece {
                    header: format!("DL {roman}, {}", a.nummer),
                    sub: h.titel.clone(),
                    paragraphs: a.tekst.clone(),
                })
            })
            .collect(),
        "apostolicum" | "nicea" | "athanasius" => {
            let creed = match doc {
                "apostolicum" => &c.apostolicum,
                "nicea" => &c.nicea,
                _ => &c.athanasius,
            };
            if creed.tekst.is_empty() {
                Vec::new()
            } else {
                vec![Piece { header: creed.naam.clone(), sub: String::new(), paragraphs: creed.tekst.clone() }]
            }
        }
        _ => Vec::new(),
    }
}

pub fn form(id: &str) -> Option<&'static Form> {
    FORMS.iter().find(|f| f.id == id)
}

/// How a confession reading is written in the liturgy / on the board: "NGB art. 14", "DL I, 1-3".
pub fn short(doc: &str, chapter: u32, articles: &[u32]) -> String {
    let arts = match (articles.first(), articles.last()) {
        (Some(a), Some(b)) if a != b && articles.len() == (b - a + 1) as usize => format!("{a}-{b}"),
        _ => articles.iter().map(u32::to_string).collect::<Vec<_>>().join(","),
    };
    match doc {
        "ngb" => format!("NGB art. {arts}"),
        "dl" => {
            let roman = ROMAN.get(chapter as usize).copied().unwrap_or("?");
            if arts.is_empty() { format!("DL {roman}") } else { format!("DL {roman}, {arts}") }
        }
        "apostolicum" => "Apostolische Geloofsbelijdenis".into(),
        "nicea" => "Geloofsbelijdenis van Nicea".into(),
        "athanasius" => "Geloofsbelijdenis van Athanasius".into(),
        _ => String::new(),
    }
}
