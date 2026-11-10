//! Draws the overlay for the YouTube stream, in the style of the church's own stream:
//! a white panel with a large round opening showing the camera (the text runs along the
//! curve), a lower-third card over the camera, or the church photo in a teal frame before
//! and after the service. Transparent pixels show the camera.

use crate::layout::{Layout, LayoutBox, parse_color};
use cosmic_text::{Align, Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, SwashCache, Weight};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, PixmapPaint, Rect, Transform};

pub const WIDTH: u32 = 1920;
pub const HEIGHT: u32 = 1080;

const TEAL: [u8; 3] = [0x00, 0x4b, 0x58];
const TEAL_2: [u8; 3] = [0x07, 0x69, 0x7a];
const INK: [u8; 3] = [0x26, 0x2d, 0x2f];
const WARM: [u8; 3] = [0x8f, 0x83, 0x77];
const SAND: [u8; 3] = [0xe3, 0xda, 0xcf];
const PAPER: [u8; 3] = [0xfb, 0xfa, 0xf7];
/// The teal of the church's own start and end screens.
const FRAME_TEAL: [u8; 3] = [0x00, 0x5e, 0x70];

/// Left margin of the panel text.
const LEFT: f32 = 96.0;
/// Air between the text and an area it avoids.
const AIR: f32 = 48.0;
/// Narrowest piece of line worth putting text on.
const MIN_LINE: f32 = 320.0;

/// An area of the panel the text keeps clear of, in pixels of the 1920x1080 picture
/// (it may stick out of the picture). With `camera` the area is cut out of the panel,
/// so the camera shows through, with a fine ring around it.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Zone {
    /// Round (circle or ellipse in the box) instead of a rectangle.
    pub round: bool,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub camera: bool,
}

impl Default for Zone {
    fn default() -> Zone {
        Zone::camera_circle()
    }
}

impl Zone {
    /// The camera opening as on the church's own stream: a large circle with its centre
    /// just right of the picture, near the top.
    pub fn camera_circle() -> Zone {
        Zone { round: true, x: 1075.0, y: -750.0, w: 1900.0, h: 1900.0, camera: true }
    }

    fn path(&self, grow: f32) -> Option<tiny_skia::Path> {
        let (x, y, w, h) = (self.x - grow, self.y - grow, self.w + 2.0 * grow, self.h + 2.0 * grow);
        if self.round {
            PathBuilder::from_oval(Rect::from_xywh(x, y, w, h)?)
        } else {
            Some(PathBuilder::from_rect(Rect::from_xywh(x, y, w, h)?))
        }
    }

    /// The stretch of a text line from `top` to `bottom` this area covers, with air.
    fn blocks(&self, top: f32, bottom: f32) -> Option<(f32, f32)> {
        let (t, b) = (self.y - AIR, self.y + self.h + AIR);
        if bottom <= t || top >= b {
            return None;
        }
        if !self.round {
            return Some((self.x - AIR, self.x + self.w + AIR));
        }
        let (rx, ry) = (self.w / 2.0, self.h / 2.0);
        let (cx, cy) = (self.x + rx, self.y + ry);
        // Widest point of the ellipse within the line.
        let d = cy.clamp(top, bottom) - cy;
        let half = if d.abs() >= ry { 0.0 } else { rx * (1.0 - (d / ry).powi(2)).sqrt() };
        Some((cx - half - AIR, cx + half + AIR))
    }
}

/// Where a text line from `y` to `y + line_h` can go: the first free stretch from the
/// left that is wide enough, as (x, width).
fn room(zones: &[Zone], y: f32, line_h: f32) -> Option<(f32, f32)> {
    let mut blocked: Vec<(f32, f32)> = zones.iter().filter_map(|z| z.blocks(y, y + line_h)).collect();
    blocked.sort_by(|a, b| a.0.total_cmp(&b.0));
    let (mut x, end) = (LEFT, WIDTH as f32 - LEFT);
    for (a, b) in blocked {
        if a - x >= MIN_LINE {
            return Some((x, a - x));
        }
        x = x.max(b);
    }
    (end - x >= MIN_LINE).then_some((x, end - x))
}

#[derive(Debug, Clone, PartialEq)]
pub enum Scene {
    /// Only the camera.
    Camera,
    /// Camera with a card at the bottom: a small label ("ZINGEN") and the text ("Psalm 42 : 5").
    Bar { label: String, text: String },
    /// White panel with the text on the left, the camera in the round opening on the right.
    Panel { label: String, title: String, paragraphs: Vec<String>, style: Body, footer: String, qr: bool },
    /// The church photo in a teal frame (logo top left, QR code bottom left) with one line
    /// in the bar below it: `left` left-aligned, `right` right-aligned. As the church's own
    /// start screen (outside, date and service) and end screen (inside, next service).
    Frame { photo: Photo, left: String, right: String },
    /// A screen the church made itself (see `layout`), with the texts for its fields.
    Custom { layout: Layout, fields: Vec<(String, String)> },
}

