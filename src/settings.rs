//! Tiny settings file (language and theme) in the user's application support folder.

use std::path::PathBuf;

use crate::i18n::Lang;

#[derive(Clone, Copy, Debug)]
pub struct Settings {
    pub lang: Lang,
    pub dark: bool,
    pub sketch: bool,
    pub colorful: bool,
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
        let mut s = Settings { lang: crate::i18n::detect(), dark: false, sketch: true, colorful: true };
        if let Some(text) = path().and_then(|p| std::fs::read_to_string(p).ok()) {
            for line in text.lines() {
                match line.split_once('=') {
                    Some(("lang", "pt")) => s.lang = Lang::Pt,
                    Some(("lang", "en")) => s.lang = Lang::En,
                    Some(("dark", v)) => s.dark = v == "1",
                    Some(("sketch", v)) => s.sketch = v == "1",
                    Some(("colorful", v)) => s.colorful = v == "1",
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
        let _ = std::fs::write(p, format!("lang={lang}\ndark={}\nsketch={}\ncolorful={}\n", u8::from(self.dark), u8::from(self.sketch), u8::from(self.colorful)));
    }
}
