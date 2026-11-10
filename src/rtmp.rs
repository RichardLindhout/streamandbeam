//! The YouTube branch of a live stream: its own small pipeline (FLV over RTMP), fed with the
//! encoded picture and sound of the main one. A failure here (wrong key, no internet, the
//! server drops) never stops the recording or the preview: the branch is closed, the operator
//! sees a warning, and it tries again every 30 seconds for as long as the stream is live.

use anyhow::{Context, Result, anyhow};
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Wait this long before trying YouTube again.
pub const RETRY: Duration = Duration::from_secs(30);
/// Counts as broken when nothing went out for this long.
const STALL: Duration = Duration::from_secs(20);
/// Counts as working once this much went out (more than the RTMP handshake).
const WORKING_BYTES: u64 = 100_000;

/// What the operator page shows about YouTube.
#[derive(Debug, Clone)]
pub struct Report {
    /// "verbinden", "ok" or "fout".
    pub state: &'static str,
    /// Why it does not work, in plain words; empty when fine.
    pub error: String,
    /// When it tries again (Unix seconds); 0 = not waiting.
    pub retry_at: i64,
}

struct Pipe {
    pipeline: gst::Pipeline,
    video: gst_app::AppSrc,
    audio: gst_app::AppSrc,
    sink: gst::Element,
    /// Running time of the first key frame sent: from there the stream starts at `base`.
    offset: Option<gst::ClockTime>,
    /// Where the stream carries on after the main pipeline started again (0 at first).
    base: gst::ClockTime,
    /// The latest timestamp sent.
    last: gst::ClockTime,
    /// Bytes sent so far and when that last grew.
    sent: u64,
    progress: Instant,
}

pub struct Sender {
    location: String,
    /// The stream key, kept out of every text the operator sees.
    key: String,
    pipe: Mutex<Option<Pipe>>,
    report: Mutex<Report>,
    stop: AtomicBool,
}

impl Sender {
    /// Starts sending to `location` (rtmp://…/<key>) in the background.
    pub fn start(location: String, key: String) -> Arc<Sender> {
        let sender = Arc::new(Sender {
            location,
            key,
            pipe: Mutex::new(None),
            report: Mutex::new(Report { state: "verbinden", error: String::new(), retry_at: 0 }),
            stop: AtomicBool::new(false),
        });
        let s = sender.clone();
        std::thread::spawn(move || s.supervise());
        sender
    }

    pub fn location(&self) -> &str {
        &self.location
    }

    pub fn report(&self) -> Report {
        self.report.lock().unwrap().clone()
    }

