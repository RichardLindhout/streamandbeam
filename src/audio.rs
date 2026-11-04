//! The sound inputs of this PC (the USB sound card from the mixer, a microphone), so the
//! operator picks the sound source by its name instead of typing a GStreamer line, and what
//! the `audioSource` setting records from.

use gstreamer as gst;
use gstreamer::prelude::*;
use serde::Serialize;
use std::sync::LazyLock;

/// `audioSource` for an input chosen by name: "apparaat:Microfoon (USB Audio CODEC)".
pub const DEVICE: &str = "apparaat:";
/// Recorded while the chosen input is not connected, so the stream keeps running.
const SILENCE: &str = "audiotestsrc is-live=true wave=silence";

/// A sound input as the operator sees it.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Input {
    /// As Windows shows it, e.g. "Microfoon (USB Audio CODEC)".
    pub name: String,
    /// Connected by USB: the sound card from the mixer.
    pub usb: bool,
    /// The default sound input of Windows.
    pub default: bool,
    /// The GStreamer source, e.g. `wasapi2src device="{0.0.1.00000000}.{…}"`.
    #[serde(skip)]
    pub launch: String,
}

/// What the pipeline records from.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Source {
    pub launch: String,
    /// For the console and the warnings: "Microfoon (USB Audio CODEC)", "testtoon", ...
    pub name: String,
    /// An input of this PC chosen by name.
    pub chosen: bool,
    /// The chosen input is not connected: silence until it is back.
    pub missing: bool,
    /// The default input of Windows (it follows what Windows has as default).
    pub default: bool,
    /// With the default input: a USB sound card that is there but not used, by name.
    pub usb_unused: Option<String>,
}

/// What the `audioSource` setting records from: "" the default input of Windows, "test" a
/// test tone, "apparaat:<name>" that input (also in another USB port), anything else a
/// GStreamer line as it is (the testdienst uses the sound of the nepcamera).
pub fn choose(setting: &str, inputs: &[Input]) -> Source {
    let setting = setting.trim();
    if let Some(name) = setting.strip_prefix(DEVICE) {
        let name = name.trim();
        return match find(inputs, name) {
            Some(i) => Source { launch: i.launch.clone(), name: i.name.clone(), chosen: true, ..Default::default() },
            None => Source { launch: SILENCE.into(), name: name.into(), chosen: true, missing: true, ..Default::default() },
        };
    }
    match setting {
        "" => Source {
            launch: "autoaudiosrc".into(),
            name: inputs.iter().find(|i| i.default).map(|i| i.name.clone()).unwrap_or_else(|| "de standaard-geluidsinvoer".into()),
            default: true,
            usb_unused: usb_unused(inputs),
            ..Default::default()
        },
        "test" => Source { launch: "audiotestsrc is-live=true wave=sine volume=0.05".into(), name: "de testtoon".into(), ..Default::default() },
        line => Source { launch: line.into(), name: "de eigen geluidsbron".into(), ..Default::default() },
    }
}

/// The running source checked against the inputs there are now: a chosen input that was
/// pulled out becomes `missing` (the pipeline records silence meanwhile), and `true` means it
/// is back and the pipeline must start again to record from it. For the default input it
/// notes a USB sound card that was plugged in.
pub fn recheck(running: &Source, inputs: &[Input]) -> (Source, bool) {
    let mut s = running.clone();
    if s.chosen {
        let present = find(inputs, &s.name).is_some();
        if s.missing && present {
            return (s, true);
        }
        s.missing = !present;
    } else if s.default {
        s.usb_unused = usb_unused(inputs);
    }
    (s, false)
}

/// When the default input is not a USB sound card while there is one: its name.
fn usb_unused(inputs: &[Input]) -> Option<String> {
    let default = inputs.iter().find(|i| i.default)?;
    if default.usb {
        return None;
    }
    inputs.iter().find(|i| i.usb).map(|i| i.name.clone())
}

