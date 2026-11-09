//! Screens the church makes itself: boxes (text, picture, plane, camera, QR code) at a
//! place on the picture, as the scenes in OBS were built. A screen is for the stream
//! (1920x1080) or for the boards in church (1080x1920). With a `moment` it replaces the
//! automatic picture at that moment of the service; without one it is put up by hand.

use serde::{Deserialize, Serialize};
use std::path::Path;

pub const STREAM: &str = "uitzending";
pub const BOARD: &str = "bord";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Layout {
    pub id: String,
    pub name: String,
    /// "uitzending" or "bord".
    pub target: String,
    /// The moment whose automatic picture this replaces (see `director::moment_of` for the
    /// stream; the board kinds "bord", "thema" and "slotzang" for the boards). Empty = only
    /// by hand.
    pub moment: String,
    /// Colour behind everything, "#rrggbb". Empty = black for the stream; for a board the
    /// board's own background (wood or the church's picture).
    pub background: String,
    /// From back to front.
    pub boxes: Vec<LayoutBox>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LayoutBox {
    pub id: String,
    /// "tekst", "afbeelding", "vlak", "camera" or "qr".
    pub kind: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// Text with fields in braces: {kop} {onder} {tekst} {voet} {datum} {dienst} {tijd}
    /// {predikant}.
    pub text: String,
    /// "serif" or "sans".
    pub font: String,
    /// Largest letter size; the letters get smaller until the text fits the box.
    pub size: f32,
    pub bold: bool,
    pub color: String,
    /// "links", "midden" or "rechts".
    pub align: String,
    /// "boven", "midden" or "onder".
    pub valign: String,
    /// Colour of a plane, or behind a text ("" = none).
    pub fill: String,
    /// How solid the fill is, 0 to 1.
    pub opacity: f32,
    pub radius: f32,
    /// A circle or ellipse instead of a rectangle (plane, camera).
    pub round: bool,
    /// File name of the picture (see `web::image`).
    pub image: String,
    /// "vullen" (covers the box, cropped) or "passend" (whole picture inside the box).
    pub fit: String,
}

impl Default for LayoutBox {
    fn default() -> LayoutBox {
        LayoutBox {
            id: String::new(),
            kind: "tekst".into(),
            x: 0.0,
            y: 0.0,
            w: 400.0,
            h: 200.0,
            text: String::new(),
            font: "sans".into(),
            size: 60.0,
            bold: false,
            color: "#ffffff".into(),
            align: "links".into(),
            valign: "boven".into(),
            fill: String::new(),
            opacity: 1.0,
            radius: 0.0,
            round: false,
            image: String::new(),
            fit: "vullen".into(),
        }
    }
}

impl Layout {
    /// Where the camera goes: the first camera box, or None when the screen has none (then
    /// the camera is not in the picture).
    pub fn camera(&self) -> Option<&LayoutBox> {
        self.boxes.iter().find(|b| b.kind == "camera")
    }
}

/// The screen for a moment, when the church made one.
pub fn for_moment<'a>(layouts: &'a [Layout], target: &str, moment: &str) -> Option<&'a Layout> {
    layouts.iter().find(|l| l.target == target && !moment.is_empty() && l.moment == moment)
}

/// Fills the fields in braces; an unknown field stays as typed.
pub fn fill(text: &str, fields: &[(String, String)]) -> String {
    let mut out = text.to_string();
    for (k, v) in fields {
        out = out.replace(&format!("{{{k}}}"), v);
    }
    out
}

/// "#rrggbb" (or "#rgb") → colour; None when it is not one.
pub fn parse_color(s: &str) -> Option<[u8; 3]> {
    let h = s.trim().strip_prefix('#')?;
    let h: String = if h.len() == 3 { h.chars().flat_map(|c| [c, c]).collect() } else { h.to_string() };
    if h.len() != 6 {
        return None;
    }
    let n = u32::from_str_radix(&h, 16).ok()?;
    Some([(n >> 16) as u8, (n >> 8) as u8, n as u8])
}

/// A file name that is safe inside the pictures folder: letters, digits, "-", "_" and
/// one extension. None when nothing usable is left.
pub fn safe_name(name: &str) -> Option<String> {
    let name = name.rsplit(['/', '\\']).next().unwrap_or_default();
    let clean: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' { c } else { '-' }).collect();
    let clean = clean.trim_matches(['.', '-']).to_string();
    (!clean.is_empty() && !clean.contains("..")).then_some(clean)
}

pub fn load(dir: &Path) -> Vec<Layout> {
    crate::texts::read_json(&dir.join("schermen.json")).unwrap_or_default()
}

pub fn save(dir: &Path, layouts: &[Layout]) -> anyhow::Result<()> {
    crate::texts::write_json(&dir.join("schermen.json"), &layouts)
}

/// Adds or replaces a screen. A new one gets an id; a moment belongs to one screen, so
/// another screen that had it goes back to "by hand".
pub fn upsert(layouts: &mut Vec<Layout>, mut layout: Layout) -> Layout {
    if layout.target != BOARD {
        layout.target = STREAM.into();
    }
    if layout.id.is_empty() {
        let mut n = layouts.len() + 1;
        while layouts.iter().any(|l| l.id == format!("scherm-{n}")) {
            n += 1;
        }
        layout.id = format!("scherm-{n}");
    }
    for (i, b) in layout.boxes.iter_mut().enumerate() {
        if b.id.is_empty() {
            b.id = format!("vak-{}", i + 1);
        }
    }
    if !layout.moment.is_empty() {
        for other in layouts.iter_mut().filter(|l| l.id != layout.id && l.target == layout.target && l.moment == layout.moment) {
            other.moment.clear();
        }
    }
    match layouts.iter_mut().find(|l| l.id == layout.id) {
        Some(old) => *old = layout.clone(),
        None => layouts.push(layout.clone()),
    }
    layout
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fills_fields() {
        let f = vec![("kop".to_string(), "Psalm 42 : 5".to_string()), ("onder".to_string(), "Zingen".to_string())];
        assert_eq!(fill("{onder}: {kop} {onbekend}", &f), "Zingen: Psalm 42 : 5 {onbekend}");
    }

    #[test]
    fn colors() {
        assert_eq!(parse_color("#44546a"), Some([0x44, 0x54, 0x6a]));
        assert_eq!(parse_color("#fff"), Some([255, 255, 255]));
        assert_eq!(parse_color("rood"), None);
    }

    #[test]
    fn names() {
        assert_eq!(safe_name("C:\\Kerkdienst\\Template 2022.png").as_deref(), Some("Template-2022.png"));
        assert_eq!(safe_name("../../etc/passwd").as_deref(), Some("passwd"));
        assert_eq!(safe_name(".."), None);
    }

    #[test]
    fn a_moment_belongs_to_one_screen() {
        let mut all = Vec::new();
        let a = upsert(&mut all, Layout { name: "A".into(), moment: "psalm".into(), ..Default::default() });
        let b = upsert(&mut all, Layout { name: "B".into(), moment: "psalm".into(), ..Default::default() });
        assert_ne!(a.id, b.id);
        assert_eq!(all[0].moment, "");
        assert_eq!(for_moment(&all, STREAM, "psalm").map(|l| l.name.as_str()), Some("B"));
        // The boards have their own moments.
        upsert(&mut all, Layout { name: "C".into(), target: BOARD.into(), moment: "psalm".into(), ..Default::default() });
        assert_eq!(for_moment(&all, STREAM, "psalm").map(|l| l.name.as_str()), Some("B"));
        assert_eq!(for_moment(&all, STREAM, ""), None);
    }
}
