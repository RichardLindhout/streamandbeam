//! Reads the liturgy from the church website: the program downloads the page, Claude reads
//! it and maps it onto the fixed service format, saying what it could not place.

use crate::claude::{RawItem, ask_json, item_schema, reference_lists};
use anyhow::Result;
use serde::Deserialize;
use serde_json::json;

pub const LITURGY_URL: &str = "https://www.hhgtholen.nl/kerkdiensten";

#[derive(Debug, Deserialize)]
pub struct Extracted {
    pub diensten: Vec<ExtractedService>,
}

#[derive(Debug, Deserialize)]
pub struct ExtractedService {
    pub datum: String,  // 2026-10-18
    pub tijd: String,   // 09:30
    pub dienst: String, // Morgendienst
    /// The psalms, readings and catechism in the order the website lists them.
    pub onderdelen: Vec<RawItem>,
    pub preektekst: RawItem,
    pub thema: String,
    pub punten: Vec<String>,
    /// Things on the page Claude could not place, with the reason.
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

pub async fn extract_liturgy(url: &str) -> Result<Extracted> {
    let page = http_get(url).await?;
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