/// The input with this name; also when Windows numbered it after a move to another USB port
/// ("Microfoon (2- USB Audio CODEC)" for "Microfoon (USB Audio CODEC)").
pub fn find<'a>(inputs: &'a [Input], name: &str) -> Option<&'a Input> {
    inputs.iter().find(|i| i.name == name).or_else(|| {
        let n = normalize(name);
        inputs.iter().find(|i| normalize(&i.name) == n)
    })
}

/// Lower case, without the number Windows puts before a device it has seen before ("2- "),
/// with single spaces.
fn normalize(name: &str) -> String {
    static NUMBER: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r"(^|\()\s*\d+\s*-\s+").unwrap());
    NUMBER.replace_all(name, "$1").to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Keeps track of the sound inputs, plugging in and out included.
pub struct Inputs {
    monitor: Option<gst::DeviceMonitor>,
}

impl Inputs {
    /// After `gst::init`.
    pub fn new() -> Inputs {
        // The NDI plugin also looks for network sources and complains at every start
        // without its SDK.
        if std::env::var_os("GST_DEBUG").is_none() {
            gst::log::set_threshold_for_name("ndideviceprovider", gst::DebugLevel::None);
        }
        let monitor = gst::DeviceMonitor::new();
        monitor.add_filter(Some("Audio/Source"), None);
        let monitor = match monitor.start() {
            Ok(()) => Some(monitor),
            Err(e) => {
                eprintln!("geluidsinvoer zoeken lukt niet: {e}");
                None
            }
        };
        Inputs { monitor }
    }

    /// The inputs there are now, USB first. What the PC plays itself (loopback) is left out,
    /// and Windows' "Default Audio Capture Device" only marks the real default.
    pub fn list(&self) -> Vec<Input> {
        let Some(monitor) = &self.monitor else { return Vec::new() };
        let mut default_id = None;
        let mut list: Vec<(Input, String)> = Vec::new();
        for d in monitor.devices() {
            let props = d.properties();
            let flag = |k: &str| props.as_ref().and_then(|p| p.get::<bool>(k).ok()).unwrap_or(false);
            let text = |k: &str| props.as_ref().and_then(|p| p.get::<String>(k).ok()).unwrap_or_default();
            if flag("wasapi2.device.loopback") || flag("wasapi.device.loopback") {
                continue;
            }
            let actual = text("device.actual-id");
            if flag("device.default") && !actual.is_empty() {
                default_id = Some(actual);
                continue;
            }
            let name = d.display_name().to_string();
            if list.iter().any(|(i, _)| i.name == name) {
                continue;
            }
            let Some(launch) = launch(&d, props.as_deref()) else { continue };
            let usb = text("device.enumerator-name").eq_ignore_ascii_case("usb")
                || text("device.bus").eq_ignore_ascii_case("usb")
                || name.to_lowercase().contains("usb");
            let input = Input { name, usb, default: flag("device.default") || flag("is-default"), launch };
            list.push((input, text("device.id")));
        }
        let mut list: Vec<Input> = list
            .into_iter()
            .map(|(mut i, id)| {
                i.default |= default_id.as_ref().is_some_and(|d| !id.is_empty() && *d == id);
                i
            })
            .collect();
        list.sort_by_key(|i| !i.usb);
        list
    }
}

