//! Text measurement with the embedded Liberation Sans (OFL), so that the app, the CLI and the
//! exported files all agree on the size of a label.

use std::sync::OnceLock;

/// Embedded fonts (SIL Open Font License, see `assets/fonts/OFL.txt`).
pub const SANS_REGULAR: &[u8] = include_bytes!("../../../assets/fonts/LiberationSans-Regular.ttf");
pub const SANS_BOLD: &[u8] = include_bytes!("../../../assets/fonts/LiberationSans-Bold.ttf");
pub const SANS_ITALIC: &[u8] = include_bytes!("../../../assets/fonts/LiberationSans-Italic.ttf");
pub const SANS_BOLD_ITALIC: &[u8] = include_bytes!("../../../assets/fonts/LiberationSans-BoldItalic.ttf");
pub const MONO_REGULAR: &[u8] = include_bytes!("../../../assets/fonts/LiberationMono-Regular.ttf");
pub const MONO_BOLD: &[u8] = include_bytes!("../../../assets/fonts/LiberationMono-Bold.ttf");

/// The family name used in generated SVG.
pub const FONT_FAMILY: &str = "Liberation Sans";

/// Default font size of diagram text, in pixels.
pub const FONT_SIZE: f32 = 15.0;

struct Metrics {
    regular: Vec<u16>,
    bold: Vec<u16>,
    units: f32,
}

fn build(data: &[u8]) -> Option<(Vec<u16>, f32)> {
    let face = ttf_parser::Face::parse(data, 0).ok()?;
    let units = f32::from(face.units_per_em()).max(1.0);
    // Advance widths for the Latin range; everything else uses the average below.
    let mut table = Vec::with_capacity(0x250);
    for cp in 0u32..0x250 {
        let adv = char::from_u32(cp).and_then(|c| face.glyph_index(c)).and_then(|g| face.glyph_hor_advance(g)).unwrap_or(0);
        table.push(adv);
    }
    Some((table, units))
}

fn metrics() -> &'static Metrics {
    static M: OnceLock<Metrics> = OnceLock::new();
    M.get_or_init(|| {
        let (regular, units) = build(SANS_REGULAR).unwrap_or((Vec::new(), 2048.0));
        let (bold, _) = build(SANS_BOLD).unwrap_or((Vec::new(), 2048.0));
        Metrics { regular, bold, units }
    })
}

/// Width of one line of text at `size` pixels.
pub fn text_width(text: &str, size: f32, bold: bool) -> f32 {
    let m = metrics();
    let table = if bold { &m.bold } else { &m.regular };
    let mut total = 0.0f32;
    for ch in text.chars().take(4096) {
        let cp = ch as usize;
        let adv = table.get(cp).copied().filter(|a| *a > 0 || ch == ' ');
        total += match adv {
            Some(a) => f32::from(a),
            None => {
                // Wide (CJK, emoji) characters are roughly one em; other unknowns about 0.6 em.
                if cp >= 0x2E80 { m.units } else { m.units * 0.6 }
            }
        };
    }
    total / m.units * size
}

/// Splits a label into lines (explicit line breaks only).
pub fn split_lines(label: &str) -> Vec<&str> {
    let v: Vec<&str> = label.split('\n').collect();
    if v.is_empty() { vec![""] } else { v }
}

/// Size (width, height) of a multi-line label.
pub fn label_size(label: &str, size: f32, bold: bool) -> (f32, f32) {
    let lines = split_lines(label);
    let w = lines.iter().map(|l| text_width(l, size, bold)).fold(0.0f32, f32::max);
    (w, lines.len() as f32 * size * 1.25)
}
