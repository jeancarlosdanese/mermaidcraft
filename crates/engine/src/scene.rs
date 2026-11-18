//! A drawing description shared by the SVG writer and the app's painter, so that what the
//! preview shows is exactly what gets exported.

pub type Color = [u8; 4];

pub const NONE: Option<Color> = None;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    Start,
    Middle,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PathCmd {
    Move(f32, f32),
    Line(f32, f32),
    Cubic(f32, f32, f32, f32, f32, f32),
    Close,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PrimShape {
    Rect { x: f32, y: f32, w: f32, h: f32, r: f32 },
    Ellipse { cx: f32, cy: f32, rx: f32, ry: f32 },
    Polygon(Vec<(f32, f32)>),
    Path(Vec<PathCmd>),
    /// One line of text; `y` is the baseline.
    Text { x: f32, y: f32, text: String, size: f32, bold: bool, italic: bool, mono: bool, anchor: Anchor },
}

/// What a drawn thing stands for, so the app can select it with the pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Tag {
    #[default]
    None,
    Node(usize),
    Edge(usize),
    Cluster(usize),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Prim {
    pub shape: PrimShape,
    /// Fill colour (text colour for text).
    pub fill: Option<Color>,
    pub stroke: Option<Color>,
    pub stroke_width: f32,
    pub dash: Option<(f32, f32)>,
    pub tag: Tag,
    /// A hatch line inside a filled shape: already hand-drawn, so `roughen` leaves it alone.
    pub fill_line: bool,
}

impl Prim {
    pub fn new(shape: PrimShape) -> Self {
        Self { shape, fill: None, stroke: None, stroke_width: 1.0, dash: None, tag: Tag::None, fill_line: false }
    }
    pub fn fill(mut self, c: Color) -> Self {
        self.fill = Some(c);
        self
    }
    pub fn stroke(mut self, c: Color, w: f32) -> Self {
        self.stroke = Some(c);
        self.stroke_width = w;
        self
    }
    pub fn dash(mut self, on: f32, off: f32) -> Self {
        self.dash = Some((on, off));
        self
    }
    pub fn dash_if(mut self, yes: bool) -> Self {
        if yes {
            self.dash = Some((7.0, 5.0));
        }
        self
    }
    pub fn tag(mut self, t: Tag) -> Self {
        self.tag = t;
        self
    }
}

/// Axis-aligned rectangle used for hit boxes.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x <= self.x + self.w && y >= self.y && y <= self.y + self.h
    }
    pub fn center(&self) -> (f32, f32) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }
}

#[derive(Clone, Debug, Default)]
pub struct Scene {
    pub width: f32,
    pub height: f32,
    pub background: Option<Color>,
    pub prims: Vec<Prim>,
    /// Boxes of the nodes (flowchart) in drawing order, indexed by node number.
    pub node_boxes: Vec<Rect>,
    /// Polyline of every edge, indexed by edge number.
    pub edge_paths: Vec<Vec<(f32, f32)>>,
    /// Total translation applied by `finish` to the raw layout coordinates.
    pub shift: (f32, f32),
    /// Typeface of the text.
    pub font: crate::look::Font,
}

impl Scene {
    pub fn push(&mut self, p: Prim) {
        self.prims.push(p);
    }
}

/// Colours of the default theme.
pub struct Theme {
    pub background: Color,
    pub node_fill: Color,
    pub node_stroke: Color,
    pub text: Color,
    pub line: Color,
    pub cluster_fill: Color,
    pub cluster_stroke: Color,
    pub label_bg: Color,
    pub note_fill: Color,
    pub note_stroke: Color,
    pub actor_fill: Color,
    pub actor_stroke: Color,
}

pub const DEFAULT_THEME: Theme = Theme {
    background: [255, 255, 255, 255],
    node_fill: [236, 236, 255, 255],
    node_stroke: [147, 112, 219, 255],
    text: [51, 51, 51, 255],
    line: [51, 51, 51, 255],
    cluster_fill: [255, 255, 222, 255],
    cluster_stroke: [170, 170, 51, 255],
    label_bg: [232, 232, 232, 255],
    note_fill: [255, 245, 173, 255],
    note_stroke: [170, 170, 51, 255],
    actor_fill: [234, 234, 234, 255],
    actor_stroke: [102, 102, 102, 255],
};

fn hex(c: Color) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            c if (c as u32) < 0x20 && c != '\t' => {}
            c => o.push(c),
        }
    }
    o
}

