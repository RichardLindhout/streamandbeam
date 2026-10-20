//! Draws the overlay for the YouTube stream, in the style of the church's stream:
//! a white panel with a round cut-out showing the camera, a bar over the camera, or the
//! church frame before and after the service. Transparent pixels show the camera.

use cosmic_text::{Align, Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, SwashCache, Weight};
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, PixmapPaint, Rect, Transform};

pub const WIDTH: u32 = 1920;
pub const HEIGHT: u32 = 1080;

const TEAL: [u8; 3] = [0x00, 0x4b, 0x58];
const INK: [u8; 3] = [0x22, 0x2a, 0x2c];

#[derive(Debug, Clone, PartialEq)]
pub enum Scene {
    /// Only the camera.
    Camera,
    /// Camera with a bar at the bottom, e.g. "Gebed" or "Slotzang: Psalm 42 : 5".
    Bar { text: String },
    /// White panel with title and text on the left, camera in a round cut-out on the right.
    Panel { title: String, paragraphs: Vec<String> },
    /// Church frame shown before and after the service.
    Holding { title: String, sub: String },
}

pub struct Renderer {
    fonts: FontSystem,
    cache: SwashCache,
    photo: Option<Pixmap>,
}

#[derive(Clone, Copy)]
struct TextStyle {
    size: f32,
    bold: bool,
    color: [u8; 3],
    align: Align,
}

impl Renderer {
    pub fn new(photo: Option<&[u8]>) -> Renderer {
        let mut db = cosmic_text::fontdb::Database::new();
        db.load_font_data(include_bytes!("../assets/fonts/Lato-Regular.ttf").to_vec());
        db.load_font_data(include_bytes!("../assets/fonts/Lato-Bold.ttf").to_vec());
        let photo = photo.and_then(|bytes| {
            let img = image::load_from_memory(bytes).ok()?.to_rgba8();
            let mut pm = Pixmap::new(img.width(), img.height())?;
            pm.data_mut().copy_from_slice(img.as_raw()); // opaque photo, so already premultiplied
            Some(pm)
        });
        Renderer { fonts: FontSystem::new_with_locale_and_db("nl-NL".into(), db), cache: SwashCache::new(), photo }
    }

    /// Renders a scene to straight-alpha RGBA pixels of WIDTH x HEIGHT.
    pub fn render(&mut self, scene: &Scene) -> Vec<u8> {
        let mut pm = Pixmap::new(WIDTH, HEIGHT).unwrap();
        let (w, h) = (WIDTH as f32, HEIGHT as f32);
        match scene {
            Scene::Camera => {}
            Scene::Bar { text } => {
                fill_rect(&mut pm, 0.0, h - 130.0, w, 130.0, TEAL, 235);
                let style = TextStyle { size: 60.0, bold: true, color: [255; 3], align: Align::Left };
                self.text(&mut pm, &[(text.as_str(), style)], 90.0, h - 130.0, w - 180.0, 130.0, true);
            }
            Scene::Panel { title, paragraphs } => {
                pm.fill(tiny_skia::Color::WHITE);
                // Round cut-out on the right through which the camera is seen.
                let mut pb = PathBuilder::new();
                pb.push_circle(2716.0, -200.0, 1653.0);
                let mut clear = Paint::default();
                clear.blend_mode = tiny_skia::BlendMode::Clear;
                pm.fill_path(&pb.finish().unwrap(), &clear, FillRule::Winding, Transform::identity(), None);
                let title_style = TextStyle { size: 54.0, bold: true, color: TEAL, align: Align::Left };
                self.text(&mut pm, &[(title.as_str(), title_style)], 90.0, 50.0, 960.0, 90.0, false);
                let body = paragraphs.join("\n");
                let style = TextStyle { size: 46.0, bold: false, color: INK, align: Align::Left };
                self.fit_text(&mut pm, &body, style, 90.0, 170.0, 960.0, h - 230.0);
            }
            Scene::Holding { title, sub } => {
                pm.fill(color(TEAL, 255));
                let (px, py, pw, ph) = (130.0, 60.0, w - 190.0, h - 230.0);
                if let Some(photo) = &self.photo {
                    draw_cover(&mut pm, photo, px, py, pw, ph);
                }
                fill_rect(&mut pm, px, h - 170.0, pw, 110.0, [255; 3], 255);
                let left = TextStyle { size: 50.0, bold: true, color: TEAL, align: Align::Left };
                let right = TextStyle { align: Align::Right, ..left };
                self.text(&mut pm, &[(title.as_str(), left)], px + 40.0, h - 170.0, pw - 80.0, 110.0, true);
                self.text(&mut pm, &[(sub.as_str(), right)], px + 40.0, h - 170.0, pw - 80.0, 110.0, true);
            }
        }
        to_straight_rgba(pm)
    }

