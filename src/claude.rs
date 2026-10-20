//! Minimal Claude API client (there is no official Rust SDK, so this uses the REST API).

use anyhow::{Context, Result, bail};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

/// Sends one prompt to Claude and decodes its schema-constrained JSON answer.
pub async fn ask_json<T: DeserializeOwned>(prompt: &str, schema: Value) -> Result<T> {
    let key = std::env::var("ANTHROPIC_API_KEY").context("ANTHROPIC_API_KEY is niet gezet")?;
    let body = json!({
        "model": "claude-opus-5-5",
        "max_tokens": 16000,
        "fallbacks": "default",
        "output_config": {
            "effort": "low",
            "format": {"type": "json_schema", "schema": schema},
        },
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