    /// Ends the stream to YouTube properly and stops trying.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
        let pipe = self.pipe.lock().unwrap().take();
        if let Some(p) = pipe {
            finish(p, true);
        }
    }

    /// Takes encoded picture (`video`) or sound from the main pipeline. Never blocks: when
    /// YouTube is slow the oldest data is dropped, the recording does not wait for it.
    pub fn push(&self, sample: &gst::Sample, video: bool) {
        let mut guard = self.pipe.lock().unwrap();
        let Some(p) = guard.as_mut() else { return };
        let (Some(buf), Some(caps)) = (sample.buffer(), sample.caps_owned()) else { return };
        let segment = sample.segment().and_then(|s| s.downcast_ref::<gst::ClockTime>());
        let running = |t: Option<gst::ClockTime>| match segment {
            Some(seg) => t.and_then(|t| seg.to_running_time(t)),
            None => t,
        };
        let (pts, dts) = (running(buf.pts()), running(buf.dts()));
        if p.offset.is_none() {
            // Start with a key frame, and the sound from that moment on.
            if !video || buf.flags().contains(gst::BufferFlags::DELTA_UNIT) {
                return;
            }
            p.offset = dts.or(pts);
        }
        let Some(offset) = p.offset else { return };
        let base = p.base;
        let shift = |t: gst::ClockTime| t.checked_sub(offset).map(|t| t + base);
        let Some(pts) = pts.and_then(shift) else { return };
        p.last = p.last.max(pts);
        let mut out = buf.copy();
        {
            let b = out.get_mut().unwrap();
            b.set_pts(pts);
            b.set_dts(dts.and_then(shift));
        }
        let src = if video { &p.video } else { &p.audio };
        let _ = src.push_sample(&gst::Sample::builder().buffer(&out).caps(&caps).build());
    }

    /// The main pipeline started again, with its clock back at 0: carry on right after what
    /// was sent, from its first key frame, on the same connection.
    pub fn rebase(&self) {
        if let Some(p) = self.pipe.lock().unwrap().as_mut() {
            p.offset = None;
            p.base = p.last + gst::ClockTime::from_mseconds(40);
        }
    }

    /// Keeps the branch going: notices failures, closes the broken pipeline and opens a new
    /// one after a while.
    fn supervise(self: Arc<Self>) {
        let mut retry = Instant::now();
        while !self.stop.load(Ordering::SeqCst) {
            if let Some(why) = self.check() {
                let pipe = self.pipe.lock().unwrap().take();
                if let Some(p) = pipe {
                    finish(p, false);
                }
                eprintln!("YouTube: {why}");
                retry = Instant::now() + RETRY;
                *self.report.lock().unwrap() =
                    Report { state: "fout", error: why, retry_at: chrono::Utc::now().timestamp() + RETRY.as_secs() as i64 };
            }
            if self.pipe.lock().unwrap().is_none() && Instant::now() >= retry {
                match self.open() {
                    Ok(p) => {
                        let mut guard = self.pipe.lock().unwrap();
                        if self.stop.load(Ordering::SeqCst) {
                            drop(guard);
                            finish(p, false);
                            return;
                        }
                        *guard = Some(p);
                        // A warning stays until YouTube works again.
                        self.report.lock().unwrap().retry_at = 0;
                    }
                    Err(e) => {
                        retry = Instant::now() + RETRY;
                        *self.report.lock().unwrap() = Report {
                            state: "fout",
                            error: self.hide_key(&format!("{e:#}")),
                            retry_at: chrono::Utc::now().timestamp() + RETRY.as_secs() as i64,
                        };
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(500));
        }
    }

    fn open(&self) -> Result<Pipe> {
        let pipeline = build(&self.location)?;
        let app = |name: &str| pipeline.by_name(name).and_then(|e| e.downcast::<gst_app::AppSrc>().ok()).ok_or_else(|| anyhow!("{name} ontbreekt"));
        let (video, audio) = (app("v")?, app("a")?);
        let sink = pipeline.by_name("out").ok_or_else(|| anyhow!("rtmp2sink ontbreekt"))?;
        pipeline.set_state(gst::State::Playing).context("YouTube-pijplijn starten mislukt")?;
        Ok(Pipe {
            pipeline,
            video,
            audio,
            sink,
            offset: None,
            base: gst::ClockTime::ZERO,
            last: gst::ClockTime::ZERO,
            sent: 0,
            progress: Instant::now(),
        })
    }

    /// Why the branch broke, if it did: an error, the server closing, or nothing going out.
    fn check(&self) -> Option<String> {
        let mut guard = self.pipe.lock().unwrap();
        let p = guard.as_mut()?;
        if let Some(msg) = p.pipeline.bus().and_then(|b| b.pop_filtered(&[gst::MessageType::Error, gst::MessageType::Eos])) {
            return Some(match msg.view() {
                gst::MessageView::Error(err) => self.explain(&err.error().to_string(), &err.debug().unwrap_or_default()),
                _ => "YouTube sloot de verbinding".into(),
            });
        }
        let sent = p
            .sink
            .property::<Option<gst::Structure>>("stats")
            .and_then(|s| s.get::<u64>("out-bytes-total").ok())
            .unwrap_or(0);
        if sent > p.sent {
            p.sent = sent;
            p.progress = Instant::now();
            if sent >= WORKING_BYTES {
                *self.report.lock().unwrap() = Report { state: "ok", error: String::new(), retry_at: 0 };
            }
        }
        (p.progress.elapsed() > STALL).then(|| "er gaat niets meer naar YouTube (geen verbinding?)".to_string())
    }

    /// The error of the RTMP element in a few plain words, without the stream key.
    fn explain(&self, error: &str, debug: &str) -> String {
        let all = format!("{error} {debug}").to_lowercase();
        let why = if ["resolv", "refused", "unreachable", "timed out", "timeout", "could not connect", "failed to connect"].iter().any(|w| all.contains(w)) {
            "geen verbinding met YouTube (internet?)"
        } else if ["closed", "reset", "broken pipe", "eof", "could not write", "code 44"].iter().any(|w| all.contains(w)) {
            // Code 44 is GLib's "connection closed": the server went away mid-stream.
            "YouTube verbrak de verbinding"
        } else {
            "YouTube weigert de uitzending (streamsleutel?)"
        };
        // The start of the error says most, e.g. "Connection refused"; GStreamer's general
        // "Could not …" adds nothing, and the debug text is only source files and elements.
        let detail = error.split(": ").next().unwrap_or_default().trim();
        let generic = ["could not", "failed to"].iter().any(|w| detail.to_lowercase().starts_with(w));
        if detail.is_empty() || generic {
            self.hide_key(why)
        } else {
            self.hide_key(&format!("{why} — {detail}"))
        }
    }

    fn hide_key(&self, text: &str) -> String {
        if self.key.is_empty() { text.to_string() } else { text.replace(&self.key, "…") }
    }
}

/// The YouTube pipeline: encoded picture and sound in, FLV over RTMP to `location` out.
fn build(location: &str) -> Result<gst::Pipeline> {
    // Not live: the muxer simply waits for picture and sound, no clock involved. The queues
    // hold at most 10 s; more means YouTube does not keep up, and old data goes.
    let src = "format=time is-live=false max-bytes=0 max-buffers=0 max-time=10000000000 leaky-type=downstream";
    // The caps up front, so the muxer's picture and sound pads can be linked before the first
    // sample (which brings the full caps, with codec data) arrives.
    let desc = format!(
        "appsrc name=v {src} caps=video/x-h264,stream-format=avc,alignment=au ! queue ! mux.video \
         appsrc name=a {src} caps=audio/mpeg,mpegversion=4,stream-format=raw ! queue ! mux.audio \
         flvmux name=mux streamable=true ! rtmp2sink name=out sync=false location=\"{location}\""
    );
    gst::parse::launch(&desc)
        .context("YouTube-pijplijn bouwen mislukt")?
        .downcast::<gst::Pipeline>()
        .map_err(|_| anyhow!("geen pijplijn"))
}

/// Closes a YouTube pipeline; `clean` first ends the stream so YouTube knows it is over.
fn finish(p: Pipe, clean: bool) {
    if clean {
        let _ = p.video.end_of_stream();
        let _ = p.audio.end_of_stream();
        if let Some(bus) = p.pipeline.bus() {
            bus.timed_pop_filtered(gst::ClockTime::from_seconds(3), &[gst::MessageType::Eos, gst::MessageType::Error]);
        }
    }
    let _ = p.pipeline.set_state(gst::State::Null);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sender(key: &str) -> Sender {
        Sender {
            location: format!("rtmp://example/live2/{key}"),
            key: key.into(),
            pipe: Mutex::new(None),
            report: Mutex::new(Report { state: "verbinden", error: String::new(), retry_at: 0 }),
            stop: AtomicBool::new(false),
        }
    }

    #[test]
    fn pipeline_links() {
        gst::init().unwrap();
        // Nothing connects before the pipeline plays.
        let p = build("rtmp://127.0.0.1:1/live2/test").unwrap();
        assert!(p.by_name("v").is_some() && p.by_name("a").is_some() && p.by_name("out").is_some());
    }

    #[test]
    fn errors_never_show_the_key() {
        let s = sender("abcd-1234");
        let text = s.explain("Could not open resource", "gstrtmp2sink.c(1): rtmp://example/live2/abcd-1234: Connection refused");
        assert!(!text.contains("abcd-1234"), "{text}");
        assert!(text.starts_with("geen verbinding met YouTube"), "{text}");
        // What rtmp2sink says when the server goes away (Windows, Dutch): a dropped line,
        // without the element path.
        let text = s.explain("Could not write to resource.", "/GstPipeline:pipeline1/GstRtmp2Sink:out:\ndomain g-io-error-quark, code 44");
        assert_eq!(text, "YouTube verbrak de verbinding", "{text}");
        let text = s.explain(
            "Connection refused: Could not connect to 127.0.0.1: Kan geen verbinding maken",
            "../gst/rtmp2/gstrtmp2sink.c(1153): send_connect_error (): /GstPipeline:pipeline3/GstRtmp2Sink:out",
        );
        assert_eq!(text, "geen verbinding met YouTube (internet?) — Connection refused", "{text}");
        assert!(text.ends_with("Connection refused"), "{text}");
        let text = s.explain("Failed to write", "connection closed by peer");
        assert!(text.starts_with("YouTube verbrak de verbinding"), "{text}");
    }
}
