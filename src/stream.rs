//! The video engine that replaces OBS: camera + USB sound + our overlay, encoded to H.264
//! and AAC, recorded to an MP4 file and, while live, sent to YouTube. Built on GStreamer.
//!
//! The engine keeps the mode the operator chose running: when the pipeline breaks (the camera
//! drops, a setting is wrong) it builds a new one a few seconds later. YouTube has its own
//! small pipeline (rtmp.rs), so a problem there never stops the recording or the preview.

use crate::render::{HEIGHT, WIDTH};
use crate::rtmp::Sender;
use anyhow::{Context, Result, anyhow};
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// Below this peak level (dB) we count it as silence.
const SILENCE_DB: f64 = -60.0;
/// Warn after this long without any sound.
const SILENCE_WARN: Duration = Duration::from_secs(10);
/// Warn when the sound input has sent no measurements for this long.
const NO_SIGNAL_WARN: Duration = Duration::from_secs(3);
/// After a failure, build a new pipeline this much later.
const RESTART: Duration = Duration::from_secs(5);
/// A new pipeline that ran this long without trouble clears the old error.
const RECOVERED: Duration = Duration::from_secs(10);
/// The camera counts as gone when no picture came for this long.
const CAMERA_GONE: Duration = Duration::from_secs(3);
/// A recording this small holds no picture yet (only the file header) and is removed.
const EMPTY_RECORDING: u64 = 4096;
/// AAC encoders, best first: FFmpeg's, Windows' own (Media Foundation), then the others.
const AAC_ENCODERS: [&str; 5] = ["avenc_aac", "mfaacenc", "voaacenc", "fdkaacenc", "faac"];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// IP address of the PTZOptics camera; empty = test picture.
    pub camera_ip: String,
    /// RTSP address of the camera; empty = rtsp://<camera_ip>:554/1 (PTZOptics main stream).
    pub camera_rtsp: String,
    /// GStreamer audio source; empty = the default sound input, "test" = test tone.
    /// On Windows a USB device can be chosen with e.g. `wasapisrc device="..."`.
    pub audio_source: String,
    /// GStreamer H.264 encoder. On Windows `mfh264enc` (any GPU), `nvh264enc` (NVIDIA)
    /// or `qsvh264enc` (Intel) take load off the processor.
    pub video_encoder: String,
    pub rtmp_url: String,
    pub stream_key: String,
    pub record_dir: String,
    pub presets: Presets,
    /// Shown as a QR code at the collecte and before/after the service.
    pub qr_link: String,
    /// Seconds the organ camera with "Psalm 42 : 5" shows before the verse (0 = off).
    pub intro_seconds: u64,
    /// Seconds the sermon theme shows before the camera turns to the predikant (0 = off).
    pub theme_seconds: u64,
    /// Go live automatically this many minutes before a service starts, with the start
    /// screen (0 = off). The church goes live about 15 minutes before.
    pub auto_start_minutes: u64,
    /// Stop automatically this many seconds after the end screen (with the church photo)
    /// appears, i.e. 20 s after "Aftiteling" is chosen (0 = off). The church stops after
    /// about 2 to 2.5 minutes.
    pub auto_stop_seconds: u64,
    /// Version of these settings, to change old saved values once (see Director::new).
    pub settings_version: u32,
    /// OAuth client ("Desktop app") from Google Cloud Console, for logging in to YouTube.
    pub youtube_client_id: String,
    pub youtube_client_secret: String,
    /// public, unlisted or private.
    pub youtube_privacy: String,
    /// Test broadcasts: private, with "TEST" in the title.
    pub youtube_test: bool,
    /// Listening along: "uit", "suggestie" (button blinks) or "automatisch".
    pub autopilot: String,
    /// The YouTube channel (id) whose live stream is checked for sound and shown on the
    /// watch page.
    pub youtube_channel: String,
    /// Areas of the stream panel the text keeps clear of; a camera area is cut out so the
    /// camera shows through. Default: the large camera circle.
    pub overlay_zones: Vec<crate::render::Zone>,
}

impl Settings {
    /// The parts the pipeline is built from: when one changes, the engine starts again.
    pub fn same_pipeline(&self, other: &Settings) -> bool {
        (&self.camera_ip, &self.camera_rtsp, &self.audio_source, &self.video_encoder, &self.record_dir)
            == (&other.camera_ip, &other.camera_rtsp, &other.audio_source, &other.video_encoder, &other.record_dir)
    }

