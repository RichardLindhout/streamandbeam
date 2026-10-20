//! Draws the overlay for the YouTube stream, in the style of the church's own stream:
//! a white panel with a large round opening showing the camera (the text runs along the
//! curve), a lower-third card over the camera, or the church photo before and after the
//! service. Transparent pixels show the camera.

use cosmic_text::{Align, Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, SwashCache, Weight};
use tiny_skia::{FillRule, GradientStop, LinearGradient, Paint, PathBuilder, Pixmap, PixmapPaint, Point, Rect, SpreadMode, Transform};

pub const WIDTH: u32 = 1920;
pub const HEIGHT: u32 = 1080;

const TEAL: [u8; 3] = [0x00, 0x4b, 0x58];
const TEAL_2: [u8; 3] = [0x07, 0x69, 0x7a];
const INK: [u8; 3] = [0x26, 0x2d, 0x2f];
const WARM: [u8; 3] = [0x8f, 0x83, 0x77];
const SAND: [u8; 3] = [0xe3, 0xda, 0xcf];
const PAPER: [u8; 3] = [0xfb, 0xfa, 0xf7];

/// The camera opening of the panel, measured on the church's stream: a circle with its
/// centre just right of the picture, near the top.
const CIRCLE: (f32, f32, f32) = (2025.0, 200.0, 950.0);
/// Left margin of the panel text.
const LEFT: f32 = 96.0;

#[derive(Debug, Clone, PartialEq)]
pub enum Scene {
    /// Only the camera.
    Camera,
    /// Camera with a card at the bottom: a small label ("ZINGEN") and the text ("Psalm 42 : 5").
    Bar { label: String, text: String },
    /// White panel with the text on the left, the camera in the round opening on the right.
    Panel { label: String, title: String, paragraphs: Vec<String>, style: Body, footer: String, qr: bool },
    /// The church photo (inside before the service, outside after it) with text.
    Holding { photo: Photo, label: String, title: String, sub: String, qr: bool },
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
}