impl Scene {
    /// Where the camera picture goes, as (x, y, width, height) in pixels: the whole
    /// picture, or for a screen of the church's own the camera box, covered completely
    /// (what sticks out is hidden by the screen itself).
    pub fn camera_rect(&self) -> [i32; 4] {
        let full = [0, 0, WIDTH as i32, HEIGHT as i32];
        let Scene::Custom { layout, .. } = self else { return full };
        let Some(b) = layout.camera().filter(|b| b.w >= 1.0 && b.h >= 1.0) else { return full };
        let scale = (b.w / WIDTH as f32).max(b.h / HEIGHT as f32);
        let (w, h) = (WIDTH as f32 * scale, HEIGHT as f32 * scale);
        [(b.x + (b.w - w) / 2.0).round() as i32, (b.y + (b.h - h) / 2.0).round() as i32, w.round() as i32, h.round() as i32]
    }
}

/// How the panel text is set.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Body {
    /// Every paragraph is one line that is never broken (psalm lines, a list): the
    /// letters get smaller until each line fits next to the curve.
    Lines,
    /// Running text, broken along the curve.
    Prose,
    /// Running text, each paragraph starting with a verse number.
    Verses,
    /// Running text whose first paragraph is a question (catechism), set in colour.
    Question,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Photo {
    Inside,
    Outside,
}

pub struct Images<'a> {
    pub inside: &'a [u8],
    pub outside: &'a [u8],
    pub logo: &'a [u8],
}

pub struct Renderer {
    fonts: FontSystem,
    cache: SwashCache,
    inside: Option<Pixmap>,
    outside: Option<Pixmap>,
    logo: Option<Pixmap>,
    /// The QR code's dark modules, row by row.
    qr: Option<(usize, Vec<bool>)>,
    /// Areas the panel text keeps clear of.
    zones: Vec<Zone>,
    /// Folder with the church's own pictures, for its own screens.
    image_dir: PathBuf,
    /// Pictures already read, by file name, with the file's time (None = built in).
    pictures: HashMap<String, (Option<SystemTime>, Option<Arc<Pixmap>>)>,
}

/// Source Serif 4 for the texts (psalms, readings), Source Sans 3 for headings and labels.
#[derive(Clone, Copy, PartialEq)]
enum Face {
    Serif,
    SansBold,
}

#[derive(Clone, Copy)]
struct TextStyle {
    size: f32,
    face: Face,
    color: [u8; 3],
    /// Letter spacing in em, for small capitals labels.
    tracking: f32,
}

impl TextStyle {
    const fn new(size: f32, face: Face, color: [u8; 3]) -> TextStyle {
        TextStyle { size, face, color, tracking: 0.0 }
    }
}

fn load(bytes: &[u8]) -> Option<Pixmap> {
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let mut pm = Pixmap::new(img.width(), img.height())?;
    // Premultiply for tiny-skia.
    for (dst, src) in pm.data_mut().chunks_exact_mut(4).zip(img.as_raw().chunks_exact(4)) {
        let a = src[3] as u32;
        dst[0] = (src[0] as u32 * a / 255) as u8;
        dst[1] = (src[1] as u32 * a / 255) as u8;
        dst[2] = (src[2] as u32 * a / 255) as u8;
        dst[3] = src[3];
    }
    Some(pm)
}

impl Renderer {
    /// `qr_link` is shown as a QR code (e.g. the giving app or the church website).
    pub fn new(images: Images, qr_link: &str) -> Renderer {
        let mut db = cosmic_text::fontdb::Database::new();
        db.load_font_data(include_bytes!("../assets/fonts/SourceSerif4-Regular.ttf").to_vec());
        db.load_font_data(include_bytes!("../assets/fonts/SourceSans3-Regular.ttf").to_vec());
        db.load_font_data(include_bytes!("../assets/fonts/SourceSans3-Semibold.ttf").to_vec());
        db.load_font_data(include_bytes!("../assets/fonts/SourceSerif4-Semibold.ttf").to_vec());
        let qr = (!qr_link.is_empty()).then(|| qrcode::QrCode::new(qr_link).ok()).flatten().map(|code| {
            (code.width(), code.to_colors().into_iter().map(|c| c == qrcode::Color::Dark).collect())
        });
        Renderer {
            zones: vec![Zone::camera_circle()],
            fonts: FontSystem::new_with_locale_and_db("nl-NL".into(), db),
            cache: SwashCache::new(),
            inside: load(images.inside),
            outside: load(images.outside),
            logo: load(images.logo),
            qr,
            image_dir: PathBuf::new(),
            pictures: HashMap::new(),
        }
    }

    /// The pictures built into the program.
    pub fn with_builtin_images(qr_link: &str) -> Renderer {
        Renderer::new(
            Images {
                inside: include_bytes!("../assets/images/interieur.jpg"),
                outside: include_bytes!("../assets/images/buiten.jpg"),
                logo: include_bytes!("../assets/images/logo-wit.png"),
            },
            qr_link,
        )
    }