    /// A camera is set up (else the test picture shows).
    pub fn has_camera(&self) -> bool {
        !(self.camera_ip.is_empty() && self.camera_rtsp.is_empty())
    }
}

/// PTZOptics camera positions (preset numbers as saved in the camera).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Presets {
    pub overzicht: u32,
    pub orgel: u32,
    pub preekstoel: u32,
    pub doopvont: u32,
}

impl Default for Presets {
    fn default() -> Self {
        Presets { overzicht: 1, orgel: 2, preekstoel: 3, doopvont: 4 }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            camera_ip: String::new(),
            camera_rtsp: String::new(),
            audio_source: String::new(),
            video_encoder: "x264enc tune=zerolatency speed-preset=veryfast bitrate=4500 key-int-max=50".into(),
            rtmp_url: "rtmp://a.rtmp.youtube.com/live2".into(),
            stream_key: String::new(),
            record_dir: "opnames".into(),
            presets: Presets::default(),
            qr_link: "https://www.hhgtholen.nl".into(),
            intro_seconds: 12,
            theme_seconds: 20,
            auto_start_minutes: 15,
            auto_stop_seconds: 150,
            settings_version: 0,
            youtube_client_id: String::new(),
            youtube_client_secret: String::new(),
            youtube_privacy: "public".into(),
            youtube_test: false,
            autopilot: "uit".into(),
            youtube_channel: "UCv0FTJ5pf3aXF7fhVYLwe2g".into(),
            overlay_zones: vec![crate::render::Zone::camera_circle()],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Stop,
    /// Only the preview for the operator.
    Voorbeeld,
    /// Preview + recording to a file.
    Opnemen,
    /// Preview + recording + YouTube (when set up; else only the recording).
    Live,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// The mode the operator chose; after a failure the engine keeps trying to get there.
    pub mode: Mode,
    /// What went wrong with the picture, sound or recording; empty when fine.
    pub error: String,
    /// Peak sound level in dB (0 = maximum), for the meter in the operator page.
    pub audio_db: f64,
    /// The file being recorded now (full path); empty when not recording.
    pub recording: String,
    /// Shown as a big red warning in the operator page; empty when the sound is fine.
    pub audio_warning: String,
    /// When the current mode started (Unix seconds, 0 = stopped), for the LIVE clock.
    pub since: i64,
    /// A camera is set up; false = the test picture.
    pub camera: bool,
    /// The camera picture came in during the last seconds.
    pub camera_live: bool,
    /// While live: "geen" (YouTube not set up, only the recording), "verbinden", "ok" or
    /// "fout" (the recording goes on); empty when not live.
    pub youtube: String,
    /// Why YouTube does not work, without the stream key; empty when fine.
    pub youtube_error: String,
    /// When YouTube is tried again (Unix seconds); 0 = not waiting.
    pub youtube_retry_at: i64,
}

/// When the sound input last sent a measurement and when it last heard something.
struct AudioWatch {
    started: Instant,
    last_level: Option<Instant>,
    last_sound: Option<Instant>,
}

impl AudioWatch {
    fn new() -> Self {
        AudioWatch { started: Instant::now(), last_level: None, last_sound: None }
    }

