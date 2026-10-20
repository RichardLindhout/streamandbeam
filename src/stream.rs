//! The video engine that replaces OBS: camera + USB sound + our overlay, encoded to H.264
//! and sent to YouTube and/or recorded to a file. Built on GStreamer.

use crate::render::{HEIGHT, WIDTH};
use anyhow::{Context, Result, anyhow};
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Below this peak level (dB) we count it as silence.
const SILENCE_DB: f64 = -60.0;
/// Warn after this long without any sound.
const SILENCE_WARN: Duration = Duration::from_secs(10);
/// Warn when the sound input has sent no measurements for this long.
const NO_SIGNAL_WARN: Duration = Duration::from_secs(3);

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
    /// Go live automatically this many minutes before a service starts (0 = off).
    pub auto_start_minutes: u64,
    /// Stop automatically this many minutes after the end screen appears (0 = off).
    pub auto_stop_minutes: u64,
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
            auto_start_minutes: 0,
            auto_stop_minutes: 10,
            youtube_client_id: String::new(),
            youtube_client_secret: String::new(),
            youtube_privacy: "public".into(),
            youtube_test: false,
            autopilot: "suggestie".into(),
            youtube_channel: "UCv0FTJ5pf3aXF7fhVYLwe2g".into(),
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
    /// Preview + recording + YouTube.
    Live,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub mode: Mode,
    pub error: String,
    /// Peak sound level in dB (0 = maximum), for the meter in the operator page.
    pub audio_db: f64,
    pub recording: String,
    /// Shown as a big red warning in the operator page; empty when the sound is fine.
    pub audio_warning: String,
    /// When the current mode started (Unix seconds), for the LIVE clock.
    pub since: i64,
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
}

pub struct Engine {
    running: Mutex<Option<Running>>,
    status: Mutex<Status>,
    overlay: Mutex<Option<gst::Buffer>>,
    preview: Mutex<Vec<u8>>,
    audio: Mutex<AudioWatch>,
    listener: Mutex<Option<Arc<crate::listen::Listener>>>,
}

