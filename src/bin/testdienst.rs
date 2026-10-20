//! Testdienst: runs a complete service automatically on one computer and checks it.
//!
//! It starts the nepcamera (recording of an earlier service), a fake YouTube (ffmpeg as
//! RTMP receiver) and streamandbeam, reads the liturgy from the example website, goes
//! live, clicks through every slide, tests the "no sound" warning and finally checks the
//! camera positions, the recording and what "YouTube" received.
//!
//!     cargo build --release && target/release/testdienst [--snel] [--youtube]
//!
//! With --youtube it goes live for real on the channel logged in under ./data (a private
//! broadcast with "TEST" in the title) and checks that YouTube reports it live and ended.

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const APP: &str = "http://127.0.0.1:18080";
const CAMERA_HTTP: &str = "127.0.0.1:18095";
const RTSP: &str = "rtsp://127.0.0.1:18554";
const RTMP: &str = "rtmp://127.0.0.1:19350/live2";

/// Kills the started programs when the test ends, also on failure.
struct Processes(Vec<Child>);

impl Drop for Processes {
    fn drop(&mut self) {
        for c in &mut self.0 {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

#[derive(Default)]
struct Report {
    lines: Vec<(bool, String)>,
}

impl Report {
    fn check(&mut self, ok: bool, what: impl Into<String>) {
        let what = what.into();
        println!("{} {what}", if ok { "  ✔" } else { "  ✘" });
        self.lines.push((ok, what));
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let fast = std::env::args().any(|a| a == "--snel");
    let real_youtube = std::env::args().any(|a| a == "--youtube");
    let bin_dir = std::env::current_exe()?.parent().unwrap().to_path_buf();
    let out = PathBuf::from("target/testdienst").join(chrono::Local::now().format("%Y-%m-%d-%H%M%S").to_string());
    std::fs::create_dir_all(&out)?;
    let out = out.canonicalize()?;
    println!("Testdienst → {}", out.display());

    let mut procs = Processes(Vec::new());
    let log = |name: &str| std::fs::File::create(out.join(format!("{name}.log"))).map(Stdio::from);

    let mut cam = Command::new(bin_dir.join("nepcamera"));
    cam.args(["--rtsp-poort", "18554", "--http-poort", "18095"]);
    if Path::new("testdata/dienst.mp4").exists() {
        cam.args(["--bestand", "testdata/dienst.mp4"]);
        if Path::new("testdata/preek-geluid.m4a").exists() {
            cam.args(["--geluid", "testdata/preek-geluid.m4a"]);
        }
        if Path::new("testdata/orgel.mp4").exists() {
            // A different shot per camera position, so switching can be seen.
            cam.args(["--posities", "testdata"]);
        }
    } else {
        println!("  (testdata/dienst.mp4 ontbreekt: nepcamera gebruikt een testbeeld)");
    }
    procs.0.push(cam.stdout(log("nepcamera")?).stderr(log("nepcamera-fouten")?).spawn().context("nepcamera starten")?);
    // The camera must answer before streamandbeam calls its first position.
    let http = reqwest::Client::new();
    wait_until(Duration::from_secs(20), || async { http.get(format!("http://{CAMERA_HTTP}/log")).send().await.ok().map(|_| ()) })
        .await
        .context("nepcamera start niet")?;

    let youtube = out.join("youtube.flv");
    let ffmpeg = Command::new("ffmpeg")
        .args(["-loglevel", "error", "-listen", "1", "-i", &format!("{RTMP}/testsleutel"), "-c", "copy", "-y"])
        .arg(&youtube)
        .stdout(Stdio::null())
        .stderr(log("youtube")?)
        .spawn()
        .context("ffmpeg starten (nodig als nep-YouTube)")?;
    procs.0.push(ffmpeg);

    let main_settings: Value = std::fs::read("data/instellingen.json").ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(json!({}));
    if real_youtube {
        std::fs::create_dir_all(out.join("data"))?;
        std::fs::copy("data/youtube.json", out.join("data/youtube.json"))
            .context("niet ingelogd bij YouTube: log eerst in via de bediening (data/youtube.json ontbreekt)")?;
    }
    let api_key_ok = std::env::var("ANTHROPIC_API_KEY").is_ok();
    let mut app = Command::new(bin_dir.join("streamandbeam"));
    app.args(["--poort", "18080", "--publiek-poort", "18090", "--liturgie-url", &format!("http://{CAMERA_HTTP}/kerkdiensten"), "--data"])
        .arg(out.join("data"));
    if !api_key_ok {
        app.arg("--oude-spelling");
    }
    procs.0.push(app.stdout(log("streamandbeam")?).stderr(log("streamandbeam-fouten")?).spawn()?);

    wait_until(Duration::from_secs(15), || async { http.get(format!("{APP}/api/state")).send().await.ok().map(|_| ()) })
        .await
        .context("streamandbeam start niet")?;

    let mut r = Report::default();
    println!("\n1. Instellingen");
    let mut settings: Value = http.get(format!("{APP}/api/settings")).send().await?.json().await?;
    settings["cameraIp"] = json!(CAMERA_HTTP);
    settings["cameraRtsp"] = json!(format!("{RTSP}/1"));
    settings["audioSource"] = json!(format!("rtspsrc location={RTSP}/geluid latency=200 ! decodebin"));
    settings["rtmpUrl"] = json!(RTMP);
    settings["streamKey"] = json!("testsleutel");
    settings["recordDir"] = json!(out.join("opnames"));
    settings["introSeconds"] = json!(1);
    settings["themeSeconds"] = json!(2);
    // This test goes live and stops by itself; no automatic start in between.
    settings["autoStartMinutes"] = json!(0);
    if real_youtube {
        settings["youtubeClientId"] = main_settings["youtubeClientId"].clone();
        settings["youtubeClientSecret"] = main_settings["youtubeClientSecret"].clone();
        settings["youtubeTest"] = json!(true);
    }
    let saved = http.post(format!("{APP}/api/settings")).json(&settings).send().await?.status().is_success();
    r.check(saved, "instellingen opgeslagen");

    println!("\n2. Liturgie van de voorbeeldwebsite");
    let fetched = http.post(format!("{APP}/api/fetch")).send().await?;
    if fetched.status().is_success() {
        r.check(true, "liturgie opgehaald door Claude");
    } else {
        let err = fetched.text().await?;
        println!("     Claude niet beschikbaar ({err}); de liturgie wordt ingeladen zoals Claude hem zou lezen");
        http.post(format!("{APP}/api/service")).json(&fallback_service()).send().await?;
        http.post(format!("{APP}/api/select?id=2026-10-18%2016:00")).send().await?;
    }
    let state: Value = http.get(format!("{APP}/api/state")).send().await?.json().await?;
    let service = state["services"].as_array().and_then(|s| s.iter().find(|s| s["id"] == state["current"])).cloned();
    let psalms: Vec<String> = service
        .iter()
        .flat_map(|s| s["items"].as_array().cloned().unwrap_or_default())
        .filter(|it| it["kind"] == "psalm" || it["kind"] == "slotzang")
        .map(|it| it["ref"].as_str().unwrap_or("").to_string())
        .collect();
    let want = ["Psalm 55 : 1", "Psalm 49 : 6", "Psalm 16 : 6", "Psalm 73 : 12, 13", "Psalm 17 : 8", "Psalm 73 : 14"];
    r.check(psalms == want, format!("psalmen in de juiste volgorde: {}", psalms.join(" / ")));
    let slides = state["slides"].as_array().cloned().unwrap_or_default();
    let missing: Vec<String> = slides
        .iter()
        .filter(|s| {
            let empty = s["lines"].as_array().is_none_or(|l| l.is_empty()) && s["verses"].as_array().is_none_or(|v| v.is_empty());
            matches!(s["kind"].as_str(), Some("psalm" | "lezing" | "catechismus")) && empty
        })
        .map(|s| format!("{} ({})", s["header"].as_str().unwrap_or(""), s["sub"].as_str().unwrap_or("")))
        .collect();
    r.check(missing.is_empty(), format!("{} dia's, alle teksten geladen {}", slides.len(), if missing.is_empty() { String::new() } else { format!("(ontbreekt: {})", missing.join(", ")) }));

    println!("\n3. Live naar (nep-)YouTube");
    let live = http.post(format!("{APP}/api/stream")).json(&json!({"mode": "live"})).send().await?;
    let live_ok = live.status().is_success();
    r.check(live_ok, format!("live gegaan {}", if live_ok { String::new() } else { live.text().await? }));
    let sound_ok = wait_until(Duration::from_secs(20), || async {
        let st: Value = http.get(format!("{APP}/api/stream")).send().await.ok()?.json().await.ok()?;
        (st["audioDb"].as_f64()? > -60.0 && st["audioWarning"] == "").then_some(())
    })
    .await
    .is_ok();
    r.check(sound_ok, "geluid komt binnen, geen waarschuwing");
    if real_youtube {
        let live = wait_until(Duration::from_secs(180), || async {
            let yt: Value = http.get(format!("{APP}/api/youtube")).send().await.ok()?.json().await.ok()?;
            (yt["status"] == "live").then(|| yt["broadcast"]["watchUrl"].as_str().unwrap_or("").to_string())
        })
        .await;
        match &live {
            Ok(url) => r.check(true, format!("YouTube meldt LIVE: {url}")),
            Err(_) => {
                let yt: Value = http.get(format!("{APP}/api/youtube")).send().await?.json().await?;
                r.check(false, format!("YouTube kwam niet op live (status: {})", yt["status"]));
            }
        }
    }
    let preview = wait_until(Duration::from_secs(10), || async {
        let jpeg = http.get(format!("{APP}/preview.jpg")).send().await.ok()?.bytes().await.ok()?;
        (jpeg.len() > 1000).then_some(jpeg.len())
    })
    .await;
    r.check(preview.is_ok(), format!("voorbeeldbeeld voor de bediener ({} kB)", preview.unwrap_or(0) / 1024));

    println!("\n4. Hele dienst doorklikken");
    let pause = Duration::from_millis(if real_youtube { 6000 } else if fast { 1200 } else { 3000 });
    let mut errors = Vec::new();
    for i in 0..slides.len() {
        http.post(format!("{APP}/api/go")).json(&json!({"index": i})).send().await?;
        tokio::time::sleep(pause).await;
        let st: Value = http.get(format!("{APP}/api/stream")).send().await?.json().await?;
        if st["error"] != "" {
            errors.push(format!("dia {i}: {}", st["error"]));
        }
        print!("\r     dia {}/{}  {}                    ", i + 1, slides.len(), slides[i]["header"].as_str().unwrap_or(""));
    }
    println!();
    r.check(errors.is_empty(), format!("geen fouten tijdens de dienst {}", errors.join("; ")));
    let rec: Value = http.get(format!("{APP}/api/stream")).send().await?.json().await?;
    let recording = rec["recording"].as_str().unwrap_or("").to_string();
    let broadcast: Value = http.get(format!("{APP}/api/youtube")).send().await?.json().await?;
    http.post(format!("{APP}/api/stream")).json(&json!({"mode": "stop"})).send().await?;
    tokio::time::sleep(Duration::from_secs(2)).await;
    if real_youtube {
        // The broadcast is closed on stop; check with YouTube itself.
        tokio::time::sleep(Duration::from_secs(10)).await;
        let yt: Value = http.get(format!("{APP}/api/youtube")).send().await?.json().await?;
        let id = broadcast["broadcast"]["id"].as_str().unwrap_or("");
        r.check(!id.is_empty() && yt["broadcast"].is_null(), format!("YouTube-uitzending {id} afgesloten"));
    }

    println!("\n5. Camera-posities");
    let expected = expected_presets(&slides);
    let calls: Vec<u32> = http.get(format!("http://{CAMERA_HTTP}/log")).send().await?.json().await?;
    r.check(calls == expected, format!("camera kreeg {calls:?}, verwacht {expected:?}"));

    println!("\n6. Opname en YouTube-ontvangst");
    let min = slides.len() as f64 * pause.as_secs_f64() * 0.8;
    let checks: Vec<(&str, PathBuf)> = if real_youtube {
        vec![("opname", PathBuf::from(&recording))]
    } else {
        vec![("opname", PathBuf::from(&recording)), ("YouTube", youtube.clone())]
    };
    for (what, file) in checks {
        match probe(&file) {
            Ok((streams, secs)) => r.check(
                streams.contains("h264") && streams.contains("aac") && secs >= min,
                format!("{what}: {streams}, {secs:.0} s (minimaal {min:.0} s)"),
            ),
            Err(e) => r.check(false, format!("{what}: {e:#}")),
        }
    }
    println!("\n6b. Geluid zoals het aankomt");
    let received = if real_youtube { PathBuf::from(&recording) } else { youtube.clone() };
    // Compare with the same stretch of the original: the chain should not make the
    // sound softer or louder, leave gaps, or make speech unintelligible.
    let source = PathBuf::from(if Path::new("testdata/preek-geluid.m4a").exists() { "testdata/preek-geluid.m4a" } else { "testdata/dienst.mp4" });
    match (audio_report(&received), audio_report_part(&source, probe(&received).map(|(_, d)| d).unwrap_or(30.0))) {
        (Ok(a), Ok(orig)) => {
            let diff = a.mean_db - orig.mean_db;
            r.check(diff.abs() <= 3.0, format!("geluidsniveau gelijk aan het origineel: {:.1} dB ontvangen, {:.1} dB origineel ({diff:+.1} dB), piek {:.0} dB", a.mean_db, orig.mean_db, a.max_db));
            r.check(a.gaps.is_empty(), format!("geen gaten in het geluid{}", if a.gaps.is_empty() { String::new() } else { format!(" (stiltes op {})", a.gaps.join(", ")) }));
            let len = a.duration.min(30.0);
            match (transcribe(&received, (a.duration - len).max(0.0) / 2.0, len), transcribe(&source, 0.0, len)) {
                (Ok(heard), Ok(original)) => {
                    let (h, o) = (heard.split_whitespace().count(), original.split_whitespace().count());
                    r.check(o > 0 && h * 10 >= o * 6, format!("verstaanbaar: {h} woorden herkend in het ontvangen geluid (origineel {o}) — \"{}…\"", heard.chars().take(80).collect::<String>()));
                }
                (Err(e), _) | (_, Err(e)) => r.check(false, format!("verstaanbaarheid niet te meten: {e:#}")),
            }
        }
        (Err(e), _) | (_, Err(e)) => r.check(false, format!("geluid niet te meten: {e:#}")),
    }

    let sheet = out.join("overzicht.jpg");
    let tiles = Command::new("ffmpeg")
        .args(["-loglevel", "error", "-y", "-i"])
        .arg(if real_youtube { PathBuf::from(&recording) } else { youtube.clone() })
        .args(["-vf", &format!("fps=1/{},scale=480:-1,tile=5x5", pause.as_secs().max(1)), "-frames:v", "1"])
        .arg(&sheet)
        .status()?;
    if tiles.success() {
        println!("     overzicht van wat YouTube ontving: {}", sheet.display());
    }

    println!("\n7. Waarschuwing bij geen geluid");
    settings["audioSource"] = json!(format!("rtspsrc location={RTSP}/stilte latency=200 ! decodebin"));
    http.post(format!("{APP}/api/settings")).json(&settings).send().await?;
    http.post(format!("{APP}/api/stream")).json(&json!({"mode": "voorbeeld"})).send().await?;
    let warned = wait_until(Duration::from_secs(20), || async {
        let st: Value = http.get(format!("{APP}/api/stream")).send().await.ok()?.json().await.ok()?;
        st["audioWarning"].as_str().filter(|w| w.contains("GEEN GELUID")).map(|_| ())
    })
    .await
    .is_ok();
    r.check(warned, "waarschuwing verschijnt bij stilte");
    http.post(format!("{APP}/api/stream")).json(&json!({"mode": "stop"})).send().await?;

    let failed = r.lines.iter().filter(|(ok, _)| !ok).count();
    let summary = r.lines.iter().map(|(ok, l)| format!("{} {l}", if *ok { "OK  " } else { "FOUT" })).collect::<Vec<_>>().join("\n");
    std::fs::write(out.join("rapport.txt"), &summary)?;
    println!("\n{} — rapport: {}", if failed == 0 { "ALLES GOED".to_string() } else { format!("{failed} FOUT(EN)") }, out.join("rapport.txt").display());
    drop(procs);
    if failed > 0 {
        bail!("{failed} controle(s) mislukt");
    }
    Ok(())
}

/// The camera positions the director should call, following its scene table
/// (overzicht 1, orgel 2, preekstoel 3, doopvont 4), without repeats: a psalm is announced
/// with the camera on the predikant, then the organ plays it in.
fn expected_presets(slides: &[Value]) -> Vec<u32> {
    let mut out: Vec<u32> = Vec::new();
    let mut push = |p: u32| {
        if out.last() != Some(&p) {
            out.push(p);
        }
    };
    for (i, s) in slides.iter().enumerate() {
        let sung = matches!(s["kind"].as_str(), Some("psalm" | "slotzang" | "nazang"));
        let has_text = s["lines"].as_array().is_some_and(|l| !l.is_empty());
        let prev = i.checked_sub(1).map(|j| &slides[j]);
        let first_verse = prev.is_none_or(|p| p["item"] != s["item"] || p["lines"].as_array().is_none_or(|l| l.is_empty()));
        match s["kind"].as_str().unwrap_or("") {
            _ if sung && !has_text => push(3),
            _ if sung && first_verse => {
                push(3);
                push(2);
            }
            _ if sung => push(2),
            // The end screen: first the empty pulpit (this test steps on within 20 s).
            "welkom" => push(1),
            "collecte" => push(2),
            "gebed" => push(4),
            _ => push(3),
        }
    }
    out
}

struct Audio {
    mean_db: f64,
    max_db: f64,
    duration: f64,
    /// Silences longer than 4 seconds, as "12.0 s".
    gaps: Vec<String>,
}

fn audio_report(file: &Path) -> Result<Audio> {
    audio_report_part(file, 0.0)
}

/// Loudness and silences of the sound in a media file (ffmpeg volumedetect +
/// silencedetect), optionally only the first `secs` seconds.
fn audio_report_part(file: &Path, secs: f64) -> Result<Audio> {
    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-hide_banner", "-nostats"]);
    if secs > 0.0 {
        cmd.args(["-t", &format!("{secs:.1}")]);
    }
    let out = cmd
        .arg("-i")
        .arg(file)
        .args(["-af", "volumedetect,silencedetect=noise=-50dB:d=4", "-f", "null", "-"])
        .output()?;
    let log = String::from_utf8_lossy(&out.stderr);
    let num = |key: &str| -> Option<f64> {
        log.lines().find_map(|l| l.split_once(key).and_then(|(_, v)| v.trim().split_whitespace().next()?.parse().ok()))
    };
    let gaps = log
        .lines()
        .filter_map(|l| l.split_once("silence_start: ").map(|(_, v)| v.trim().to_string()))
        .map(|t| format!("{t} s"))
        .collect();
    Ok(Audio {
        mean_db: num("mean_volume:").context("geen geluid gevonden")?,
        max_db: num("max_volume:").unwrap_or(-100.0),
        duration: probe(file).map(|(_, d)| d).unwrap_or(0.0),
        gaps,
    })
}

/// Transcribes part of the received sound with Whisper, to check it is still speech.
fn transcribe(file: &Path, start: f64, secs: f64) -> Result<String> {
    let pcm = Command::new("ffmpeg")
        .args(["-loglevel", "error", "-ss", &format!("{:.1}", start.max(0.0)), "-t", &format!("{secs}"), "-i"])
        .arg(file)
        .args(["-ac", "1", "-ar", "16000", "-f", "f32le", "-"])
        .output()?
        .stdout;
    let samples: Vec<f32> = pcm.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
    let ctx = whisper_rs::WhisperContext::new_with_params("modellen/ggml-small-q5_1.bin", Default::default())
        .context("Whisper-model ontbreekt (start eerst streamandbeam met meeluisteren aan)")?;
    let mut state = ctx.create_state()?;
    let mut params = whisper_rs::FullParams::new(whisper_rs::SamplingStrategy::Greedy { best_of: 1 });
    params.set_language(Some("nl"));
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_special(false);
    params.set_print_timestamps(false);
    state.full(params, &samples)?;
    Ok(state.as_iter().map(|s| s.to_string()).collect::<Vec<_>>().join(" ").trim().to_string())
}

/// Codecs and duration of a media file.
fn probe(file: &Path) -> Result<(String, f64)> {
    let out = Command::new("ffprobe")
        .args(["-v", "error", "-show_entries", "stream=codec_name:format=duration", "-of", "compact"])
        .arg(file)
        .output()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let streams: Vec<&str> = text.lines().filter_map(|l| l.strip_prefix("stream|codec_name=")).collect();
    let secs = text.lines().find_map(|l| l.strip_prefix("format|duration=")).and_then(|d| d.parse().ok()).unwrap_or(0.0);
    if streams.is_empty() {
        bail!("bestand niet leesbaar: {}", file.display());
    }
    Ok((streams.join("+"), secs))
}

async fn wait_until<F, Fut, T>(timeout: Duration, mut f: F) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Option<T>>,
{
    let start = Instant::now();
    loop {
        if let Some(v) = f().await {
            return Ok(v);
        }
        if start.elapsed() > timeout {
            bail!("time-out na {} s", timeout.as_secs());
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
}

/// The example website's service, as Claude maps it; used when the Claude API is not available.
fn fallback_service() -> Value {
    let simple = |id: &str, kind: &str, label: &str| json!({"id": id, "kind": kind, "label": label});
    let psalm = |id: &str, kind: &str, label: &str, n: u32, verses: &[u32]| {
        json!({"id": id, "kind": kind, "label": label, "psalm": n, "verses": verses})
    };
    json!({
        "id": "2026-10-18 16:00", "date": "2026-10-18", "label": "Middagdienst", "time": "16:00",
        "text": {"kind": "catechismus", "label": "Heidelbergse Catechismus", "questions": [57]},
        "theme": "Dit mijn lichaam", "points": ["Nu ik leef", "Als ik sterf", "Als Jezus terugkomt"],
        "items": [
            simple("i1", "welkom", "Startscherm"),
            psalm("i2", "psalm", "Voorzang", 55, &[1]),
            psalm("i3", "psalm", "Zingen", 49, &[6]),
            psalm("i4", "psalm", "Zingen", 16, &[6]),
            simple("i5", "gebed", "Gebed"),
            json!({"id": "i6", "kind": "lezing", "label": "Schriftlezing", "passage": {"book": 46, "chapter": 6, "from": 15, "to": 20}}),
            json!({"id": "i7", "kind": "catechismus", "label": "Heidelbergse Catechismus", "questions": [57]}),
            json!({"id": "i8", "kind": "collecte", "label": "Collecte", "text": "1. GDC Noodhulp en Rampenfonds\n2. Beheer en onderhoud Levensbron\n3. Aflossing Bouwfonds"}),
            psalm("i9", "psalm", "Zingen", 73, &[12, 13]),
            simple("i10", "preek", "Prediking"),
            psalm("i11", "psalm", "Tussenzang", 17, &[8]),
            simple("i12", "preek", "Prediking"),
            simple("i13", "gebed", "Dankgebed"),
            psalm("i14", "slotzang", "Slotzang", 73, &[14]),
            simple("i16", "zegen", "Zegen"),
            simple("i15", "einde", "Aftiteling"),
        ]
    })
}
