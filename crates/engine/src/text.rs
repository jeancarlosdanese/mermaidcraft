//! Text measurement with the embedded fonts (OFL), so that the app, the CLI and the exported
//! files all agree on the size of a label in every font the look can use.

use std::sync::OnceLock;

use crate::look::Font;

/// Embedded fonts (SIL Open Font License, see `assets/fonts/OFL.txt`).
pub const SANS_REGULAR: &[u8] = include_bytes!("../../../assets/fonts/LiberationSans-Regular.ttf");
pub const SANS_BOLD: &[u8] = include_bytes!("../../../assets/fonts/LiberationSans-Bold.ttf");
pub const SANS_ITALIC: &[u8] = include_bytes!("../../../assets/fonts/LiberationSans-Italic.ttf");
pub const SANS_BOLD_ITALIC: &[u8] = include_bytes!("../../../assets/fonts/LiberationSans-BoldItalic.ttf");
pub const MONO_REGULAR: &[u8] = include_bytes!("../../../assets/fonts/LiberationMono-Regular.ttf");
pub const MONO_BOLD: &[u8] = include_bytes!("../../../assets/fonts/LiberationMono-Bold.ttf");
pub const SHANTELL_REGULAR: &[u8] = include_bytes!("../../../assets/fonts/ShantellSans-Regular.ttf");
pub const SHANTELL_BOLD: &[u8] = include_bytes!("../../../assets/fonts/ShantellSans-Bold.ttf");
pub const PATRICK_HAND_REGULAR: &[u8] = include_bytes!("../../../assets/fonts/PatrickHand-Regular.ttf");
pub const KALAM_REGULAR: &[u8] = include_bytes!("../../../assets/fonts/Kalam-Regular.ttf");
pub const KALAM_BOLD: &[u8] = include_bytes!("../../../assets/fonts/Kalam-Bold.ttf");
pub const ARCHITECTS_REGULAR: &[u8] = include_bytes!("../../../assets/fonts/ArchitectsDaughter-Regular.ttf");

/// Every diagram font file, for font databases.
pub const ALL_FONT_DATA: &[&[u8]] = &[
    SANS_REGULAR,
    SANS_BOLD,
    SANS_ITALIC,
    SANS_BOLD_ITALIC,
    MONO_REGULAR,
    MONO_BOLD,
    SHANTELL_REGULAR,
    SHANTELL_BOLD,
    PATRICK_HAND_REGULAR,
    KALAM_REGULAR,
    KALAM_BOLD,
    ARCHITECTS_REGULAR,
];

/// The fallback family of generated SVG (and the family of the clean look).
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

fn metrics(font: Font) -> &'static Metrics {
    static M: OnceLock<Vec<Metrics>> = OnceLock::new();
    let all = M.get_or_init(|| {
        Font::ALL
            .iter()
            .map(|f| {
                let (regular, units) = build(f.regular()).unwrap_or((Vec::new(), 2048.0));
                let (bold, _) = build(f.bold()).unwrap_or((Vec::new(), 2048.0));
                Metrics { regular, bold, units }
            })
            .collect()
    });
    let i = Font::ALL.iter().position(|f| *f == font).unwrap_or(0);
    // `all` always holds one entry per font; the empty fallback is never reached.
    static EMPTY: Metrics = Metrics { regular: Vec::new(), bold: Vec::new(), units: 2048.0 };
    all.get(i).unwrap_or(&EMPTY)
}

/// Width of one line of text at `size` pixels in `font`.
pub fn text_width(text: &str, size: f32, bold: bool, font: Font) -> f32 {
    let m = metrics(font);
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
pub fn label_size(label: &str, size: f32, bold: bool, font: Font) -> (f32, f32) {
    let lines = split_lines(label);
    let w = lines.iter().map(|l| text_width(l, size, bold, font)).fold(0.0f32, f32::max);
    (w, lines.len() as f32 * size * 1.25)
}