fn n(v: f32) -> String {
    if !v.is_finite() {
        return "0".into();
    }
    let s = format!("{v:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-" { "0".into() } else { s.to_string() }
}

impl Scene {
    /// Serialises the scene as a standalone SVG document.
    pub fn to_svg(&self) -> String {
        let w = self.width.max(1.0);
        let h = self.height.max(1.0);
        let mut s = String::with_capacity(self.prims.len() * 120 + 400);
        s.push_str(&format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\" font-family=\"'{}', '{}', Arial, Helvetica, sans-serif\">\n",
            n(w),
            n(h),
            n(w),
            n(h),
            self.font.family(),
            crate::text::FONT_FAMILY
        ));
        if let Some(bg) = self.background {
            s.push_str(&format!("<rect width=\"{}\" height=\"{}\" fill=\"{}\"/>\n", n(w), n(h), hex(bg)));
        }
        for p in &self.prims {
            let mut attrs = String::new();
            match p.fill {
                Some(c) if !matches!(p.shape, PrimShape::Text { .. }) && c[3] > 0 => attrs.push_str(&format!(" fill=\"{}\"", hex(c))),
                _ if !matches!(p.shape, PrimShape::Text { .. }) => attrs.push_str(" fill=\"none\""),
                _ => {}
            }
            if let Some(c) = p.stroke.filter(|c| c[3] > 0) {
                attrs.push_str(&format!(" stroke=\"{}\" stroke-width=\"{}\"", hex(c), n(p.stroke_width)));
                if let Some((a, b)) = p.dash {
                    attrs.push_str(&format!(" stroke-dasharray=\"{} {}\"", n(a), n(b)));
                }
                attrs.push_str(" stroke-linejoin=\"round\"");
            }
            match &p.shape {
                PrimShape::Rect { x, y, w, h, r } => {
                    let rr = if *r > 0.0 { format!(" rx=\"{}\"", n(*r)) } else { String::new() };
                    s.push_str(&format!("<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{rr}{attrs}/>\n", n(*x), n(*y), n(*w), n(*h)));
                }
                PrimShape::Ellipse { cx, cy, rx, ry } => {
                    s.push_str(&format!("<ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\"{attrs}/>\n", n(*cx), n(*cy), n(*rx), n(*ry)));
                }
                PrimShape::Polygon(pts) => {
                    let pts: Vec<String> = pts.iter().map(|(x, y)| format!("{},{}", n(*x), n(*y))).collect();
                    s.push_str(&format!("<polygon points=\"{}\"{attrs}/>\n", pts.join(" ")));
                }
                PrimShape::Path(cmds) => {
                    let mut d = String::new();
                    for c in cmds {
                        match c {
                            PathCmd::Move(x, y) => d.push_str(&format!("M{} {} ", n(*x), n(*y))),
                            PathCmd::Line(x, y) => d.push_str(&format!("L{} {} ", n(*x), n(*y))),
                            PathCmd::Cubic(a, b, c2, d2, x, y) => {
                                d.push_str(&format!("C{} {} {} {} {} {} ", n(*a), n(*b), n(*c2), n(*d2), n(*x), n(*y)))
                            }
                            PathCmd::Close => d.push_str("Z "),
                        }
                    }
                    s.push_str(&format!("<path d=\"{}\"{attrs}/>\n", d.trim_end()));
                }
                PrimShape::Text { x, y, text, size, bold, italic, mono, anchor } => {
                    let a = match anchor {
                        Anchor::Start => "start",
                        Anchor::Middle => "middle",
                        Anchor::End => "end",
                    };
                    let col = p.fill.map_or(String::new(), |c| format!(" fill=\"{}\"", hex(c)));
                    let wgt = if *bold { " font-weight=\"bold\"" } else { "" };
                    let sty = if *italic { " font-style=\"italic\"" } else { "" };
                    let fam = if *mono { " font-family=\"Liberation Mono, Courier New, monospace\"" } else { "" };
                    s.push_str(&format!(
                        "<text x=\"{}\" y=\"{}\" font-size=\"{}\" text-anchor=\"{a}\"{col}{wgt}{sty}{fam} xml:space=\"preserve\">{}</text>\n",
                        n(*x),
                        n(*y),
                        n(*size),
                        esc(text)
                    ));
                }
            }
        }
        s.push_str("</svg>\n");
        s
    }
}

