//! YouTube: log in once with Google (OAuth), then the program creates the broadcast for each
//! service itself, with the title the church uses ("18 oktober 2026 - 16:00 middagdienst -
//! Heidelbergse Catechismus v&a 57 - Ds. A. den Hartog"), and closes it after the service.
//!
//! Needs an OAuth client of type "Desktop app" from Google Cloud Console with the YouTube
//! Data API v3 enabled; its client id and secret go in the settings.

use crate::show::Service;
use crate::texts::{read_json, write_json};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::Mutex;

const API: &str = "https://www.googleapis.com/youtube/v3";
const SCOPE: &str = "https://www.googleapis.com/auth/youtube";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct Tokens {
    refresh_token: String,
    access_token: String,
    /// Unix seconds.
    expires_at: i64,
    channel: String,
    /// Our reusable YouTube stream: where the video goes.
    stream_id: String,
    ingestion_url: String,
    stream_name: String,
}

pub struct YouTube {
    path: PathBuf,
    tokens: Mutex<Tokens>,
}

/// Where to send the video for a broadcast, and where viewers watch it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Broadcast {
    pub id: String,
    pub title: String,
    pub watch_url: String,
    #[serde(skip)]
    pub rtmp_url: String,
    #[serde(skip)]
    pub stream_key: String,
}

impl YouTube {
    pub fn new(path: PathBuf) -> YouTube {
        let tokens = read_json(&path).unwrap_or_default();
        YouTube { path, tokens: Mutex::new(tokens) }
    }

    /// The channel name when logged in.
    pub fn channel(&self) -> Option<String> {
        let t = self.tokens.lock().unwrap();
        (!t.refresh_token.is_empty()).then(|| t.channel.clone())
    }

    pub fn logout(&self) {
        *self.tokens.lock().unwrap() = Tokens::default();
        let _ = std::fs::remove_file(&self.path);
    }

    pub fn login_url(client_id: &str, redirect: &str) -> String {
        let q = [
            ("client_id", client_id),
            ("redirect_uri", redirect),
            ("response_type", "code"),
            ("scope", SCOPE),
            ("access_type", "offline"),
            ("prompt", "consent"),
        ];
        let query: Vec<String> = q.iter().map(|(k, v)| format!("{k}={}", urlencode(v))).collect();
        format!("https://accounts.google.com/o/oauth2/v2/auth?{}", query.join("&"))
    }

    /// Finishes the login: exchanges the code for tokens and remembers the channel.
    pub async fn finish_login(&self, client_id: &str, secret: &str, redirect: &str, code: &str) -> Result<String> {
        let resp: Value = reqwest::Client::new()
            .post("https://oauth2.googleapis.com/token")
            .form(&[
                ("code", code),
                ("client_id", client_id),
                ("client_secret", secret),
                ("redirect_uri", redirect),
                ("grant_type", "authorization_code"),
            ])
            .send()
            .await?
            .json()
            .await?;
        let refresh = resp["refresh_token"].as_str().context(format!("Google gaf geen toegang: {resp}"))?;
        {
            let mut t = self.tokens.lock().unwrap();
            *t = Tokens {
                refresh_token: refresh.into(),
                access_token: resp["access_token"].as_str().unwrap_or_default().into(),
                expires_at: now() + resp["expires_in"].as_i64().unwrap_or(0) - 60,
                ..Default::default()
            };
        }
        let me = self.get(client_id, secret, "channels?part=snippet&mine=true").await?;
        let channel = me["items"][0]["snippet"]["title"].as_str().unwrap_or("(onbekend kanaal)").to_string();
        self.tokens.lock().unwrap().channel = channel.clone();
        self.save();
        Ok(channel)
    }

    fn save(&self) {
        let t = self.tokens.lock().unwrap().clone();
        if let Err(e) = write_json(&self.path, &t) {
            eprintln!("YouTube-login opslaan: {e}");
        }
    }