/// The GStreamer source for a device.
fn launch(d: &gst::Device, props: Option<&gst::StructureRef>) -> Option<String> {
    // Windows: by its endpoint id. When the cable is pulled it goes on with silence, so the
    // stream keeps running and the warning tells what is wrong.
    if props.and_then(|p| p.get::<&str>("device.api").ok()) == Some("wasapi2") {
        let id = props?.get::<&str>("device.id").ok()?;
        return Some(format!("wasapi2src device=\"{id}\" continue-on-error=true"));
    }
    // Elsewhere: the element the device makes, with its device property.
    let el = d.create_element(None).ok()?;
    let factory = el.factory()?.name().to_string();
    if el.find_property("device").is_none() {
        return Some(factory);
    }
    let v = el.property_value("device");
    let device = match v.get::<Option<String>>() {
        Ok(Some(s)) => format!("\"{s}\""),
        _ => v.serialize().ok()?.to_string(),
    };
    Some(format!("{factory} device={device}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(name: &str, usb: bool, default: bool) -> Input {
        Input { name: name.into(), usb, default, launch: format!("wasapi2src device=\"{name}\"") }
    }

    fn church() -> Vec<Input> {
        vec![input("Microfoon (2- USB Audio CODEC)", true, false), input("Microphone Array (Realtek(R) Audio)", false, true)]
    }

    #[test]
    fn same_card_in_another_port() {
        let inputs = church();
        assert_eq!(find(&inputs, "Microfoon (USB Audio CODEC)").map(|i| i.usb), Some(true));
        assert_eq!(find(&inputs, "microfoon (3- usb audio codec)").map(|i| i.usb), Some(true));
        assert!(find(&inputs, "Line (Behringer UMC202HD)").is_none());
        assert_eq!(normalize("Microphone Array (Realtek(R) Audio)"), "microphone array (realtek(r) audio)");
        assert_eq!(normalize("2- USB Audio CODEC"), "usb audio codec");
        // A range in the name itself stays.
        assert_eq!(normalize("Mic 1-2 (Focusrite USB)"), "mic 1-2 (focusrite usb)");
    }

    #[test]
    fn chosen_input() {
        let s = choose("apparaat:Microfoon (USB Audio CODEC)", &church());
        assert_eq!(s.launch, "wasapi2src device=\"Microfoon (2- USB Audio CODEC)\"");
        assert_eq!(s.name, "Microfoon (2- USB Audio CODEC)");
        assert!(s.chosen && !s.missing && s.usb_unused.is_none());
    }

    #[test]
    fn chosen_input_missing_records_silence() {
        let s = choose("apparaat:Microfoon (USB Audio CODEC)", &[input("Microphone Array (Realtek(R) Audio)", false, true)]);
        assert_eq!(s.launch, SILENCE);
        assert_eq!(s.name, "Microfoon (USB Audio CODEC)");
        assert!(s.missing);
    }

    #[test]
    fn default_input_names_the_unused_usb_card() {
        let s = choose("", &church());
        assert_eq!(s.launch, "autoaudiosrc");
        assert_eq!(s.name, "Microphone Array (Realtek(R) Audio)");
        assert_eq!(s.usb_unused.as_deref(), Some("Microfoon (2- USB Audio CODEC)"));
        // The USB card is the default itself: nothing to say.
        let s = choose("", &[input("Microfoon (USB Audio CODEC)", true, true), input("Microphone Array", false, false)]);
        assert!(s.usb_unused.is_none());
        // Default unknown: no guess.
        assert!(choose("", &[input("Microfoon (USB Audio CODEC)", true, false)]).usb_unused.is_none());
    }

    #[test]
    fn test_tone_and_own_line() {
        assert!(choose("test", &[]).launch.starts_with("audiotestsrc"));
        let line = "rtspsrc location=rtsp://127.0.0.1:8554/geluid latency=200 ! decodebin";
        let s = choose(line, &church());
        assert_eq!(s.launch, line);
        assert!(!s.chosen && !s.missing && !s.default);
    }

    #[test]
    fn pulled_out_and_back() {
        let running = choose("apparaat:Microfoon (USB Audio CODEC)", &church());
        // Still there.
        assert_eq!(recheck(&running, &church()), (running.clone(), false));
        // Pulled out: missing, nothing to restart.
        let (gone, restart) = recheck(&running, &[input("Microphone Array (Realtek(R) Audio)", false, true)]);
        assert!(gone.missing && !restart);
        // Still out.
        assert!(!recheck(&gone, &[]).1);
        // Back, in yet another port: start again.
        assert!(recheck(&gone, &[input("Microfoon (4- USB Audio CODEC)", true, false)]).1);
    }

    #[test]
    fn usb_card_plugged_in_later() {
        let running = choose("", &[input("Microphone Array (Realtek(R) Audio)", false, true)]);
        assert!(running.usb_unused.is_none());
        let (now, restart) = recheck(&running, &church());
        assert_eq!(now.usb_unused.as_deref(), Some("Microfoon (2- USB Audio CODEC)"));
        assert!(!restart);
    }
}
