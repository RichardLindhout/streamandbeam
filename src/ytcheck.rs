//! Checks what viewers actually get: every minute a few seconds of the channel's live
//! stream are fetched from YouTube itself (yt-dlp finds the stream, ffmpeg measures it),
//! to see whether there is sound and how loud it is.

use serde::Serialize;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    /// When the last check finished (Unix seconds); 0 = never.
    pub at: i64,
    /// The channel is live on YouTube.
    pub live: bool,
    /// Sound arrives on YouTube.
    pub audio: bool,
    pub mean_db: f64,
    pub max_db: f64,
    /// What went wrong, in plain Dutch; empty when fine.
    pub problem: String,
}

pub struct YouTubeCheck {
    status: Mutex<Check>,
}

impl YouTubeCheck {
    pub fn new() -> Arc<YouTubeCheck> {
        Arc::new(YouTubeCheck { status: Mutex::new(Check::default()) })
    }

    pub fn status(&self) -> Check {
        self.status.lock().unwrap().clone()
    }

    /// Runs for the lifetime of the program; `channel` gives the channel id to check
    /// (empty = nothing to check), `streaming` whether we are sending to YouTube now.
    pub async fn run(self: Arc<Self>, channel: impl Fn() -> String + Send + 'static, streaming: impl Fn() -> bool + Send + 'static) {
        loop {
            let id = channel();
            if !id.is_empty() && streaming() {
                let result = tokio::task::spawn_blocking(move || measure(&id)).await.unwrap_or_else(|e| Check { problem: e.to_string(), ..Default::default() });
                *self.status.lock().unwrap() = Check { at: chrono::Utc::now().timestamp(), ..result };
            }
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    }
}

/// Fetches 8 seconds of the live stream and measures the sound in it.
pub fn measure(channel: &str) -> Check {
    let page = if channel.starts_with("http") { channel.to_string() } else { format!("https://www.youtube.com/channel/{channel}/live") };
    let url = match Command::new("yt-dlp").args(["-g", "-f", "bestaudio/best", "--no-warnings", &page]).output() {
        Err(_) => return Check { problem: "yt-dlp ontbreekt op deze computer".into(), ..Default::default() },
        Ok(o) if !o.status.success() => {
            let err = String::from_utf8_lossy(&o.stderr);
            let problem = if err.contains("not currently live") || err.contains("not live") || err.contains("will begin") {
                "het kanaal is (nog) niet live op YouTube".to_string()
            } else {
                format!("YouTube niet bereikbaar: {}", err.lines().last().unwrap_or(""))
            };
            return Check { problem, ..Default::default() };
        }
        Ok(o) => String::from_utf8_lossy(&o.stdout).lines().next().unwrap_or("").to_string(),
    };
    let out = match Command::new("ffmpeg")
        .args(["-hide_banner", "-nostats", "-t", "8", "-i", &url, "-vn", "-af", "volumedetect", "-f", "null", "-"])
        .output()
    {
        Ok(o) => String::from_utf8_lossy(&o.stderr).into_owned(),
        Err(_) => return Check { live: true, problem: "ffmpeg ontbreekt op deze computer".into(), ..Default::default() },
    };
    let num = |key: &str| out.lines().find_map(|l| l.split_once(key).and_then(|(_, v)| v.trim().split_whitespace().next()?.parse::<f64>().ok()));
    match (num("mean_volume:"), num("max_volume:")) {
        (Some(mean), Some(max)) => {
            let audio = mean > -55.0;
            let problem = if audio { String::new() } else { format!("YouTube ontvangt (bijna) geen geluid: gemiddeld {mean:.0} dB") };
            Check { live: true, audio, mean_db: mean, max_db: max, problem, ..Default::default() }
        }
        _ => Check { live: true, problem: "de uitzending op YouTube heeft geen geluidsspoor".into(), ..Default::default() },
    }
}