    pub fn set_zones(&mut self, zones: &[Zone]) {
        self.zones = zones.to_vec();
    }

    /// Where the church's own pictures are.
    pub fn set_image_dir(&mut self, dir: PathBuf) {
        self.image_dir = dir;
    }

    /// Renders a scene to straight-alpha RGBA pixels of WIDTH x HEIGHT.
    pub fn render(&mut self, scene: &Scene) -> Vec<u8> {
        let mut pm = Pixmap::new(WIDTH, HEIGHT).unwrap();
        match scene {
            Scene::Camera => {}
            Scene::Bar { label, text } => self.bar(&mut pm, label, text),
            Scene::Panel { label, title, paragraphs, style, footer, qr } => self.panel(&mut pm, label, title, paragraphs, *style, footer, *qr),
            Scene::Frame { photo, left, right } => self.frame(&mut pm, *photo, left, right),
            Scene::Custom { layout, fields } => self.custom(&mut pm, layout, fields),
        }
        to_straight_rgba(pm)
    }

    /// A screen of the church's own: the boxes from back to front on a solid background.
    /// A camera box is an opening: the camera shows through it, and boxes after it lie
    /// over the camera.
    fn custom(&mut self, pm: &mut Pixmap, layout: &Layout, fields: &[(String, String)]) {
        pm.fill(color(parse_color(&layout.background).unwrap_or([0; 3]), 255));
        for b in &layout.boxes {
            if b.w < 1.0 || b.h < 1.0 {
                continue;
            }
            match b.kind.as_str() {
                "camera" => {
                    let mut clear = Paint::default();
                    clear.blend_mode = tiny_skia::BlendMode::Clear;
                    clear.anti_alias = true;
                    if let Some(path) = box_path(b) {
                        pm.fill_path(&path, &clear, FillRule::Winding, Transform::identity(), None);
                    }
                }
                "vlak" => fill_path(pm, box_path(b), parse_color(&b.fill).unwrap_or([0; 3]), alpha(b.opacity)),
                "afbeelding" => {
                    let Some(img) = self.picture(&b.image) else { continue };
                    let Some(mut part) = Pixmap::new(b.w.round() as u32, b.h.round() as u32) else { continue };
                    if b.fit == "passend" {
                        draw_contain(&mut part, &img, b.w, b.h);
                    } else {
                        draw_cover(&mut part, &img, 0.0, 0.0, b.w, b.h);
                    }
                    pm.draw_pixmap(b.x.round() as i32, b.y.round() as i32, part.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
                }
                "qr" => self.draw_qr(pm, b.x, b.y, b.w.min(b.h)),
                _ => {
                    if let Some(fill) = parse_color(&b.fill) {
                        fill_path(pm, box_path(b), fill, alpha(b.opacity));
                    }
                    let text = crate::layout::fill(&b.text, fields);
                    if !text.trim().is_empty() {
                        self.text_box(pm, b, &text);
                    }
                }
            }
        }
    }

    /// One of the church's own pictures (read again when the file changed), or one built
    /// into the program: buiten.jpg, interieur.jpg and logo-wit.png.
    fn picture(&mut self, name: &str) -> Option<Arc<Pixmap>> {
        let name = crate::layout::safe_name(name)?;
        let path = self.image_dir.join(&name);
        let time = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
        if let Some((t, img)) = self.pictures.get(&name)
            && *t == time
        {
            return img.clone();
        }
        let img = match time {
            Some(_) => std::fs::read(&path).ok().and_then(|bytes| load(&bytes)),
            None => match name.as_str() {
                "buiten.jpg" => self.outside.clone(),
                "interieur.jpg" => self.inside.clone(),
                "logo-wit.png" => self.logo.clone(),
                _ => None,
            },
        }
        .map(Arc::new);
        self.pictures.insert(name, (time, img.clone()));
        img
    }

    /// Text in a box: broken at the box's width, in the largest letters (up to the box's
    /// size) at which it all fits.
    fn text_box(&mut self, pm: &mut Pixmap, b: &LayoutBox, text: &str) {
        let pad = if parse_color(&b.fill).is_some() { 24.0_f32.min(b.w / 8.0).min(b.h / 8.0) } else { 0.0 };
        let (w, h) = ((b.w - 2.0 * pad).max(8.0), (b.h - 2.0 * pad).max(8.0));
        let [r, g, bl] = parse_color(&b.color).unwrap_or([255; 3]);
        let serif = b.font == "serif";
        let align = match b.align.as_str() {
            "midden" => Align::Center,
            "rechts" => Align::Right,
            _ => Align::Left,
        };
        // Typographic apostrophe: ’t instead of 't.
        let text = text.replace('\'', "\u{2019}");
        let mut size = b.size.clamp(MIN_BOX_TEXT, 500.0);
        let (mut buf, height) = loop {
            let metrics = Metrics::new(size, size * 1.3);
            let mut buf = Buffer::new(&mut self.fonts, metrics);
            buf.set_size(Some(w), None);
            let attrs = Attrs::new()
                .family(Family::Name(if serif { "Source Serif 4" } else { "Source Sans 3" }))
                .weight(if b.bold { Weight::SEMIBOLD } else { Weight::NORMAL })
                .metrics(metrics)
                .color(Color::rgb(r, g, bl));
            buf.set_text(&text, &attrs, Shaping::Advanced, Some(align));
            buf.shape_until_scroll(&mut self.fonts, false);
            let (height, widest) = buf.layout_runs().fold((0.0_f32, 0.0_f32), |(hh, ww), run| (hh.max(run.line_top + run.line_height), ww.max(run.line_w)));
            if (height <= h && widest <= w + 1.0) || size <= MIN_BOX_TEXT {
                break (buf, height);
            }
            size = (size * 0.95).floor().max(MIN_BOX_TEXT);
        };
        let y = b.y + pad + match b.valign.as_str() {
            "midden" => ((h - height) / 2.0).max(0.0),
            "onder" => (h - height).max(0.0),
            _ => 0.0,
        };
        self.draw_buffer(pm, &mut buf, b.x + pad, y);
    }

    /// Lower-third card: a small label above the main text, a warm stripe on the left. A long
    /// text (a point of the theme, the theme as label) gets smaller letters to stay on screen.
    fn bar(&mut self, pm: &mut Pixmap, label: &str, text: &str) {
        let (x, pad) = (80.0, 140.0);
        let room = WIDTH as f32 - 2.0 * x - pad;
        let label = label.to_uppercase();
        let mut main = TextStyle::new(56.0, Face::SansBold, [255; 3]);
        while main.size > 36.0 && self.measure(&[(text, main)]) > room {
            main.size -= 2.0;
        }
        let mut small = TextStyle { tracking: 0.16, ..TextStyle::new(24.0, Face::SansBold, SAND) };
        while small.size > 18.0 && self.measure(&[(&label, small)]) > room {
            small.size -= 1.0;
        }
        let width = (self.measure(&[(text, main)]).max(self.measure(&[(&label, small)])) + pad).min(WIDTH as f32 - 2.0 * x);
        let h = if label.is_empty() { 112.0 } else { 140.0 };
        let y = HEIGHT as f32 - 90.0 - h;
        fill_round_rect(pm, x, y, width, h, 16.0, TEAL, 238);
        fill_round_rect(pm, x, y, 12.0, h, 0.0, SAND, 255);
        if label.is_empty() {
            self.line(pm, &[(text, main)], x + 60.0, y + (h - main.size * 1.25) / 2.0);
        } else {
            self.line(pm, &[(&label, small)], x + 60.0, y + 22.0 + (24.0 - small.size) / 2.0);
            // Smaller letters sit lower, in the middle of the room for the text.
            self.line(pm, &[(text, main)], x + 60.0, y + 52.0 + (56.0 - main.size) * 0.6);
        }
    }

    /// White panel with the camera in a large round opening; the text runs along the curve.
    #[allow(clippy::too_many_arguments)]
    fn panel(&mut self, pm: &mut Pixmap, label: &str, title: &str, paragraphs: &[String], style: Body, footer: &str, qr: bool) {
        pm.fill(color(PAPER, 255));
        // Camera openings: a fine coloured ring, then the opening itself.
        let mut clear = Paint::default();
        clear.blend_mode = tiny_skia::BlendMode::Clear;
        for z in self.zones.iter().filter(|z| z.camera) {
            fill_path(pm, z.path(9.0), TEAL_2, 255);
            if let Some(path) = z.path(0.0) {
                pm.fill_path(&path, &clear, FillRule::Winding, Transform::identity(), None);
            }
        }

        let mut y = 64.0;
        if !label.is_empty() {
            let small = TextStyle { tracking: 0.16, ..TextStyle::new(24.0, Face::SansBold, WARM) };
            self.line(pm, &[(&label.to_uppercase(), small)], LEFT, y);
            y += 40.0;
        }
        self.line(pm, &[(title, TextStyle::new(60.0, Face::SansBold, TEAL))], LEFT, y);
        y += 84.0;
        fill_round_rect(pm, LEFT, y, 72.0, 5.0, 2.5, TEAL_2, 255);
        y += 40.0;

        let bottom = HEIGHT as f32 - if qr || !footer.is_empty() { 230.0 } else { 70.0 };
        // Largest size at which everything fits next to the curve.
        let mut size = 50.0;
        let lines = loop {
            if let Some(lines) = self.flow(paragraphs, style, size, y, bottom) {
                break lines;
            }
            if size <= 24.0 {
                // Too long to fit: break lines anyway rather than show nothing.
                let fallback = if style == Body::Lines { Body::Prose } else { style };
                break self.flow(paragraphs, fallback, size, y, f32::MAX).unwrap_or_default();
            }
            size -= 2.0;
        };
        for (lx, ly, spans) in lines {
            let spans: Vec<(&str, TextStyle)> = spans.iter().map(|(t, s)| (t.as_str(), *s)).collect();
            self.line(pm, &spans, lx, ly);
        }
        if !footer.is_empty() {
            self.line(pm, &[(footer, TextStyle::new(34.0, Face::SansBold, TEAL))], LEFT, HEIGHT as f32 - 110.0);
        }
        if qr {
            let x = if footer.is_empty() { LEFT } else { LEFT + 360.0 };
            self.draw_qr(pm, x, HEIGHT as f32 - 210.0, 150.0);
        }
    }

    /// Lays out paragraphs line by line; each line takes the free stretch at its height,
    /// keeping clear of the zones (so it follows the curve of the camera opening, and
    /// skips heights where nothing fits). Returns None when it does not fit above `bottom`.
    #[allow(clippy::type_complexity)]
    fn flow(&mut self, paragraphs: &[String], style: Body, size: f32, top: f32, bottom: f32) -> Option<Vec<(f32, f32, Vec<(String, TextStyle)>)>> {
        let numbered = style == Body::Verses;
        let number = TextStyle::new(size * 0.6, Face::SansBold, TEAL_2);
        let line_h = size * if style == Body::Lines { 1.38 } else { 1.42 };
        let gap = if style == Body::Lines { 0.0 } else { size * 0.35 };
        let mut out = Vec::new();
        let mut y = top;
        let zones = self.zones.clone();
        // The next height with room for a line, and that room.
        let next = |y: &mut f32| -> Option<(f32, f32)> {
            while *y < HEIGHT as f32 + 2000.0 {
                if let Some(r) = room(&zones, *y, line_h) {
                    return Some(r);
                }
                *y += line_h / 2.0;
            }
            None
        };
        for (k, p) in paragraphs.iter().enumerate() {
            let colour = if style == Body::Question && k == 0 { TEAL_2 } else { INK };
            let body = TextStyle::new(size, Face::Serif, colour);
            if style == Body::Lines {
                let (x, width) = next(&mut y)?;
                if self.measure(&[(p, body)]) > width {
                    return None;
                }
                out.push((x, y, vec![(p.clone(), body)]));
                y += line_h;
                continue;
            }
            let mut words: Vec<&str> = p.split_whitespace().collect();
            let mut line: Vec<(String, TextStyle)> = Vec::new();
            if numbered && !words.is_empty() {
                line.push((format!("{} ", words.remove(0)), number));
            }
            let mut text = String::new();
            let (mut x, mut width) = next(&mut y)?;
            for w in words {
                let candidate = if text.is_empty() { w.to_string() } else { format!("{text} {w}") };
                let mut spans = line.clone();
                spans.push((candidate.clone(), body));
                let refs: Vec<(&str, TextStyle)> = spans.iter().map(|(t, s)| (t.as_str(), *s)).collect();
                if !text.is_empty() && self.measure(&refs) > width {
                    line.push((text, body));
                    out.push((x, y, std::mem::take(&mut line)));
                    y += line_h;
                    (x, width) = next(&mut y)?;
                    text = w.to_string();
                } else {
                    text = candidate;
                }
            }
            if !text.is_empty() {
                line.push((text, body));
            }
            if !line.is_empty() {
                out.push((x, y, line));
                y += line_h;
            }
            y += gap;
        }
        (y - gap <= bottom).then_some(out)
    }

    /// The church's start/end screen, measured from its own stream: a teal bar on the left
    /// (178 px) and below (96 px), a white band between the bars and the photo with a thin
    /// white line 8 px inside the photo, the logo on a white tile top left, the QR code bottom
    /// left and one line of large white text in the bottom bar.
    fn frame(&mut self, pm: &mut Pixmap, photo: Photo, left: &str, right: &str) {
        const BAR: f32 = 178.0;
        const PHOTO_X: f32 = 196.0;
        const PHOTO_BOTTOM: f32 = 965.0;
        const BAR_TOP: f32 = 984.0;
        let w = WIDTH as f32;
        pm.fill(color(FRAME_TEAL, 255));
        fill_rect(pm, BAR, 0.0, w - BAR, BAR_TOP, [255; 3], 255);
        let img = if photo == Photo::Inside { &self.inside } else { &self.outside };
        if let Some(img) = img {
            let mut part = Pixmap::new((w - PHOTO_X) as u32, PHOTO_BOTTOM as u32).unwrap();
            draw_cover(&mut part, img, 0.0, 0.0, w - PHOTO_X, PHOTO_BOTTOM);
            pm.draw_pixmap(PHOTO_X as i32, 0, part.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
        }
        // The thin white L inside the photo.
        fill_rect(pm, PHOTO_X + 8.0, 0.0, 3.0, PHOTO_BOTTOM - 8.0, [255; 3], 255);
        fill_rect(pm, PHOTO_X + 8.0, PHOTO_BOTTOM - 11.0, w - PHOTO_X - 8.0, 3.0, [255; 3], 255);
        // Logo, dark on a white tile.
        fill_rect(pm, 0.0, 0.0, 195.0, 140.0, [255; 3], 255);
        if let Some(logo) = &self.logo {
            let scale = (187.0 / logo.width() as f32).min(134.0 / logo.height() as f32);
            let (lw, lh) = (logo.width() as f32 * scale, logo.height() as f32 * scale);
            let mut dark = logo.clone();
            for px in dark.data_mut().chunks_exact_mut(4) {
                px[0] = 0;
                px[1] = 0;
                px[2] = 0;
            }
            let t = Transform::from_scale(scale, scale).post_translate((195.0 - lw) / 2.0, (140.0 - lh) / 2.0);
            let paint = PixmapPaint { quality: tiny_skia::FilterQuality::Bicubic, ..Default::default() };
            pm.draw_pixmap(0, 0, dark.as_ref(), &paint, t, None);
        }
        self.draw_qr(pm, (BAR - 115.0) / 2.0, 940.0, 115.0);
        // One line in the bar, as large as the church's own (70 px), smaller only when it
        // does not fit, down to 40 px.
        let (x0, x1) = (BAR, 1890.0);
        let style = |size: f32| TextStyle::new(size, Face::SansBold, [255; 3]);
        let mut size = 70.0;
        loop {
            let lw = self.measure(&[(left, style(size))]);
            let rw = if right.is_empty() { 0.0 } else { 60.0 + self.measure(&[(right, style(size))]) };
            if lw + rw <= x1 - x0 || size <= 40.0 {
                break;
            }
            size -= 2.0;
        }
        // Capitals centred in the bar.
        let y = (BAR_TOP + HEIGHT as f32) / 2.0 - 0.68 * size;
        if !left.is_empty() {
            self.line(pm, &[(left, style(size))], x0, y);
        }
        if !right.is_empty() {
            let rw = self.measure(&[(right, style(size))]);
            self.line(pm, &[(right, style(size))], x1 - rw, y);
        }
    }

    /// Draws the QR code on a white rounded tile of `size` pixels.
    fn draw_qr(&self, pm: &mut Pixmap, x: f32, y: f32, size: f32) {
        let Some((width, dark)) = &self.qr else { return };
        fill_round_rect(pm, x, y, size, size, 14.0, [255; 3], 255);
        let quiet = 2.0;
        let cell = size / (*width as f32 + 2.0 * quiet);
        for (i, is_dark) in dark.iter().enumerate() {
            if *is_dark {
                let (cx, cy) = ((i % width) as f32, (i / width) as f32);
                fill_rect(pm, x + (cx + quiet) * cell, y + (cy + quiet) * cell, cell + 0.5, cell + 0.5, [0x10, 0x1a, 0x1c], 255);
            }
        }
    }

    fn buffer(&mut self, spans: &[(&str, TextStyle)]) -> Buffer {
        let base = spans.first().map(|s| s.1).unwrap();
        let mut buf = Buffer::new(&mut self.fonts, Metrics::new(base.size, base.size * 1.25));
        buf.set_size(None, None);
        let attrs = |s: TextStyle| {
            let [r, g, b] = s.color;
            Attrs::new()
                .family(Family::Name(if s.face == Face::Serif { "Source Serif 4" } else { "Source Sans 3" }))
                .weight(if s.face == Face::SansBold { Weight::SEMIBOLD } else { Weight::NORMAL })
                .metrics(Metrics::new(s.size, s.size * 1.25))
                .letter_spacing(s.tracking)
                .color(Color::rgb(r, g, b))
        };
        // Typographic apostrophe: ’t instead of 't.
        let texts: Vec<String> = spans.iter().map(|(t, _)| t.replace('\'', "\u{2019}")).collect();
        let rich: Vec<_> = spans.iter().zip(&texts).map(|((_, s), t)| (t.as_str(), attrs(*s))).collect();
        buf.set_rich_text(rich, &attrs(base), Shaping::Advanced, Some(Align::Left));
        buf.shape_until_scroll(&mut self.fonts, false);
        buf
    }

    /// Width of one line of text.
    fn measure(&mut self, spans: &[(&str, TextStyle)]) -> f32 {
        self.buffer(spans).layout_runs().map(|r| r.line_w).fold(0.0, f32::max)
    }

    /// Draws one line of text with its top at y.
    fn line(&mut self, pm: &mut Pixmap, spans: &[(&str, TextStyle)], x: f32, y: f32) {
        let mut buf = self.buffer(spans);
        self.draw_buffer(pm, &mut buf, x, y);
    }

    /// Draws laid-out text with its top left corner at (x, y).
    fn draw_buffer(&mut self, pm: &mut Pixmap, buf: &mut Buffer, x: f32, y: f32) {
        let (pw, ph) = (pm.width() as i32, pm.height() as i32);
        let data = pm.data_mut();
        buf.draw(&mut self.fonts, &mut self.cache, Color::rgb(0, 0, 0), |gx, gy, gw, gh, c| {
            for dy in 0..gh as i32 {
                for dx in 0..gw as i32 {
                    let (px, py) = (x as i32 + gx + dx, y as i32 + gy + dy);
                    if px >= 0 && py >= 0 && px < pw && py < ph {
                        blend(data, (py * pw + px) as usize * 4, c);
                    }
                }
            }
        });
    }
}

/// Smallest letters in a box of the church's own screens.
const MIN_BOX_TEXT: f32 = 14.0;

fn alpha(opacity: f32) -> u8 {
    (opacity.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// The outline of a box: an ellipse, a rectangle, or one with round corners.
fn box_path(b: &LayoutBox) -> Option<tiny_skia::Path> {
    if b.round {
        PathBuilder::from_oval(Rect::from_xywh(b.x, b.y, b.w, b.h)?)
    } else {
        round_rect_path(b.x, b.y, b.w, b.h, b.radius)
    }
}

fn color(c: [u8; 3], a: u8) -> tiny_skia::Color {
    tiny_skia::Color::from_rgba8(c[0], c[1], c[2], a)
}

fn fill_path(pm: &mut Pixmap, path: Option<tiny_skia::Path>, c: [u8; 3], a: u8) {
    let Some(path) = path else { return };
    let mut paint = Paint::default();
    paint.set_color(color(c, a));
    paint.anti_alias = true;
    pm.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
}

fn fill_rect(pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, c: [u8; 3], a: u8) {
    let mut paint = Paint::default();
    paint.set_color(color(c, a));
    if let Some(r) = Rect::from_xywh(x, y, w, h) {
        pm.fill_rect(r, &paint, Transform::identity(), None);
    }
}

fn fill_round_rect(pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, radius: f32, c: [u8; 3], a: u8) {
    fill_path(pm, round_rect_path(x, y, w, h, radius), c, a);
}

fn round_rect_path(x: f32, y: f32, w: f32, h: f32, radius: f32) -> Option<tiny_skia::Path> {
    let r = radius.min(w / 2.0).min(h / 2.0);
    if r <= 0.0 {
        return Some(PathBuilder::from_rect(Rect::from_xywh(x, y, w, h)?));
    }
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.quad_to(x + w, y, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.quad_to(x + w, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.quad_to(x, y + h, x, y + h - r);
    pb.line_to(x, y + r);
    pb.quad_to(x, y, x + r, y);
    pb.close();
    pb.finish()
}

/// Draws the whole photo inside a box of w x h at the top left, centred.
fn draw_contain(pm: &mut Pixmap, photo: &Pixmap, w: f32, h: f32) {
    let scale = (w / photo.width() as f32).min(h / photo.height() as f32);
    let (sw, sh) = (photo.width() as f32 * scale, photo.height() as f32 * scale);
    let t = Transform::from_scale(scale, scale).post_translate((w - sw) / 2.0, (h - sh) / 2.0);
    let paint = PixmapPaint { quality: tiny_skia::FilterQuality::Bicubic, ..Default::default() };
    pm.draw_pixmap(0, 0, photo.as_ref(), &paint, t, None);
}

/// Draws the photo scaled to cover the box, cropping what sticks out.
fn draw_cover(pm: &mut Pixmap, photo: &Pixmap, x: f32, y: f32, w: f32, h: f32) {
    let scale = (w / photo.width() as f32).max(h / photo.height() as f32);
    let (sw, sh) = (photo.width() as f32 * scale, photo.height() as f32 * scale);
    let t = Transform::from_scale(scale, scale).post_translate(x + (w - sw) / 2.0, y + (h - sh) / 2.0);
    let paint = PixmapPaint { quality: tiny_skia::FilterQuality::Bicubic, ..Default::default() };
    pm.draw_pixmap(0, 0, photo.as_ref(), &paint, t, None);
}

/// Source-over blend of a straight-alpha glyph colour into premultiplied pixels.
fn blend(data: &mut [u8], i: usize, c: Color) {
    let a = c.a() as u32;
    if a == 0 {
        return;
    }
    let inv = 255 - a;
    for (k, s) in [c.r(), c.g(), c.b()].into_iter().enumerate() {
        data[i + k] = ((s as u32 * a + data[i + k] as u32 * inv) / 255) as u8;
    }
    data[i + 3] = (a + data[i + 3] as u32 * inv / 255) as u8;
}

/// tiny-skia stores premultiplied alpha; GStreamer's RGBA is straight alpha.
fn to_straight_rgba(pm: Pixmap) -> Vec<u8> {
    let mut data = pm.take();
    for px in data.chunks_exact_mut(4) {
        let a = px[3] as u32;
        if a != 0 && a != 255 {
            for c in &mut px[..3] {
                *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
    data
}

/// Writes every scene as a PNG, to check the design without a camera.
pub fn write_examples(dir: &std::path::Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    let mut r = Renderer::with_builtin_images("https://www.hhgtholen.nl");
    let p = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let scenes = [
        ("welkom", Scene::Frame { photo: Photo::Outside, left: "Zondag 18 oktober 2026".into(), right: "Middagdienst".into() }),
        ("einde", Scene::Frame { photo: Photo::Inside, left: "Vanmiddag is er een dienst om 16.00 uur.".into(), right: String::new() }),
        ("einde-www", Scene::Frame {
            photo: Photo::Inside,
            left: "Voor meer informatie over onze gemeente kijk op www.hhgtholen.nl".into(),
            right: String::new(),
        }),
        ("balk", Scene::Bar { label: "Slotzang".into(), text: "Psalm 42 : 5".into() }),
        ("gebed", Scene::Bar { label: String::new(), text: "Gebed".into() }),
        // A point of the theme the operator tapped during the sermon, short and long.
        ("punt", Scene::Bar { label: "Verrast door vreugde".into(), text: "2. Straks volkomen".into() }),
        ("punt-lang", Scene::Bar { label: LONG_THEME.into(), text: LONG_POINT.into() }),
        ("psalm", Scene::Panel {
            label: "Zingen".into(),
            title: "Psalm 49 : 6".into(),
            paragraphs: p(&[
                "Men denkt niet meer aan hun verleden staat,",
                "Wijl al hun glans met hen in 't graf vergaat;",
                "Maar na den dood is 't leven mij bereid;",
                "God neemt mij op in Zijne heerlijkheid.",
                "Vreest hem dan niet, die grote schatten heeft,",
                "Wiens machtig huis in eer en aanzien leeft;",
                "Want hij zal niets in 't sterven met zich dragen;",
                "Zijn naam, zijn roem, 't ligt al terneer geslagen.",
            ]),
            style: Body::Lines,
            footer: String::new(),
            qr: false,
        }),
        ("lezing", Scene::Panel {
            label: "Schriftlezing".into(),
            title: "1 Korinthe 6:15-20".into(),
            paragraphs: p(&[
                "15 Weet gij niet, dat uw lichamen leden van Christus zijn? Zal ik dan de leden van Christus nemen, en maken ze leden ener hoer? Dat zij verre.",
                "16 Of weet gij niet, dat die de hoer aanhangt, een lichaam met haar is? Want die twee, zegt Hij, zullen tot een vlees wezen.",
                "17 Maar die den Heere aanhangt, is een geest met Hem.",
                "18 Vliedt de hoererij. Alle zonde, die de mens doet, is buiten het lichaam, maar die hoererij bedrijft, zondigt tegen zijn eigen lichaam.",
            ]),
            style: Body::Verses,
            footer: String::new(),
            qr: false,
        }),
        ("catechismus", Scene::Panel {
            label: "Heidelbergse Catechismus".into(),
            title: "Vraag 57".into(),
            paragraphs: p(&[
                "Wat troost geeft u de opstanding des vleses?",
                "Dat niet alleen mijn ziel na dit leven van stonden aan tot Christus, haar Hoofd, zal opgenomen worden, maar dat ook dit mijn vlees, door de kracht van Christus opgewekt zijnde, wederom met mijn ziel verenigd en aan het heerlijk lichaam van Christus gelijkvormig zal worden.",
            ]),
            style: Body::Question,
            footer: "Zondag 22".into(),
            qr: false,
        }),
        ("collecte", Scene::Panel {
            label: "Collecte".into(),
            title: "Collectedoelen".into(),
            paragraphs: p(&["1. GDC Noodhulp en Rampenfonds", "2. Beheer en onderhoud Levensbron", "3. Aflossing Bouwfonds"]),
            style: Body::Lines,
            footer: String::new(),
            qr: true,
        }),
    ];
    for (name, scene) in scenes {
        let img = image::RgbaImage::from_raw(WIDTH, HEIGHT, r.render(&scene)).unwrap();
        img.save(dir.join(format!("{name}.png")))?;
    }
    println!("scenes geschreven naar {}", dir.display());
    Ok(())
}

/// A long theme and point, as some predikanten have them.
const LONG_THEME: &str = "De Goede Herder Die Zijn leven geeft voor de schapen van Zijn kudde";
const LONG_POINT: &str = "3. Hij kent de Zijnen en Hij wordt gekend van de Zijnen, tot in eeuwigheid";

#[cfg(test)]
mod tests {
    use super::*;

    /// The columns with anything drawn in them, as (leftmost, rightmost).
    fn drawn(rgba: &[u8]) -> (u32, u32) {
        let xs = rgba.chunks_exact(4).enumerate().filter(|(_, px)| px[3] > 0).map(|(i, _)| i as u32 % WIDTH);
        xs.fold((WIDTH, 0), |(lo, hi), x| (lo.min(x), hi.max(x)))
    }

    #[test]
    fn a_long_point_stays_on_screen() {
        let mut r = Renderer::with_builtin_images("");
        let short = drawn(&r.render(&Scene::Bar { label: "Verrast door vreugde".into(), text: "2. Straks volkomen".into() }));
        assert_eq!(short.0, 80);
        assert!(short.1 < 1000, "{short:?}");
        // Smaller letters, the card as wide as the picture allows and the text inside it.
        let long = drawn(&r.render(&Scene::Bar { label: LONG_THEME.into(), text: LONG_POINT.into() }));
        assert_eq!(long.0, 80);
        assert!(long.1 > 1500 && long.1 < WIDTH - 80, "{long:?}");
    }
}
