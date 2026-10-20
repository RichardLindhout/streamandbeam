//! Listening along: Whisper turns what is said in church into text on this computer (free,
//! no internet), and every few seconds Claude compares it with the liturgy to suggest the
//! next part ("laten we bidden" → Gebed, "we zingen psalm 42 vers 5" → that psalm).
//! The operator sees the suggestion blink; in "automatisch" mode a sure suggestion is
//! followed by itself.

use crate::show::{Show, View};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

const RATE: usize = 16_000;
const MODEL_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small-q5_1.bin";

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Suggestion {
    /// Slide to go to.
    pub slide: usize,
    pub label: String,
    pub reason: String,
    pub sure: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListenStatus {
    pub active: bool,
    /// The last thing understood, for the operator to see that listening works.
    pub heard: String,
    pub suggestion: Option<Suggestion>,
    pub error: String,
}

pub struct Listener {
    /// 16 kHz mono samples from the engine, newest at the back (at most ~30 s).
    audio: Mutex<VecDeque<f32>>,
    /// (when, text) of the last two minutes.
    transcript: Mutex<VecDeque<(Instant, String)>>,
    status: Mutex<ListenStatus>,
    model: PathBuf,
}

impl Listener {
    pub fn new(model: PathBuf) -> Arc<Listener> {
        Arc::new(Listener {
            audio: Mutex::new(VecDeque::new()),
            transcript: Mutex::new(VecDeque::new()),
            status: Mutex::new(ListenStatus::default()),
            model,
        })
    }

    pub fn status(&self) -> ListenStatus {
        self.status.lock().unwrap().clone()
    }

    pub fn clear_suggestion(&self) {
        self.status.lock().unwrap().suggestion = None;
    }

    /// Called by the engine with new 16 kHz mono audio.
    pub fn push_audio(&self, samples: &[f32]) {
        let mut a = self.audio.lock().unwrap();
        a.extend(samples);
        let excess = a.len().saturating_sub(30 * RATE);
        a.drain(..excess);
    }

    /// Transcribes in the background for the lifetime of the program.
    pub fn start(self: &Arc<Self>) {
        let me = self.clone();
        std::thread::spawn(move || {
            if let Err(e) = me.transcribe_loop() {
                eprintln!("meeluisteren: {e:#}");
                me.status.lock().unwrap().error = format!("{e:#}");
            }
        });
    }

    fn transcribe_loop(&self) -> Result<()> {
        if !self.model.exists() {
            println!("Whisper-model downloaden (eenmalig, ±190 MB)…");
            std::fs::create_dir_all(self.model.parent().unwrap())?;
            let status = std::process::Command::new("curl").args(["-sSL", "-o"]).arg(&self.model).arg(MODEL_URL).status()?;
            anyhow::ensure!(status.success() && self.model.exists(), "Whisper-model downloaden mislukt");
        }
        let ctx = WhisperContext::new_with_params(self.model.to_str().unwrap(), WhisperContextParameters::default())
            .context("Whisper-model laden")?;
        let mut state = ctx.create_state()?;
        self.status.lock().unwrap().active = true;
        loop {
            std::thread::sleep(Duration::from_secs(5));
            // The last 6 seconds (1 s overlap with the previous window).
            let window: Vec<f32> = {
                let mut a = self.audio.lock().unwrap();
                let take = a.len().min(6 * RATE);
                let w = a.iter().skip(a.len() - take).copied().collect();
                let keep = a.len().saturating_sub(RATE); // drop all but the overlap
                a.drain(..keep);
                w
            };
            let loud = (window.iter().map(|s| s * s).sum::<f32>() / window.len().max(1) as f32).sqrt();
            if window.len() < 2 * RATE || loud < 0.005 {
                continue;
            }
            let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
            params.set_language(Some("nl"));
            params.set_n_threads(4);
            params.set_print_special(false);
            params.set_print_progress(false);
            params.set_print_realtime(false);
            params.set_print_timestamps(false);
            params.set_suppress_blank(true);
            if state.full(params, &window).is_err() {
                continue;
            }
            // During organ music and singing Whisper may "hear" words that were never said;
            // it marks those pieces as probably not speech, so they are skipped.
            let text: String = state
                .as_iter()
                .filter(|s| s.no_speech_probability() < 0.3)
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
                .join(" ")
                .trim()
                .to_string();
            if text.is_empty() || text.starts_with('[') || text.starts_with('(') {
                continue;
            }
            let mut t = self.transcript.lock().unwrap();
            t.push_back((Instant::now(), text.clone()));
            while t.front().is_some_and(|(at, _)| at.elapsed() > Duration::from_secs(120)) {
                t.pop_front();
            }
            self.status.lock().unwrap().heard = text;
        }
    }

    /// What was said in the last `secs` seconds.
    fn recent(&self, secs: u64) -> String {
        let t = self.transcript.lock().unwrap();
        t.iter().filter(|(at, _)| at.elapsed() <= Duration::from_secs(secs)).map(|(_, s)| s.as_str()).collect::<Vec<_>>().join(" ")
    }

    /// Every 15 seconds of new speech, asks Claude whether the service has moved on.
    /// In "automatisch" mode a sure suggestion is followed.
    pub async fn suggest_loop(self: Arc<Self>, show: Arc<Show>, mode: impl Fn() -> String + Send + 'static) {
        let mut last_asked = String::new();
        let mut last_index = usize::MAX;
        loop {
            tokio::time::sleep(Duration::from_secs(15)).await;
            let mode = mode();
            let view = show.view();
            if view.index != last_index {
                // The operator moved on; an old suggestion no longer applies.
                last_index = view.index;
                self.clear_suggestion();
            }
            if mode == "uit" {
                continue;
            }
            let heard = self.recent(45);
            if heard.is_empty() || heard == last_asked {
                continue;
            }
            last_asked = heard.clone();
            match ask(&view, &heard).await {
                Ok(Some(s)) if s.sure && mode == "automatisch" => {
                    println!("meeluisteren: naar {} ({})", s.label, s.reason);
                    show.go(Some(s.slide), 0, "").await;
                    last_index = s.slide;
                    self.clear_suggestion();
                }
                Ok(s) => self.status.lock().unwrap().suggestion = s,
                Err(e) => self.status.lock().unwrap().error = format!("{e:#}"),
            }
        }
    }
}

#[derive(Deserialize)]
struct Answer {
    verder: bool,
    onderdeel: i64,
    zekerheid: String,
    reden: String,
}

/// Gives Claude the current and coming parts of the service and what was just said.
async fn ask(view: &View, heard: &str) -> Result<Option<Suggestion>> {
    let Some(svc) = view.service() else { return Ok(None) };
    let current = view.slides.get(view.index).map(|s| s.item).unwrap_or(0);
    let parts: Vec<String> = svc
        .items
        .iter()
        .enumerate()
        .skip(current)
        .take(5)
        .map(|(i, it)| format!("{i}: {} {}{}", it.label, it.reference, if i == current { "  ← NU" } else { "" }))
        .collect();
    let schema = json!({
        "type": "object", "additionalProperties": false,
        "required": ["verder", "onderdeel", "zekerheid", "reden"],
        "properties": {
            "verder": {"type": "boolean", "description": "true when what was said shows a later part has begun"},
            "onderdeel": {"type": "integer", "description": "Number of the part that has begun, else -1"},
            "zekerheid": {"type": "string", "enum": ["hoog", "middel", "laag"]},
            "reden": {"type": "string", "description": "Short, in Dutch: which words show it"},
        }
    });
    let prompt = format!(
        "A Dutch Reformed church service is going on. Below are the parts of the service from the current one (← NU) \
         onward, and an automatic transcript of the last 45 seconds (it can contain mistakes). Has one of the later \
         parts clearly begun? Typical signs: the predikant announces a psalm (\"we zingen psalm 42 vers 5\"), starts \
         praying (\"laten we bidden\"), announces the reading (\"we lezen uit\"), says amen after the sermon, or the \
         collecte is announced. Only say verder=true when the words show it.\n\nParts:\n{}\n\nTranscript:\n{heard}",
        parts.join("\n")
    );
    let a: Answer = crate::claude::ask_json(&prompt, schema).await?;
    if !a.verder || a.onderdeel < 0 || a.zekerheid == "laag" {
        return Ok(None);
    }
    let item = a.onderdeel as usize;
    let Some(slide) = view.slides.iter().position(|s| s.item == item) else { return Ok(None) };
    let label = svc.items.get(item).map(|i| format!("{} {}", i.label, i.reference).trim().to_string()).unwrap_or_default();
    Ok(Some(Suggestion { slide, label, reason: a.reden, sure: a.zekerheid == "hoog" }))
}
