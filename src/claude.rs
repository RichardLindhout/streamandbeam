//! Claude reads the website and what the operator types, and maps both onto one fixed
//! format: the service items below. Per item it says whether it understood it; the
//! program then checks the numbers against its own texts.

use crate::books::{BOOK_NAMES, BibleRef};
use crate::show::Item;
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

/// Sends one prompt to Claude (no official Rust SDK, so the REST API) and decodes its
/// schema-constrained JSON answer.
pub async fn ask_json<T: DeserializeOwned>(prompt: &str, schema: Value) -> Result<T> {
    let key = std::env::var("ANTHROPIC_API_KEY").context("ANTHROPIC_API_KEY is niet gezet")?;
    let body = json!({
        "model": "claude-opus-5-5",
        "max_tokens": 16000,
        "fallbacks": "default",
        "output_config": {"effort": "low", "format": {"type": "json_schema", "schema": schema}},
        "messages": [{"role": "user", "content": prompt}],
    });
    let resp = reqwest::Client::new()
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", key)
        .header("anthropic-version", "2023-06-01")
        .header("anthropic-beta", "server-side-fallback-2026-07-01")
        .timeout(std::time::Duration::from_secs(300))
        .json(&body)
        .send()
        .await?;
    let status = resp.status();
    let msg: Value = resp.json().await?;
    if !status.is_success() {
        bail!("Claude API {status}: {}", msg["error"]["message"].as_str().unwrap_or("onbekende fout"));
    }
    if msg["stop_reason"] == "refusal" {
        bail!("Claude weigerde het verzoek");
    }
    let text = msg["content"]
        .as_array()
        .and_then(|blocks| blocks.iter().find(|b| b["type"] == "text"))
        .and_then(|b| b["text"].as_str())
        .with_context(|| format!("geen antwoord van Claude (stop: {})", msg["stop_reason"]))?;
    Ok(serde_json::from_str(text)?)
}

/// The fixed format for one service item, as Claude returns it.
#[derive(Debug, Clone, Deserialize)]
pub struct RawItem {
    pub soort: String,
    pub opschrift: String,
    pub psalm: u32,
    pub verzen: Vec<u32>,
    pub boek: u32,
    pub hoofdstuk: u32,
    pub van: u32,
    pub tot: u32,
    pub vragen: Vec<u32>,
    pub zondag: u32,
    pub tekst: String,
    pub herkend: bool,
    pub reden: String,
}

pub const KINDS: [&str; 11] =
    ["welkom", "psalm", "slotzang", "nazang", "lezing", "catechismus", "gebed", "collecte", "preek", "mededeling", "einde"];

pub fn item_schema() -> Value {
    let int = |d: &str| json!({"type": "integer", "description": d});
    json!({
        "type": "object", "additionalProperties": false,
        "required": ["soort", "opschrift", "psalm", "verzen", "boek", "hoofdstuk", "van", "tot", "vragen", "zondag", "tekst", "herkend", "reden"],
        "properties": {
            "soort": {"type": "string", "enum": KINDS},
            "opschrift": {"type": "string", "description": "Short label, e.g. Voorzang, Zingen, Schriftlezing, Gebed, Dankgebed, Slotzang"},
            "psalm": int("Psalm number 1-150, or 151-160 for the gezangen listed below; 0 if not a psalm"),
            "verzen": {"type": "array", "items": {"type": "integer"}, "description": "Verse numbers, ranges expanded (5-6 → [5,6])"},
            "boek": int("Bible book number from the list below; 0 if not a reading"),
            "hoofdstuk": int("Chapter; 0 if not a reading"),
            "van": int("First verse; 1 for a whole chapter"),
            "tot": int("Last verse; 999 for a whole chapter"),
            "vragen": {"type": "array", "items": {"type": "integer"}, "description": "Heidelberg Catechism question numbers"},
            "zondag": int("Heidelberg Catechism Lord's Day (zondag) when given instead of questions, else 0"),
            "tekst": {"type": "string", "description": "Free text for a mededeling, or the collecte purposes one per line"},
            "herkend": {"type": "boolean", "description": "false when you are not sure what is meant"},
            "reden": {"type": "string", "description": "When herkend is false: why, in Dutch. Else empty."},
        }
    })
}