    fn warning(&self) -> String {
        let now = Instant::now();
        let since_level = now - self.last_level.unwrap_or(self.started);
        if since_level > NO_SIGNAL_WARN {
            return "GEEN GELUIDSINVOER — er komt geen signaal van de USB-geluidskaart binnen. \
                    Zit de USB-kabel erin en is de juiste geluidsbron ingesteld?"
                .into();
        }
        let since_sound = now - self.last_sound.unwrap_or(self.started);
        if since_sound > SILENCE_WARN {
            return format!(
                "GEEN GELUID — al {} seconden stilte op de USB-geluidsinvoer. Staat de mixer aan en zijn de kanalen open?",
                since_sound.as_secs()
            );
        }
        String::new()
    }
}

struct Running {
    pipeline: gst::Pipeline,
    overlay: gst_app::AppSrc,
    /// Set by the bus thread when this pipeline failed; the supervisor then builds a new one.
    broken: Arc<AtomicBool>,
    /// Set by the bus thread at the end of the stream (or an error), for a clean stop.
    ended: Arc<(Mutex<bool>, Condvar)>,
    recording: String,
    started: Instant,
}

/// Where a live stream goes besides the recording.
enum Upload {
    /// Nowhere: YouTube is not set up, only the recording runs.
    None,
    /// YouTube is wanted but there is no address yet (the broadcast could not be made); the
    /// director tries again.
    Waiting { error: String, retry_at: i64 },
    Sending(Arc<Sender>),
}

pub struct Engine {
    running: Mutex<Option<Running>>,
    /// The mode the operator chose and its settings; kept running by the supervisor.
    want: Mutex<(Mode, Settings)>,
    /// One switch at a time: the operator, a settings change or the supervisor.
    switching: Mutex<()>,
    /// When the supervisor may build a new pipeline after a failure.
    retry_at: Mutex<Option<Instant>>,
    status: Mutex<Status>,
    overlay: Mutex<Option<gst::Buffer>>,
    preview: Mutex<Vec<u8>>,
    /// The camera alone (without overlay) and when its last picture came.
    camera: Mutex<(Vec<u8>, Option<Instant>)>,
    audio: Mutex<AudioWatch>,
    listener: Mutex<Option<Arc<crate::listen::Listener>>>,
    upload: Mutex<Upload>,
    /// The last problem printed.
    logged: Mutex<String>,
}

impl Engine {
    pub fn new() -> Result<Arc<Engine>> {
        gst::init()?;
        let engine = Arc::new(Engine {
            running: Mutex::new(None),
            want: Mutex::new((Mode::Stop, Settings::default())),
            switching: Mutex::new(()),
            retry_at: Mutex::new(None),
            status: Mutex::new(Status {
                mode: Mode::Stop,
                error: String::new(),
                audio_db: -100.0,
                recording: String::new(),
                audio_warning: String::new(),
                since: 0,
                camera: false,
                camera_live: false,
                youtube: String::new(),
                youtube_error: String::new(),
                youtube_retry_at: 0,
            }),
            overlay: Mutex::new(None),
            preview: Mutex::new(Vec::new()),
            camera: Mutex::new((Vec::new(), None)),
            audio: Mutex::new(AudioWatch::new()),
            listener: Mutex::new(None),
            upload: Mutex::new(Upload::None),
            logged: Mutex::new(String::new()),
        });
        // Keep feeding the current overlay: the compositor needs a steady flow of frames.
        let e = engine.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_millis(200));
            e.push_overlay();
        });
        // Keep the chosen mode running.
        let e = engine.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_millis(500));
            e.keep_running();
        });
        Ok(engine)
    }

    /// The listener gets a copy of the sound while the engine runs.
    pub fn set_listener(&self, l: Arc<crate::listen::Listener>) {
        *self.listener.lock().unwrap() = Some(l);
    }

    pub fn status(&self) -> Status {
        let mut st = self.status.lock().unwrap().clone();
        st.camera = self.want.lock().unwrap().1.has_camera();
        // Sound and picture are only measured while the engine runs.
        if st.mode != Mode::Stop {
            st.audio_warning = self.audio.lock().unwrap().warning();
            st.camera_live = self.camera.lock().unwrap().1.is_some_and(|t| t.elapsed() < CAMERA_GONE);
        } else {
            st.audio_db = -100.0;
        }
        if st.mode == Mode::Live {
            match &*self.upload.lock().unwrap() {
                Upload::None => st.youtube = "geen".into(),
                Upload::Waiting { error, retry_at } => {
                    st.youtube = "fout".into();
                    st.youtube_error = error.clone();
                    st.youtube_retry_at = *retry_at;
                }
                Upload::Sending(s) => {
                    let r = s.report();
                    st.youtube = r.state.into();
                    st.youtube_error = r.error;
                    st.youtube_retry_at = r.retry_at;
                }
            }
        }
        st
    }

    /// Live and sending to YouTube (not only recording).
    pub fn sending_to_youtube(&self) -> bool {
        self.want.lock().unwrap().0 == Mode::Live && matches!(*self.upload.lock().unwrap(), Upload::Sending(_))
    }

    /// The composed picture as it goes out (camera + overlay), small.
    pub fn preview_jpeg(&self) -> Vec<u8> {
        self.preview.lock().unwrap().clone()
    }

    /// The camera alone, before the overlay: what the camera sees right now.
    pub fn camera_jpeg(&self) -> Vec<u8> {
        self.camera.lock().unwrap().0.clone()
    }

    /// Replaces the overlay with new straight-alpha RGBA pixels.
    pub fn set_overlay(&self, rgba: Vec<u8>) {
        *self.overlay.lock().unwrap() = Some(gst::Buffer::from_mut_slice(rgba));
        self.push_overlay();
    }

    fn push_overlay(&self) {
        let Some(buf) = self.overlay.lock().unwrap().clone() else { return };
        let running = self.running.lock().unwrap();
        if let Some(r) = running.as_ref() {
            // A shallow copy so each push gets its own timestamps; the pixels are shared.
            let mut b = buf.copy();
            b.get_mut().unwrap().set_duration(gst::ClockTime::from_mseconds(400));
            let _ = r.overlay.push_buffer(b);
        }
    }

    /// Switches to `mode` (stopping first closes the recording properly) and keeps it
    /// running: after a failure the supervisor builds a new pipeline every few seconds. An
    /// error is returned and also kept in the status.
    pub fn set_mode(self: &Arc<Self>, mode: Mode, settings: &Settings) -> Result<()> {
        let _one = self.switching.lock().unwrap();
        let before = std::mem::replace(&mut *self.want.lock().unwrap(), (mode, settings.clone())).0;
        *self.retry_at.lock().unwrap() = None;
        self.stop_pipeline();
        // YouTube only while live, and a new live stream starts on a new connection.
        if mode != Mode::Live || before != Mode::Live {
            self.replace_upload(Upload::None);
        }
        {
            let mut st = self.status.lock().unwrap();
            if mode != before {
                st.since = if mode == Mode::Stop { 0 } else { chrono::Utc::now().timestamp() };
            }
            st.mode = mode;
            st.error.clear();
        }
        if mode == Mode::Stop {
            return Ok(());
        }
        if mode == Mode::Live {
            self.send_to(&settings.rtmp_url, &settings.stream_key);
        }
        self.start(mode, settings).inspect_err(|e| self.failed(e))
    }

    /// Builds the pipeline again with new settings, in the mode it has now.
    pub fn restart(self: &Arc<Self>, settings: &Settings) -> Result<()> {
        let mode = self.want.lock().unwrap().0;
        self.set_mode(mode, settings)
    }

    /// The mode the operator chose (also while the engine is still trying to get there).
    pub fn mode(&self) -> Mode {
        self.want.lock().unwrap().0
    }

    /// While live: sends to `rtmp_url`/`key`, or nowhere when the key is empty (only the
    /// recording). The same address keeps its connection.
    pub fn send_to(&self, rtmp_url: &str, key: &str) {
        if self.mode() != Mode::Live {
            return;
        }
        if key.is_empty() {
            // A broadcast that could not be made yet stays waiting; the director tries again.
            if !matches!(*self.upload.lock().unwrap(), Upload::Waiting { .. }) {
                self.replace_upload(Upload::None);
            }
            return;
        }
        let location = format!("{}/{key}", rtmp_url.trim_end_matches('/'));
        if matches!(&*self.upload.lock().unwrap(), Upload::Sending(s) if s.location() == location) {
            return;
        }
        println!("uitzending: naar YouTube");
        self.replace_upload(Upload::Sending(Sender::start(location, key.to_string())));
    }

    /// While live: YouTube is wanted but cannot be reached yet (e.g. the broadcast could not
    /// be made). The recording goes on; `youtube_due` says when to try again.
    pub fn youtube_waiting(&self, error: String) {
        if self.mode() == Mode::Live {
            let retry_at = chrono::Utc::now().timestamp() + crate::rtmp::RETRY.as_secs() as i64;
            self.replace_upload(Upload::Waiting { error, retry_at });
        }
    }

    /// Live, waiting for YouTube, and time to try again.
    pub fn youtube_due(&self) -> bool {
        self.mode() == Mode::Live
            && matches!(*self.upload.lock().unwrap(), Upload::Waiting { retry_at, .. } if chrono::Utc::now().timestamp() >= retry_at)
    }

    /// Puts a new YouTube connection in place; the old one (if any) is ended outside the lock,
    /// so the recording never waits for it.
    fn replace_upload(&self, new: Upload) {
        let old = std::mem::replace(&mut *self.upload.lock().unwrap(), new);
        if let Upload::Sending(s) = old {
            s.stop();
        }
    }

    fn failed(&self, e: &anyhow::Error) {
        let text = format!("{e:#}");
        self.log(&text);
        self.status.lock().unwrap().error = text;
        *self.retry_at.lock().unwrap() = Some(Instant::now() + RESTART);
    }

    /// Prints a problem once, not again on every new try (a camera can be off all week).
    fn log(&self, text: &str) {
        let mut last = self.logged.lock().unwrap();
        if *last != text {
            eprintln!("uitzending: {text}");
            *last = text.to_string();
        }
    }

    /// Builds and starts the pipeline for `mode`.
    fn start(self: &Arc<Self>, mode: Mode, settings: &Settings) -> Result<()> {
        let aac = AAC_ENCODERS
            .into_iter()
            .find(|n| gst::ElementFactory::find(n).is_some())
            .ok_or_else(|| anyhow!("geen AAC-encoder in GStreamer (installeer de 'Complete' versie)"))?;
        let (desc, recording) = pipeline_description(mode, settings, aac)?;
        let pipeline = gst::parse::launch(&desc)
            .with_context(|| format!("pijplijn bouwen mislukt:\n{desc}"))?
            .downcast::<gst::Pipeline>()
            .map_err(|_| anyhow!("geen pijplijn"))?;
        let overlay = pipeline.by_name("overlay").unwrap().downcast::<gst_app::AppSrc>().unwrap();
        self.on_samples(&pipeline, "preview", |e, sample| {
            if let Some(map) = sample.buffer().and_then(|b| b.map_readable().ok()) {
                *e.preview.lock().unwrap() = map.to_vec();
            }
        });
        self.on_samples(&pipeline, "camera", |e, sample| {
            if let Some(map) = sample.buffer().and_then(|b| b.map_readable().ok()) {
                *e.camera.lock().unwrap() = (map.to_vec(), Some(Instant::now()));
            }
        });
        self.on_samples(&pipeline, "luister", |e, sample| {
            let listener = e.listener.lock().unwrap().clone();
            if let (Some(l), Some(map)) = (listener, sample.buffer().and_then(|b| b.map_readable().ok())) {
                let samples: Vec<f32> = map.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
                l.push_audio(&samples);
            }
        });
        // The encoded picture and sound for YouTube, while live.
        for (name, video) in [("ytv", true), ("yta", false)] {
            self.on_samples(&pipeline, name, move |e, sample| {
                if let Upload::Sending(s) = &*e.upload.lock().unwrap() {
                    s.push(sample, video);
                }
            });
        }
        // A sender that stays connected carries on after what it sent (this clock starts at 0).
        if let Upload::Sending(s) = &*self.upload.lock().unwrap() {
            s.rebase();
        }
        *self.audio.lock().unwrap() = AudioWatch::new();
        *self.camera.lock().unwrap() = (Vec::new(), None);
        if let Err(e) = pipeline.set_state(gst::State::Playing) {
            let why = pipeline
                .bus()
                .and_then(|b| b.pop_filtered(&[gst::MessageType::Error]))
                .and_then(|m| match m.view() {
                    gst::MessageView::Error(err) => Some(explain(&err)),
                    _ => None,
                })
                .unwrap_or_else(|| e.to_string());
            let _ = pipeline.set_state(gst::State::Null);
            remove_if_empty(&recording);
            return Err(anyhow!("starten mislukt: {why}"));
        }
        let broken = Arc::new(AtomicBool::new(false));
        let ended = Arc::new((Mutex::new(false), Condvar::new()));
        self.watch_bus(&pipeline, broken.clone(), ended.clone());
        if !recording.is_empty() {
            println!("opname: {recording}");
        }
        *self.running.lock().unwrap() =
            Some(Running { pipeline, overlay, broken, ended, recording: recording.clone(), started: Instant::now() });
        self.status.lock().unwrap().recording = recording;
        self.push_overlay();
        Ok(())
    }

    /// Calls `f` for every sample of the appsink `name`, if the pipeline has it.
    fn on_samples(self: &Arc<Self>, pipeline: &gst::Pipeline, name: &str, f: impl Fn(&Engine, &gst::Sample) + Send + Sync + 'static) {
        let Some(sink) = pipeline.by_name(name).and_then(|e| e.downcast::<gst_app::AppSink>().ok()) else { return };
        let e = Arc::downgrade(self);
        sink.set_callbacks(
            gst_app::AppSinkCallbacks::builder()
                .new_sample(move |sink| {
                    let sample = sink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                    if let Some(e) = e.upgrade() {
                        f(&e, &sample);
                    }
                    Ok(gst::FlowSuccess::Ok)
                })
                .build(),
        );
    }

    /// Ends the running pipeline: end-of-stream first, so the recording is closed properly.
    fn stop_pipeline(&self) {
        let Some(r) = self.running.lock().unwrap().take() else { return };
        if !r.broken.load(Ordering::SeqCst) {
            r.pipeline.send_event(gst::event::Eos::new());
            let (lock, cvar) = &*r.ended;
            let _ = cvar.wait_timeout_while(lock.lock().unwrap(), Duration::from_secs(5), |ended| !*ended);
        }
        let _ = r.pipeline.set_state(gst::State::Null);
        remove_if_empty(&r.recording);
        self.status.lock().unwrap().recording.clear();
    }

    /// Builds a new pipeline a few seconds after the running one broke or could not start,
    /// and clears the error once the new one runs well.
    fn keep_running(self: &Arc<Self>) {
        let (mode, settings) = self.want.lock().unwrap().clone();
        if mode == Mode::Stop {
            return;
        }
        let (broken, healthy) = match self.running.lock().unwrap().as_ref() {
            None => (true, false),
            Some(r) => {
                let broken = r.broken.load(Ordering::SeqCst);
                (broken, !broken && r.started.elapsed() > RECOVERED)
            }
        };
        if healthy {
            if self.retry_at.lock().unwrap().take().is_some() {
                self.status.lock().unwrap().error.clear();
                self.logged.lock().unwrap().clear();
            }
            return;
        }
        if !broken || self.retry_at.lock().unwrap().is_some_and(|t| Instant::now() < t) {
            return;
        }
        let Ok(_one) = self.switching.try_lock() else { return };
        // The operator may have switched in the meantime.
        if self.want.lock().unwrap().0 != mode {
            return;
        }
        self.stop_pipeline();
        if let Err(e) = self.start(mode, &settings) {
            self.failed(&e);
        } else {
            // Kept until the new pipeline has run a while (see `healthy`).
            *self.retry_at.lock().unwrap() = Some(Instant::now());
        }
    }

    /// Reads errors, the end of the stream and sound levels, for as long as the pipeline lives.
    fn watch_bus(self: &Arc<Self>, pipeline: &gst::Pipeline, broken: Arc<AtomicBool>, ended: Arc<(Mutex<bool>, Condvar)>) {
        let bus = pipeline.bus().unwrap();
        let e = Arc::downgrade(self);
        let weak = pipeline.downgrade();
        std::thread::spawn(move || {
            let end = || {
                *ended.0.lock().unwrap() = true;
                ended.1.notify_all();
            };
            while weak.upgrade().is_some() {
                let Some(msg) = bus.timed_pop(gst::ClockTime::from_mseconds(500)) else { continue };
                let Some(e) = e.upgrade() else { break };
                match msg.view() {
                    gst::MessageView::Error(err) => {
                        let ours = e.running.lock().unwrap().as_ref().is_some_and(|r| Some(&r.pipeline) == weak.upgrade().as_ref());
                        if ours {
                            let text = explain(&err);
                            e.log(&format!("{text} ({})", err.debug().unwrap_or_default()));
                            e.status.lock().unwrap().error = text;
                            *e.retry_at.lock().unwrap() = Some(Instant::now() + RESTART);
                        }
                        broken.store(true, Ordering::SeqCst);
                        end();
                        break;
                    }
                    gst::MessageView::Eos(_) => {
                        end();
                        break;
                    }
                    gst::MessageView::Element(el) => {
                        if let Some(s) = el.structure().filter(|s| s.name() == "level")
                            && let Ok(peaks) = s.get::<gst::glib::ValueArray>("peak")
                        {
                            let db = peaks.iter().filter_map(|v| v.get::<f64>().ok()).fold(-100.0, f64::max);
                            e.status.lock().unwrap().audio_db = db;
                            let mut audio = e.audio.lock().unwrap();
                            audio.last_level = Some(Instant::now());
                            if db > SILENCE_DB {
                                audio.last_sound = Some(Instant::now());
                            }
                        }
                    }
                    _ => {}
                }
            }
        });
    }
}

