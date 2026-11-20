//! The MermaidCraft window: text editor, live preview, visual tools and export.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use egui::{Align, Color32, Key, KeyboardShortcut, Layout, Modifiers, RichText, Sense, Stroke, Vec2};
use mermaidcraft_engine as engine;
use mermaidcraft_engine::model::{Direction, Head, LineStyle, LinkKind, NodeShape};
use mermaidcraft_engine::{Diagram, edit, markdown};

use crate::i18n::{Lang, t};
use crate::paint::{self, Hit, View};
use crate::platform::OpenEvents;
use crate::settings::Settings;

const SAMPLE: &str = include_str!("../examples/flowchart.mmd");
const MAX_FILE: u64 = 4_000_000;

pub struct Options {
    pub screenshot: Option<PathBuf>,
    pub files: Vec<PathBuf>,
    pub events: OpenEvents,
}

#[derive(Clone, Debug, PartialEq)]
enum Sel {
    None,
    Node(String),
    Edge(usize),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tool {
    Select,
    Link,
}

enum Drag {
    Pan,
    Link { from: usize },
    /// Moving a node or a whole group by hand: its key in the layout comment (the node id, or
    /// `sub:<id>` for a group) and the offset it has now.
    Move { id: String, offset: (f32, f32), group: bool },
}

struct MdState {
    full: String,
    blocks: Vec<markdown::MdBlock>,
    current: Option<usize>,
}

/// What to do once unsaved changes are dealt with.
#[derive(Clone, Debug)]
enum After {
    Close,
    Open(PathBuf),
    New(&'static str),
}

struct Shot {
    path: PathBuf,
    requested: bool,
}

pub struct App {
    lang: Lang,
    dark: bool,
    look: engine::RenderOptions,
    text: String,
    path: Option<PathBuf>,
    md: Option<MdState>,
    dirty: bool,
    parsed: engine::Parsed,
    scene: Option<Arc<engine::Scene>>,
    zoom: f32,
    pan: Vec2,
    auto_fit: bool,
    sel: Sel,
    tool: Tool,
    link_from: Option<usize>,
    drag: Option<Drag>,
    history: Vec<String>,
    future: Vec<String>,
    status: Option<(String, f64)>,
    label_buf: String,
    /// What the label box was being typed for: saved when the box loses focus, however it does.
    label_editing: Option<Sel>,
    focus_label: bool,
    prompt: Option<After>,
    allow_close: bool,
    show_about: bool,
    events: OpenEvents,
    shot: Option<Shot>,
    frames: u32,
    title: String,
    untitled_kind: &'static str,
    /// Lines of the text changed by the last edit made on the drawing, and when (they light up).
    flash_lines: Option<(usize, usize, f64)>,
    /// Selection the text panel last followed, and whether the current one came from the text.
    last_sel: Sel,
    sel_from_text: bool,
    /// Line the text panel must scroll to.
    scroll_to_line: Option<usize>,
    /// Place of the text cursor (byte), to select what is named there.
    cursor_byte: Option<usize>,
    /// Where the canvas is, for the floating tools, and how tall the tool bar came out.
    canvas_rect: Option<egui::Rect>,
    tools_h: f32,
    /// Where the style panel is in this frame, and the selection last moved out from under it.
    inspector_rect: Option<egui::Rect>,
    revealed: Sel,
}

/// Lines of `new` (1-based, inclusive) that differ from `old`; for a pure deletion, the line where
/// the text was removed.
fn changed_lines(old: &str, new: &str) -> Option<(usize, usize)> {
    let o: Vec<&str> = old.lines().collect();
    let n: Vec<&str> = new.lines().collect();
    let prefix = o.iter().zip(&n).take_while(|(a, b)| a == b).count();
    let room = o.len().min(n.len()) - prefix;
    let suffix = o.iter().rev().zip(n.iter().rev()).take_while(|(a, b)| a == b).count().min(room);
    let end = n.len() - suffix;
    if end > prefix {
        Some((prefix + 1, end))
    } else if o.len() > n.len() && !n.is_empty() {
        let at = (prefix + 1).min(n.len());
        Some((at, at))
    } else {
        None
    }
}

fn read_text(path: &Path) -> Result<String, String> {
    let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if meta.len() > MAX_FILE {
        return Err("The file is too large.".into());
    }
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    String::from_utf8(bytes).map_err(|_| "not valid UTF-8 text".to_string())
}

/// The View › Appearance menu: every choice of the diagram look, plus a reset.
fn appearance_menu(ui: &mut egui::Ui, lang: Lang, look: &mut engine::RenderOptions) {
    use engine::look::{Arrows, ColorMode, Fill, Font, Stroke};
    let tr = |s: &'static str| crate::i18n::t(lang, s);
    let heading = |ui: &mut egui::Ui, s: &'static str| {
        ui.label(egui::RichText::new(tr(s)).small().strong());
    };
    heading(ui, "Font");
    for f in Font::ALL {
        ui.radio_value(&mut look.font, *f, f.family());
    }
    ui.separator();
    heading(ui, "Colors");
    for (v, l) in [(ColorMode::Group, "By group"), (ColorMode::Shape, "By shape"), (ColorMode::Neutral, "Neutral"), (ColorMode::PerNode, "One per node"), (ColorMode::Classic, "Classic (Mermaid)")] {
        ui.radio_value(&mut look.colors, v, tr(l));
    }
    ui.separator();
    heading(ui, "Fill");
    for (v, l) in [(Fill::Solid, "Solid"), (Fill::Hachure, "Hatched"), (Fill::CrossHatch, "Cross-hatched")] {
        ui.radio_value(&mut look.fill, v, tr(l));
    }
    ui.separator();
    heading(ui, "Stroke");
    for (v, l) in [(Stroke::Clean, "Clean"), (Stroke::Artist, "Artist"), (Stroke::Cartoonist, "Cartoonist")] {
        ui.radio_value(&mut look.stroke, v, tr(l));
    }
    ui.separator();
    heading(ui, "Arrows");
    for (v, l) in [(Arrows::Graphite, "Graphite"), (Arrows::Source, "Color of the source node")] {
        ui.radio_value(&mut look.arrows, v, tr(l));
    }
    ui.separator();
    if ui.add_enabled(*look != engine::RenderOptions::default(), egui::Button::new(tr("Restore default"))).clicked() {
        *look = engine::RenderOptions::default();
    }
}

fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let add = |fonts: &mut egui::FontDefinitions, name: &str, data: &'static [u8]| {
        fonts.font_data.insert(name.to_string(), Arc::new(egui::FontData::from_static(data)));
    };
    add(&mut fonts, "lib-sans", engine::text::SANS_REGULAR);
    add(&mut fonts, "lib-sans-bold", engine::text::SANS_BOLD);
    add(&mut fonts, "lib-mono", engine::text::MONO_REGULAR);
    fonts.families.entry(egui::FontFamily::Proportional).or_default().insert(0, "lib-sans".to_string());
    fonts.families.entry(egui::FontFamily::Monospace).or_default().insert(0, "lib-mono".to_string());
    let mut bold = vec!["lib-sans-bold".to_string(), "lib-sans".to_string()];
    if let Some(fallback) = fonts.families.get(&egui::FontFamily::Proportional) {
        bold.extend(fallback.iter().filter(|n| n.as_str() != "lib-sans").cloned());
    }
    // One family (and its bold) per diagram font, falling back to the interface fonts for
    // characters the hand-drawn faces lack.
    let fallback: Vec<String> = fonts.families.get(&egui::FontFamily::Proportional).cloned().unwrap_or_default();
    for f in engine::look::Font::ALL {
        let (reg, bld) = (format!("d-{}", f.id()), format!("d-{}-b", f.id()));
        add(&mut fonts, &reg, f.regular());
        add(&mut fonts, &bld, f.bold());
        let mut fam = vec![reg.clone()];
        fam.extend(fallback.iter().cloned());
        fonts.families.insert(paint::diagram_family(*f, false), fam);
        let mut fam_b = vec![bld, reg];
        fam_b.extend(bold.iter().cloned());
        fonts.families.insert(paint::diagram_family(*f, true), fam_b);
    }
    ctx.set_fonts(fonts);
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, opts: Options) -> Self {
        setup_fonts(&cc.egui_ctx);
        let shot_mode = opts.screenshot.is_some();
        // Tests start from the defaults, never from the person's preferences.
        let settings = if shot_mode || cfg!(test) { Settings::fresh() } else { Settings::load() };
        opts.events.set_wake(&cc.egui_ctx);
        let mut app = App {
            lang: settings.lang,
            dark: settings.dark,
            look: settings.look,
            text: String::new(),
            path: None,
            md: None,
            dirty: false,
            parsed: engine::Parsed { diagram: None, diagnostics: Vec::new() },
            scene: None,
            zoom: 1.0,
            pan: Vec2::ZERO,
            auto_fit: true,
            sel: Sel::None,
            tool: Tool::Select,
            link_from: None,
            drag: None,
            history: Vec::new(),
            future: Vec::new(),
            status: None,
            label_buf: String::new(),
            label_editing: None,
            focus_label: false,
            prompt: None,
            allow_close: false,
            show_about: false,
            events: opts.events,
            shot: opts.screenshot.map(|path| Shot { path, requested: false }),
            frames: 0,
            title: String::new(),
            untitled_kind: "flowchart",
            flash_lines: None,
            last_sel: Sel::None,
            sel_from_text: false,
            scroll_to_line: None,
            cursor_byte: None,
            canvas_rect: None,
            tools_h: 40.0,
            inspector_rect: None,
            revealed: Sel::None,
        };
        let mut opened = false;
        for f in &opts.files {
            if app.open_path(f) {
                opened = true;
                break;
            }
        }
        if !opened {
            app.set_doc(SAMPLE.to_string(), None, None);
        }
        // Screenshot mode only: a selection and the dark theme, to check the interface by image.
        if shot_mode {
            if let Ok(id) = std::env::var("MERMAIDCRAFT_SHOT_SELECT") {
                app.sel = Sel::Node(id);
            }
            if std::env::var("MERMAIDCRAFT_SHOT_DARK").is_ok_and(|v| v == "1") {
                app.dark = true;
            }
        }
        app
    }

    /// Line of the text that defines the selected node or edge.
    fn sel_line(&self) -> Option<usize> {
        let f = self.flow()?;
        match &self.sel {
            // The line that gives the node its shape and label, else its first mention.
            Sel::Node(id) => f.node_index(id).and_then(|i| f.nodes.get(i)).map(|n| n.def_span.map_or(n.line, |d| 1 + self.text.get(..d.start).unwrap_or("").matches('\n').count())),
            Sel::Edge(i) => f.edges.get(*i).map(|e| e.line),
            Sel::None => None,
        }
    }

    /// Changes the style of a node through `change`, as one undo step.
    fn restyle(&mut self, id: &str, change: impl FnOnce(&mut engine::StyleMap), now: f64) {
        let old = self.flow().map(|f| edit::own_node_style(&self.text, f, id)).unwrap_or_default();
        let mut map = old.clone();
        change(&mut map);
        if map != old {
            let r = edit::set_node_style(&self.text, id, &map);
            self.visual(r, now);
        }
    }

    fn tr(&self, s: &'static str) -> &'static str {
        t(self.lang, s)
    }

    fn flash(&mut self, msg: String, now: f64) {
        self.status = Some((msg, now));
    }

    // ----------------------------------------------------------------------------- documents

    fn set_doc(&mut self, text: String, path: Option<PathBuf>, md: Option<MdState>) {
        self.text = text;
        self.path = path;
        self.md = md;
        self.dirty = false;
        self.history.clear();
        self.future.clear();
        self.sel = Sel::None;
        self.link_from = None;
        self.auto_fit = true;
        self.reparse();
    }

    fn reparse(&mut self) {
        self.parsed = engine::parse(&self.text);
        let opts = self.opts();
        let scene = self.parsed.diagram.as_ref().map(|d| Arc::new(engine::render_scene_with(d, &opts)));
        self.set_scene(scene);
        // Drop a selection that no longer exists.
        let valid = match (&self.sel, self.flow()) {
            (Sel::Node(id), Some(f)) => f.node_index(id).is_some(),
            (Sel::Edge(i), Some(f)) => *i < f.edges.len(),
            (Sel::None, _) => true,
            _ => false,
        };
        if !valid {
            self.sel = Sel::None;
        }
        if let Some(l) = self.link_from {
            if self.flow().is_none_or(|f| l >= f.nodes.len()) {
                self.link_from = None;
            }
        }
    }

    fn opts(&self) -> engine::RenderOptions {
        self.look
    }

    /// Swaps the drawn scene, keeping what is on screen where it was when the content moved.
    fn set_scene(&mut self, new: Option<Arc<engine::Scene>>) {
        if let (Some(old), Some(n)) = (&self.scene, &new) {
            if !self.auto_fit {
                self.pan -= Vec2::new(n.shift.0 - old.shift.0, n.shift.1 - old.shift.1) * self.zoom;
            }
        }
        self.scene = new;
    }

    /// Draws the diagram with one node held at `offset` (while it is being dragged).
    fn redraw_with_offset(&mut self, id: &str, offset: (f32, f32)) {
        let Some(Diagram::Flowchart(f)) = &self.parsed.diagram else { return };
        let mut f = f.clone();
        f.offsets.insert(id.to_string(), offset);
        let scene = Arc::new(engine::render_scene_with(&Diagram::Flowchart(f), &self.opts()));
        self.set_scene(Some(scene));
    }

    /// Writes the moved positions into the text (one undo step).
    fn commit_offset(&mut self, id: &str, offset: (f32, f32), now: f64) {
        let mut map = self.flow().map(|f| f.offsets.clone()).unwrap_or_default();
        map.insert(id.to_string(), offset);
        let r = edit::set_offsets(&self.text, &map);
        self.visual(r, now);
        self.reparse();
    }

    fn auto_arrange(&mut self, now: f64) {
        let r = edit::set_offsets(&self.text, &std::collections::BTreeMap::new());
        self.visual(r, now);
        self.auto_fit = true;
    }

    fn flow(&self) -> Option<&engine::Flowchart> {
        match &self.parsed.diagram {
            Some(Diagram::Flowchart(f)) => Some(f),
            _ => None,
        }
    }

    fn display_name(&self) -> String {
        match &self.path {
            Some(p) => p.file_name().map_or_else(|| p.display().to_string(), |n| n.to_string_lossy().to_string()),
            None => self.tr("Untitled").to_string(),
        }
    }

    fn open_path(&mut self, path: &Path) -> bool {
        match read_text(path) {
            Ok(text) => {
                let lower = path.to_string_lossy().to_ascii_lowercase();
                if lower.ends_with(".md") || lower.ends_with(".markdown") {
                    let blocks = markdown::find_blocks(&text);
                    let first = blocks.first().map(|b| markdown::block_text(&text, b).to_string()).unwrap_or_default();
                    let cur = if blocks.is_empty() { None } else { Some(0) };
                    self.set_doc(first, Some(path.to_path_buf()), Some(MdState { full: text, blocks, current: cur }));
                } else {
                    self.set_doc(text, Some(path.to_path_buf()), None);
                }
                true
            }
            Err(e) => {
                let msg = format!("{}: {} ({e})", self.tr("Could not open"), path.display());
                self.status = Some((msg, 0.0));
                false
            }
        }
    }

    /// Runs `after` now, or asks about unsaved changes first.
    fn guard(&mut self, ctx: &egui::Context, after: After) {
        if self.dirty && self.shot.is_none() {
            self.prompt = Some(after);
        } else {
            self.run_after(ctx, after);
        }
    }

    fn run_after(&mut self, ctx: &egui::Context, after: After) {
        match after {
            After::Close => {
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            After::Open(p) => {
                self.open_path(&p);
            }
            After::New(kind) => {
                self.untitled_kind = kind;
                self.set_doc(edit::template(kind).to_string(), None, None);
            }
        }
    }

    /// Writes the pending block text back into the Markdown file text.
    fn commit_md(&mut self) {
        let Some(md) = &mut self.md else { return };
        let Some(cur) = md.current else { return };
        let Some(b) = md.blocks.get(cur).cloned() else { return };
        if markdown::block_text(&md.full, &b) == self.text {
            return;
        }
        if let Some(new) = markdown::replace_block(&md.full, &b, &self.text) {
            md.full = new;
            md.blocks = markdown::find_blocks(&md.full);
        }
    }

    fn select_block(&mut self, idx: usize) {
        self.commit_md();
        let Some(md) = &self.md else { return };
        let Some(b) = md.blocks.get(idx) else { return };
        let text = markdown::block_text(&md.full, b).to_string();
        if let Some(md) = &mut self.md {
            md.current = Some(idx);
        }
        self.text = text;
        self.history.clear();
        self.future.clear();
        self.sel = Sel::None;
        self.auto_fit = true;
        self.reparse();
    }

    fn add_block(&mut self) {
        self.commit_md();
        let tpl = edit::template("flowchart");
        let Some(md) = &mut self.md else { return };
        if !md.full.is_empty() && !md.full.ends_with('\n') {
            md.full.push('\n');
        }
        md.full.push_str(&format!("\n```mermaid\n{tpl}```\n"));
        md.blocks = markdown::find_blocks(&md.full);
        self.dirty = true;
        let last = md.blocks.len().saturating_sub(1);
        self.select_block(last);
    }

    fn save(&mut self, now: f64) -> bool {
        match self.path.clone() {
            Some(p) => self.write_to(&p, now),
            None => self.save_as(now),
        }
    }

    fn save_as(&mut self, now: f64) -> bool {
        if self.shot.is_some() {
            return false;
        }
        let ext = if self.md.is_some() { "md" } else { "mmd" };
        let name = self.path.as_ref().and_then(|p| p.file_name()).map_or(format!("diagram.{ext}"), |n| n.to_string_lossy().to_string());
        let Some(p) = rfd::FileDialog::new().set_file_name(name).add_filter("Mermaid", &["mmd", "mermaid", "md"]).save_file() else { return false };
        self.write_to(&p, now)
    }

    fn write_to(&mut self, path: &Path, now: f64) -> bool {
        self.commit_md();
        let data = match &self.md {
            Some(md) => md.full.clone(),
            None => {
                let mut s = self.text.clone();
                if !s.ends_with('\n') {
                    s.push('\n');
                }
                s
            }
        };
        match std::fs::write(path, data) {
            Ok(()) => {
                self.path = Some(path.to_path_buf());
                self.dirty = false;
                let msg = format!("{}: {}", self.tr("Saved"), path.display());
                self.flash(msg, now);
                true
            }
            Err(e) => {
                let msg = format!("{}: {e}", self.tr("Could not save"));
                self.flash(msg, now);
                false
            }
        }
    }

    fn export(&mut self, kind: &str, scale: f32, now: f64) {
        if self.parsed.has_errors() || self.scene.is_none() {
            let m = self.tr("Fix the errors before exporting.").to_string();
            self.flash(m, now);
            return;
        }
        let Some(scene) = self.scene.clone() else { return };
        let stem = self.path.as_ref().and_then(|p| p.file_stem()).map_or("diagram".to_string(), |n| n.to_string_lossy().to_string());
        let (ext, label) = match kind {
            "svg" => ("svg", "SVG"),
            "pdf" => ("pdf", "PDF"),
            _ => ("png", "PNG"),
        };
        let suffix = if kind == "png" && scale > 1.0 { format!("@{}x", scale as u32) } else { String::new() };
        let Some(dest) = rfd::FileDialog::new().set_file_name(format!("{stem}{suffix}.{ext}")).add_filter(label, &[ext]).save_file() else { return };
        let svg = scene.to_svg();
        let result = match kind {
            "svg" => Ok(svg.into_bytes()),
            "pdf" => engine::export::svg_to_pdf(&svg),
            _ => engine::export::svg_to_png(&svg, scale),
        };
        let msg = match result.and_then(|b| std::fs::write(&dest, b).map_err(|e| e.to_string())) {
            Ok(()) => format!("{}: {}", self.tr("Exported to"), dest.display()),
            Err(e) => format!("{}: {e}", self.tr("Could not export")),
        };
        self.flash(msg, now);
    }

    // ------------------------------------------------------------------------------ editing

    /// Replaces the text with the result of a visual edit (kept in the undo history).
    fn visual(&mut self, result: Result<String, edit::EditError>, now: f64) -> bool {
        match result {
            Ok(new) => {
                if new != self.text {
                    self.flash_lines = changed_lines(&self.text, &new).map(|(a, b)| (a, b, now));
                    self.history.push(std::mem::replace(&mut self.text, new));
                    self.future.clear();
                    if self.history.len() > 200 {
                        self.history.remove(0);
                    }
                    self.dirty = true;
                    self.reparse();
                }
                true
            }
            Err(e) => {
                self.flash(e.to_string(), now);
                false
            }
        }
    }

    fn undo(&mut self, now: f64) {
        self.commit_label(false, now);
        if let Some(prev) = self.history.pop() {
            self.future.push(std::mem::replace(&mut self.text, prev));
            self.dirty = true;
            self.reparse();
        }
    }

    fn redo(&mut self, now: f64) {
        self.commit_label(false, now);
        if let Some(next) = self.future.pop() {
            self.history.push(std::mem::replace(&mut self.text, next));
            self.dirty = true;
            self.reparse();
        }
    }

    fn add_node(&mut self, now: f64) {
        let label = self.tr("New node");
        let r = edit::add_node(&self.text, None, label, NodeShape::Rect);
        let id = r.as_ref().ok().map(|x| x.1.clone());
        if self.visual(r.map(|x| x.0), now) {
            if let Some(id) = id {
                self.sel = Sel::Node(id);
                self.focus_label = true;
                let m = self.tr("Created a node").to_string();
                self.flash(m, now);
            }
        }
    }

    fn add_link(&mut self, from: &str, to: &str, now: f64) {
        let r = edit::add_edge(&self.text, from, to, None, LinkKind::default());
        if self.visual(r, now) {
            let n = self.flow().map_or(0, |f| f.edges.len());
            self.sel = Sel::Edge(n.saturating_sub(1));
            let m = self.tr("Created a link").to_string();
            self.flash(m, now);
        }
    }

    fn delete_selected(&mut self, now: f64) {
        self.commit_label(false, now);
        let r = match &self.sel {
            Sel::Node(id) => edit::remove_node(&self.text, id),
            Sel::Edge(i) => edit::remove_edge(&self.text, *i),
            Sel::None => return,
        };
        if self.visual(r, now) {
            self.sel = Sel::None;
        }
    }

    // ---------------------------------------------------------------------------------- UI

    fn menu(&mut self, ui: &mut egui::Ui, now: f64) {
        let ctx = ui.ctx().clone();
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button(self.tr("File"), |ui| {
                ui.menu_button(self.tr("New"), |ui| {
                    for (label, kind) in [("New flowchart", "flowchart"), ("New sequence diagram", "sequence"), ("New ER diagram", "er")] {
                        if ui.button(self.tr(label)).clicked() {
                            ui.close();
                            self.guard(&ctx, After::New(kind));
                        }
                    }
                });
                if ui.add(egui::Button::new(self.tr("Open…")).shortcut_text("⌘O")).clicked() {
                    ui.close();
                    self.open_dialog(&ctx);
                }
                ui.separator();
                if ui.add(egui::Button::new(self.tr("Save")).shortcut_text("⌘S")).clicked() {
                    ui.close();
                    self.save(now);
                }
                if ui.add(egui::Button::new(self.tr("Save As…")).shortcut_text("⇧⌘S")).clicked() {
                    ui.close();
                    self.save_as(now);
                }
                ui.separator();
                ui.menu_button(self.tr("Export"), |ui| {
                    for (label, kind, scale) in [
                        ("Export SVG…", "svg", 1.0),
                        ("Export PNG 1x…", "png", 1.0),
                        ("Export PNG 2x…", "png", 2.0),
                        ("Export PNG 3x…", "png", 3.0),
                        ("Export PDF…", "pdf", 1.0),
                    ] {
                        if ui.button(self.tr(label)).clicked() {
                            ui.close();
                            self.export(kind, scale, now);
                        }
                    }
                });
                ui.separator();
                if ui.button(self.tr("Quit")).clicked() {
                    ui.close();
                    self.guard(&ctx, After::Close);
                }
            });
            ui.menu_button(self.tr("Edit"), |ui| {
                if ui.add_enabled(!self.history.is_empty(), egui::Button::new(self.tr("Undo")).shortcut_text("⌘Z")).clicked() {
                    ui.close();
                    self.undo(now);
                }
                if ui.add_enabled(!self.future.is_empty(), egui::Button::new(self.tr("Redo")).shortcut_text("⇧⌘Z")).clicked() {
                    ui.close();
                    self.redo(now);
                }
            });
            ui.menu_button(self.tr("View"), |ui| {
                if ui.add(egui::Button::new(self.tr("Fit to window")).shortcut_text("⌘0")).clicked() {
                    ui.close();
                    self.auto_fit = true;
                }
                if ui.add(egui::Button::new(self.tr("Actual size")).shortcut_text("⌘1")).clicked() {
                    ui.close();
                    self.zoom = 1.0;
                    self.auto_fit = false;
                }
                if ui.add(egui::Button::new(self.tr("Zoom in")).shortcut_text("⌘+")).clicked() {
                    self.zoom_by(1.25);
                }
                if ui.add(egui::Button::new(self.tr("Zoom out")).shortcut_text("⌘−")).clicked() {
                    self.zoom_by(0.8);
                }
                ui.separator();
                let dark_label = self.tr("Dark theme");
                if ui.checkbox(&mut self.dark, dark_label).changed() {
                    self.save_settings();
                }
                let mut look = self.look;
                ui.menu_button(self.tr("Appearance"), |ui| appearance_menu(ui, self.lang, &mut look));
                if look != self.look {
                    self.look = look;
                    self.save_settings();
                    self.reparse();
                }
                ui.menu_button(self.tr("Language"), |ui| {
                    for (lang, name) in [(Lang::En, "English"), (Lang::Pt, "Portuguese (Brazil)")] {
                        if ui.radio(self.lang == lang, self.tr(name)).clicked() {
                            self.lang = lang;
                            self.save_settings();
                            ui.close();
                        }
                    }
                });
            });
            ui.menu_button(self.tr("Help"), |ui| {
                if ui.button(self.tr("About MermaidCraft")).clicked() {
                    ui.close();
                    self.show_about = true;
                }
            });
        });
    }

    fn save_settings(&self) {
        if self.shot.is_none() {
            Settings { lang: self.lang, dark: self.dark, look: self.look }.save();
        }
    }

    fn open_dialog(&mut self, ctx: &egui::Context) {
        if self.shot.is_some() {
            return;
        }
        if let Some(p) = rfd::FileDialog::new().add_filter("Mermaid", &["mmd", "mermaid", "md", "markdown"]).pick_file() {
            self.guard(ctx, After::Open(p));
        }
    }

    fn zoom_by(&mut self, k: f32) {
        self.zoom = (self.zoom * k).clamp(0.1, 8.0);
        self.auto_fit = false;
    }

    fn shortcuts(&mut self, ctx: &egui::Context, now: f64) {
        let pressed = |ctx: &egui::Context, m: Modifiers, k: Key| ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(m, k)));
        if pressed(ctx, Modifiers::COMMAND | Modifiers::SHIFT, Key::S) {
            self.save_as(now);
        }
        if pressed(ctx, Modifiers::COMMAND, Key::S) {
            self.save(now);
        }
        if pressed(ctx, Modifiers::COMMAND, Key::O) {
            self.open_dialog(ctx);
        }
        if pressed(ctx, Modifiers::COMMAND, Key::N) {
            let k = self.untitled_kind;
            self.guard(ctx, After::New(k));
        }
        if pressed(ctx, Modifiers::COMMAND, Key::Num0) {
            self.auto_fit = true;
        }
        if pressed(ctx, Modifiers::COMMAND, Key::Num1) {
            self.zoom = 1.0;
            self.auto_fit = false;
        }
        if pressed(ctx, Modifiers::COMMAND, Key::Equals) || pressed(ctx, Modifiers::COMMAND, Key::Plus) {
            self.zoom_by(1.25);
        }
        if pressed(ctx, Modifiers::COMMAND, Key::Minus) {
            self.zoom_by(0.8);
        }
        if !ctx.egui_wants_keyboard_input() {
            if pressed(ctx, Modifiers::COMMAND | Modifiers::SHIFT, Key::Z) {
                self.redo(now);
            } else if pressed(ctx, Modifiers::COMMAND, Key::Z) {
                self.undo(now);
            }
            let del = ctx.input(|i| i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace));
            if del && self.flow().is_some() {
                self.delete_selected(now);
            }
            if ctx.input(|i| i.key_pressed(Key::Escape)) {
                self.link_from = None;
                self.sel = Sel::None;
            }
        }
    }

    fn status_bar(&mut self, ui: &mut egui::Ui, now: f64) {
        ui.horizontal_centered(|ui| {
            let name = format!("{}{}", self.display_name(), if self.dirty { " •" } else { "" });
            ui.label(RichText::new(name).strong());
            ui.separator();
            match &self.parsed.diagram {
                Some(d) => {
                    let s = engine::stats(d);
                    ui.label(format!("{} · {} {} · {} {}", s.kind, s.nodes, self.tr("nodes"), s.edges, self.tr("edges")));
                }
                None => {
                    ui.label(self.tr("Nothing to draw yet."));
                }
            }
            let errs = self.parsed.errors().count();
            if errs > 0 {
                ui.separator();
                ui.colored_label(Color32::from_rgb(220, 60, 60), format!("{errs} {}", if errs == 1 { self.tr("error") } else { self.tr("errors") }));
            }
            if let Some((msg, at)) = self.status.clone() {
                if now - at < 8.0 || at == 0.0 {
                    ui.separator();
                    ui.label(msg);
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(format!("{} {:.0}%", self.tr("Zoom"), self.zoom * 100.0));
            });
        });
    }

    fn editor(&mut self, ui: &mut egui::Ui) {
        // Markdown: choose which mermaid block to edit.
        if self.md.is_some() {
            let mut pick: Option<usize> = None;
            let mut add = false;
            let titles: Vec<String> = match &self.md {
                Some(md) => md.blocks.iter().map(|b| markdown::block_title(&md.full, b)).collect(),
                None => Vec::new(),
            };
            let current = self.md.as_ref().and_then(|m| m.current);
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(self.tr("Mermaid blocks")).strong());
                for (i, title) in titles.iter().enumerate() {
                    let label = format!("{} {} · {}", self.tr("Block"), i + 1, title);
                    if ui.selectable_label(current == Some(i), label).clicked() {
                        pick = Some(i);
                    }
                }
                if titles.is_empty() {
                    ui.label(self.tr("No mermaid blocks in this file."));
                }
                if ui.button(self.tr("Add block")).clicked() {
                    add = true;
                }
            });
            if let Some(i) = pick {
                self.select_block(i);
            }
            if add {
                self.add_block();
            }
            ui.separator();
        }
        // Problems list at the bottom.
        let diags = self.parsed.diagnostics.clone();
        let lang = self.lang;
        egui::Panel::bottom("problems").resizable(true).default_size(if diags.is_empty() { 28.0 } else { 90.0 }).show(ui, |ui| {
            ui.label(RichText::new(t(lang, "Problems")).strong());
            if diags.is_empty() {
                ui.weak(t(lang, "No problems"));
            }
            egui::ScrollArea::vertical().id_salt("problems_scroll").show(ui, |ui| {
                for d in diags.iter().take(100) {
                    let c = if d.severity == engine::Severity::Error { Color32::from_rgb(220, 60, 60) } else { Color32::from_rgb(200, 140, 20) };
                    ui.colored_label(c, format!("{} {}: {}", t(lang, "Line"), d.line, d.message));
                }
            });
        });
        // Follow a selection made on the drawing: scroll to its line.
        if self.sel != self.last_sel {
            if !self.sel_from_text {
                self.scroll_to_line = self.sel_line();
            }
            self.sel_from_text = false;
            self.last_sel = self.sel.clone();
        }
        let now = ui.input(|i| i.time);
        let sel_line = self.sel_line();
        let flash = self.flash_lines.filter(|f| now - f.2 < 1.4);
        if flash.is_some() {
            ui.ctx().request_repaint();
        } else {
            self.flash_lines = None;
        }
        let mut scroll_to = self.scroll_to_line.take();
        let mut cursor_at: Option<(usize, usize, bool)> = None;
        // Text editor with a line-number gutter.
        let dark = self.dark;
        let font = egui::FontId::monospace(14.0);
        let big = self.text.len() > 200_000;
        let font_for_layout = font.clone();
        let mut layouter = move |ui: &egui::Ui, buf: &dyn egui::TextBuffer, _wrap: f32| {
            let text = buf.as_str();
            let job = if big {
                let mut j = egui::text::LayoutJob::simple(text.to_string(), font_for_layout.clone(), ui.visuals().text_color(), f32::INFINITY);
                j.wrap.max_width = f32::INFINITY;
                j
            } else {
                Arc::unwrap_or_clone(crate::highlight::layout(text, dark, font_for_layout.clone()))
            };
            ui.ctx().fonts_mut(|f| f.layout_job(job))
        };
        let avail_h = ui.available_height();
        let row_h = ui.ctx().fonts_mut(|f| f.row_height(&font));
        let mut changed = false;
        let err_lines: Vec<(usize, bool)> = self.parsed.diagnostics.iter().map(|d| (d.line, d.severity == engine::Severity::Error)).collect();
        egui::ScrollArea::both().id_salt("editor_scroll").auto_shrink([false, false]).show(ui, |ui| {
            ui.horizontal_top(|ui| {
                let gutter_w = 46.0;
                let (gutter, _) = ui.allocate_exact_size(Vec2::new(gutter_w, 1.0), Sense::hover());
                let out = egui::TextEdit::multiline(&mut self.text)
                    .id_salt("editor_text")
                    .font(font.clone())
                    .desired_width(f32::INFINITY)
                    .desired_rows((avail_h / row_h).max(8.0) as usize)
                    .lock_focus(true)
                    .hint_text(t(lang, "Edit the Mermaid text here"))
                    .frame(egui::Frame::NONE)
                    .layouter(&mut layouter)
                    .show(ui);
                if out.response.changed() {
                    changed = true;
                }
                // The line of the text cursor (to select what is defined there).
                if let Some(cr) = out.cursor_range {
                    let idx: usize = cr.primary.index.into();
                    let byte = self.text.char_indices().nth(idx).map_or(self.text.len(), |(b, _)| b);
                    let line = 1 + self.text.get(..byte).unwrap_or("").matches('\n').count();
                    cursor_at = Some((byte, line, out.response.has_focus()));
                }
                let painter = ui.painter();
                let clip = ui.clip_rect();
                let dim = if dark { Color32::from_gray(120) } else { Color32::from_gray(150) };
                for (i, row) in out.galley.rows.iter().enumerate() {
                    let rect = row.rect().translate(out.galley_pos.to_vec2());
                    if scroll_to == Some(i + 1) {
                        ui.scroll_to_rect(egui::Rect::from_x_y_ranges(gutter.x_range(), rect.y_range()).expand2(Vec2::new(0.0, 40.0)), None);
                        scroll_to = None;
                    }
                    if rect.max.y < clip.min.y || rect.min.y > clip.max.y {
                        continue;
                    }
                    let line = i + 1;
                    let err = err_lines.iter().find(|(l, _)| *l == line);
                    let wide = egui::Rect::from_min_max(egui::pos2(gutter.min.x, rect.min.y), egui::pos2(out.response.rect.max.x.max(clip.max.x), rect.max.y));
                    if sel_line == Some(line) {
                        painter.rect_filled(wide, 0.0, Color32::from_rgba_unmultiplied(40, 120, 255, if dark { 46 } else { 30 }));
                    }
                    if let Some((a, b, t0)) = flash {
                        if (a..=b).contains(&line) {
                            let k = (1.0 - ((now - t0) / 1.4) as f32).clamp(0.0, 1.0);
                            painter.rect_filled(wide, 0.0, Color32::from_rgba_unmultiplied(250, 176, 5, (90.0 * k) as u8));
                        }
                    }
                    if let Some((_, is_err)) = err {
                        let c = if *is_err { Color32::from_rgba_unmultiplied(230, 60, 60, 38) } else { Color32::from_rgba_unmultiplied(230, 170, 30, 38) };
                        painter.rect_filled(wide, 0.0, c);
                        let uc = if *is_err { Color32::from_rgb(220, 60, 60) } else { Color32::from_rgb(210, 150, 20) };
                        painter.line_segment([egui::pos2(rect.min.x, rect.max.y - 1.0), egui::pos2(rect.min.x + row.row.size.x.max(24.0), rect.max.y - 1.0)], Stroke::new(1.5, uc));
                        painter.circle_filled(egui::pos2(gutter.min.x + 7.0, rect.center().y), 3.5, uc);
                    }
                    painter.text(egui::pos2(gutter.min.x + gutter_w - 8.0, rect.center().y), egui::Align2::RIGHT_CENTER, line.to_string(), font.clone(), if err.is_some() { Color32::from_rgb(220, 60, 60) } else { dim });
                }
            });
        });
        if changed {
            self.dirty = true;
            self.reparse();
        }
        // A cursor moved in the text selects the node or edge named there.
        if let Some((byte, line, focused)) = cursor_at {
            if focused && self.cursor_byte != Some(byte) {
                self.cursor_byte = Some(byte);
                let found = self.flow().and_then(|f| edit::item_at(f, byte, line)).map(|it| match it {
                    edit::TextItem::Node(id) => Sel::Node(id),
                    edit::TextItem::Edge(i) => Sel::Edge(i),
                });
                if let Some(sel) = found {
                    if sel != self.sel {
                        self.sel = sel;
                        self.sel_from_text = true;
                    }
                }
            } else if !focused {
                self.cursor_byte = None;
            }
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui, now: f64) {
        ui.horizontal_wrapped(|ui| {
            let is_flow = self.flow().is_some();
            if is_flow {
                if ui.selectable_label(self.tool == Tool::Select, self.tr("Select")).clicked() {
                    self.tool = Tool::Select;
                    self.link_from = None;
                }
                if ui.selectable_label(self.tool == Tool::Link, self.tr("Link")).clicked() {
                    self.tool = Tool::Link;
                    self.link_from = None;
                }
                if ui.button(self.tr("+ Node")).clicked() {
                    self.add_node(now);
                }
                if ui.button(self.tr("Auto-arrange")).clicked() {
                    self.auto_arrange(now);
                }
                ui.separator();
                let cur = self.flow().map_or(Direction::TopDown, |f| f.direction);
                let mut chosen = cur;
                egui::ComboBox::from_id_salt("direction").selected_text(format!("{}: {}", self.tr("Direction"), cur.code())).show_ui(ui, |ui| {
                    for d in Direction::all() {
                        let name = match d {
                            Direction::TopDown => "Top to bottom",
                            Direction::BottomUp => "Bottom to top",
                            Direction::LeftRight => "Left to right",
                            Direction::RightLeft => "Right to left",
                        };
                        ui.selectable_value(&mut chosen, d, format!("{} ({})", self.tr(name), d.code()));
                    }
                });
                if chosen != cur {
                    let r = edit::set_direction(&self.text, chosen);
                    self.visual(r, now);
                }
                ui.separator();
            }
            if ui.add_enabled(!self.history.is_empty(), egui::Button::new(self.tr("Undo"))).clicked() {
                self.undo(now);
            }
            if ui.add_enabled(!self.future.is_empty(), egui::Button::new(self.tr("Redo"))).clicked() {
                self.redo(now);
            }
            ui.separator();
            if ui.button("−").on_hover_text(self.tr("Zoom out")).clicked() {
                self.zoom_by(0.8);
            }
            if ui.button(format!("{:.0}%", self.zoom * 100.0)).on_hover_text(self.tr("Actual size")).clicked() {
                self.zoom = 1.0;
                self.auto_fit = false;
            }
            if ui.button("+").on_hover_text(self.tr("Zoom in")).clicked() {
                self.zoom_by(1.25);
            }
            if ui.button(self.tr("Fit to window")).clicked() {
                self.auto_fit = true;
            }
        });
    }

    /// Whether the label box of the style panel has the keyboard.
    fn label_focused(ctx: &egui::Context) -> bool {
        ctx.memory(|m| m.has_focus(egui::Id::new("label_field")))
    }

    /// Saves a label typed in the style panel on the node or edge it was typed for. It waits while
    /// the box still has the keyboard for that same selection; it saves at once when the box lost
    /// focus (`focused` false), when the selection moved elsewhere, and before anything else
    /// changes the diagram (delete, undo, redo, a new node).
    fn commit_label(&mut self, focused: bool, now: f64) {
        let Some(target) = self.label_editing.clone() else { return };
        if focused && target == self.sel {
            return;
        }
        self.label_editing = None;
        let text = self.label_buf.clone();
        let Some(f) = self.flow() else { return };
        let r = match &target {
            Sel::Node(id) => match f.node_index(id).and_then(|i| f.nodes.get(i)) {
                Some(n) if n.label != text => edit::set_node(&self.text, id, Some(&text), None),
                _ => return,
            },
            Sel::Edge(i) => match f.edges.get(*i) {
                Some(e) if e.label.clone().unwrap_or_default() != text => {
                    let ends = (e.from.clone(), e.to.clone());
                    let r = edit::set_edge(&self.text, *i, Some(Some(text.as_str())), None);
                    // A shared link (`A & B --> C`) moves the edge to a line of its own and the edge
                    // numbers shift: keep the selection on the same edge.
                    if self.visual(r, now) && self.sel == target {
                        let label = Some(text);
                        if let Some(n) = self.flow().and_then(|f| f.edges.iter().position(|e| e.from == ends.0 && e.to == ends.1 && e.label == label)) {
                            self.sel = Sel::Edge(n);
                            self.last_sel = self.sel.clone();
                        }
                    }
                    return;
                }
                _ => return,
            },
            Sel::None => return,
        };
        self.visual(r, now);
    }

    /// The tools and the style panel, floating over the canvas.
    fn floating(&mut self, ctx: &egui::Context, now: f64) {
        // A label left behind by a click in the text is saved before the panel follows the text.
        // Esc gives up what was typed (egui drops the focus on Esc as the frame starts).
        if self.label_editing.is_some() && ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.label_editing = None;
        }
        self.commit_label(Self::label_focused(ctx), now);
        let Some(rect) = self.canvas_rect else {
            // The canvas is placed in this frame; draw the tools in the next one.
            ctx.request_repaint();
            return;
        };
        let frame = egui::Frame::popup(&ctx.global_style()).corner_radius(10.0).inner_margin(egui::Margin::symmetric(10, 6));
        let bar = egui::Area::new(egui::Id::new("float_toolbar")).fixed_pos(rect.center_top() + Vec2::new(0.0, 10.0)).pivot(egui::Align2::CENTER_TOP).constrain_to(rect).show(ctx, |ui| {
            frame.show(ui, |ui| {
                // Wraps to a second row when the canvas is narrow.
                ui.set_max_width((rect.width() - 40.0).max(200.0));
                self.toolbar(ui, now);
            });
        });
        self.tools_h = bar.response.rect.height();
        self.inspector_rect = None;
        if self.flow().is_some() && self.sel != Sel::None {
            let panel = egui::Frame::popup(&ctx.global_style()).corner_radius(10.0).inner_margin(egui::Margin::same(12));
            let shown = egui::Area::new(egui::Id::new("float_inspector")).fixed_pos(rect.right_top() + Vec2::new(-12.0, 10.0 + self.tools_h + 10.0)).pivot(egui::Align2::RIGHT_TOP).constrain_to(rect).show(ctx, |ui| {
                panel.show(ui, |ui| {
                    ui.set_width(236.0);
                    self.inspector(ui, now);
                });
            });
            // The first frame only measures the panel: its place is known from the next one.
            let layer = shown.response.layer_id;
            if ctx.memory(|m| m.areas().visible_last_frame(&layer)) {
                self.inspector_rect = Some(shown.response.rect);
            }
        }
    }

    /// Style panel of the selected node or edge.
    fn inspector(&mut self, ui: &mut egui::Ui, now: f64) {
        // A new node asks for the label box: save what was being typed for the previous one first.
        if self.focus_label {
            self.commit_label(false, now);
        }
        let Some(flow) = self.flow().cloned() else { return };
        let sel = self.sel.clone();
        // Keep the label box in step with the selection unless it is being edited.
        let current_label = match &sel {
            Sel::Node(id) => flow.node_index(id).and_then(|i| flow.nodes.get(i)).map(|n| n.label.clone()),
            Sel::Edge(i) => flow.edges.get(*i).map(|e| e.label.clone().unwrap_or_default()),
            Sel::None => None,
        };
        // Keep the label box in step with the selection and with the text, unless a label typed in it
        // is still waiting to be saved.
        if self.label_editing.is_none() {
            self.label_buf = current_label.clone().unwrap_or_default();
        }
        let section = |ui: &mut egui::Ui, text: &str| {
            ui.add_space(4.0);
            ui.label(RichText::new(text).small().weak());
        };
        match &sel {
            Sel::None => {}
            Sel::Node(id) => {
                let Some(node) = flow.node_index(id).and_then(|i| flow.nodes.get(i)).cloned() else { return };
                ui.label(RichText::new(format!("{} {}", self.tr("Node"), node.id)).strong());
                section(ui, self.tr("Label"));
                let id_label = egui::Id::new("label_field");
                let resp = ui.add(egui::TextEdit::singleline(&mut self.label_buf).id(id_label).desired_width(f32::INFINITY));
                if self.focus_label {
                    self.focus_label = false;
                    resp.request_focus();
                    if let Some(mut st) = egui::TextEdit::load_state(ui.ctx(), id_label) {
                        let n = self.label_buf.chars().count();
                        st.cursor.set_char_range(Some(egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(n))));
                        st.store(ui.ctx(), id_label);
                    }
                }
                if resp.has_focus() {
                    self.label_editing = Some(sel.clone());
                }
                section(ui, self.tr("Shape"));
                let mut shape = node.shape;
                egui::ComboBox::from_id_salt("shape").width(ui.available_width()).selected_text(self.tr(shape.name())).show_ui(ui, |ui| {
                    for s in NodeShape::all() {
                        ui.selectable_value(&mut shape, s, self.tr(s.name()));
                    }
                });
                if shape != node.shape {
                    let r = edit::set_node(&self.text, &node.id, None, Some(shape));
                    self.visual(r, now);
                }
                // Colour: automatic (look and group) or one of the palette tones.
                // The panel shows and rewrites the node's own `style` line.
                let styles = edit::own_node_style(&self.text, &flow, &node.id);
                let shared = flow.node_styles.get(&node.id).is_some_and(|all| all != &styles) || !node.classes.is_empty();
                let fill = styles.get("fill").map(|v| v.to_ascii_lowercase());
                section(ui, self.tr("Colour"));
                let mut pick: Option<Option<engine::look::Hue>> = None;
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::splat(5.0);
                    if ui.selectable_label(fill.is_none(), self.tr("Auto")).clicked() {
                        pick = Some(None);
                    }
                    for (name, hue) in engine::look::SWATCHES {
                        let chosen = fill.as_deref() == Some(engine::look::hex(hue.light).as_str());
                        let (r, resp) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::click());
                        let p = ui.painter();
                        p.rect_filled(r.shrink(1.0), 5.0, paint::shade(hue.light, self.dark));
                        p.rect_stroke(r.shrink(1.0), 5.0, Stroke::new(if chosen { 2.6 } else { 1.2 }, paint::shade(hue.stroke, self.dark)), egui::StrokeKind::Inside);
                        if resp.on_hover_text(self.tr(name)).clicked() {
                            pick = Some(Some(hue));
                        }
                    }
                });
                if let Some(choice) = pick {
                    self.restyle(
                        &node.id,
                        |m| match choice {
                            None => {
                                m.remove("fill");
                                m.remove("stroke");
                            }
                            Some(h) => {
                                m.insert("fill".into(), engine::look::hex(h.light));
                                m.insert("stroke".into(), engine::look::hex(h.stroke));
                            }
                        },
                        now,
                    );
                }
                // Outline: solid or dashed.
                section(ui, self.tr("Outline"));
                let dashed = styles.contains_key("stroke-dasharray");
                let mut want = dashed;
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut want, false, self.tr("Solid"));
                    ui.selectable_value(&mut want, true, self.tr("Dashed"));
                });
                if want != dashed {
                    self.restyle(
                        &node.id,
                        |m| {
                            if want {
                                m.insert("stroke-dasharray".into(), "5 5".into());
                            } else {
                                m.remove("stroke-dasharray");
                            }
                        },
                        now,
                    );
                }
                if shared {
                    ui.add_space(4.0);
                    ui.label(RichText::new(self.tr("Part of this node's style comes from a class or a shared style line; the panel changes only its own line.")).small().weak());
                }
                ui.add_space(6.0);
                ui.separator();
                if ui.button(self.tr("Delete")).clicked() {
                    self.delete_selected(now);
                }
            }
            Sel::Edge(i) => {
                let Some(e) = flow.edges.get(*i).cloned() else { return };
                ui.label(RichText::new(format!("{} {} → {}", self.tr("Edge"), e.from, e.to)).strong());
                section(ui, self.tr("Label"));
                let resp = ui.add(egui::TextEdit::singleline(&mut self.label_buf).id(egui::Id::new("label_field")).desired_width(f32::INFINITY));
                if resp.has_focus() {
                    self.label_editing = Some(sel.clone());
                }
                section(ui, self.tr("Line"));
                let mut style = e.kind.style;
                egui::ComboBox::from_id_salt("edge_style").width(ui.available_width()).selected_text(self.tr(style.name())).show_ui(ui, |ui| {
                    for s in [LineStyle::Solid, LineStyle::Dotted, LineStyle::Thick, LineStyle::Invisible] {
                        ui.selectable_value(&mut style, s, self.tr(s.name()));
                    }
                });
                section(ui, self.tr("Arrow"));
                let mut head = e.kind.head_end;
                egui::ComboBox::from_id_salt("edge_head").width(ui.available_width()).selected_text(self.tr(head.name())).show_ui(ui, |ui| {
                    for h in [Head::Arrow, Head::None, Head::Circle, Head::Cross] {
                        ui.selectable_value(&mut head, h, self.tr(h.name()));
                    }
                });
                if style != e.kind.style || head != e.kind.head_end {
                    let kind = LinkKind { style, head_end: head, ..e.kind };
                    let r = edit::set_edge(&self.text, *i, None, Some(kind));
                    self.visual(r, now);
                }
                ui.add_space(6.0);
                ui.separator();
                if ui.button(self.tr("Delete")).clicked() {
                    self.delete_selected(now);
                }
            }
        }
    }

    fn canvas(&mut self, ui: &mut egui::Ui, now: f64) {
        let size = ui.available_size();
        let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        self.canvas_rect = Some(rect);
        painter.rect_filled(rect, 0.0, paint::paper(self.scene.as_deref(), self.dark));
        let Some(scene) = self.scene.clone() else {
            let msg = self.parsed.diagnostics.first().map_or(self.tr("Nothing to draw yet.").to_string(), |d| format!("{} {}: {}", self.tr("Line"), d.line, d.message));
            painter.text(rect.center(), egui::Align2::CENTER_CENTER, msg, egui::FontId::proportional(15.0), Color32::from_rgb(200, 80, 80));
            return;
        };
        let (sw, sh) = (scene.width.max(1.0), scene.height.max(1.0));
        if self.auto_fit {
            // Leave room for the floating tools at the top.
            let top = self.tools_h + 16.0;
            let z = ((rect.width() - 40.0) / sw).min((rect.height() - 40.0 - top) / sh).clamp(0.1, 1.5);
            self.zoom = z;
            self.pan = Vec2::new((rect.width() - sw * z) / 2.0, top + (rect.height() - top - sh * z) / 2.0);
        }
        let pointer = ui.input(|i| i.pointer.hover_pos()).filter(|p| rect.contains(*p));
        if resp.hovered() {
            let zd = ui.input(|i| i.zoom_delta());
            let scroll = ui.input(|i| i.smooth_scroll_delta);
            if (zd - 1.0).abs() > 1e-4 {
                let anchor = pointer.unwrap_or(rect.center());
                let old = self.zoom;
                let new = (old * zd).clamp(0.1, 8.0);
                let sp = ((anchor.x - rect.min.x - self.pan.x) / old, (anchor.y - rect.min.y - self.pan.y) / old);
                self.pan = Vec2::new(anchor.x - rect.min.x - sp.0 * new, anchor.y - rect.min.y - sp.1 * new);
                self.zoom = new;
                self.auto_fit = false;
            } else if scroll != Vec2::ZERO {
                if ui.input(|i| i.modifiers.shift) {
                    // Shift + wheel moves the view.
                    self.pan += Vec2::new(scroll.x + scroll.y, 0.0);
                    self.auto_fit = false;
                } else if scroll.y.abs() > 0.0 {
                    // The wheel zooms around the pointer.
                    let anchor = pointer.unwrap_or(rect.center());
                    let old = self.zoom;
                    let new = (old * (scroll.y * 0.004).exp()).clamp(0.1, 8.0);
                    let sp = ((anchor.x - rect.min.x - self.pan.x) / old, (anchor.y - rect.min.y - self.pan.y) / old);
                    self.pan = Vec2::new(anchor.x - rect.min.x - sp.0 * new, anchor.y - rect.min.y - sp.1 * new);
                    self.zoom = new;
                    self.auto_fit = false;
                } else {
                    self.pan += Vec2::new(scroll.x, 0.0);
                    self.auto_fit = false;
                }
            }
        }
        // A node or edge selected under the style panel slides left, out from under it (once per
        // selection, never while dragging nor while typing: an id typed in the text is a new
        // selection at every letter).
        if self.drag.is_some() || ui.ctx().egui_wants_keyboard_input() {
            self.revealed = self.sel.clone();
        } else if self.sel != self.revealed {
            let view = View { origin: rect.min + self.pan, zoom: self.zoom };
            let target = self.flow().and_then(|flow| match &self.sel {
                Sel::Node(id) => flow.node_index(id).and_then(|i| scene.node_boxes.get(i)).map(|b| egui::Rect::from_min_size(view.to_screen((b.x, b.y)), Vec2::new(b.w, b.h) * view.zoom)),
                Sel::Edge(i) => scene.edge_paths.get(*i).and_then(|path| path.get(path.len() / 2)).map(|p| egui::Rect::from_center_size(view.to_screen(*p), Vec2::splat(24.0))),
                Sel::None => None,
            });
            // A click may still be the first half of a double-click (which opens the label): sliding
            // now would put the node away from the second click. egui's window is 0.3 s.
            let settling = ui.input(|i| i.pointer.any_down() || i.pointer.time_since_last_click() < 0.35);
            match (target, self.inspector_rect) {
                (Some(_), Some(_)) if settling => ui.ctx().request_repaint_after(std::time::Duration::from_millis(50)),
                (Some(target), Some(panel)) => {
                    let dx = reveal_shift(target, panel, rect);
                    if dx != 0.0 {
                        self.pan.x += dx;
                        self.auto_fit = false;
                    }
                    self.revealed = self.sel.clone();
                }
                (Some(_), None) => ui.ctx().request_repaint(),
                (None, _) => self.revealed = self.sel.clone(),
            }
        }
        let view = View { origin: rect.min + self.pan, zoom: self.zoom };
        paint::dot_grid(&painter, rect, &view, self.dark);
        paint::draw(&painter, &scene, &view, self.dark);
        // With nothing selected, a short hint at the foot of the canvas.
        if self.sel == Sel::None {
            let hint = if self.flow().is_none() {
                self.tr("Visual editing works on flowcharts; edit the text for this diagram type.")
            } else if self.link_from.is_some() {
                self.tr("Click the node to connect to")
            } else {
                self.tr("Click a node or an edge to edit it. + Node adds a node; Link (or dragging from one node to another) connects them. Drag a group by its title to move it.")
            };
            let weak = if self.dark { Color32::from_gray(150) } else { Color32::from_gray(120) };
            let galley = painter.layout(hint.to_string(), egui::FontId::proportional(12.5), weak, (rect.width() - 28.0).max(120.0));
            painter.galley(rect.left_bottom() + Vec2::new(14.0, -12.0 - galley.size().y), galley, weak);
        }
        let is_flow = self.flow().is_some();
        let accent = Color32::from_rgb(40, 120, 255);
        let hit_at = |p: egui::Pos2| paint::hit_test(&scene, view.to_scene(p), 6.0 / view.zoom.max(0.1));
        // Selection overlay.
        if is_flow {
            let flow = self.flow().cloned();
            if let (Some(flow), sel) = (flow, self.sel.clone()) {
                match sel {
                    Sel::Node(id) => {
                        if let Some(b) = flow.node_index(&id).and_then(|i| scene.node_boxes.get(i)) {
                            let r = egui::Rect::from_min_size(view.to_screen((b.x, b.y)), Vec2::new(b.w, b.h) * view.zoom).expand(4.0);
                            painter.rect_stroke(r, 4.0, Stroke::new(2.0, accent), egui::StrokeKind::Middle);
                        }
                    }
                    Sel::Edge(i) => {
                        if let Some(path) = scene.edge_paths.get(i) {
                            let pts: Vec<egui::Pos2> = path.iter().map(|p| view.to_screen(*p)).collect();
                            painter.add(egui::Shape::line(pts, Stroke::new(5.0, Color32::from_rgba_unmultiplied(40, 120, 255, 110))));
                        }
                    }
                    Sel::None => {}
                }
                if let Some(l) = self.link_from {
                    if let Some(b) = scene.node_boxes.get(l) {
                        let r = egui::Rect::from_min_size(view.to_screen((b.x, b.y)), Vec2::new(b.w, b.h) * view.zoom).expand(4.0);
                        painter.rect_stroke(r, 4.0, Stroke::new(2.0, Color32::from_rgb(230, 130, 20)), egui::StrokeKind::Middle);
                        if let Some(p) = pointer {
                            painter.line_segment([r.center(), p], Stroke::new(2.0, Color32::from_rgb(230, 130, 20)));
                        }
                    }
                }
            }
        }
        // Pointer interactions.
        if let Some(p) = pointer {
            if is_flow && matches!(hit_at(p), Hit::Node(_)) {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            } else if is_flow && self.drag.is_none() && matches!(hit_at(p), Hit::Cluster(_) | Hit::Nothing) && paint::group_handle_at(&scene, view.to_scene(p)).is_some() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
            }
        }
        let flow_ids: Vec<String> = self.flow().map(|f| f.nodes.iter().map(|n| n.id.clone()).collect()).unwrap_or_default();
        if resp.drag_started() {
            if let Some(p) = resp.interact_pointer_pos() {
                let linking = self.tool == Tool::Link || ui.input(|i| i.modifiers.shift);
                self.drag = Some(match hit_at(p) {
                    Hit::Node(i) if is_flow && linking => Drag::Link { from: i },
                    Hit::Node(i) if is_flow => match flow_ids.get(i) {
                        Some(id) => {
                            let offset = self.flow().and_then(|f| f.offsets.get(id).copied()).unwrap_or((0.0, 0.0));
                            self.auto_fit = false;
                            self.sel = Sel::Node(id.clone());
                            Drag::Move { id: id.clone(), offset, group: false }
                        }
                        None => Drag::Pan,
                    },
                    // The title strip of a group frame moves the whole group.
                    Hit::Cluster(_) | Hit::Nothing if is_flow && !linking => {
                        // Only a group whose id is unique and fits the layout comment can be kept.
                        let key = paint::group_handle_at(&scene, view.to_scene(p)).and_then(|c| {
                            let f = self.flow()?;
                            let id = &f.subgraphs.get(c)?.id;
                            let unique = f.subgraphs.iter().filter(|sg| &sg.id == id).count() == 1;
                            let storable = !id.is_empty() && !id.contains(|ch: char| ch.is_whitespace() || ch == '=' || ch == ',');
                            (unique && storable).then(|| engine::flowchart::group_offset_key(id))
                        });
                        match key {
                            Some(key) => {
                                let offset = self.flow().and_then(|f| f.offsets.get(&key).copied()).unwrap_or((0.0, 0.0));
                                self.auto_fit = false;
                                Drag::Move { id: key, offset, group: true }
                            }
                            None => Drag::Pan,
                        }
                    }
                    _ => Drag::Pan,
                });
            }
        }
        if resp.dragged() {
            match &self.drag {
                Some(Drag::Pan) => {
                    self.pan += resp.drag_delta();
                    self.auto_fit = false;
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                }
                Some(Drag::Link { from }) => {
                    if let (Some(b), Some(p)) = (scene.node_boxes.get(*from), resp.interact_pointer_pos()) {
                        let start = view.to_screen(b.center());
                        painter.line_segment([start, p], Stroke::new(2.5, accent));
                        painter.circle_filled(p, 4.0, accent);
                        if let Hit::Node(j) = hit_at(p) {
                            if j != *from {
                                if let Some(tb) = scene.node_boxes.get(j) {
                                    let r = egui::Rect::from_min_size(view.to_screen((tb.x, tb.y)), Vec2::new(tb.w, tb.h) * view.zoom).expand(4.0);
                                    painter.rect_stroke(r, 4.0, Stroke::new(2.0, accent), egui::StrokeKind::Middle);
                                }
                            }
                        }
                    }
                }
                Some(Drag::Move { id, offset, group }) => {
                    let d = resp.drag_delta() / view.zoom;
                    let (id, new, group) = (id.clone(), (offset.0 + d.x, offset.1 + d.y), *group);
                    self.drag = Some(Drag::Move { id: id.clone(), offset: new, group });
                    self.redraw_with_offset(&id, new);
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                }
                None => {}
            }
        }
        if resp.drag_stopped() {
            // The drag is taken once: a link drag must not be dropped by the check for a move.
            let ended = self.drag.take();
            if let Some(Drag::Move { id, offset, group }) = ended {
                self.commit_offset(&id, offset, now);
                let m = self.tr(if group { "Moved a group" } else { "Moved a node" }).to_string();
                self.flash(m, now);
                return;
            }
            if let (Some(Drag::Link { from }), Some(p)) = (ended, ui.input(|i| i.pointer.interact_pos())) {
                let moved = scene.node_boxes.get(from).is_some_and(|b| {
                    let c = view.to_screen(b.center());
                    (p - c).length() > 20.0 * view.zoom
                });
                if let Some(src) = flow_ids.get(from).cloned() {
                    match hit_at(p) {
                        Hit::Node(j) if j != from => {
                            if let Some(dst) = flow_ids.get(j).cloned() {
                                self.add_link(&src, &dst, now);
                            }
                        }
                        Hit::Nothing if moved && rect.contains(p) => {
                            // Dropped on empty space: a new node, linked.
                            let label = self.tr("New node");
                            if let Ok((text, id)) = edit::add_node(&self.text, None, label, NodeShape::Rect) {
                                if let Ok(text2) = edit::add_edge(&text, &src, &id, None, LinkKind::default()) {
                                    if self.visual(Ok(text2), now) {
                                        self.sel = Sel::Node(id);
                                        self.focus_label = true;
                                        let m = self.tr("Created a node and a link").to_string();
                                        self.flash(m, now);
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            } else {
                self.drag = None;
            }
        }
        if resp.clicked() {
            if let Some(p) = resp.interact_pointer_pos() {
                let hit = hit_at(p);
                if is_flow {
                    match (self.tool, hit) {
                        (Tool::Link, Hit::Node(i)) => match self.link_from {
                            Some(a) if a != i => {
                                if let (Some(s), Some(d)) = (flow_ids.get(a).cloned(), flow_ids.get(i).cloned()) {
                                    self.link_from = None;
                                    self.add_link(&s, &d, now);
                                }
                            }
                            _ => self.link_from = Some(i),
                        },
                        (_, Hit::Node(i)) => {
                            if let Some(id) = flow_ids.get(i) {
                                self.sel = Sel::Node(id.clone());
                            }
                        }
                        (_, Hit::Edge(i)) => self.sel = Sel::Edge(i),
                        _ => {
                            self.sel = Sel::None;
                            self.link_from = None;
                        }
                    }
                }
            }
        }
        if resp.double_clicked() {
            if let Some(p) = resp.interact_pointer_pos() {
                if let (true, Hit::Node(_)) = (is_flow, hit_at(p)) {
                    self.focus_label = true;
                }
            }
        }
    }

    fn prompt_window(&mut self, ctx: &egui::Context, now: f64) {
        let Some(after) = self.prompt.clone() else { return };
        let mut done = false;
        egui::Window::new(self.tr("Unsaved changes")).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            ui.label(self.tr("Save the changes before closing?"));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(self.tr("Save")).clicked() && self.save(now) {
                    done = true;
                    self.prompt = None;
                    self.run_after(ctx, after.clone());
                }
                if ui.button(self.tr("Discard")).clicked() {
                    done = true;
                    self.prompt = None;
                    self.dirty = false;
                    self.run_after(ctx, after.clone());
                }
                if ui.button(self.tr("Cancel")).clicked() {
                    done = true;
                    self.prompt = None;
                }
            });
        });
        let _ = done;
    }

    fn handle_screenshot(&mut self, ctx: &egui::Context) {
        let frames = self.frames;
        let Some(shot) = &mut self.shot else { return };
        ctx.request_repaint();
        if frames >= 120 && !shot.requested {
            shot.requested = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        let image = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(img) = image {
            let [w, h] = img.size;
            let rgba: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
            let code = match engine::export::rgba_to_png(w as u32, h as u32, &rgba).and_then(|png| std::fs::write(&shot.path, png).map_err(|e| e.to_string())) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("could not save the screenshot: {e}");
                    1
                }
            };
            std::process::exit(code);
        }
        if frames > 900 {
            eprintln!("the screenshot never arrived");
            std::process::exit(1);
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.frames += 1;
        let now = ctx.input(|i| i.time);
        ctx.set_visuals(if self.dark { egui::Visuals::dark() } else { egui::Visuals::light() });
        // Documents from the system or dropped on the window.
        let (files, quit) = self.events.drain();
        let mut files = files;
        files.extend(ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).collect::<Vec<_>>()));
        if let Some(f) = files.into_iter().next() {
            self.guard(&ctx, After::Open(f));
        }
        if quit {
            self.guard(&ctx, After::Close);
        }
        if ctx.input(|i| i.viewport().close_requested()) && self.dirty && !self.allow_close && self.shot.is_none() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.prompt = Some(After::Close);
        }
        self.shortcuts(&ctx, now);
        // Window title.
        let title = format!("{}{} — MermaidCraft", self.display_name(), if self.dirty { " •" } else { "" });
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }

        egui::Panel::top("menu").show(ui, |ui| self.menu(ui, now));
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui, now));
        let width = ui.available_width();
        egui::Panel::left("editor").resizable(true).default_size(width * 0.42).min_size(240.0).show(ui, |ui| self.editor(ui));
        // The floating tools go first (placed over last frame's canvas): a label being typed in the
        // style panel is saved before a click on the canvas changes the selection.
        self.floating(&ctx, now);
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(ui, |ui| self.canvas(ui, now));
        // The label box may have lost focus to a click anywhere this frame (canvas, tool bar, Enter).
        self.commit_label(Self::label_focused(&ctx), now);
        if self.show_about {
            let mut open = true;
            egui::Window::new(self.tr("About MermaidCraft")).open(&mut open).collapsible(false).resizable(false).show(&ctx, |ui| {
                ui.heading("MermaidCraft");
                ui.label(format!("{} {}", self.tr("Version"), env!("CARGO_PKG_VERSION")));
                ui.label(self.tr("Supported diagrams: flowchart, sequence, ER."));
            });
            self.show_about = open;
        }
        self.prompt_window(&ctx, now);
        self.handle_screenshot(&ctx);
    }
}

