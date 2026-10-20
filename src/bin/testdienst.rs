//! Testdienst: runs a complete service automatically on one computer and checks it.
//!
//! It starts the nepcamera (recording of an earlier service), a fake YouTube (ffmpeg as
//! RTMP receiver) and streamandbeam, reads the liturgy from the example website, goes
//! live, clicks through every slide, tests the "no sound" warning and finally checks the
//! camera positions, the recording and what "YouTube" received.
//!
//!     cargo build --release && target/release/testdienst [--snel]

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
    } else {
        println!("  (testdata/dienst.mp4 ontbreekt: nepcamera gebruikt een testbeeld)");
    }
    procs.0.push(cam.stdout(log("nepcamera")?).stderr(log("nepcamera-fouten")?).spawn().context("nepcamera starten")?);

    let youtube = out.join("youtube.flv");
    let ffmpeg = Command::new("ffmpeg")
        .args(["-loglevel", "error", "-listen", "1", "-i", &format!("{RTMP}/testsleutel"), "-c", "copy", "-y"])
        .arg(&youtube)
        .stdout(Stdio::null())
        .stderr(log("youtube")?)
        .spawn()
        .context("ffmpeg starten (nodig als nep-YouTube)")?;
    procs.0.push(ffmpeg);

    let api_key_ok = std::env::var("ANTHROPIC_API_KEY").is_ok();
    let mut app = Command::new(bin_dir.join("streamandbeam"));
    app.args(["--poort", "18080", "--liturgie-url", &format!("http://{CAMERA_HTTP}/kerkdiensten"), "--data"])
        .arg(out.join("data"));
    if !api_key_ok {
        app.arg("--oude-spelling");
    }
    procs.0.push(app.stdout(log("streamandbeam")?).stderr(log("streamandbeam-fouten")?).spawn()?);

    let http = reqwest::Client::new();
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
        .map(|it| it["ref"].as_str().unwrap_or("").replace(' ', ""))
        .collect();
    r.check(
        psalms == ["55:1", "49:6", "16:6", "73:12,13", "17:8", "73:14"],
        format!("psalmen in de juiste volgorde: {}", psalms.join(" / ")),
    );
    let slides = state["slides"].as_array().cloned().unwrap_or_default();
    let missing: Vec<String> = slides
        .iter()
        .filter(|s| s["lines"].as_array().is_some_and(|l| l.iter().any(|l| l.as_str().unwrap_or("").starts_with('('))))
        .map(|s| s["header"].as_str().unwrap_or("").to_string())
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
    let preview = wait_until(Duration::from_secs(10), || async {
        let jpeg = http.get(format!("{APP}/preview.jpg")).send().await.ok()?.bytes().await.ok()?;
        (jpeg.len() > 1000).then_some(jpeg.len())
    })
    .await;
    r.check(preview.is_ok(), format!("voorbeeldbeeld voor de bediener ({} kB)", preview.unwrap_or(0) / 1024));

    println!("\n4. Hele dienst doorklikken");
    let pause = Duration::from_millis(if fast { 1200 } else { 3000 });
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
    http.post(format!("{APP}/api/stream")).json(&json!({"mode": "stop"})).send().await?;
    tokio::time::sleep(Duration::from_secs(2)).await;

    println!("\n5. Camera-posities");
    let expected = expected_presets(&slides);
    let calls: Vec<u32> = http.get(format!("http://{CAMERA_HTTP}/log")).send().await?.json().await?;
    r.check(calls == expected, format!("camera kreeg {calls:?}, verwacht {expected:?}"));

    println!("\n6. Opname en YouTube-ontvangst");
    let min = slides.len() as f64 * pause.as_secs_f64() * 0.8;
    for (what, file) in [("opname", PathBuf::from(&recording)), ("YouTube", youtube.clone())] {
        match probe(&file) {
            Ok((streams, secs)) => r.check(
                streams.contains("h264") && streams.contains("aac") && secs >= min,
                format!("{what}: {streams}, {secs:.0} s (minimaal {min:.0} s)"),
            ),
            Err(e) => r.check(false, format!("{what}: {e:#}")),
        }
    }
    let sheet = out.join("overzicht.jpg");
    let tiles = Command::new("ffmpeg")
        .args(["-loglevel", "error", "-y", "-i"])
        .arg(&youtube)
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
/// (overzicht 1, orgel 2, preekstoel 3, doopvont 4), without repeats.
fn expected_presets(slides: &[Value]) -> Vec<u32> {
    let mut out: Vec<u32> = Vec::new();
    for s in slides {
        let p = match s["kind"].as_str().unwrap_or("") {
            "welkom" => 1,
            "psalm" | "slotzang" => 2,
            "gebed" => 4,
            _ => 3,
        };
        if out.last() != Some(&p) {
            out.push(p);
        }
    }
    out
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

/// The example website's service, as Claude reads it; used when the Claude API is not available.
fn fallback_service() -> Value {
    let item = |id: &str, kind: &str, label: &str, r: &str| json!({"id": id, "kind": kind, "label": label, "ref": r});
    json!({
        "id": "2026-10-18 16:00", "date": "2026-10-18", "label": "Middagdienst", "time": "16:00",
        "textRef": "Heidelbergse Catechismus, vraag en antwoord 57",
        "theme": "Dit mijn lichaam", "points": ["Nu ik leef", "Als ik sterf", "Als Jezus terugkomt"],
        "items": [
            item("i1", "welkom", "Welkom", ""),
            item("i2", "psalm", "Voorzang", "55: 1"),
            item("i3", "psalm", "Zingen", "49: 6"),
            item("i4", "gebed", "Gebed", ""),
            item("i5", "lezing", "Schriftlezing", "1 Korinthe 6:15-20"),
            item("i6", "psalm", "Zingen", "16: 6"),
            item("i7", "preek", "Prediking", ""),
            item("i8", "psalm", "Tussenzang", "73: 12, 13"),
            item("i9", "preek", "Prediking", ""),
            item("i10", "psalm", "Zingen", "17: 8"),
            item("i11", "gebed", "Dankgebed", ""),
            item("i12", "slotzang", "Slotzang", "73: 14"),
        ]
    })
}
