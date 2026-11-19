//! The preferences file shared by the app, the CLI and the MCP server: one `key=value` per line,
//! in the user's application folder (`MERMAIDCRAFT_SETTINGS` points elsewhere): Application
//! Support on macOS, `%APPDATA%` on Windows, `$XDG_CONFIG_HOME` (or `~/.config`) elsewhere.

use std::path::PathBuf;

use crate::RenderOptions;
use crate::look::{Arrows, ColorMode, Fill, Font, Stroke};

/// Where the preferences live.
pub fn path() -> Option<PathBuf> {
    path_from(|k| std::env::var_os(k))
}

/// The preferences path for this system, reading the environment through `env`.
fn path_from(env: impl Fn(&str) -> Option<std::ffi::OsString>) -> Option<PathBuf> {
    let var = |k: &str| env(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    if let Some(p) = var("MERMAIDCRAFT_SETTINGS") {
        return Some(p);
    }
    let mut p = if cfg!(target_os = "macos") {
        let mut p = var("HOME")?;
        p.push("Library/Application Support/MermaidCraft");
        p
    } else if cfg!(windows) {
        let mut p = var("APPDATA")?;
        p.push("MermaidCraft");
        p
    } else {
        // A relative XDG_CONFIG_HOME is ignored, as the XDG specification asks.
        let mut p = var("XDG_CONFIG_HOME").filter(|p| p.is_absolute()).or_else(|| var("HOME").map(|h| h.join(".config")))?;
        p.push("mermaidcraft");
        p
    };
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
    fn path_follows_each_system() {
        let env = |pairs: &'static [(&'static str, &'static str)]| move |k: &str| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| std::ffi::OsString::from(v));
        let all = env(&[("HOME", "/home/u"), ("APPDATA", "C:/Users/u/AppData/Roaming"), ("XDG_CONFIG_HOME", "/home/u/cfg")]);
        let p = path_from(all).unwrap();
        if cfg!(target_os = "macos") {
            assert_eq!(p, PathBuf::from("/home/u/Library/Application Support/MermaidCraft/settings.txt"));
        } else if cfg!(windows) {
            assert_eq!(p, PathBuf::from("C:/Users/u/AppData/Roaming").join("MermaidCraft").join("settings.txt"));
            // Windows has no HOME of its own: APPDATA is what counts.
            assert_eq!(path_from(env(&[("HOME", "/x")])), None);
        } else {
            assert_eq!(p, PathBuf::from("/home/u/cfg/mermaidcraft/settings.txt"));
            assert_eq!(path_from(env(&[("HOME", "/home/u")])), Some(PathBuf::from("/home/u/.config/mermaidcraft/settings.txt")));
            assert_eq!(path_from(env(&[("HOME", "/home/u"), ("XDG_CONFIG_HOME", "rel")])), Some(PathBuf::from("/home/u/.config/mermaidcraft/settings.txt")));
        }
        // The override wins everywhere; an empty one is ignored.
        assert_eq!(path_from(env(&[("MERMAIDCRAFT_SETTINGS", "/tmp/s.txt"), ("HOME", "/h")])), Some(PathBuf::from("/tmp/s.txt")));
        assert_ne!(path_from(env(&[("MERMAIDCRAFT_SETTINGS", ""), ("APPDATA", "/a"), ("HOME", "/h")])), Some(PathBuf::from("")));
    }

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