    async fn access_token(&self, client_id: &str, secret: &str) -> Result<String> {
        let (refresh, token, expires) = {
            let t = self.tokens.lock().unwrap();
            (t.refresh_token.clone(), t.access_token.clone(), t.expires_at)
        };
        if refresh.is_empty() {
            bail!("niet ingelogd bij YouTube");
        }
        if now() < expires && !token.is_empty() {
            return Ok(token);
        }
        let resp: Value = reqwest::Client::new()
            .post("https://oauth2.googleapis.com/token")
            .form(&[("refresh_token", refresh.as_str()), ("client_id", client_id), ("client_secret", secret), ("grant_type", "refresh_token")])
            .send()
            .await?
            .json()
            .await?;
        let token = resp["access_token"].as_str().context(format!("YouTube-login verlopen, log opnieuw in ({resp})"))?.to_string();
        {
            let mut t = self.tokens.lock().unwrap();
            t.access_token = token.clone();
            t.expires_at = now() + resp["expires_in"].as_i64().unwrap_or(0) - 60;
        }
        self.save();
        Ok(token)
    }

    async fn call(&self, client_id: &str, secret: &str, method: reqwest::Method, path: &str, body: Option<Value>) -> Result<Value> {
        let token = self.access_token(client_id, secret).await?;
        let mut req = reqwest::Client::new().request(method, format!("{API}/{path}")).bearer_auth(token);
        if let Some(b) = body {
            req = req.json(&b);
        }
        let resp = req.send().await?;
        let status = resp.status();
        let text = resp.text().await?;
        let v: Value = if text.is_empty() { Value::Null } else { serde_json::from_str(&text)? };
        if !status.is_success() {
            bail!("YouTube {status}: {}", v["error"]["message"].as_str().unwrap_or(&text));
        }
        Ok(v)
    }

    async fn get(&self, id: &str, secret: &str, path: &str) -> Result<Value> {
        self.call(id, secret, reqwest::Method::GET, path, None).await
    }

    /// Our reusable stream (one per channel): the RTMP address the engine sends to.
    async fn stream(&self, id: &str, secret: &str) -> Result<(String, String, String)> {
        {
            let t = self.tokens.lock().unwrap();
            if !t.stream_id.is_empty() {
                return Ok((t.stream_id.clone(), t.ingestion_url.clone(), t.stream_name.clone()));
            }
        }
        let body = json!({
            "snippet": {"title": "Stream & Beam"},
            "cdn": {"frameRate": "variable", "ingestionType": "rtmp", "resolution": "variable"},
            "contentDetails": {"isReusable": true},
        });
        let s = self.call(id, secret, reqwest::Method::POST, "liveStreams?part=snippet,cdn,contentDetails", Some(body)).await?;
        let info = &s["cdn"]["ingestionInfo"];
        let out = (
            s["id"].as_str().context("geen stream-id")?.to_string(),
            info["ingestionAddress"].as_str().unwrap_or_default().to_string(),
            info["streamName"].as_str().unwrap_or_default().to_string(),
        );
        {
            let mut t = self.tokens.lock().unwrap();
            (t.stream_id, t.ingestion_url, t.stream_name) = out.clone();
        }
        self.save();
        Ok(out)
    }

    /// Creates (or reuses) the broadcast for a service and binds it to our stream.
    /// It goes live by itself as soon as video arrives and ends when the video stops.
    pub async fn broadcast_for(&self, id: &str, secret: &str, svc: &Service, privacy: &str, existing: &str, test: bool) -> Result<Broadcast> {
        let (stream_id, rtmp_url, stream_key) = self.stream(id, secret).await?;
        let title = if test { format!("TEST {}", title_for(svc)) } else { title_for(svc) };
        if !existing.is_empty() {
            let b = self.get(id, secret, &format!("liveBroadcasts?part=status&id={existing}")).await?;
            let status = b["items"][0]["status"]["lifeCycleStatus"].as_str().unwrap_or("");
            if !status.is_empty() && status != "complete" && status != "revoked" {
                return Ok(Broadcast { id: existing.into(), title, watch_url: watch(existing), rtmp_url, stream_key });
            }
        }
        let body = json!({
            "snippet": {
                "title": title,
                "description": description_for(svc),
                "scheduledStartTime": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            },
            "status": {"privacyStatus": privacy, "selfDeclaredMadeForKids": false},
            "contentDetails": {
                "enableAutoStart": true,
                "enableAutoStop": true,
                "enableDvr": true,
                "recordFromStart": true,
                "latencyPreference": "normal",
                "monitorStream": {"enableMonitorStream": false},
            },
        });
        let b = self.call(id, secret, reqwest::Method::POST, "liveBroadcasts?part=snippet,status,contentDetails", Some(body)).await?;
        let bid = b["id"].as_str().context("geen uitzending-id")?.to_string();
        self.call(id, secret, reqwest::Method::POST, &format!("liveBroadcasts/bind?id={bid}&part=id,contentDetails&streamId={stream_id}"), None)
            .await?;
        Ok(Broadcast { id: bid.clone(), title, watch_url: watch(&bid), rtmp_url, stream_key })
    }

