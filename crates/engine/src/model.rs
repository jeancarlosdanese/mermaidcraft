//! The diagram model shared by the parsers, the layout, the renderer and the editor.

use std::collections::BTreeMap;

/// A byte range in the source text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end: end.max(start) }
    }
    pub fn len(&self) -> usize {
        self.end - self.start
    }
    pub fn is_empty(&self) -> bool {
        self.end == self.start
    }
}

/// How serious a diagnostic is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

/// A message about one line of the source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    /// 1-based line number.
    pub line: usize,
    /// 1-based column (characters), 0 when unknown.
    pub column: usize,
    pub message: String,
    pub severity: Severity,
}

impl Diagnostic {
    pub fn error(line: usize, column: usize, message: impl Into<String>) -> Self {
        Self { line, column, message: message.into(), severity: Severity::Error }
    }
    pub fn warning(line: usize, column: usize, message: impl Into<String>) -> Self {
        Self { line, column, message: message.into(), severity: Severity::Warning }
    }
}

// ---------------------------------------------------------------------------------------------
// Flowchart
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Direction {
    #[default]
    TopDown,
    BottomUp,
    LeftRight,
    RightLeft,
}

impl Direction {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "TB" | "TD" => Some(Self::TopDown),
            "BT" => Some(Self::BottomUp),
            "LR" => Some(Self::LeftRight),
            "RL" => Some(Self::RightLeft),
            _ => None,
        }
    }
    pub fn code(self) -> &'static str {
        match self {
            Self::TopDown => "TD",
            Self::BottomUp => "BT",
            Self::LeftRight => "LR",
            Self::RightLeft => "RL",
        }
    }
    pub fn all() -> [Direction; 4] {
        [Self::TopDown, Self::BottomUp, Self::LeftRight, Self::RightLeft]
    }
    pub fn horizontal(self) -> bool {
        matches!(self, Self::LeftRight | Self::RightLeft)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum NodeShape {
    #[default]
    Rect,
    Round,
    Stadium,
    Subroutine,
    Cylinder,
    Circle,
    DoubleCircle,
    Diamond,
    Hexagon,
    Parallelogram,
    ParallelogramAlt,
    Trapezoid,
    TrapezoidAlt,
    Asymmetric,
}

impl NodeShape {
    pub fn all() -> [NodeShape; 14] {
        [
            Self::Rect,
            Self::Round,
            Self::Stadium,
            Self::Subroutine,
            Self::Cylinder,
            Self::Circle,
            Self::DoubleCircle,
            Self::Diamond,
            Self::Hexagon,
            Self::Parallelogram,
            Self::ParallelogramAlt,
            Self::Trapezoid,
            Self::TrapezoidAlt,
            Self::Asymmetric,
        ]
    }
    /// The opening and closing brackets in Mermaid syntax.
    pub fn brackets(self) -> (&'static str, &'static str) {
        match self {
            Self::Rect => ("[", "]"),
            Self::Round => ("(", ")"),
            Self::Stadium => ("([", "])"),
            Self::Subroutine => ("[[", "]]"),
            Self::Cylinder => ("[(", ")]"),
            Self::Circle => ("((", "))"),
            Self::DoubleCircle => ("(((", ")))"),
            Self::Diamond => ("{", "}"),
            Self::Hexagon => ("{{", "}}"),
            Self::Parallelogram => ("[/", "/]"),
            Self::ParallelogramAlt => ("[\\", "\\]"),
            Self::Trapezoid => ("[/", "\\]"),
            Self::TrapezoidAlt => ("[\\", "/]"),
            Self::Asymmetric => (">", "]"),
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Rect => "rect",
            Self::Round => "round",
            Self::Stadium => "stadium",
            Self::Subroutine => "subroutine",
            Self::Cylinder => "cylinder",
            Self::Circle => "circle",
            Self::DoubleCircle => "double-circle",
            Self::Diamond => "diamond",
            Self::Hexagon => "hexagon",
            Self::Parallelogram => "parallelogram",
            Self::ParallelogramAlt => "parallelogram-alt",
            Self::Trapezoid => "trapezoid",
            Self::TrapezoidAlt => "trapezoid-alt",
            Self::Asymmetric => "asymmetric",
        }
    }
    pub fn from_name(s: &str) -> Option<Self> {
        Self::all().into_iter().find(|x| x.name() == s)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LineStyle {
    #[default]
    Solid,
    Dotted,
    Thick,
    Invisible,
}

impl LineStyle {
    pub fn name(self) -> &'static str {
        match self {
            Self::Solid => "solid",
            Self::Dotted => "dotted",
            Self::Thick => "thick",
            Self::Invisible => "invisible",
        }
    }
    pub fn from_name(s: &str) -> Option<Self> {
        [Self::Solid, Self::Dotted, Self::Thick, Self::Invisible].into_iter().find(|x| x.name() == s)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Head {
    #[default]
    None,
    Arrow,
    Circle,
    Cross,
}

impl Head {
    pub fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Arrow => "arrow",
            Self::Circle => "circle",
            Self::Cross => "cross",
        }
    }
    pub fn from_name(s: &str) -> Option<Self> {
        [Self::None, Self::Arrow, Self::Circle, Self::Cross].into_iter().find(|x| x.name() == s)
    }
}

/// The kind of a connection: line style, both ends and minimum length.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkKind {
    pub style: LineStyle,
    pub head_start: Head,
    pub head_end: Head,
    pub min_len: usize,
}

impl Default for LinkKind {
    fn default() -> Self {
        Self { style: LineStyle::Solid, head_start: Head::None, head_end: Head::Arrow, min_len: 1 }
    }
}

impl LinkKind {
    /// The canonical Mermaid token for this kind (without a label).
    pub fn token(&self) -> String {
        let len = self.min_len.clamp(1, 8);
        let h = |h: Head, left: bool| -> &'static str {
            match (h, left) {
                (Head::None, _) => "",
                (Head::Arrow, true) => "<",
                (Head::Arrow, false) => ">",
                (Head::Circle, _) => "o",
                (Head::Cross, _) => "x",
            }
        };
        match self.style {
            LineStyle::Invisible => "~".repeat(len + 2),
            LineStyle::Solid | LineStyle::Thick => {
                let c = if self.style == LineStyle::Solid { "-" } else { "=" };
                let open = self.head_end == Head::None;
                let count = if open { len + 2 } else { len + 1 };
                format!("{}{}{}", h(self.head_start, true), c.repeat(count), h(self.head_end, false))
            }
            LineStyle::Dotted => format!("{}-{}-{}", h(self.head_start, true), ".".repeat(len), h(self.head_end, false)),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FlowNode {
    pub id: String,
    pub label: String,
    pub shape: NodeShape,
    pub line: usize,
    pub classes: Vec<String>,
    pub parent: Option<usize>,
    /// Span of the first mention of the id.
    pub id_span: Span,
    /// Span of the shape and label (`[text]`) in the first mention that has one.
    pub def_span: Option<Span>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FlowEdge {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
    pub kind: LinkKind,
    pub line: usize,
    /// Statement number and link number inside the statement (for edits).
    pub stmt: usize,
    pub link: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Subgraph {
    pub id: String,
    pub title: String,
    pub parent: Option<usize>,
    pub direction: Option<Direction>,
    pub line: usize,
    /// Text range of the `subgraph ...` statement.
    pub header: Span,
    /// Text range of the matching `end` (None while the `end` is missing).
    pub end: Option<Span>,
}

/// One id inside a statement, with its text range.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeRef {
    pub id: String,
    pub span: Span,
    /// True when the text carries a shape/label of its own.
    pub has_def: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub span: Span,
    pub nodes: Vec<NodeRef>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Link {
    /// Token plus label.
    pub span: Span,
    pub kind: LinkKind,
    pub label: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum StmtKind {
    Chain { groups: Vec<Group>, links: Vec<Link> },
    /// Keyword statements: subgraph, end, direction, style, class, click...
    Keyword { word: String, ids: Vec<NodeRef> },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Stmt {
    pub span: Span,
    pub line: usize,
    pub kind: StmtKind,
}

/// Style properties from `style` and `classDef`.
pub type StyleMap = BTreeMap<String, String>;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Flowchart {
    pub direction: Direction,
    /// Span of the direction word in the header (or an empty span after the keyword).
    pub direction_span: Span,
    pub header_end: usize,
    pub nodes: Vec<FlowNode>,
    pub edges: Vec<FlowEdge>,
    pub subgraphs: Vec<Subgraph>,
    pub statements: Vec<Stmt>,
    pub class_defs: BTreeMap<String, StyleMap>,
    pub node_styles: BTreeMap<String, StyleMap>,
    /// Manual node offsets from the `%% mermaidcraft:layout` comment (moved nodes).
    pub offsets: BTreeMap<String, (f32, f32)>,
}

impl Flowchart {
    pub fn node_index(&self, id: &str) -> Option<usize> {
        self.nodes.iter().position(|n| n.id == id)
    }
    pub fn subgraph_index(&self, id: &str) -> Option<usize> {
        self.subgraphs.iter().position(|s| s.id == id)
    }
}

// ---------------------------------------------------------------------------------------------
// Sequence diagram
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct Participant {
    pub id: String,
    pub label: String,
    pub actor: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeqHead {
    /// Filled arrow (`->>`).
    Filled,
    /// Open head (`->`): a line without a head.
    None,
    Cross,
    Async,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SeqArrow {
    pub dotted: bool,
    pub head: SeqHead,
    /// Bidirectional (`<<->>`).
    pub both: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotePlace {
    Over,
    LeftOf,
    RightOf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockKind {
    Loop,
    Alt,
    Opt,
    Par,
    Critical,
    Break,
    Rect,
    Box,
}

impl BlockKind {
    pub fn word(self) -> &'static str {
        match self {
            Self::Loop => "loop",
            Self::Alt => "alt",
            Self::Opt => "opt",
            Self::Par => "par",
            Self::Critical => "critical",
            Self::Break => "break",
            Self::Rect => "rect",
            Self::Box => "box",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum SeqItem {
    Message { from: usize, to: usize, text: String, arrow: SeqArrow, activate: bool, deactivate: bool, line: usize },
    Note { place: NotePlace, targets: Vec<usize>, text: String, line: usize },
    Activate { who: usize },
    Deactivate { who: usize },
    BlockStart { kind: BlockKind, label: String, line: usize },
    BlockElse { label: String, line: usize },
    BlockEnd,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sequence {
    pub participants: Vec<Participant>,
    pub items: Vec<SeqItem>,
    pub autonumber: bool,
}

// ---------------------------------------------------------------------------------------------
// ER diagram
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cardinality {
    ZeroOrOne,
    ExactlyOne,
    ZeroOrMore,
    OneOrMore,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ErAttribute {
    pub ty: String,
    pub name: String,
    pub keys: Vec<String>,
    pub comment: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ErEntity {
    pub id: String,
    pub label: String,
    pub attributes: Vec<ErAttribute>,
    pub line: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ErRelation {
    pub a: usize,
    pub b: usize,
    pub card_a: Cardinality,
    pub card_b: Cardinality,
    pub identifying: bool,
    pub label: String,
    pub line: usize,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ErDiagram {
    pub direction: Direction,
    pub entities: Vec<ErEntity>,
    pub relations: Vec<ErRelation>,
}

// ---------------------------------------------------------------------------------------------
// Top level
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub enum Diagram {
    Flowchart(Flowchart),
    Sequence(Sequence),
    Er(ErDiagram),
}

impl Diagram {
    pub fn kind_name(&self) -> &'static str {
        match self {
            Self::Flowchart(_) => "flowchart",
            Self::Sequence(_) => "sequenceDiagram",
            Self::Er(_) => "erDiagram",
        }
    }
}
