//! Tiny settings file (language, theme and diagram look), in the place `prefs::path` gives for
//! each system.

use mermaidcraft_engine::{RenderOptions, prefs};

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

impl Settings {
    pub fn load() -> Self {
        let mut s = Settings::fresh();
        for (k, v) in prefs::read() {
            match (k.as_str(), v.as_str()) {
                ("lang", "pt") => s.lang = Lang::Pt,
                ("lang", "en") => s.lang = Lang::En,
                ("dark", v) => s.dark = v == "1",
                (k, v) => prefs::apply(&mut s.look, k, v),
            }
        }
        s
    }

    pub fn save(&self) {
        let Some(p) = prefs::path() else { return };
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let lang = if self.lang == Lang::Pt { "pt" } else { "en" };
        let _ = std::fs::write(p, format!("lang={lang}\ndark={}\n{}", u8::from(self.dark), prefs::look_lines(&self.look)));
    }
}