    /// "created", "ready", "testing", "live", "complete", ...
    pub async fn status(&self, id: &str, secret: &str, broadcast: &str) -> Result<String> {
        let b = self.get(id, secret, &format!("liveBroadcasts?part=status&id={broadcast}")).await?;
        Ok(b["items"][0]["status"]["lifeCycleStatus"].as_str().unwrap_or("onbekend").to_string())
    }

    /// Ends the broadcast (YouTube also does this by itself when the video stops).
    pub async fn complete(&self, id: &str, secret: &str, broadcast: &str) -> Result<()> {
        self.call(id, secret, reqwest::Method::POST, &format!("liveBroadcasts/transition?broadcastStatus=complete&id={broadcast}&part=status"), None)
            .await
            .map(|_| ())
    }
}

fn watch(id: &str) -> String {
    format!("https://www.youtube.com/watch?v={id}")
}

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

fn urlencode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// "18 oktober 2026 - 16:00 middagdienst - Heidelbergse Catechismus v&a 57 - Ds. A. den Hartog"
pub fn title_for(svc: &Service) -> String {
    let date = crate::show::long_date(&svc.date);
    // Drop the weekday: "Zondag 18 oktober 2026" → "18 oktober 2026".
    let date = date.split_once(' ').map_or(date.clone(), |(_, rest)| rest.to_string());
    let mut parts = vec![date, format!("{} {}", svc.time, svc.label.to_lowercase())];
    let text = if svc.text.kind == "catechismus" {
        format!("Heidelbergse Catechismus v&a {}", svc.text.questions.iter().map(u32::to_string).collect::<Vec<_>>().join(", "))
    } else {
        svc.text.reference.clone()
    };
    if !text.trim().is_empty() {
        parts.push(text);
    }
    if !svc.predikant.is_empty() {
        let mut p = svc.predikant.clone();
        p[..1].make_ascii_uppercase();
        parts.push(p);
    }
    parts.join(" - ")
}

fn description_for(svc: &Service) -> String {
    let mut lines: Vec<String> = svc
        .items
        .iter()
        .filter(|i| !i.reference.is_empty())
        .map(|i| format!("{}: {}", i.label, i.reference))
        .collect();
    if !svc.theme.is_empty() {
        lines.push(format!("\nThema: {}", svc.theme));
        lines.extend(svc.points.iter().enumerate().map(|(i, p)| format!("{}. {p}", i + 1)));
    }
    lines.push("\nHersteld Hervormde Gemeente Tholen — www.hhgtholen.nl".into());
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::show::Item;

    #[test]
    fn title_like_the_church() {
        let svc = Service {
            date: "2026-10-18".into(),
            time: "16:00".into(),
            label: "Middagdienst".into(),
            predikant: "ds. A. den Hartog".into(),
            text: Item { kind: "catechismus".into(), questions: vec![57], ..Default::default() },
            ..Default::default()
        };
        assert_eq!(title_for(&svc), "18 oktober 2026 - 16:00 middagdienst - Heidelbergse Catechismus v&a 57 - Ds. A. den Hartog");
    }
}