/// A pipeline error in a few words for the operator, with the element it came from.
fn explain(err: &gst::message::Error) -> String {
    let from = err.src().map(|s| s.name().to_string()).unwrap_or_default();
    let what = err.error().to_string();
    let lower = format!("{what} {}", err.debug().unwrap_or_default()).to_lowercase();
    if from.starts_with("rtspsrc") || lower.contains("rtsp") {
        return format!("Geen beeld van de camera: {what}. Staat de camera aan en klopt het RTSP-adres?");
    }
    if from.starts_with("filesink") || from == "rec" {
        return format!("Opnemen lukt niet: {what}. Is de map voor opnames bereikbaar en is de schijf niet vol?");
    }
    what
}

/// A recording without any picture (the pipeline broke right away) is only a file header.
fn remove_if_empty(file: &str) {
    if !file.is_empty() && std::fs::metadata(file).is_ok_and(|m| m.len() < EMPTY_RECORDING) {
        let _ = std::fs::remove_file(file);
    }
}

/// A sound input the operator can pick, with the pipeline text stored in the settings.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioDevice {
    pub name: String,
    pub source: String,
    pub default: bool,
}

/// The sound inputs Windows offers right now (USB sound cards, microphones). Recording what
/// the PC itself plays (loopback) is left out: that is never the church's sound.
pub fn audio_devices() -> Vec<AudioDevice> {
    let monitor = gst::DeviceMonitor::new();
    monitor.add_filter(Some("Audio/Source"), None);
    if monitor.start().is_err() {
        return Vec::new();
    }
    let mut list: Vec<AudioDevice> = monitor
        .devices()
        .into_iter()
        .filter_map(|d| {
            let props = d.properties()?;
            let get = |k: &str| props.get::<String>(k).ok();
            let flag = |k: &str| props.get::<bool>(k).unwrap_or(false);
            // "Default Audio Capture Device" follows the Windows default, which the empty setting
            // (autoaudiosrc) already offers as "Standaard".
            if flag("wasapi2.device.loopback") || flag("wasapi.device.loopback") || d.display_name().starts_with("Default Audio") {
                return None;
            }
            let source = match get("device.api")?.as_str() {
                "wasapi2" => format!("wasapi2src device=\"{}\"", get("device.id")?),
                "wasapi" => format!("wasapisrc device=\"{}\"", get("device.strid")?),
                _ => return None,
            };
            Some(AudioDevice { name: d.display_name().to_string(), source, default: flag("device.default") })
        })
        .collect();
    monitor.stop();
    // By name; one entry per device.
    list.sort_by(|a, b| a.name.cmp(&b.name));
    list.dedup_by(|a, b| a.source == b.source);
    list
}