/// How far to move the view (negative is left) so `target` is no longer under `panel`, keeping its
/// left edge inside the canvas. Zero when they do not overlap.
fn reveal_shift(target: egui::Rect, panel: egui::Rect, canvas: egui::Rect) -> f32 {
    const GAP: f32 = 16.0;
    if !target.intersects(panel) {
        return 0.0;
    }
    let need = target.max.x - (panel.min.x - GAP);
    let room = target.min.x - (canvas.min.x + GAP);
    -need.min(room).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::{changed_lines, reveal_shift};
    use eframe::egui::{pos2, Rect};

    #[test]
    fn a_node_under_the_panel_slides_left_of_it() {
        let canvas = Rect::from_min_max(pos2(0.0, 0.0), pos2(800.0, 600.0));
        let panel = Rect::from_min_max(pos2(528.0, 60.0), pos2(788.0, 400.0));
        // Under the panel: its right edge ends 16 px left of the panel.
        let node = Rect::from_min_max(pos2(600.0, 100.0), pos2(700.0, 140.0));
        assert_eq!(reveal_shift(node, panel, canvas), -(700.0 - 512.0));
        // Below the panel, or clear of it: nothing moves.
        assert_eq!(reveal_shift(node.translate(eframe::egui::vec2(0.0, 400.0)), panel, canvas), 0.0);
        assert_eq!(reveal_shift(node.translate(eframe::egui::vec2(-300.0, 0.0)), panel, canvas), 0.0);
        // A node wider than the room left keeps its left edge on the canvas.
        let wide = Rect::from_min_max(pos2(100.0, 100.0), pos2(700.0, 140.0));
        assert_eq!(reveal_shift(wide, panel, canvas), -84.0);
    }

    #[test]
    fn app_version_matches_the_workspace() {
        // The app writes its version in its own [package]; the engine and the CLI inherit it.
        let manifest = include_str!("../Cargo.toml");
        let ws = manifest.split("[workspace.package]").nth(1).and_then(|r| r.lines().find_map(|l| l.trim().strip_prefix("version = "))).map(|v| v.trim_matches('"'));
        assert_eq!(ws, Some(env!("CARGO_PKG_VERSION")), "[package] and [workspace.package] versions differ in Cargo.toml");
    }

    #[test]
    fn changed_lines_cover_edits_inserts_and_deletes() {
        let base = "flowchart TD\n  A --> B\n  B --> C\n";
        // A line changed in the middle.
        assert_eq!(changed_lines(base, "flowchart TD\n  A --> X\n  B --> C\n"), Some((2, 2)));
        // A line added at the end (style written by the panel).
        assert_eq!(changed_lines(base, "flowchart TD\n  A --> B\n  B --> C\n  style A fill:#fff\n"), Some((4, 4)));
        // A line replaced and one appended (old style line removed, new one at the end).
        assert_eq!(changed_lines("a\nstyle A x\nb\n", "a\nb\nstyle A y\n"), Some((2, 3)));
        // A pure deletion marks where the text went.
        assert_eq!(changed_lines(base, "flowchart TD\n  B --> C\n"), Some((2, 2)));
        assert_eq!(changed_lines(base, base), None);
    }
}