impl Scene {
    /// Fits the scene to its content: moves everything so that the content starts at `margin`
    /// and sets the final width and height.
    pub fn finish(&mut self, margin: f32) {
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;
        let mut grow = |x: f32, y: f32| {
            if x.is_finite() && y.is_finite() {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        };
        for p in &self.prims {
            let sw = if p.stroke.is_some() { p.stroke_width / 2.0 } else { 0.0 };
            match &p.shape {
                PrimShape::Rect { x, y, w, h, .. } => {
                    grow(x - sw, y - sw);
                    grow(x + w + sw, y + h + sw);
                }
                PrimShape::Ellipse { cx, cy, rx, ry } => {
                    grow(cx - rx - sw, cy - ry - sw);
                    grow(cx + rx + sw, cy + ry + sw);
                }
                PrimShape::Polygon(pts) => {
                    for (x, y) in pts {
                        grow(*x, *y);
                    }
                }
                PrimShape::Path(cmds) => {
                    for c in cmds {
                        match c {
                            PathCmd::Move(x, y) | PathCmd::Line(x, y) => grow(*x, *y),
                            PathCmd::Cubic(a, b, c2, d, x, y) => {
                                grow(*a, *b);
                                grow(*c2, *d);
                                grow(*x, *y);
                            }
                            PathCmd::Close => {}
                        }
                    }
                }
                PrimShape::Text { x, y, text, size, bold, anchor, .. } => {
                    let w = crate::text::text_width(text, *size, *bold, self.font);
                    let (x0, x1) = match anchor {
                        Anchor::Start => (*x, x + w),
                        Anchor::Middle => (x - w / 2.0, x + w / 2.0),
                        Anchor::End => (x - w, *x),
                    };
                    grow(x0, y - size);
                    grow(x1, y + size * 0.3);
                }
            }
        }
        if !min_x.is_finite() {
            self.width = margin * 2.0;
            self.height = margin * 2.0;
            return;
        }
        let dx = margin - min_x;
        let dy = margin - min_y;
        self.shift.0 += dx;
        self.shift.1 += dy;
        for p in &mut self.prims {
            match &mut p.shape {
                PrimShape::Rect { x, y, .. } => {
                    *x += dx;
                    *y += dy;
                }
                PrimShape::Ellipse { cx, cy, .. } => {
                    *cx += dx;
                    *cy += dy;
                }
                PrimShape::Polygon(pts) => {
                    for q in pts {
                        q.0 += dx;
                        q.1 += dy;
                    }
                }
                PrimShape::Path(cmds) => {
                    for c in cmds {
                        match c {
                            PathCmd::Move(x, y) | PathCmd::Line(x, y) => {
                                *x += dx;
                                *y += dy;
                            }
                            PathCmd::Cubic(a, b, c2, d, x, y) => {
                                *a += dx;
                                *b += dy;
                                *c2 += dx;
                                *d += dy;
                                *x += dx;
                                *y += dy;
                            }
                            PathCmd::Close => {}
                        }
                    }
                }
                PrimShape::Text { x, y, .. } => {
                    *x += dx;
                    *y += dy;
                }
            }
        }
        for b in &mut self.node_boxes {
            b.x += dx;
            b.y += dy;
        }
        for path in &mut self.edge_paths {
            for q in path {
                q.0 += dx;
                q.1 += dy;
            }
        }
        self.width = (max_x - min_x + margin * 2.0).max(1.0);
        self.height = (max_y - min_y + margin * 2.0).max(1.0);
    }
}

/// Parses a CSS colour: `#rgb`, `#rrggbb`, `#rrggbbaa` and a few names.
pub fn parse_color(s: &str) -> Option<Color> {
    let s = s.trim().to_ascii_lowercase();
    if let Some(h) = s.strip_prefix('#') {
        let v: Vec<u8> = h.chars().filter_map(|c| c.to_digit(16).map(|d| d as u8)).collect();
        if v.len() != h.chars().count() {
            return None;
        }
        return match v.as_slice() {
            [r, g, b] => Some([r * 17, g * 17, b * 17, 255]),
            [r1, r2, g1, g2, b1, b2] => Some([r1 * 16 + r2, g1 * 16 + g2, b1 * 16 + b2, 255]),
            [r1, r2, g1, g2, b1, b2, a1, a2] => Some([r1 * 16 + r2, g1 * 16 + g2, b1 * 16 + b2, a1 * 16 + a2]),
            _ => None,
        };
    }
    let c = match s.as_str() {
        "white" => [255, 255, 255],
        "black" => [0, 0, 0],
        "red" => [255, 0, 0],
        "green" => [0, 128, 0],
        "lime" => [0, 255, 0],
        "blue" => [0, 0, 255],
        "yellow" => [255, 255, 0],
        "orange" => [255, 165, 0],
        "purple" => [128, 0, 128],
        "pink" => [255, 192, 203],
        "gray" | "grey" => [128, 128, 128],
        "lightgray" | "lightgrey" => [211, 211, 211],
        "lightblue" => [173, 216, 230],
        "lightgreen" => [144, 238, 144],
        "darkblue" => [0, 0, 139],
        "darkgreen" => [0, 100, 0],
        "brown" => [165, 42, 42],
        "cyan" => [0, 255, 255],
        "magenta" => [255, 0, 255],
        "navy" => [0, 0, 128],
        "teal" => [0, 128, 128],
        "gold" => [255, 215, 0],
        "silver" => [192, 192, 192],
        "transparent" | "none" => return Some([0, 0, 0, 0]),
        _ => return None,
    };
    Some([c[0], c[1], c[2], 255])
}

/// Pastel fill and strong stroke pairs used to colour steps of a diagram.
pub const PALETTE: [(Color, Color); 8] = [
    ([165, 216, 255, 255], [25, 113, 194, 255]),
    ([178, 242, 187, 255], [47, 158, 68, 255]),
    ([255, 236, 153, 255], [224, 150, 0, 255]),
    ([255, 201, 201, 255], [224, 49, 49, 255]),
    ([208, 191, 255, 255], [103, 65, 217, 255]),
    ([153, 233, 242, 255], [12, 133, 153, 255]),
    ([255, 216, 168, 255], [232, 89, 12, 255]),
    ([216, 245, 162, 255], [102, 168, 15, 255]),
];

pub fn palette(i: usize) -> (Color, Color) {
    PALETTE.get(i % PALETTE.len()).copied().unwrap_or(PALETTE[0])
}
