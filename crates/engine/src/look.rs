//! The look of a drawing: font, colours, fill, stroke and arrows. Every choice has a short id
//! (used by the CLI, the MCP server and the app's settings file) and a default, which together
//! make the hand-drawn "proposal A" look.

use crate::scene::Color;

/// Typeface of the diagram text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Font {
    #[default]
    Shantell,
    PatrickHand,
    Kalam,
    Architects,
    Liberation,
}

/// How shapes get their colours.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ColorMode {
    /// Nodes take the colour of their group (subgraph); loose nodes follow their shape.
    #[default]
    Group,
    /// Colour by the role of the shape: decision, start and end, data, and so on.
    Shape,
    /// Everything grey.
    Neutral,
    /// One colour per node (the first hand-drawn look).
    PerNode,
    /// The classic Mermaid colours.
    Classic,
}

/// How closed shapes are filled.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Fill {
    #[default]
    Solid,
    Hachure,
    CrossHatch,
}

/// How hand-drawn the strokes are.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Stroke {
    /// Straight, precise lines.
    Clean,
    #[default]
    Artist,
    Cartoonist,
}

/// Colour of the edges.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Arrows {
    #[default]
    Graphite,
    /// The colour of the node the edge leaves from.
    Source,
}

macro_rules! ids {
    ($t:ty { $($v:ident => $id:literal),+ $(,)? }) => {
        impl $t {
            pub const ALL: &'static [$t] = &[$(<$t>::$v),+];
            /// Short name used on the command line and in settings.
            pub fn id(self) -> &'static str {
                match self { $(<$t>::$v => $id),+ }
            }
            pub fn from_id(s: &str) -> Option<Self> {
                let s = s.trim().to_ascii_lowercase();
                Self::ALL.iter().copied().find(|v| v.id() == s)
            }
            /// Every accepted name, for error messages.
            pub fn ids() -> String {
                Self::ALL.iter().map(|v| v.id()).collect::<Vec<_>>().join(", ")
            }
        }
    };
}

ids!(Font { Shantell => "shantell", PatrickHand => "patrick-hand", Kalam => "kalam", Architects => "architects-daughter", Liberation => "liberation" });
ids!(ColorMode { Group => "group", Shape => "shape", Neutral => "neutral", PerNode => "per-node", Classic => "classic" });
ids!(Fill { Solid => "solid", Hachure => "hachure", CrossHatch => "cross-hatch" });
ids!(Stroke { Clean => "clean", Artist => "artist", Cartoonist => "cartoonist" });
ids!(Arrows { Graphite => "graphite", Source => "source" });

impl Font {
    /// Family name written into SVG and registered with the font database.
    pub fn family(self) -> &'static str {
        match self {
            Font::Shantell => "Shantell Sans",
            Font::PatrickHand => "Patrick Hand",
            Font::Kalam => "Kalam",
            Font::Architects => "Architects Daughter",
            Font::Liberation => "Liberation Sans",
        }
    }

    pub fn regular(self) -> &'static [u8] {
        use crate::text::*;
        match self {
            Font::Shantell => SHANTELL_REGULAR,
            Font::PatrickHand => PATRICK_HAND_REGULAR,
            Font::Kalam => KALAM_REGULAR,
            Font::Architects => ARCHITECTS_REGULAR,
            Font::Liberation => SANS_REGULAR,
        }
    }

    /// The bold face; fonts without one use the regular face.
    pub fn bold(self) -> &'static [u8] {
        use crate::text::*;
        match self {
            Font::Shantell => SHANTELL_BOLD,
            Font::Kalam => KALAM_BOLD,
            Font::Liberation => SANS_BOLD,
            Font::PatrickHand | Font::Architects => self.regular(),
        }
    }

    pub fn has_bold(self) -> bool {
        !matches!(self, Font::PatrickHand | Font::Architects)
    }
}

impl Stroke {
    /// Strength of the wobble: 0 for clean lines.
    pub fn amount(self) -> f32 {
        match self {
            Stroke::Clean => 0.0,
            Stroke::Artist => 1.0,
            Stroke::Cartoonist => 2.0,
        }
    }
}

