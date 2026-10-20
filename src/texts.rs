//! Psalm and Bible texts, fetched once from the internet and cached in the data folder,
//! so a service never depends on the church's internet connection.

use crate::extract::{http_get, strip_tags};
use anyhow::{Result, bail};
use regex::Regex;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

pub struct Texts {
    pub data_dir: PathBuf,
    pub modernize: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Psalm {
    pub number: u32,
    /// Verses as lines; verses[0] is verse 1.
    pub verses: Vec<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Verse {
    #[serde(rename = "verse")]
    pub number: u32,
    pub text: String,
}

/// DBNL "Het boek der psalmen" (1774, copyright free): psalm N is page 12+N.
fn psalm_url(n: u32) -> String {
    format!("https://www.dbnl.org/tekst/_boe008boek01_01/_boe008boek01_01_{:04}.php", 12 + n)
}

static VERSE_HEAD_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<h4>&sect;(\d+)</h4>").unwrap());
static LINE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"<div class="line-content">(.*?)</div>"#).unwrap());

impl Texts {
    pub async fn psalm(&self, n: u32) -> Result<Psalm> {
        let path = self.data_dir.join("psalmen").join(format!("{n:03}.json"));
        if let Ok(p) = read_json(&path) {
            return Ok(p);
        }
        if !(1..=150).contains(&n) {
            bail!("psalm {n} bestaat niet");
        }
        let page = http_get(&psalm_url(n)).await?;
        let heads: Vec<_> = VERSE_HEAD_RE.find_iter(&page).collect();
        if heads.is_empty() {
            bail!("geen verzen gevonden voor psalm {n}");
        }
        let mut psalm = Psalm { number: n, verses: Vec::new() };
        for (i, head) in heads.iter().enumerate() {
            let end = match heads.get(i + 1) {
                Some(next) => next.start(),
                None => page[head.end()..].find(r#"id="right""#).map_or(page.len(), |j| head.end() + j),
            };
            let lines = LINE_RE
                .captures_iter(&page[head.end()..end])
                .map(|m| strip_tags(&m[1]).trim().to_string())
                .filter(|l| !l.is_empty())
                .collect();
            psalm.verses.push(lines);
        }
        write_json(&self.data_dir.join("psalmen").join(format!("{n:03}-1774.json")), &psalm)?;
        if self.modernize {
            // Without modern spelling the psalm is still usable: show the 1774 text and
            // try modernising again next time.
            if let Err(e) = modernize_spelling(&mut psalm).await {
                eprintln!("psalm {n} blijft in oude spelling: {e:#}");
                return Ok(psalm);
            }
        }
        write_json(&path, &psalm)?;
        Ok(psalm)
    }

    pub async fn chapter(&self, book: u32, chapter: u32) -> Result<Vec<Verse>> {
        let path = self.data_dir.join("bijbel").join(format!("{book:02}-{chapter:03}.json"));
        if let Ok(v) = read_json(&path) {
            return Ok(v);
        }
        #[derive(Deserialize)]
        struct Resp {
            verses: Vec<Verse>,
        }
        let body = http_get(&format!("https://api.getbible.net/v2/statenvertaling/{book}/{chapter}.json")).await?;
        let mut verses = serde_json::from_str::<Resp>(&body)?.verses;
        for v in &mut verses {
            v.text = v.text.trim().to_string();
        }
        write_json(&path, &verses)?;
        Ok(verses)
    }

    /// Verses from..=to of a chapter.
    pub async fn passage(&self, r: crate::books::BibleRef) -> Result<Vec<Verse>> {
        let all = self.chapter(r.book, r.chapter).await?;
        Ok(all.into_iter().filter(|v| v.number >= r.from && v.number <= r.to).collect())
    }
}

/// Asks Claude to bring the 1774 orthography ("vreezen", "draagen") to the spelling used
/// in today's psalm books, without changing any words.
async fn modernize_spelling(p: &mut Psalm) -> Result<()> {
    let schema = json!({
        "type": "object", "additionalProperties": false, "required": ["verses"],
        "properties": {"verses": {"type": "array", "items": {"type": "array", "items": {"type": "string"}}}}
    });
    let prompt = format!(
        r#"This is Psalm {} from the Dutch 1773 psalm rhyming (berijming), in the original 1774 spelling, as a JSON array of verses, each an array of lines.
Rewrite it in the spelling used in current printings of the 1773 berijming (e.g. "vreezen" -> "vrezen", "draagen" -> "dragen", "zig" -> "zich"). Change spelling only: keep every word, the word order, the punctuation, the capitals and exactly the same number of verses and lines.

{}"#,
        p.number,
        serde_json::to_string(&p.verses)?
    );
    #[derive(Deserialize)]
    struct Out {
        verses: Vec<Vec<String>>,
    }
    let out: Out = crate::claude::ask_json(&prompt, schema).await?;
    if out.verses.len() != p.verses.len() {
        bail!("aantal verzen klopt niet ({} i.p.v. {})", out.verses.len(), p.verses.len());
    }
    for (i, (a, b)) in out.verses.iter().zip(&p.verses).enumerate() {
        if a.len() != b.len() {
            bail!("aantal regels in vers {} klopt niet", i + 1);
        }
    }
    p.verses = out.verses;
    Ok(())
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