    /// Draws text in the box, shrinking the font until it fits.
    fn fit_text(&mut self, pm: &mut Pixmap, text: &str, mut style: TextStyle, x: f32, y: f32, w: f32, h: f32) {
        while style.size > 20.0 && self.measure(text, style, w) > h {
            style.size -= 2.0;
        }
        self.text(pm, &[(text, style)], x, y, w, h, false);
    }

    fn buffer(&mut self, spans: &[(&str, TextStyle)], w: f32) -> Buffer {
        let base = spans.first().map(|s| s.1).unwrap();
        let mut buf = Buffer::new(&mut self.fonts, Metrics::new(base.size, base.size * 1.3));
        buf.set_size(Some(w), None);
        let attrs = |s: TextStyle| {
            let [r, g, b] = s.color;
            Attrs::new()
                .family(Family::Name("Lato"))
                .weight(if s.bold { Weight::BOLD } else { Weight::NORMAL })
                .color(Color::rgb(r, g, b))
        };
        let rich: Vec<_> = spans.iter().map(|(t, s)| (*t, attrs(*s))).collect();
        buf.set_rich_text(rich, &attrs(base), Shaping::Advanced, Some(base.align));
        buf.shape_until_scroll(&mut self.fonts, false);
        buf
    }

    fn measure(&mut self, text: &str, style: TextStyle, w: f32) -> f32 {
        let buf = self.buffer(&[(text, style)], w);
        buf.layout_runs().map(|r| r.line_top + r.line_height).fold(0.0, f32::max)
    }

    fn text(&mut self, pm: &mut Pixmap, spans: &[(&str, TextStyle)], x: f32, y: f32, w: f32, h: f32, center_v: bool) {
        let mut buf = self.buffer(spans, w);
        let height = buf.layout_runs().map(|r| r.line_top + r.line_height).fold(0.0, f32::max);
        let top = if center_v { y + (h - height) / 2.0 } else { y };
        let (pw, ph) = (pm.width() as i32, pm.height() as i32);
        let data = pm.data_mut();
        buf.draw(&mut self.fonts, &mut self.cache, Color::rgb(0, 0, 0), |gx, gy, gw, gh, c| {
            for dy in 0..gh as i32 {
                for dx in 0..gw as i32 {
                    let (px, py) = (x as i32 + gx + dx, top as i32 + gy + dy);
                    if px >= 0 && py >= 0 && px < pw && py < ph {
                        blend(data, (py * pw + px) as usize * 4, c);
                    }
                }
            }
        });
    }
}

fn color(c: [u8; 3], a: u8) -> tiny_skia::Color {
    tiny_skia::Color::from_rgba8(c[0], c[1], c[2], a)
}

fn fill_rect(pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, c: [u8; 3], a: u8) {
    let mut paint = Paint::default();
    paint.set_color(color(c, a));
    if let Some(r) = Rect::from_xywh(x, y, w, h) {
        pm.fill_rect(r, &paint, Transform::identity(), None);
    }
}

/// Draws the photo scaled to cover the box, cropping what sticks out.
fn draw_cover(pm: &mut Pixmap, photo: &Pixmap, x: f32, y: f32, w: f32, h: f32) {
    let scale = (w / photo.width() as f32).max(h / photo.height() as f32);
    let (sw, sh) = (photo.width() as f32 * scale, photo.height() as f32 * scale);
    let clip = {
        let mut mask = tiny_skia::Mask::new(pm.width(), pm.height()).unwrap();
        let rect = PathBuilder::from_rect(Rect::from_xywh(x, y, w, h).unwrap());
        mask.fill_path(&rect, FillRule::Winding, false, Transform::identity());
        mask
    };
    let t = Transform::from_scale(scale, scale).post_translate(x + (w - sw) / 2.0, y + (h - sh) / 2.0);
    let paint = PixmapPaint { quality: tiny_skia::FilterQuality::Bicubic, ..Default::default() };
    pm.draw_pixmap(0, 0, photo.as_ref(), &paint, t, Some(&clip));
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
    let mut r = Renderer::new(Some(include_bytes!("../assets/images/kerk.jpg")));
    let scenes = [
        ("welkom", Scene::Holding { title: "Zondag 18 oktober 2026".into(), sub: "Middagdienst".into() }),
        ("balk", Scene::Bar { text: "Slotzang: Psalm 42 : 5".into() }),
        ("psalm", Scene::Panel {
            title: "Psalm 42 : 5".into(),
            paragraphs: ["Maar de HEER zal uitkomst geven,", "Hij, die 's daags zijn gunst gebiedt:", "'k Zal in dit vertrouwen leven,", "En dat melden in mijn lied."]
                .map(String::from).to_vec(),
        }),
    ];
    for (name, scene) in scenes {
        let img = image::RgbaImage::from_raw(WIDTH, HEIGHT, r.render(&scene)).unwrap();
        img.save(dir.join(format!("{name}.png")))?;
    }
    println!("scenes geschreven naar {}", dir.display());
    Ok(())
}
