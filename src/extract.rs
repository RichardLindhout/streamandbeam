//! Reads the liturgy from the church website with Claude.

use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::LazyLock;

pub const LITURGY_URL: &str = "https://www.hhgtholen.nl/kerkdiensten";

#[derive(Debug, Serialize, Deserialize)]
pub struct Extracted {
    pub services: Vec<ExtractedService>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExtractedService {
    pub date: String,  // 2026-10-18
    pub label: String, // Morgendienst
    pub time: String,  // 09:30
    pub psalms: Vec<ExtractedPsalm>,
    pub readings: Vec<ExtractedRef>,
    pub text: ExtractedText,
    pub theme: String,
    pub points: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExtractedPsalm {
    pub psalm: u32,
    pub verses: Vec<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExtractedRef {
    pub book: u32,
    pub chapter: u32,
    pub from: u32,
    pub to: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExtractedText {
    pub display: String,
    pub book: u32,
    pub chapter: u32,
    pub from: u32,
    pub to: u32,
    pub catechism_question: u32,
}

static TAG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)<(script|style)[^>]*>.*?</(script|style)>|<[^>]+>").unwrap());
static SPACE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[ \t\r\f]+").unwrap());
static NL_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n\s*\n+").unwrap());

pub fn strip_tags(html: &str) -> String {
    html_escape::decode_html_entities(&TAG_RE.replace_all(html, " ")).into_owned()
}

/// GET with a few retries: sites like DBNL refuse some requests when several arrive at once.
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

async fn page_text(url: &str) -> Result<String> {
    let mut t = http_get(url).await?;
    for tag in ["<br>", "<br/>", "<br />", "</p>", "</div>"] {
        t = t.replace(tag, "\n");
    }
    let t = strip_tags(&t);
    let t = NL_RE.replace_all(&SPACE_RE.replace_all(&t, " "), "\n").into_owned();
    // Keep only the liturgy part of the page.
    let start = t.find("Liturgie").unwrap_or(0);
    let end = t[start..].find("Komende diensten").map_or(t.len(), |i| start + i);
    Ok(t[start..end].trim().to_string())
}

fn ref_schema() -> Value {
    json!({
        "type": "object", "additionalProperties": false,
        "required": ["book", "chapter", "from", "to"],
        "properties": {
            "book": {"type": "integer", "description": "Protestant canon book number 1-66 (Genesis=1, Johannes=43, 1 Korinthe=46)"},
            "chapter": {"type": "integer"},
            "from": {"type": "integer", "description": "First verse; 1 when a whole chapter is read"},
            "to": {"type": "integer", "description": "Last verse; 999 when a whole chapter is read"},
        }
    })
}

fn schema() -> Value {
    json!({
        "type": "object", "additionalProperties": false, "required": ["services"],
        "properties": {"services": {"type": "array", "items": {
            "type": "object", "additionalProperties": false,
            "required": ["date", "label", "time", "psalms", "readings", "text", "theme", "points"],
            "properties": {
                "date": {"type": "string", "description": "YYYY-MM-DD"},
                "label": {"type": "string", "description": "e.g. Morgendienst"},
                "time": {"type": "string", "description": "HH:MM"},
                "psalms": {"type": "array", "description": "All sung psalms in the order listed, the voorzang first.", "items": {
                    "type": "object", "additionalProperties": false, "required": ["psalm", "verses"],
                    "properties": {
                        "psalm": {"type": "integer"},
                        "verses": {"type": "array", "items": {"type": "integer"}, "description": "Expand ranges: 5-6 becomes [5,6]"},
                    }
                }},
                "readings": {"type": "array", "items": ref_schema(), "description": "Schriftlezing passages"},
                "text": {
                    "type": "object", "additionalProperties": false,
                    "required": ["display", "book", "chapter", "from", "to", "catechism_question"],
                    "properties": {
                        "display": {"type": "string", "description": "Sermon text as written, e.g. Johannes 5:25"},
                        "book": {"type": "integer", "description": "1-66, 0 when the text is not a Bible passage"},
                        "chapter": {"type": "integer"},
                        "from": {"type": "integer"},
                        "to": {"type": "integer"},
                        "catechism_question": {"type": "integer", "description": "Heidelberg Catechism question number, else 0"},
                    }
                },
                "theme": {"type": "string", "description": "Thema title, without the numbered points"},
                "points": {"type": "array", "items": {"type": "string"}, "description": "The numbered thema points, without numbers"},
            }
        }}}
    })
}

const PROMPT: &str = r#"Below is the text of the "Kerkdiensten" page of a Dutch Reformed (Hersteld Hervormde) church. It lists the liturgy for upcoming services. Extract every service.

Notes:
- "Ps." means a psalm from the 1773 berijming; "Ps. 86: 5-6" is psalm 86, verses 5 and 6.
- The "Voorzang" is the first psalm; keep all psalms in the order they are listed.
- Readings may be written as "Joh. 5: 16-29 en Joh. 11: 40-44" (two passages) or "1 Korinthe 6 vers 15 t/m 20".
- Ignore discussion questions ("Vragen").

Page text:
"#;

pub async fn extract_liturgy(url: &str) -> Result<Extracted> {
    let text = page_text(url).await?;
    crate::claude::ask_json(&format!("{PROMPT}{text}"), schema()).await
}