/// The GStreamer pipeline for `mode`, and the file it records to (empty for the preview).
/// `aac` is the AAC encoder to use.
fn pipeline_description(mode: Mode, s: &Settings, aac: &str) -> Result<(String, String)> {
    let video = if !s.has_camera() {
        "videotestsrc is-live=true pattern=smpte".to_string()
    } else {
        let url = if s.camera_rtsp.is_empty() { format!("rtsp://{}:554/1", s.camera_ip) } else { s.camera_rtsp.clone() };
        format!("rtspsrc location=\"{url}\" latency=300 protocols=tcp ! decodebin")
    };
    let audio = match s.audio_source.as_str() {
        "" => "autoaudiosrc",
        "test" => "audiotestsrc is-live=true wave=sine volume=0.05",
        other => other,
    };
    // The camera goes to the compositor and, by itself and small, to the operator's header.
    // Both small pictures drop frames instead of slowing anything down.
    let small = "videorate drop-only=true ! video/x-raw,framerate=5/1 ! videoscale ! video/x-raw,width=640,height=360 ! jpegenc quality=70";
    let mut d = format!(
        "{video} ! videoconvert ! videoscale ! videorate ! video/x-raw,width={WIDTH},height={HEIGHT},framerate=25/1 ! tee name=ct \
         ct. ! queue ! comp.sink_0 \
         ct. ! queue leaky=downstream max-size-buffers=2 ! {small} ! appsink name=camera max-buffers=1 drop=true sync=false \
         appsrc name=overlay is-live=true format=time do-timestamp=true caps=video/x-raw,format=RGBA,width={WIDTH},height={HEIGHT},framerate=0/1 \
           ! videoconvert ! queue ! comp.sink_1 \
         compositor name=comp background=black ! video/x-raw,width={WIDTH},height={HEIGHT},framerate=25/1 ! videoconvert ! tee name=vt \
         vt. ! queue leaky=downstream max-size-buffers=2 ! {small} ! appsink name=preview max-buffers=1 drop=true sync=false \
         {audio} ! audioconvert ! audioresample ! audio/x-raw,rate=48000,channels=2 ! level interval=100000000 ! tee name=at \
         at. ! queue leaky=downstream max-size-time=2000000000 ! audioconvert ! audioresample \
           ! audio/x-raw,format=F32LE,rate=16000,channels=1 ! appsink name=luister sync=false max-buffers=100 drop=true "
    );
    if mode == Mode::Voorbeeld {
        d.push_str("at. ! queue ! fakesink sync=false");
        return Ok((d, String::new()));
    }
    std::fs::create_dir_all(&s.record_dir).with_context(|| format!("map voor opnames maken lukt niet: {}", s.record_dir))?;
    let file = PathBuf::from(&s.record_dir).join(format!("dienst-{}.mp4", chrono::Local::now().format("%Y-%m-%d-%H%M%S")));
    let file = std::path::absolute(&file).unwrap_or(file).to_string_lossy().replace('\\', "/");
    // MP4 in fragments of two seconds: after a crash or power cut the file plays up to the
    // last fragment; a normal stop rewrites it into an ordinary MP4.
    d.push_str(&format!(
        "vt. ! queue ! videoconvert ! video/x-raw,format=I420 ! {enc} ! h264parse config-interval=-1 \
           ! video/x-h264,stream-format=avc,alignment=au ! tee name=et \
         at. ! queue ! audioconvert ! {aac} bitrate=160000 ! aacparse ! tee name=ae \
         et. ! queue ! rec.video_0 ae. ! queue ! rec.audio_0 \
         mp4mux name=rec fragment-duration=2000 fragment-mode=first-moov-then-finalise ! filesink location=\"{file}\" ",
        enc = s.video_encoder
    ));
    if mode == Mode::Live {
        // Picture and sound for YouTube (rtmp.rs). Leaky, so a slow connection never holds up
        // the recording.
        let q = "queue leaky=downstream max-size-time=3000000000 max-size-buffers=0 max-size-bytes=0";
        d.push_str(&format!("et. ! {q} ! appsink name=ytv sync=false ae. ! {q} ! appsink name=yta sync=false "));
    }
    Ok((d, file))
}