/// What Claude needs to know to map names onto numbers.
pub fn reference_lists() -> String {
    let books: Vec<String> = BOOK_NAMES.iter().enumerate().map(|(i, b)| format!("{}={b}", i + 1)).collect();
    let gezangen: Vec<String> = crate::texts::gezangen().iter().map(|(n, name)| format!("{n}={name}")).collect();
    format!("Bible books: {}\nGezangen (use as psalm number): {}", books.join(", "), gezangen.join(", "))
}

pub const RULES: &str = r#"Rules for items:
- "Ps. 86: 5-6" or "86:5,6" is psalm 86, verses [5, 6] of the 1773 rhyming. soort "psalm"; "slotzang" only when it is called the last/closing psalm.
- A song sung after the service ("na de dienst", "nazang", "lied na de dienst") is soort "nazang": a psalm with psalm/verzen, or for another song the title in tekst.
- A reading ("Joh. 5: 16-29", "1 Korinthe 6 vers 15 t/m 20") is soort "lezing" with boek/hoofdstuk/van/tot.
- "HC 57", "vraag en antwoord 57", "Heidelbergse Catechismus zondag 22" is soort "catechismus" with vragen or zondag.
- gebed, dankgebed, collecte, preek/prediking, welkom, einde are their own soort.
- Anything that is an announcement is soort "mededeling" with the text as typed.
- If you are not sure, still give your best guess but set herkend false and explain in reden. Never invent verses that were not mentioned."#;

impl RawItem {
    /// Turns Claude's answer into an item and checks it against the program's own texts.
    pub fn into_item(self) -> Item {
        let mut problems: Vec<String> = Vec::new();
        if !self.herkend {
            problems.push(if self.reden.is_empty() { "niet zeker herkend".into() } else { self.reden.clone() });
        }
        let kind = if KINDS.contains(&self.soort.as_str()) { self.soort.clone() } else { "mededeling".into() };
        let mut item = Item { kind: kind.clone(), label: self.opschrift.clone(), text: self.tekst.clone(), ..Default::default() };
        match kind.as_str() {
            "nazang" if self.psalm == 0 => {}
            "psalm" | "slotzang" | "nazang" => {
                item.psalm = self.psalm;
                item.verses = self.verzen.clone();
                match crate::texts::psalm(self.psalm) {
                    None => problems.push(format!("psalm {} bestaat niet", self.psalm)),
                    Some(p) => {
                        if item.verses.is_empty() {
                            problems.push("geen verzen opgegeven".into());
                        }
                        for v in &item.verses {
                            if *v == 0 || *v as usize > p.verzen.len() {
                                problems.push(format!("{} heeft maar {} verzen (niet {v})", p.naam, p.verzen.len()));
                            }
                        }
                    }
                }
            }
            "lezing" => {
                let r = BibleRef { book: self.boek, chapter: self.hoofdstuk, from: self.van.max(1), to: self.tot.max(self.van) };
                if let Err(e) = crate::texts::passage(r) {
                    problems.push(e.to_string());
                }
                item.passage = Some(r);
            }
            "catechismus" => {
                item.questions = self.vragen.clone();
                item.zondag = self.zondag;
                if crate::catechism::for_item(&item).is_empty() {
                    problems.push("deze catechismusvraag/-zondag bestaat niet".into());
                }
            }
            _ => {}
        }
        item.problem = problems.join("; ");
        item.refresh();
        item
    }
}

#[derive(Deserialize)]
struct Items {
    onderdelen: Vec<RawItem>,
}

/// Quick entry: "ps 42:5, joh 5:16-29, gebed, HC 57" → items.
pub async fn parse_typed(input: &str) -> Result<Vec<Item>> {
    let schema = json!({
        "type": "object", "additionalProperties": false, "required": ["onderdelen"],
        "properties": {"onderdelen": {"type": "array", "items": item_schema()}}
    });
    let prompt = format!(
        "An operator in a Dutch Reformed church typed parts of a church service, separated by commas or new lines. \
         Map every part, in order, onto one item.\n\n{RULES}\n\n{}\n\nTyped:\n{input}",
        reference_lists()
    );
    let parsed: Items = ask_json(&prompt, schema).await?;
    Ok(parsed.onderdelen.into_iter().map(RawItem::into_item).collect())
}

