//! MermaidCraft engine: parser, model, layout, rendering and source edits. It has no dependency
//! on the user interface; the app, the CLI and the MCP server all use it.

pub mod edit;
pub mod er;
pub mod er_render;
pub mod export;
pub mod flow_render;
pub mod flowchart;
pub mod geom;
pub mod layout;
pub mod markdown;
pub mod model;
pub mod scene;
pub mod seq_render;
pub mod sketch;
pub mod sequence;
pub mod text;
pub mod util;

pub use model::*;
pub use scene::Scene;

/// The outcome of reading a diagram: whatever could be understood plus every problem found.
#[derive(Clone, Debug)]
pub struct Parsed {
    pub diagram: Option<Diagram>,
    pub diagnostics: Vec<Diagnostic>,
}

impl Parsed {
    pub fn errors(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics.iter().filter(|d| d.severity == Severity::Error)
    }
    pub fn has_errors(&self) -> bool {
        self.errors().next().is_some()
    }
}

const UNSUPPORTED: &[&str] = &[
    "classDiagram",
    "classDiagram-v2",
    "stateDiagram",
    "stateDiagram-v2",
    "gantt",
    "pie",
    "journey",
    "gitGraph",
    "mindmap",
    "timeline",
    "quadrantChart",
    "requirementDiagram",
    "C4Context",
    "sankey-beta",
    "xychart-beta",
    "block-beta",
    "architecture-beta",
    "packet-beta",
    "kanban",
];

/// Reads Mermaid source. Never panics; hostile input produces diagnostics.
pub fn parse(src: &str) -> Parsed {
    use util::*;
    if src.len() > MAX_SOURCE_BYTES {
        return Parsed { diagram: None, diagnostics: vec![Diagnostic::error(1, 1, format!("The diagram is too large (limit {} KB)", MAX_SOURCE_BYTES / 1000))] };
    }
    let lines = split_lines(src);
    let Some(first) = first_significant(&lines) else {
        return Parsed { diagram: None, diagnostics: vec![Diagnostic::error(1, 1, "The diagram is empty: start with 'flowchart TD', 'sequenceDiagram' or 'erDiagram'")] };
    };
    let head_line = lines.get(first).copied().unwrap_or(Line { no: 1, start: 0, text: "" });
    let head = head_line.text.trim();
    let word = head.split(|c: char| c.is_whitespace() || c == ';').next().unwrap_or("");
    let mut diags = Vec::new();
    let diagram = match word {
        "graph" | "flowchart" | "flowchart-elk" => Some(Diagram::Flowchart(flowchart::parse(&lines, first, &mut diags))),
        "sequenceDiagram" => Some(Diagram::Sequence(sequence::parse(&lines, first, &mut diags))),
        "erDiagram" => Some(Diagram::Er(er::parse(&lines, first, &mut diags))),
        w if UNSUPPORTED.contains(&w) => {
            diags.push(Diagnostic::error(head_line.no, 1, format!("'{w}' diagrams are not supported yet; supported types: flowchart, sequenceDiagram, erDiagram")));
            None
        }
        w => {
            let shown: String = w.chars().take(30).collect();
            diags.push(Diagnostic::error(head_line.no, 1, format!("Unknown diagram type '{shown}': start with 'flowchart TD', 'sequenceDiagram' or 'erDiagram'")));
            None
        }
    };
    if lines.len() >= MAX_LINES {
        diags.push(Diagnostic::warning(MAX_LINES, 1, format!("Only the first {MAX_LINES} lines were read")));
    }
    diags.sort_by_key(|d| (d.line, d.column));
    Parsed { diagram, diagnostics: diags }
}

/// How a diagram is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderOptions {
    /// Hand-drawn strokes.
    pub sketch: bool,
    /// Pastel colours that follow the flow (one colour per step of the diagram).
    pub colorful: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self { sketch: true, colorful: true }
    }
}

impl RenderOptions {
    /// Plain look: thin straight lines and the classic Mermaid colours.
    pub fn clean() -> Self {
        Self { sketch: false, colorful: false }
    }
}

/// Draws a diagram into a scene with the default (hand-drawn, colourful) look.
pub fn render_scene(d: &Diagram) -> Scene {
    render_scene_with(d, &RenderOptions::default())
}

pub fn render_scene_with(d: &Diagram, opts: &RenderOptions) -> Scene {
    let mut scene = match d {
        Diagram::Flowchart(f) => flow_render::render(f, opts),
        Diagram::Sequence(s) => seq_render::render(s, opts),
        Diagram::Er(e) => er_render::render(e, opts),
    };
    if opts.sketch {
        sketch::roughen(&mut scene);
        scene.finish(24.0);
    }
    scene
}

/// Reads and draws in one step. The error is the list of problems when nothing can be drawn.
pub fn render_source(src: &str) -> Result<(Scene, Vec<Diagnostic>), Vec<Diagnostic>> {
    render_source_with(src, &RenderOptions::default())
}

pub fn render_source_with(src: &str, opts: &RenderOptions) -> Result<(Scene, Vec<Diagnostic>), Vec<Diagnostic>> {
    let p = parse(src);
    match p.diagram {
        Some(d) => Ok((render_scene_with(&d, opts), p.diagnostics)),
        None => Err(p.diagnostics),
    }
}

/// Summary numbers of a diagram.
pub struct Stats {
    pub kind: &'static str,
    pub nodes: usize,
    pub edges: usize,
}

pub fn stats(d: &Diagram) -> Stats {
    match d {
        Diagram::Flowchart(f) => Stats { kind: "flowchart", nodes: f.nodes.len(), edges: f.edges.len() },
        Diagram::Sequence(s) => Stats {
            kind: "sequenceDiagram",
            nodes: s.participants.len(),
            edges: s.items.iter().filter(|i| matches!(i, SeqItem::Message { .. })).count(),
        },
        Diagram::Er(e) => Stats { kind: "erDiagram", nodes: e.entities.len(), edges: e.relations.len() },
    }
}