/// Moves the PTZOptics camera to a saved position (HTTP-CGI interface).
pub async fn recall_preset(camera_ip: &str, preset: u32) {
    if camera_ip.is_empty() || preset == 0 {
        return;
    }
    let url = format!("http://{camera_ip}/cgi-bin/ptzctrl.cgi?ptzcmd&poscall&{preset}");
    let result = reqwest::Client::new().get(&url).timeout(Duration::from_secs(3)).send().await;
    if let Err(e) = result {
        eprintln!("camera naar positie {preset}: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(dir: &std::path::Path) -> Settings {
        Settings { record_dir: dir.to_string_lossy().into(), ..Default::default() }
    }

    #[test]
    fn records_mp4_with_picture_and_sound() {
        let dir = std::env::temp_dir().join(format!("sb-test-{}", std::process::id()));
        for mode in [Mode::Opnemen, Mode::Live] {
            let (d, file) = pipeline_description(mode, &settings(&dir), "mfaacenc").unwrap();
            assert!(file.ends_with(".mp4") && file.contains("/dienst-"), "{file}");
            assert!(d.contains("mp4mux name=rec fragment-duration=2000 fragment-mode=first-moov-then-finalise"), "{d}");
            assert!(d.contains("rec.video_0") && d.contains("rec.audio_0") && d.contains("mfaacenc bitrate=160000"), "{d}");
            // YouTube is never part of the recording pipeline: a failure there cannot stop it.
            assert!(!d.contains("rtmp") && !d.contains("flvmux"), "{d}");
            assert_eq!(d.contains("appsink name=ytv"), mode == Mode::Live, "{d}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn live_needs_no_stream_key() {
        let dir = std::env::temp_dir().join(format!("sb-test-key-{}", std::process::id()));
        let s = Settings { stream_key: String::new(), ..settings(&dir) };
        assert!(pipeline_description(Mode::Live, &s, "avenc_aac").is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn preview_shows_the_camera_alone_and_records_nothing() {
        let s = Settings { camera_rtsp: "rtsp://127.0.0.1:8554/1".into(), ..Default::default() };
        let (d, file) = pipeline_description(Mode::Voorbeeld, &s, "avenc_aac").unwrap();
        assert!(file.is_empty());
        assert!(d.contains("rtspsrc location=\"rtsp://127.0.0.1:8554/1\""), "{d}");
        assert!(d.contains("appsink name=camera") && d.contains("appsink name=preview"), "{d}");
        assert!(!d.contains("mp4mux"), "{d}");
        assert!(!Settings::default().has_camera() && s.has_camera());
    }

    #[test]
    fn which_settings_restart_the_pipeline() {
        let a = Settings::default();
        assert!(a.same_pipeline(&Settings { intro_seconds: 3, stream_key: "x".into(), ..a.clone() }));
        assert!(!a.same_pipeline(&Settings { camera_rtsp: "rtsp://camera/1".into(), ..a.clone() }));
        assert!(!a.same_pipeline(&Settings { audio_source: "test".into(), ..a.clone() }));
        assert!(!a.same_pipeline(&Settings { record_dir: "elders".into(), ..a.clone() }));
    }
}