/// One hue in five tones (Open Color): very light, light, medium, hatch lines, and stroke.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hue {
    pub pale: Color,
    pub light: Color,
    pub medium: Color,
    pub hatch: Color,
    pub stroke: Color,
}

const fn hue(p: [u8; 3], l: [u8; 3], m: [u8; 3], h: [u8; 3], s: [u8; 3]) -> Hue {
    Hue {
        pale: [p[0], p[1], p[2], 255],
        light: [l[0], l[1], l[2], 255],
        medium: [m[0], m[1], m[2], 255],
        hatch: [h[0], h[1], h[2], 255],
        stroke: [s[0], s[1], s[2], 255],
    }
}

pub const BLUE: Hue = hue([231, 245, 255], [208, 235, 255], [165, 216, 255], [116, 192, 252], [25, 113, 194]);
pub const GREEN: Hue = hue([235, 251, 238], [211, 249, 216], [178, 242, 187], [140, 233, 154], [47, 158, 68]);
pub const YELLOW: Hue = hue([255, 249, 219], [255, 243, 191], [255, 236, 153], [255, 224, 102], [224, 150, 0]);
pub const RED: Hue = hue([255, 245, 245], [255, 227, 227], [255, 201, 201], [255, 168, 168], [224, 49, 49]);
pub const VIOLET: Hue = hue([243, 240, 255], [229, 219, 255], [208, 191, 255], [177, 151, 252], [103, 65, 217]);
pub const CYAN: Hue = hue([227, 250, 252], [197, 246, 250], [153, 233, 242], [102, 217, 232], [12, 133, 153]);
pub const ORANGE: Hue = hue([255, 244, 230], [255, 232, 204], [255, 216, 168], [255, 192, 120], [232, 89, 12]);
pub const LIME: Hue = hue([244, 252, 227], [233, 250, 200], [216, 245, 162], [192, 235, 117], [102, 168, 15]);
pub const GRAY: Hue = hue([248, 249, 250], [241, 243, 245], [233, 236, 239], [206, 212, 218], [73, 80, 87]);

/// Hues used in turn for groups, participants and entities.
pub const HUES: [Hue; 8] = [BLUE, GREEN, YELLOW, RED, VIOLET, CYAN, ORANGE, LIME];

/// Colour of edges in the graphite style.
pub const GRAPHITE: Color = [73, 80, 87, 255];

/// Text colour of the hand-drawn looks.
pub const INK: Color = [30, 30, 30, 255];

pub fn hue_at(i: usize) -> Hue {
    HUES.get(i % HUES.len()).copied().unwrap_or(BLUE)
}

/// The fill and stroke of the `i`-th participant, entity or step for a colour mode; `None` for
/// the classic Mermaid colours.
pub fn indexed(mode: ColorMode, i: usize) -> Option<(Color, Color)> {
    match mode {
        ColorMode::Classic => None,
        ColorMode::PerNode => Some(crate::scene::palette(i)),
        ColorMode::Neutral => Some((GRAY.light, GRAY.stroke)),
        ColorMode::Group | ColorMode::Shape => {
            let h = hue_at(i);
            Some((h.light, h.stroke))
        }
    }
}

/// Mixes `a` toward `b` by `k` (0 keeps `a`, 1 gives `b`).
pub fn mix(a: Color, b: Color, k: f32) -> Color {
    let k = k.clamp(0.0, 1.0);
    let m = |x: u8, y: u8| (f32::from(x) * (1.0 - k) + f32::from(y) * k).round().clamp(0.0, 255.0) as u8;
    [m(a[0], b[0]), m(a[1], b[1]), m(a[2], b[2]), a[3].max(b[3])]
}

/// Perceived brightness, 0..255.
pub fn luma(c: Color) -> f32 {
    0.299 * f32::from(c[0]) + 0.587 * f32::from(c[1]) + 0.114 * f32::from(c[2])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip() {
        for f in Font::ALL {
            assert_eq!(Font::from_id(f.id()), Some(*f));
        }
        for c in ColorMode::ALL {
            assert_eq!(ColorMode::from_id(c.id()), Some(*c));
        }
        assert_eq!(Fill::from_id(" Hachure "), Some(Fill::Hachure));
        assert_eq!(Stroke::from_id("nope"), None);
    }
}
