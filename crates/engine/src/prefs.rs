//! The preferences file shared by the app, the CLI and the MCP server: one `key=value` per line,
//! in the user's application support folder (`MERMAIDCRAFT_SETTINGS` points elsewhere).

use std::path::PathBuf;

use crate::RenderOptions;
use crate::look::{Arrows, ColorMode, Fill, Font, Stroke};

/// Where the preferences live.
pub fn path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("MERMAIDCRAFT_SETTINGS").filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(p));
    }
    let mut p = PathBuf::from(std::env::var_os("HOME")?);
    if cfg!(target_os = "macos") {
        p.push("Library/Application Support/MermaidCraft");
    } else {
        p.push(".config/mermaidcraft");
    }
    p.push("settings.txt");
    Some(p)
}

/// The `key=value` pairs of the file; empty when there is none.
pub fn read() -> Vec<(String, String)> {
    let Some(text) = path().and_then(|p| std::fs::read_to_string(p).ok()) else { return Vec::new() };
    text.lines().filter_map(|l| l.split_once('=')).map(|(k, v)| (k.trim().to_string(), v.trim().to_string())).collect()
}

/// Applies one preference to a look; unknown keys and values are ignored.
pub fn apply(look: &mut RenderOptions, key: &str, value: &str) {
    match key {
        // Files from before the look options: keep a plain look for whoever chose it.
        "sketch" if value == "0" => look.stroke = Stroke::Clean,
        "colorful" if value == "0" => look.colors = ColorMode::Classic,
        "font" => look.font = Font::from_id(value).unwrap_or(look.font),
        "colors" => look.colors = ColorMode::from_id(value).unwrap_or(look.colors),
        "fill" => look.fill = Fill::from_id(value).unwrap_or(look.fill),
        "stroke" => look.stroke = Stroke::from_id(value).unwrap_or(look.stroke),
        "arrows" => look.arrows = Arrows::from_id(value).unwrap_or(look.arrows),
        _ => {}
    }
}

/// The look saved in the preferences, over the default one.
pub fn look() -> RenderOptions {
    let mut l = RenderOptions::default();
    for (k, v) in read() {
        apply(&mut l, &k, &v);
    }
    l
}

/// The lines that store a look.
pub fn look_lines(l: &RenderOptions) -> String {
    format!("font={}\ncolors={}\nfill={}\nstroke={}\narrows={}\n", l.font.id(), l.colors.id(), l.fill.id(), l.stroke.id(), l.arrows.id())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn look_round_trips_and_old_files_still_read() {
        let l = RenderOptions { stroke: Stroke::Clean, font: Font::Kalam, ..RenderOptions::default() };
        let mut back = RenderOptions::default();
        for line in look_lines(&l).lines() {
            if let Some((k, v)) = line.split_once('=') {
                apply(&mut back, k, v);
            }
        }
        assert_eq!(back, l);
        let mut old = RenderOptions::default();
        apply(&mut old, "sketch", "0");
        apply(&mut old, "nonsense", "1");
        apply(&mut old, "font", "nope");
        assert_eq!(old, RenderOptions { stroke: Stroke::Clean, ..RenderOptions::default() });
    }
}