/// Source Serif 4 for the texts (psalms, readings), Source Sans 3 for headings and labels.
#[derive(Clone, Copy, PartialEq)]
enum Face {
    Serif,
    Sans,
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
        let qr = (!qr_link.is_empty()).then(|| qrcode::QrCode::new(qr_link).ok()).flatten().map(|code| {
            (code.width(), code.to_colors().into_iter().map(|c| c == qrcode::Color::Dark).collect())
        });
        Renderer {
            fonts: FontSystem::new_with_locale_and_db("nl-NL".into(), db),
            cache: SwashCache::new(),
            inside: load(images.inside),
            outside: load(images.outside),
            logo: load(images.logo),
            qr,
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

    /// Renders a scene to straight-alpha RGBA pixels of WIDTH x HEIGHT.
    pub fn render(&mut self, scene: &Scene) -> Vec<u8> {
        let mut pm = Pixmap::new(WIDTH, HEIGHT).unwrap();
        match scene {
            Scene::Camera => {}
            Scene::Bar { label, text } => self.bar(&mut pm, label, text),
            Scene::Panel { label, title, paragraphs, style, footer, qr } => self.panel(&mut pm, label, title, paragraphs, *style, footer, *qr),
            Scene::Holding { photo, label, title, sub, qr } => self.holding(&mut pm, *photo, label, title, sub, *qr),
        }
        to_straight_rgba(pm)
    }

    /// Lower-third card: a small label above the main text, a warm stripe on the left.
    fn bar(&mut self, pm: &mut Pixmap, label: &str, text: &str) {
        let main = TextStyle::new(56.0, Face::SansBold, [255; 3]);
        let small = TextStyle { tracking: 0.16, ..TextStyle::new(24.0, Face::SansBold, SAND) };
        let label = label.to_uppercase();
        let width = self.measure(&[(text, main)]).max(self.measure(&[(&label, small)])) + 140.0;
        let (x, h) = (80.0, if label.is_empty() { 112.0 } else { 140.0 });
        let y = HEIGHT as f32 - 90.0 - h;
        fill_round_rect(pm, x, y, width, h, 16.0, TEAL, 238);
        fill_round_rect(pm, x, y, 12.0, h, 0.0, SAND, 255);
        if label.is_empty() {
            self.line(pm, &[(text, main)], x + 60.0, y + (h - 56.0 * 1.25) / 2.0);
        } else {
            self.line(pm, &[(&label, small)], x + 60.0, y + 22.0);
            self.line(pm, &[(text, main)], x + 60.0, y + 52.0);
        }
    }

    /// White panel with the camera in a large round opening; the text runs along the curve.
    #[allow(clippy::too_many_arguments)]
    fn panel(&mut self, pm: &mut Pixmap, label: &str, title: &str, paragraphs: &[String], style: Body, footer: &str, qr: bool) {
        let (cx, cy, r) = CIRCLE;
        pm.fill(color(PAPER, 255));
        // A fine coloured ring along the opening, then the opening itself.
        fill_path(pm, PathBuilder::from_circle(cx, cy, r + 9.0), TEAL_2, 255);
        let mut clear = Paint::default();
        clear.blend_mode = tiny_skia::BlendMode::Clear;
        pm.fill_path(&PathBuilder::from_circle(cx, cy, r).unwrap(), &clear, FillRule::Winding, Transform::identity(), None);

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
        for (ly, spans) in lines {
            let spans: Vec<(&str, TextStyle)> = spans.iter().map(|(t, s)| (t.as_str(), *s)).collect();
            self.line(pm, &spans, LEFT, ly);
        }
        if !footer.is_empty() {
            self.line(pm, &[(footer, TextStyle::new(34.0, Face::SansBold, TEAL))], LEFT, HEIGHT as f32 - 110.0);
        }
        if qr {
            let x = if footer.is_empty() { LEFT } else { LEFT + 360.0 };
            self.draw_qr(pm, x, HEIGHT as f32 - 210.0, 150.0);
        }
    }

    /// Lays out paragraphs line by line; each line is as wide as the panel is at that
    /// height (narrow at the top, wider lower down where the curve moves right).
    /// Returns None when it does not fit above `bottom`.
    fn flow(&mut self, paragraphs: &[String], style: Body, size: f32, top: f32, bottom: f32) -> Option<Vec<(f32, Vec<(String, TextStyle)>)>> {
        let numbered = style == Body::Verses;
        let number = TextStyle::new(size * 0.6, Face::SansBold, TEAL_2);
        let line_h = size * if style == Body::Lines { 1.38 } else { 1.42 };
        let gap = if style == Body::Lines { 0.0 } else { size * 0.35 };
        let mut out = Vec::new();
        let mut y = top;
        for (k, p) in paragraphs.iter().enumerate() {
            let colour = if style == Body::Question && k == 0 { TEAL_2 } else { INK };
            let body = TextStyle::new(size, Face::Serif, colour);
            if style == Body::Lines {
                if self.measure(&[(p, body)]) > available(y, line_h) {
                    return None;
                }
                out.push((y, vec![(p.clone(), body)]));
                y += line_h;
                continue;
            }
            let mut words: Vec<&str> = p.split_whitespace().collect();
            let mut line: Vec<(String, TextStyle)> = Vec::new();
            if numbered && !words.is_empty() {
                line.push((format!("{} ", words.remove(0)), number));
            }
            let mut text = String::new();
            for w in words {
                let candidate = if text.is_empty() { w.to_string() } else { format!("{text} {w}") };
                let mut spans = line.clone();
                spans.push((candidate.clone(), body));
                let refs: Vec<(&str, TextStyle)> = spans.iter().map(|(t, s)| (t.as_str(), *s)).collect();
                if !text.is_empty() && self.measure(&refs) > available(y, line_h) {
                    line.push((text, body));
                    out.push((y, std::mem::take(&mut line)));
                    y += line_h;
                    text = w.to_string();
                } else {
                    text = candidate;
                }
            }
            if !text.is_empty() {
                line.push((text, body));
            }
            if !line.is_empty() {
                out.push((y, line));
                y += line_h;
            }
            y += gap;
        }
        (y - gap <= bottom).then_some(out)
    }

    /// Church photo over the whole picture, a dark fade at the bottom with the text,
    /// the logo top left and the QR code bottom right.
    fn holding(&mut self, pm: &mut Pixmap, photo: Photo, label: &str, title: &str, sub: &str, qr: bool) {
        let (w, h) = (WIDTH as f32, HEIGHT as f32);
        pm.fill(color(TEAL, 255));
        let img = if photo == Photo::Inside { &self.inside } else { &self.outside };
        if let Some(img) = img {
            draw_cover(pm, img, 0.0, 0.0, w, h);
        }
        let fade = LinearGradient::new(
            Point::from_xy(0.0, h * 0.38),
            Point::from_xy(0.0, h),
            vec![GradientStop::new(0.0, color([0, 0x1c, 0x22], 0)), GradientStop::new(1.0, color([0, 0x1c, 0x22], 235))],
            SpreadMode::Pad,
            Transform::identity(),
        );
        if let Some(shader) = fade {
            let mut paint = Paint::default();
            paint.shader = shader;
            pm.fill_rect(Rect::from_xywh(0.0, 0.0, w, h).unwrap(), &paint, Transform::identity(), None);
        }
        if let Some(logo) = &self.logo {
            let scale = 230.0 / logo.width() as f32;
            let t = Transform::from_scale(scale, scale).post_translate(80.0, 64.0);
            let paint = PixmapPaint { quality: tiny_skia::FilterQuality::Bicubic, ..Default::default() };
            pm.draw_pixmap(0, 0, logo.as_ref(), &paint, t, None);
        }
        let mut y = h - 120.0 - 56.0;
        if !sub.is_empty() {
            self.line(pm, &[(sub, TextStyle::new(42.0, Face::Sans, SAND))], 96.0, y);
            y -= 96.0;
        } else {
            y += 40.0;
        }
        self.line(pm, &[(title, TextStyle::new(76.0, Face::SansBold, [255; 3]))], 96.0, y);
        if !label.is_empty() {
            let small = TextStyle { tracking: 0.18, ..TextStyle::new(26.0, Face::SansBold, SAND) };
            self.line(pm, &[(&label.to_uppercase(), small)], 96.0, y - 44.0);
        }
        if qr && self.qr.is_some() {
            let (s, x) = (180.0, w - 96.0 - 180.0);
            let yq = h - 96.0 - s - 40.0;
            self.draw_qr(pm, x, yq, s);
            let small = TextStyle::new(26.0, Face::Sans, SAND);
            let cap = "www.hhgtholen.nl";
            let cw = self.measure(&[(cap, small)]);
            self.line(pm, &[(cap, small)], x + (s - cw) / 2.0, yq + s + 12.0);
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

/// Room for text on a line from y to y+line_h: up to the curve, minus some air.
fn available(y: f32, line_h: f32) -> f32 {
    let (cx, cy, r) = CIRCLE;
    let edge = |yy: f32| {
        let d = yy - cy;
        if d.abs() >= r { WIDTH as f32 } else { cx - (r * r - d * d).sqrt() }
    };
    edge(y).min(edge(y + line_h)) - LEFT - 48.0
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
    let r = radius.min(w / 2.0).min(h / 2.0);
    if r <= 0.0 {
        return fill_rect(pm, x, y, w, h, c, a);
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
    fill_path(pm, pb.finish(), c, a);
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
        ("welkom", Scene::Holding {
            photo: Photo::Inside,
            label: "Welkom".into(),
            title: "Zondag 18 oktober 2026".into(),
            sub: "Middagdienst · ds. A. den Hartog jr.".into(),
            qr: true,
        }),
        ("einde", Scene::Holding {
            photo: Photo::Outside,
            label: "Volgende dienst".into(),
            title: "Zondag 25 oktober om 9.30 uur".into(),
            sub: "ds. K. Klopstra".into(),
            qr: true,
        }),
        ("balk", Scene::Bar { label: "Slotzang".into(), text: "Psalm 42 : 5".into() }),
        ("gebed", Scene::Bar { label: String::new(), text: "Gebed".into() }),
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