impl Engine {
    pub fn new() -> Result<Arc<Engine>> {
        gst::init()?;
        let engine = Arc::new(Engine {
            running: Mutex::new(None),
            status: Mutex::new(Status {
                mode: Mode::Stop,
                error: String::new(),
                audio_db: -100.0,
                recording: String::new(),
                audio_warning: String::new(),
                since: 0,
            }),
            overlay: Mutex::new(None),
            preview: Mutex::new(Vec::new()),
            audio: Mutex::new(AudioWatch::new()),
            listener: Mutex::new(None),
        });
        // Keep feeding the current overlay: the compositor needs a steady flow of frames.
        let e = engine.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_millis(200));
            e.push_overlay();
        });
        Ok(engine)
    }

    /// The listener gets a copy of the sound while the engine runs.
    pub fn set_listener(&self, l: Arc<crate::listen::Listener>) {
        *self.listener.lock().unwrap() = Some(l);
    }

    pub fn status(&self) -> Status {
        let mut st = self.status.lock().unwrap().clone();
        // Sound is only measured while the engine runs.
        if st.mode != Mode::Stop {
            st.audio_warning = self.audio.lock().unwrap().warning();
        } else {
            st.audio_db = -100.0;
        }
        st
    }

    pub fn preview_jpeg(&self) -> Vec<u8> {
        self.preview.lock().unwrap().clone()
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

    /// Stops the current pipeline (finishing the recording) and starts one in the new mode.
    pub fn set_mode(self: &Arc<Self>, mode: Mode, settings: &Settings) -> Result<()> {
        self.stop();
        {
            let mut st = self.status.lock().unwrap();
            st.mode = Mode::Stop;
            st.error.clear();
            st.recording.clear();
        }
        if mode == Mode::Stop {
            return Ok(());
        }
        let (desc, recording) = pipeline_description(mode, settings)?;
        let pipeline = gst::parse::launch(&desc)
            .with_context(|| format!("pijplijn bouwen mislukt:\n{desc}"))?
            .downcast::<gst::Pipeline>()
            .map_err(|_| anyhow!("geen pijplijn"))?;
        let overlay = pipeline.by_name("overlay").unwrap().downcast::<gst_app::AppSrc>().unwrap();
        let preview = pipeline.by_name("preview").unwrap().downcast::<gst_app::AppSink>().unwrap();
        let e = self.clone();
        preview.set_callbacks(
            gst_app::AppSinkCallbacks::builder()
                .new_sample(move |sink| {
                    let sample = sink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                    if let Some(map) = sample.buffer().and_then(|b| b.map_readable().ok()) {
                        *e.preview.lock().unwrap() = map.to_vec();
                    }
                    Ok(gst::FlowSuccess::Ok)
                })
                .build(),
        );
        if let Some(listen) = pipeline.by_name("luister").and_then(|e| e.downcast::<gst_app::AppSink>().ok()) {
            let e = self.clone();
            listen.set_callbacks(
                gst_app::AppSinkCallbacks::builder()
                    .new_sample(move |sink| {
                        let sample = sink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                        let listener = e.listener.lock().unwrap().clone();
                        if let (Some(l), Some(map)) = (listener, sample.buffer().and_then(|b| b.map_readable().ok())) {
                            let samples: Vec<f32> = map.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
                            l.push_audio(&samples);
                        }
                        Ok(gst::FlowSuccess::Ok)
                    })
                    .build(),
            );
        }
        self.watch_bus(&pipeline, mode, settings.clone());
        *self.audio.lock().unwrap() = AudioWatch::new();
        pipeline.set_state(gst::State::Playing)?;
        *self.running.lock().unwrap() = Some(Running { pipeline, overlay });
        let mut st = self.status.lock().unwrap();
        st.mode = mode;
        st.since = chrono::Utc::now().timestamp();
        st.recording = recording;
        drop(st);
        self.push_overlay();
        Ok(())
    }

    /// Sends end-of-stream so the recording file is closed properly, then stops.
    pub fn stop(&self) {
        let Some(r) = self.running.lock().unwrap().take() else { return };
        r.pipeline.send_event(gst::event::Eos::new());
        if let Some(bus) = r.pipeline.bus() {
            bus.timed_pop_filtered(gst::ClockTime::from_seconds(5), &[gst::MessageType::Eos, gst::MessageType::Error]);
        }
        let _ = r.pipeline.set_state(gst::State::Null);
        self.status.lock().unwrap().mode = Mode::Stop;
    }

    /// Reads errors and sound levels. When the internet drops during a live stream,
    /// it restarts the stream after a few seconds.
    fn watch_bus(self: &Arc<Self>, pipeline: &gst::Pipeline, mode: Mode, settings: Settings) {
        let bus = pipeline.bus().unwrap();
        let e = self.clone();
        let weak = pipeline.downgrade();
        std::thread::spawn(move || {
            for msg in bus.iter_timed(gst::ClockTime::NONE) {
                if weak.upgrade().is_none() {
                    break;
                }
                match msg.view() {
                    gst::MessageView::Error(err) => {
                        let text = format!("{} ({})", err.error(), err.debug().unwrap_or_default());
                        eprintln!("uitzending: {text}");
                        e.status.lock().unwrap().error = text;
                        let still_ours = e.running.lock().unwrap().as_ref().is_some_and(|r| Some(&r.pipeline) == weak.upgrade().as_ref());
                        if mode == Mode::Live && still_ours {
                            std::thread::sleep(Duration::from_secs(3));
                            if let Err(err) = e.set_mode(mode, &settings) {
                                e.status.lock().unwrap().error = format!("herstart mislukt: {err:#}");
                            }
                        }
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
                    gst::MessageView::Eos(_) => break,
                    _ => {}
                }
            }
        });
    }
}

fn pipeline_description(mode: Mode, s: &Settings) -> Result<(String, String)> {
    let video = if s.camera_ip.is_empty() && s.camera_rtsp.is_empty() {
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
    let mut d = format!(
        "{video} ! videoconvert ! videoscale ! videorate ! video/x-raw,width={WIDTH},height={HEIGHT},framerate=25/1 ! queue ! comp.sink_0 \
         appsrc name=overlay is-live=true format=time do-timestamp=true caps=video/x-raw,format=RGBA,width={WIDTH},height={HEIGHT},framerate=0/1 \
           ! videoconvert ! queue ! comp.sink_1 \
         compositor name=comp background=black ! video/x-raw,width={WIDTH},height={HEIGHT},framerate=25/1 ! videoconvert ! tee name=vt \
         vt. ! queue leaky=downstream max-size-buffers=2 ! videoscale ! videorate ! video/x-raw,width=640,height=360,framerate=5/1 \
           ! jpegenc quality=70 ! appsink name=preview max-buffers=1 drop=true sync=false \
         {audio} ! audioconvert ! audioresample ! audio/x-raw,rate=48000,channels=2 ! level interval=100000000 ! tee name=at \
         at. ! queue leaky=downstream max-size-time=2000000000 ! audioconvert ! audioresample \
           ! audio/x-raw,format=F32LE,rate=16000,channels=1 ! appsink name=luister sync=false max-buffers=100 drop=true "
    );
    if mode == Mode::Voorbeeld {
        d.push_str("at. ! queue ! fakesink sync=false");
        return Ok((d, String::new()));
    }
    std::fs::create_dir_all(&s.record_dir)?;
    let file = PathBuf::from(&s.record_dir).join(format!("dienst-{}.mkv", chrono::Local::now().format("%Y-%m-%d-%H%M%S")));
    let file = file.to_string_lossy().replace('\\', "/");
    d.push_str(&format!(
        "vt. ! queue ! videoconvert ! video/x-raw,format=I420 ! {enc} ! h264parse config-interval=-1 ! tee name=et \
         at. ! queue ! audioconvert ! avenc_aac bitrate=160000 ! aacparse ! tee name=ae \
         et. ! queue ! rec. ae. ! queue ! rec. matroskamux name=rec ! filesink location=\"{file}\" ",
        enc = s.video_encoder
    ));
    if mode == Mode::Live {
        if s.stream_key.is_empty() {
            anyhow::bail!("vul eerst de YouTube-streamsleutel in bij de instellingen");
        }
        d.push_str(&format!(
            "et. ! queue ! mux. ae. ! queue ! mux. flvmux name=mux streamable=true ! rtmp2sink location=\"{}/{}\"",
            s.rtmp_url.trim_end_matches('/'),
            s.stream_key
        ));
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
