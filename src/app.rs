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
    /// Moving a node by hand: its id and the offset it has now.
    Move { id: String, offset: (f32, f32) },
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
    sketch: bool,
    colorful: bool,
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
    label_for: Option<Sel>,
    focus_label: bool,
    prompt: Option<After>,
    allow_close: bool,
    show_about: bool,
    events: OpenEvents,
    shot: Option<Shot>,
    frames: u32,
    title: String,
    untitled_kind: &'static str,
}

fn read_text(path: &Path) -> Result<String, String> {
    let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if meta.len() > MAX_FILE {
        return Err("The file is too large.".into());
    }
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    String::from_utf8(bytes).map_err(|_| "not valid UTF-8 text".to_string())
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
    fonts.families.insert(paint::bold_family(), bold);
    ctx.set_fonts(fonts);
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, opts: Options) -> Self {
        setup_fonts(&cc.egui_ctx);
        let shot_mode = opts.screenshot.is_some();
        let settings = if shot_mode { Settings { lang: crate::i18n::detect(), dark: false, sketch: true, colorful: true } } else { Settings::load() };
        opts.events.set_wake(&cc.egui_ctx);
        let mut app = App {
            lang: settings.lang,
            dark: settings.dark,
            sketch: settings.sketch,
            colorful: settings.colorful,
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
            label_for: None,
            focus_label: false,
            prompt: None,
            allow_close: false,
            show_about: false,
            events: opts.events,
            shot: opts.screenshot.map(|path| Shot { path, requested: false }),
            frames: 0,
            title: String::new(),
            untitled_kind: "flowchart",
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
        app
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
        engine::RenderOptions { sketch: self.sketch, colorful: self.colorful }
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

    fn undo(&mut self) {
        if let Some(prev) = self.history.pop() {
            self.future.push(std::mem::replace(&mut self.text, prev));
            self.dirty = true;
            self.reparse();
        }
    }

    fn redo(&mut self) {
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
                    self.undo();
                }
                if ui.add_enabled(!self.future.is_empty(), egui::Button::new(self.tr("Redo")).shortcut_text("⇧⌘Z")).clicked() {
                    ui.close();
                    self.redo();
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
                let sketch_label = self.tr("Hand-drawn look");
                let color_label = self.tr("Colors follow the flow");
                let a = ui.checkbox(&mut self.sketch, sketch_label).changed();
                let b = ui.checkbox(&mut self.colorful, color_label).changed();
                if a || b {
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
            Settings { lang: self.lang, dark: self.dark, sketch: self.sketch, colorful: self.colorful }.save();
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
                self.redo();
            } else if pressed(ctx, Modifiers::COMMAND, Key::Z) {
                self.undo();
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
                let painter = ui.painter();
                let clip = ui.clip_rect();
                let dim = if dark { Color32::from_gray(120) } else { Color32::from_gray(150) };
                for (i, row) in out.galley.rows.iter().enumerate() {
                    let rect = row.rect().translate(out.galley_pos.to_vec2());
                    if rect.max.y < clip.min.y || rect.min.y > clip.max.y {
                        continue;
                    }
                    let line = i + 1;
                    let err = err_lines.iter().find(|(l, _)| *l == line);
                    let wide = egui::Rect::from_min_max(egui::pos2(gutter.min.x, rect.min.y), egui::pos2(out.response.rect.max.x.max(clip.max.x), rect.max.y));
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
                self.undo();
            }
            if ui.add_enabled(!self.future.is_empty(), egui::Button::new(self.tr("Redo"))).clicked() {
                self.redo();
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

    fn inspector(&mut self, ui: &mut egui::Ui, now: f64) {
        let Some(flow) = self.flow().cloned() else {
            ui.weak(self.tr("Visual editing works on flowcharts; edit the text for this diagram type."));
            return;
        };
        let sel = self.sel.clone();
        // Keep the label box in step with the selection unless it is being edited.
        let current_label = match &sel {
            Sel::Node(id) => flow.node_index(id).and_then(|i| flow.nodes.get(i)).map(|n| n.label.clone()),
            Sel::Edge(i) => flow.edges.get(*i).map(|e| e.label.clone().unwrap_or_default()),
            Sel::None => None,
        };
        if self.label_for.as_ref() != Some(&sel) {
            self.label_buf = current_label.clone().unwrap_or_default();
            self.label_for = Some(sel.clone());
        }
        match &sel {
            Sel::None => {
                ui.weak(if self.link_from.is_some() { self.tr("Click the node to connect to") } else { self.tr("Click a node or an edge to edit it. + Node adds a node; Link (or dragging from one node to another) connects them.") });
            }
            Sel::Node(id) => {
                let Some(node) = flow.node_index(id).and_then(|i| flow.nodes.get(i)).cloned() else { return };
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(format!("{} {}", self.tr("Node"), node.id)).strong());
                    ui.label(self.tr("Label"));
                    let id_label = egui::Id::new("label_field");
                    let resp = ui.add(egui::TextEdit::singleline(&mut self.label_buf).id(id_label).desired_width(180.0));
                    if self.focus_label {
                        self.focus_label = false;
                        resp.request_focus();
                        if let Some(mut st) = egui::TextEdit::load_state(ui.ctx(), id_label) {
                            let n = self.label_buf.chars().count();
                            st.cursor.set_char_range(Some(egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(n))));
                            st.store(ui.ctx(), id_label);
                        }
                    }
                    if resp.lost_focus() && self.label_buf != node.label {
                        let r = edit::set_node(&self.text, &node.id, Some(&self.label_buf), None);
                        self.visual(r, now);
                    }
                    ui.label(self.tr("Shape"));
                    let mut shape = node.shape;
                    egui::ComboBox::from_id_salt("shape").selected_text(self.tr(shape.name())).show_ui(ui, |ui| {
                        for s in NodeShape::all() {
                            ui.selectable_value(&mut shape, s, self.tr(s.name()));
                        }
                    });
                    if shape != node.shape {
                        let r = edit::set_node(&self.text, &node.id, None, Some(shape));
                        self.visual(r, now);
                    }
                    if ui.button(self.tr("Delete")).clicked() {
                        self.delete_selected(now);
                    }
                });
            }
            Sel::Edge(i) => {
                let Some(e) = flow.edges.get(*i).cloned() else { return };
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(format!("{} {} → {}", self.tr("Edge"), e.from, e.to)).strong());
                    ui.label(self.tr("Label"));
                    let resp = ui.add(egui::TextEdit::singleline(&mut self.label_buf).id(egui::Id::new("label_field")).desired_width(160.0));
                    if resp.lost_focus() && self.label_buf != e.label.clone().unwrap_or_default() {
                        let text = self.label_buf.clone();
                        let r = edit::set_edge(&self.text, *i, Some(Some(text.as_str())), None);
                        self.visual(r, now);
                    }
                    ui.label(self.tr("Line"));
                    let mut style = e.kind.style;
                    egui::ComboBox::from_id_salt("edge_style").selected_text(self.tr(style.name())).show_ui(ui, |ui| {
                        for s in [LineStyle::Solid, LineStyle::Dotted, LineStyle::Thick, LineStyle::Invisible] {
                            ui.selectable_value(&mut style, s, self.tr(s.name()));
                        }
                    });
                    ui.label(self.tr("Arrow"));
                    let mut head = e.kind.head_end;
                    egui::ComboBox::from_id_salt("edge_head").selected_text(self.tr(head.name())).show_ui(ui, |ui| {
                        for h in [Head::Arrow, Head::None, Head::Circle, Head::Cross] {
                            ui.selectable_value(&mut head, h, self.tr(h.name()));
                        }
                    });
                    if style != e.kind.style || head != e.kind.head_end {
                        let kind = LinkKind { style, head_end: head, ..e.kind };
                        let r = edit::set_edge(&self.text, *i, None, Some(kind));
                        self.visual(r, now);
                    }
                    if ui.button(self.tr("Delete")).clicked() {
                        self.delete_selected(now);
                    }
                });
            }
        }
    }

    fn canvas(&mut self, ui: &mut egui::Ui, now: f64) {
        let size = ui.available_size();
        let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        let bg = if self.dark { Color32::from_gray(38) } else { Color32::from_gray(244) };
        painter.rect_filled(rect, 0.0, bg);
        let Some(scene) = self.scene.clone() else {
            let msg = self.parsed.diagnostics.first().map_or(self.tr("Nothing to draw yet.").to_string(), |d| format!("{} {}: {}", self.tr("Line"), d.line, d.message));
            painter.text(rect.center(), egui::Align2::CENTER_CENTER, msg, egui::FontId::proportional(15.0), Color32::from_rgb(200, 80, 80));
            return;
        };
        let (sw, sh) = (scene.width.max(1.0), scene.height.max(1.0));
        if self.auto_fit {
            let z = ((rect.width() - 40.0) / sw).min((rect.height() - 40.0) / sh).clamp(0.1, 1.5);
            self.zoom = z;
            self.pan = Vec2::new((rect.width() - sw * z) / 2.0, (rect.height() - sh * z) / 2.0);
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
        let view = View { origin: rect.min + self.pan, zoom: self.zoom };
        paint::draw(&painter, &scene, &view);
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
                            Drag::Move { id: id.clone(), offset }
                        }
                        None => Drag::Pan,
                    },
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
                Some(Drag::Move { id, offset }) => {
                    let d = resp.drag_delta() / view.zoom;
                    let (id, new) = (id.clone(), (offset.0 + d.x, offset.1 + d.y));
                    self.drag = Some(Drag::Move { id: id.clone(), offset: new });
                    self.redraw_with_offset(&id, new);
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                }
                None => {}
            }
        }
        if resp.drag_stopped() {
            if let Some(Drag::Move { id, offset }) = self.drag.take() {
                self.commit_offset(&id, offset, now);
                let m = self.tr("Moved a node").to_string();
                self.flash(m, now);
                return;
            }
            if let (Some(Drag::Link { from }), Some(p)) = (self.drag.take(), ui.input(|i| i.pointer.interact_pos())) {
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
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(ui, |ui| {
            egui::Panel::top("toolbar").show(ui, |ui| self.toolbar(ui, now));
            egui::Panel::top("inspector").show(ui, |ui| self.inspector(ui, now));
            egui::CentralPanel::default().frame(egui::Frame::NONE).show(ui, |ui| self.canvas(ui, now));
        });
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
