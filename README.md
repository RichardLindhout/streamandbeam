# Stream & Beam

Shows the liturgy of HHG Tholen (psalms, Bible readings, sermon text and theme) on the
screens in church, instead of a hand-made PowerPoint, and makes the YouTube stream
(replacing OBS): PTZOptics camera + USB sound + our own scenes.

## How it works

1. **Liturgie ophalen van website** reads https://www.hhgtholen.nl/kerkdiensten and lets
   Claude extract every service: psalms and verses, readings, text, theme and points.
2. The psalm texts (1773 berijming) come from DBNL, the Bible text (Statenvertaling)
   from getbible.net. Both are stored in `data/` so a service works without internet.
3. The service gets the usual order. Each predikant does it differently, so drag the
   items (or use ↑↓) into the right order; changes are saved immediately.
4. The TVs in church open `http://<pc>:8080/beam` and follow the operator live.
   No laptop per screen is needed: any TV browser, Android TV / Google TV stick or
   other small player that can show a web page works. The address is shown at startup
   and at the top of the operator page.
5. During the service, press **Volgende** (or →, spatie, a presentation clicker).
   **Preek** jumps to the sermon screen, **Slotzang** to the separate closing-psalm
   screen, **Zwart** blanks the screens.

## YouTube stream

The **Uitzending** panel on the operator page shows a preview and has four modes:
**Voorbeeld** (preview only), **Alleen opnemen** (record to a file), **Live op YouTube**
(stream + record) and **Stop**. Every click on Volgende also picks the stream scene and
moves the camera:

| Item | Stream | Camera position |
|---|---|---|
| Welkom | church frame with photo, date and service | overzicht |
| Psalm / slotzang | white panel with the verse, camera in the round cut-out | orgel |
| Slotzang (announcement) | camera with bar "Slotzang: Psalm 42 : 5" | orgel |
| Schriftlezing, mededeling | white panel with the text | preekstoel |
| Gebed | camera with bar "Gebed" (church screens go black) | doopvont |
| Prediking | camera only | preekstoel |

Camera positions are the preset numbers saved in the PTZOptics camera; set them under
**Instellingen**, together with the camera IP, the sound source and the YouTube stream
key. Recordings are saved as `.mkv` in the `opnames` folder. When the internet drops
during a live stream, the stream restarts by itself after a few seconds.

`streamandbeam scenes` writes example images of the stream scenes to `data/scenes`.

## Testing without the church equipment

`nepcamera` stands in for the church's equipment: an RTSP camera like the PTZOptics
playing a recording of an earlier service (`testdata/dienst.mp4`), its sound as the
"USB" input, the camera's position interface (logs every call) and an example church
website with a liturgy page.

`testdienst` runs a complete service automatically against it and checks everything:
liturgy from the example website, live to a fake YouTube (ffmpeg as RTMP receiver),
clicking through every slide, the camera positions, the recording, what "YouTube"
received, and the "no sound" warning. It writes a report and a contact sheet to
`target/testdienst/<time>/`.

```sh
cargo build --release
target/release/testdienst          # --snel: shorter pause per slide
```

Needs `ffmpeg`/`ffprobe` on the PATH. A test clip can be made from one of the church's
own streams, e.g. with `yt-dlp --download-sections "*00:40:00-00:45:00"`, converted to
`testdata/dienst.mp4` (H.264 + AAC). Without it, nepcamera uses a test picture.

## Windows setup

1. Install GStreamer 1.28 (MSVC 64-bit) from https://gstreamer.freedesktop.org/download/,
   both the **runtime** and the **development** installer, with the "Complete" option.
2. Install Rust from https://rustup.rs.
3. `cargo build --release`
4. For less load on the processor, set the video encoder to `mfh264enc bitrate=4500`
   (any graphics card), `nvh264enc bitrate=4500` (NVIDIA) or `qsvh264enc bitrate=4500`
   (Intel).
5. Sound: leave the sound source empty for the default input, or pick the USB device
   with `wasapisrc device="..."` (`gst-device-monitor-1.0 Audio/Source` lists the names).

## Running

```sh
set ANTHROPIC_API_KEY=...          # needed for reading the website and modern spelling
cargo build --release
target\release\streamandbeam.exe   # prints the operator and TV addresses
```

Options:

- `--poort 8080` port to listen on
- `--data data` folder for cached texts and the saved liturgy
- `--oude-spelling` show the psalms in the original 1774 spelling ("vreezen") instead of
  letting Claude modernise it once per psalm

`streamandbeam fetch` prints what Claude read from the website, for checking.

Give the PC a fixed IP address on the church network (or a DHCP reservation in the
router) so the TVs always find it at the same address.