/// Mouse gestures on the canvas, run frame by frame without a window.
#[cfg(test)]
mod gestures {
    use super::*;
    use egui::{Event, Modifiers, PointerButton, Pos2, RawInput, Rect};

    struct Harness {
        ctx: egui::Context,
        app: App,
        frame: eframe::Frame,
        t: f64,
    }

    impl Harness {
        fn new(text: &str) -> Harness {
            let ctx = egui::Context::default();
            let cc = eframe::CreationContext::_new_kittest(ctx.clone());
            let mut app = App::new(&cc, Options { screenshot: None, files: Vec::new(), events: crate::platform::OpenEvents::none() });
            app.set_doc(text.to_string(), None, None);
            let mut h = Harness { ctx, app, frame: eframe::Frame::_new_kittest(), t: 0.0 };
            for _ in 0..4 {
                h.step(Vec::new(), Modifiers::NONE);
            }
            h
        }

        fn step(&mut self, events: Vec<Event>, m: Modifiers) {
            self.t += 0.05;
            let events = std::iter::once(Event::ModifiersChanged(m)).chain(events).collect();
            let input = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1280.0, 800.0))), time: Some(self.t), focused: true, events, ..Default::default() };
            let (app, frame) = (&mut self.app, &mut self.frame);
            let mut out = self.ctx.run_ui(input, |ui| eframe::App::ui(app, ui, frame));
            out.textures_delta.clear();
        }

        /// Centre of a node on the screen.
        fn node(&self, id: &str) -> Pos2 {
            let i = self.app.flow().and_then(|f| f.node_index(id)).expect("node");
            let b = self.app.scene.as_ref().and_then(|s| s.node_boxes.get(i).copied()).expect("box");
            let o = self.app.canvas_rect.expect("canvas").min + self.app.pan;
            Pos2::new(o.x + (b.x + b.w / 2.0) * self.app.zoom, o.y + (b.y + b.h / 2.0) * self.app.zoom)
        }

        fn button(&mut self, p: Pos2, down: bool, m: Modifiers) {
            self.step(vec![Event::PointerMoved(p), Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: down, modifiers: m }], m);
        }

        fn click(&mut self, p: Pos2) {
            self.step(vec![Event::PointerMoved(p)], Modifiers::NONE);
            self.button(p, true, Modifiers::NONE);
            self.button(p, false, Modifiers::NONE);
            self.step(Vec::new(), Modifiers::NONE);
        }

        fn drag(&mut self, a: Pos2, b: Pos2, m: Modifiers) {
            self.step(vec![Event::PointerMoved(a)], m);
            self.button(a, true, m);
            for k in 1..=10 {
                self.step(vec![Event::PointerMoved(a + (b - a) * (k as f32 / 10.0))], m);
            }
            self.button(b, false, m);
            self.step(Vec::new(), m);
        }

        /// Lets the double-click window pass.
        fn wait(&mut self) {
            for _ in 0..10 {
                self.step(Vec::new(), Modifiers::NONE);
            }
        }
    }

    const SRC: &str = "flowchart TD\n  A[Um] --> B[Dois]\n  C[Tres]\n";

    #[test]
    fn click_selects_and_the_link_tool_connects() {
        let mut h = Harness::new(SRC);
        let c = h.node("C");
        h.click(c);
        assert_eq!(h.app.sel, Sel::Node("C".into()));
        h.wait();
        h.app.tool = Tool::Link;
        let (a, c) = (h.node("A"), h.node("C"));
        h.click(a);
        h.click(c);
        assert!(h.app.text.contains("  A --> C\n"), "{}", h.app.text);
    }

    #[test]
    fn shift_drag_links_to_a_node_or_to_a_new_one() {
        // The move check used to take the drag, so a link drag was always lost on release.
        let mut h = Harness::new(SRC);
        let (a, c) = (h.node("A"), h.node("C"));
        h.drag(a, c, Modifiers::SHIFT);
        assert!(h.app.text.contains("  A --> C\n"), "{}", h.app.text);
        let mut h = Harness::new(SRC);
        let a = h.node("A");
        h.drag(a, a + egui::vec2(-250.0, 200.0), Modifiers::SHIFT);
        assert!(h.app.text.ends_with("  D[Novo nó]\n  A --> D\n") || h.app.text.ends_with("  D[New node]\n  A --> D\n"), "{}", h.app.text);
    }

    #[test]
    fn dragging_a_node_keeps_its_place() {
        let mut h = Harness::new(SRC);
        let a = h.node("A");
        h.drag(a, a + egui::vec2(60.0, 0.0), Modifiers::NONE);
        assert!(h.app.text.contains("%% mermaidcraft:layout A="), "{}", h.app.text);
    }

    #[test]
    fn double_click_opens_the_label_even_beside_the_panel() {
        // C sits by the style panel: the view must not slide between the two clicks.
        let mut h = Harness::new(SRC);
        let c = h.node("C");
        h.click(c);
        h.click(c);
        assert!(App::label_focused(&h.ctx), "the label box should have the keyboard");
        h.step(vec![Event::Text("X".into())], Modifiers::NONE);
        let away = h.node("B") + egui::vec2(-300.0, 150.0);
        h.click(away);
        assert!(h.app.text.contains("  C[X]\n"), "{}", h.app.text);
    }

    #[test]
    fn a_new_node_takes_the_typed_label() {
        let mut h = Harness::new(SRC);
        let t = h.t;
        h.app.add_node(t);
        h.step(Vec::new(), Modifiers::NONE);
        h.step(Vec::new(), Modifiers::NONE);
        h.step(vec![Event::Text("Pagar".into())], Modifiers::NONE);
        let away = h.node("A") + egui::vec2(-250.0, 250.0);
        h.click(away);
        assert!(h.app.text.ends_with("  D[Pagar]\n"), "{}", h.app.text);
    }
}
