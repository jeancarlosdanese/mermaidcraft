//! Tiny settings file (language, theme and diagram look) in the user's application support
//! folder.

use std::path::PathBuf;

use mermaidcraft_engine::RenderOptions;
use mermaidcraft_engine::look::{Arrows, ColorMode, Fill, Font, Stroke};

use crate::i18n::Lang;

#[derive(Clone, Copy, Debug)]
pub struct Settings {
    pub lang: Lang,
    pub dark: bool,
    pub look: RenderOptions,
}

impl Settings {
    /// Defaults: the system language, light theme and the default look.
    pub fn fresh() -> Self {
        Settings { lang: crate::i18n::detect(), dark: false, look: RenderOptions::default() }
    }
}

fn path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    let mut p = PathBuf::from(home);
    if cfg!(target_os = "macos") {
        p.push("Library/Application Support/MermaidCraft");
    } else {
        p.push(".config/mermaidcraft");
    }
    p.push("settings.txt");
    Some(p)
}

impl Settings {
    pub fn load() -> Self {
        let mut s = Settings::fresh();
        if let Some(text) = path().and_then(|p| std::fs::read_to_string(p).ok()) {
            for line in text.lines() {
                match line.split_once('=') {
                    Some(("lang", "pt")) => s.lang = Lang::Pt,
                    Some(("lang", "en")) => s.lang = Lang::En,
                    Some(("dark", v)) => s.dark = v == "1",
                    Some(("font", v)) => s.look.font = Font::from_id(v).unwrap_or(s.look.font),
                    Some(("colors", v)) => s.look.colors = ColorMode::from_id(v).unwrap_or(s.look.colors),
                    Some(("fill", v)) => s.look.fill = Fill::from_id(v).unwrap_or(s.look.fill),
                    Some(("stroke", v)) => s.look.stroke = Stroke::from_id(v).unwrap_or(s.look.stroke),
                    Some(("arrows", v)) => s.look.arrows = Arrows::from_id(v).unwrap_or(s.look.arrows),
                    _ => {}
                }
            }
        }
        s
    }

    pub fn save(&self) {
        let Some(p) = path() else { return };
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let lang = if self.lang == Lang::Pt { "pt" } else { "en" };
        let l = &self.look;
        let _ = std::fs::write(
            p,
            format!(
                "lang={lang}\ndark={}\nfont={}\ncolors={}\nfill={}\nstroke={}\narrows={}\n",
                u8::from(self.dark),
                l.font.id(),
                l.colors.id(),
                l.fill.id(),
                l.stroke.id(),
                l.arrows.id()
            ),
        );
    }
}
