//! PNG and PDF from SVG, using resvg/usvg/svg2pdf and the embedded fonts.

use std::sync::{Arc, OnceLock};

use resvg::usvg;

/// Largest PNG side, in pixels.
const MAX_SIDE: f32 = 16_384.0;
/// Most pixels in one PNG (about 512 MB of image memory).
const MAX_PIXELS: f32 = 128_000_000.0;

fn too_large(w: f32, h: f32) -> bool {
    w > MAX_SIDE || h > MAX_SIDE || w * h > MAX_PIXELS
}

fn fontdb() -> Arc<usvg::fontdb::Database> {
    static DB: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();
    DB.get_or_init(|| {
        let mut db = usvg::fontdb::Database::new();
        for data in crate::text::ALL_FONT_DATA {
            db.load_font_data(data.to_vec());
        }
        db.set_sans_serif_family(crate::text::FONT_FAMILY);
        db.set_serif_family(crate::text::FONT_FAMILY);
        db.set_monospace_family("Liberation Mono");
        Arc::new(db)
    })
    .clone()
}

fn tree_from(svg: &str) -> Result<usvg::Tree, String> {
    let opt = usvg::Options { fontdb: fontdb(), font_family: crate::text::FONT_FAMILY.to_string(), ..usvg::Options::default() };
    usvg::Tree::from_str(svg, &opt).map_err(|e| format!("could not read the SVG: {e}"))
}

/// The `width` and `height` written on the root `<svg>` element, when they are plain numbers.
fn declared_size(svg: &str) -> Option<(f32, f32)> {
    let start = svg.find("<svg")?;
    let tag = svg.get(start..)?.split('>').next()?;
    let attr = |name: &str| -> Option<f32> {
        let key = format!(" {name}=\"");
        let from = tag.find(&key)? + key.len();
        let rest = tag.get(from..)?;
        rest.get(..rest.find('"')?)?.trim_end_matches("px").trim().parse().ok()
    };
    Some((attr("width")?, attr("height")?))
}

/// Renders SVG text to PNG bytes at the given scale (1.0 = one pixel per SVG unit).
pub fn svg_to_png(svg: &str, scale: f32) -> Result<Vec<u8>, String> {
    let scale = if scale.is_finite() { scale.clamp(0.1, 8.0) } else { 1.0 };
    // Refuse an image that is too large before reading the whole drawing.
    if let Some((w, h)) = declared_size(svg) {
        if too_large(w * scale, h * scale) {
            return Err("the image would be too large".into());
        }
    }
    let tree = tree_from(svg)?;
    let size = tree.size();
    let (w, h) = (size.width() * scale, size.height() * scale);
    if too_large(w, h) {
        return Err("the image would be too large".into());
    }
    let (pw, ph) = ((w.ceil() as u32).max(1), (h.ceil() as u32).max(1));
    let mut pixmap = resvg::tiny_skia::Pixmap::new(pw, ph).ok_or("could not allocate the image")?;
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    pixmap.encode_png().map_err(|e| format!("could not encode the PNG: {e}"))
}

/// Renders SVG text to a PDF (one page the size of the diagram).
pub fn svg_to_pdf(svg: &str) -> Result<Vec<u8>, String> {
    let tree = tree_from(svg)?;
    svg2pdf::to_pdf(&tree, svg2pdf::ConversionOptions::default(), svg2pdf::PageOptions::default()).map_err(|e| format!("could not make the PDF: {e}"))
}

/// Renders SVG text to raw RGBA pixels of the given size (for window icons).
pub fn svg_to_rgba(svg: &str, size: u32) -> Result<Vec<u8>, String> {
    let tree = tree_from(svg)?;
    let size = size.clamp(1, 2048);
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size).ok_or("could not allocate the image")?;
    let s = tree.size();
    let k = (size as f32 / s.width().max(1.0)).min(size as f32 / s.height().max(1.0));
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(k, k), &mut pixmap.as_mut());
    // Un-premultiply for consumers that expect straight alpha.
    let mut data = pixmap.take();
    for px in data.chunks_exact_mut(4) {
        let a = u32::from(px.get(3).copied().unwrap_or(255));
        if a > 0 && a < 255 {
            for c in px.iter_mut().take(3) {
                *c = ((u32::from(*c) * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
    Ok(data)
}

/// Encodes straight-alpha RGBA pixels as a PNG (used for the screenshot option).
pub fn rgba_to_png(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    let mut data = rgba.to_vec();
    for px in data.chunks_exact_mut(4) {
        let a = u32::from(px.get(3).copied().unwrap_or(255));
        if a < 255 {
            for c in px.iter_mut().take(3) {
                *c = ((u32::from(*c) * a + 127) / 255) as u8;
            }
        }
    }
    let size = resvg::tiny_skia::IntSize::from_wh(width, height).ok_or("bad image size")?;
    let pixmap = resvg::tiny_skia::Pixmap::from_vec(data, size).ok_or("bad image data")?;
    pixmap.encode_png().map_err(|e| format!("could not encode the PNG: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_is_read_from_the_root_tag() {
        assert_eq!(declared_size("<svg xmlns=\"x\" width=\"733.5\" height=\"20px\">"), Some((733.5, 20.0)));
        assert_eq!(declared_size("<svg viewBox=\"0 0 1 1\">"), None);
        assert!(too_large(16_000.0, 16_000.0));
        assert!(!too_large(8_113.0, 4_595.0));
    }
}
