//! Flowchart parser (`graph` / `flowchart`), written from the public Mermaid syntax documentation.

use std::collections::HashMap;

use crate::model::*;
use crate::util::*;

/// A cursor over one statement.
struct Cur<'a> {
    s: &'a str,
    pos: usize,
    /// Absolute byte offset of `s` in the whole source.
    base: usize,
}

impl<'a> Cur<'a> {
    fn rest(&self) -> &'a str {
        self.s.get(self.pos..).unwrap_or("")
    }
    fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }
    fn peek_at(&self, n: usize) -> Option<char> {
        self.rest().chars().nth(n)
    }
    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += c.len_utf8();
        Some(c)
    }
    fn skip_ws(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.bump();
        }
    }
    fn abs(&self) -> usize {
        self.base + self.pos
    }
    fn at_end(&self) -> bool {
        self.pos >= self.s.len()
    }
    fn starts_with(&self, p: &str) -> bool {
        self.rest().starts_with(p)
    }
    fn col(&self) -> usize {
        self.s.get(..self.pos).map_or(0, |p| p.chars().count()) + 1
    }
}

struct ParsedNode {
    id: String,
    shape: Option<(NodeShape, String, Span)>,
    classes: Vec<String>,
    span: Span,
    id_span: Span,
}

type PResult<T> = Result<T, (usize, String)>;

fn err<T>(cur: &Cur<'_>, msg: impl Into<String>) -> PResult<T> {
    Err((cur.col(), msg.into()))
}

fn read_id(cur: &mut Cur<'_>) -> String {
    let mut id = String::new();
    while let Some(c) = cur.peek() {
        if is_id_char(c) {
            id.push(c);
            cur.bump();
        } else if c == '-' {
            // A dash belongs to the id only when an id character follows (`my-node`), never for `-->`.
            let next = cur.peek_at(1);
            if next.is_some_and(is_id_char) && !id.is_empty() {
                id.push(c);
                cur.bump();
            } else {
                break;
            }
        } else {
            break;
        }
        if id.chars().count() > 200 {
            break;
        }
    }
    id
}

const OPENERS: &[(&str, &[&str])] = &[
    ("(((", &[")))"]),
    ("((", &["))"]),
    ("([", &["])"]),
    ("(", &[")"]),
    ("[[", &["]]"]),
    ("[(", &[")]"]),
    ("[/", &["/]", "\\]"]),
    ("[\\", &["\\]", "/]"]),
    ("[", &["]"]),
    ("{{", &["}}"]),
    ("{", &["}"]),
    (">", &["]"]),
];

fn shape_for(open: &str, close: &str) -> NodeShape {
    match (open, close) {
        ("(((", _) => NodeShape::DoubleCircle,
        ("((", _) => NodeShape::Circle,
        ("([", _) => NodeShape::Stadium,
        ("(", _) => NodeShape::Round,
        ("[[", _) => NodeShape::Subroutine,
        ("[(", _) => NodeShape::Cylinder,
        ("[/", "/]") => NodeShape::Parallelogram,
        ("[/", _) => NodeShape::Trapezoid,
        ("[\\", "\\]") => NodeShape::ParallelogramAlt,
        ("[\\", _) => NodeShape::TrapezoidAlt,
        ("[", _) => NodeShape::Rect,
        ("{{", _) => NodeShape::Hexagon,
        ("{", _) => NodeShape::Diamond,
        _ => NodeShape::Asymmetric,
    }
}

fn parse_shape(cur: &mut Cur<'_>) -> PResult<Option<(NodeShape, String, Span)>> {
    let start = cur.abs();
    let Some((open, closers)) = OPENERS.iter().find(|(o, _)| cur.starts_with(o)) else {
        return Ok(None);
    };
    for _ in 0..open.chars().count() {
        cur.bump();
    }
    let rest = cur.rest();
    let (raw, close): (String, &str) = if rest.trim_start().starts_with('"') {
        let lead = rest.len() - rest.trim_start().len();
        let after = rest.get(lead + 1..).unwrap_or("");
        let Some(q) = after.find('"') else {
            return err(cur, "Unterminated quoted text");
        };
        let text = after.get(..q).unwrap_or("").to_string();
        let tail = after.get(q + 1..).unwrap_or("");
        let tail_trim = tail.trim_start();
        let Some(c) = closers.iter().find(|c| tail_trim.starts_with(**c)) else {
            let want = closers.first().copied().unwrap_or("]");
            return err(cur, format!("Expected '{want}' after the quoted text"));
        };
        let consumed = lead + 1 + q + 1 + (tail.len() - tail_trim.len()) + c.len();
        cur.pos += consumed;
        (text, c)
    } else {
        let mut best: Option<(usize, &str)> = None;
        for c in closers.iter() {
            if let Some(i) = rest.find(*c) {
                if best.is_none_or(|(b, _)| i < b) {
                    best = Some((i, c));
                }
            }
        }
        let Some((i, c)) = best else {
            let want = closers.first().copied().unwrap_or("]");
            return err(cur, format!("Unclosed shape: expected '{want}'"));
        };
        let text = rest.get(..i).unwrap_or("").to_string();
        cur.pos += i + c.len();
        (text, c)
    };
    let shape = shape_for(open, close);
    Ok(Some((shape, clean_label(&raw), Span::new(start, cur.abs()))))
}

fn parse_node(cur: &mut Cur<'_>) -> PResult<ParsedNode> {
    let start = cur.abs();
    let id = read_id(cur);
    if id.is_empty() {
        let found = cur.peek().map_or("end of line".to_string(), |c| format!("'{c}'"));
        return err(cur, format!("Expected a node name but found {found}"));
    }
    let id_span = Span::new(start, cur.abs());
    let shape = parse_shape(cur)?;
    let mut classes = Vec::new();
    while cur.starts_with(":::") {
        cur.bump();
        cur.bump();
        cur.bump();
        let c = read_id(cur);
        if c.is_empty() {
            return err(cur, "Expected a class name after ':::'");
        }
        classes.push(c);
    }
    Ok(ParsedNode { id, shape, classes, span: Span::new(start, cur.abs()), id_span })
}

fn parse_group(cur: &mut Cur<'_>) -> PResult<(Vec<ParsedNode>, Span)> {
    let start = cur.abs();
    let mut nodes = vec![parse_node(cur)?];
    loop {
        let save = cur.pos;
        cur.skip_ws();
        if cur.peek() == Some('&') {
            cur.bump();
            cur.skip_ws();
            nodes.push(parse_node(cur)?);
            if nodes.len() > 200 {
                return err(cur, "Too many nodes joined with '&'");
            }
        } else {
            cur.pos = save;
            break;
        }
    }
    Ok((nodes, Span::new(start, cur.abs())))
}

fn head_of(c: char) -> Head {
    match c {
        'x' => Head::Cross,
        'o' => Head::Circle,
        _ => Head::Arrow,
    }
}

fn run_of(cur: &mut Cur<'_>, ch: char) -> usize {
    let mut n = 0usize;
    while cur.peek() == Some(ch) {
        cur.bump();
        n += 1;
        if n > 64 {
            break;
        }
    }
    n
}

/// The right-hand head after the line: `>`, `x`, `o` (the last two only when not part of a name).
fn right_head(cur: &mut Cur<'_>) -> Head {
    match cur.peek() {
        Some('>') => {
            cur.bump();
            Head::Arrow
        }
        Some(c @ ('x' | 'o')) => {
            let next = cur.peek_at(1);
            if next.is_none_or(|n| !is_id_char(n)) {
                cur.bump();
                head_of(c)
            } else {
                Head::None
            }
        }
        _ => Head::None,
    }
}

fn parse_link(cur: &mut Cur<'_>) -> PResult<Option<Link>> {
    let start = cur.abs();
    let mut head_start = Head::None;
    match (cur.peek(), cur.peek_at(1)) {
        (Some('<'), Some('-' | '=')) => {
            cur.bump();
            head_start = Head::Arrow;
        }
        (Some(c @ ('x' | 'o')), Some('-' | '=')) => {
            cur.bump();
            head_start = head_of(c);
        }
        _ => {}
    }
    let Some(first) = cur.peek() else {
        return Ok(None);
    };
    let mut label: Option<String> = None;
    let kind;
    match first {
        '~' => {
            let n = run_of(cur, '~');
            if n < 3 {
                return err(cur, "An invisible link needs at least three '~'");
            }
            kind = LinkKind { style: LineStyle::Invisible, head_start: Head::None, head_end: Head::None, min_len: n - 2 };
        }
        '-' if cur.peek_at(1) == Some('.') => {
            cur.bump();
            let mut k = run_of(cur, '.');
            if cur.peek().is_some_and(char::is_whitespace) {
                // `-. text .->`
                let rest = cur.rest();
                let Some(idx) = rest.find(".-") else {
                    return err(cur, "Link text is not closed: expected '.->'");
                };
                label = Some(clean_label(rest.get(..idx).unwrap_or("")));
                cur.pos += idx;
                k = run_of(cur, '.');
            }
            if cur.peek() != Some('-') {
                return err(cur, "A dotted link must end with '-' (for example '-.->')");
            }
            cur.bump();
            let head = right_head(cur);
            kind = LinkKind { style: LineStyle::Dotted, head_start, head_end: head, min_len: k.max(1) };
        }
        c @ ('-' | '=') => {
            let style = if c == '-' { LineStyle::Solid } else { LineStyle::Thick };
            let d = run_of(cur, c);
            if d == 1 {
                return err(cur, format!("Invalid link: use '{c}{c}>' or '{c}{c}{c}'"));
            }
            let mut dashes = d;
            if d == 2 && cur.peek().is_some_and(char::is_whitespace) {
                // `-- text -->`
                let rest = cur.rest();
                let pat: String = std::iter::repeat_n(c, 2).collect();
                let Some(idx) = rest.find(&pat) else {
                    return err(cur, format!("Link text is not closed: expected '{c}{c}>'"));
                };
                label = Some(clean_label(rest.get(..idx).unwrap_or("")));
                cur.pos += idx;
                dashes = run_of(cur, c);
            }
            let head = right_head(cur);
            let min_len = if head == Head::None { dashes.saturating_sub(2).max(1) } else { dashes.saturating_sub(1).max(1) };
            if head == Head::None && dashes < 3 && label.is_none() {
                return err(cur, format!("Invalid link: use '{c}{c}>' or '{c}{c}{c}'"));
            }
            kind = LinkKind { style, head_start, head_end: head, min_len };
        }
        _ => return Ok(None),
    }
    // Optional `|text|` after the link.
    let save = cur.pos;
    cur.skip_ws();
    if cur.peek() == Some('|') {
        cur.bump();
        let rest = cur.rest();
        let Some(idx) = rest.find('|') else {
            return err(cur, "Link label is not closed: expected '|'");
        };
        label = Some(clean_label(rest.get(..idx).unwrap_or("")));
        cur.pos += idx + 1;
    } else {
        cur.pos = save;
    }
    Ok(Some(Link { span: Span::new(start, cur.abs()), kind, label }))
}

struct Builder {
    fc: Flowchart,
    index: HashMap<String, usize>,
    stack: Vec<usize>,
    diags: Vec<Diagnostic>,
}

impl Builder {
    fn mention(&mut self, p: &ParsedNode, line: usize) -> NodeRef {
        // An id that names a subgraph refers to the subgraph, not to a node.
        if self.fc.subgraph_index(&p.id).is_some() && p.shape.is_none() {
            return NodeRef { id: p.id.clone(), span: p.span, has_def: false };
        }
        let idx = match self.index.get(&p.id) {
            Some(i) => *i,
            None => {
                if self.fc.nodes.len() >= MAX_NODES {
                    self.diags.push(Diagnostic::error(line, 0, format!("Too many nodes (limit {MAX_NODES})")));
                    return NodeRef { id: p.id.clone(), span: p.span, has_def: false };
                }
                let i = self.fc.nodes.len();
                self.fc.nodes.push(FlowNode {
                    id: p.id.clone(),
                    label: p.id.clone(),
                    shape: NodeShape::Rect,
                    line,
                    classes: Vec::new(),
                    parent: None,
                    id_span: p.id_span,
                    def_span: None,
                });
                self.index.insert(p.id.clone(), i);
                i
            }
        };
        let parent = self.stack.last().copied();
        if let Some(n) = self.fc.nodes.get_mut(idx) {
            if let Some((shape, label, span)) = &p.shape {
                n.shape = *shape;
                n.label = label.clone();
                n.def_span = Some(*span);
            }
            for c in &p.classes {
                if !n.classes.contains(c) {
                    n.classes.push(c.clone());
                }
            }
            if parent.is_some() {
                n.parent = parent;
            }
        }
        NodeRef { id: p.id.clone(), span: p.span, has_def: p.shape.is_some() }
    }
}

fn parse_chain(b: &mut Builder, text: &str, base: usize, line: usize, stmt_no: usize) -> PResult<StmtKind> {
    let mut cur = Cur { s: text, pos: 0, base };
    let mut groups: Vec<Group> = Vec::new();
    let mut links: Vec<Link> = Vec::new();
    let (nodes, span) = parse_group(&mut cur)?;
    let refs: Vec<NodeRef> = nodes.iter().map(|n| b.mention(n, line)).collect();
    groups.push(Group { span, nodes: refs });
    loop {
        cur.skip_ws();
        if cur.at_end() {
            break;
        }
        let Some(link) = parse_link(&mut cur)? else {
            let rest: String = cur.rest().chars().take(20).collect();
            return err(&cur, format!("Unexpected '{rest}': expected a link such as '-->' or the end of the line"));
        };
        cur.skip_ws();
        if cur.at_end() {
            return err(&cur, "A link needs a node on its right");
        }
        let (nodes, span) = parse_group(&mut cur)?;
        let refs: Vec<NodeRef> = nodes.iter().map(|n| b.mention(n, line)).collect();
        let link_no = links.len();
        if let Some(prev) = groups.last() {
            for a in &prev.nodes {
                for z in &refs {
                    if b.fc.edges.len() >= MAX_EDGES {
                        return err(&cur, format!("Too many links (limit {MAX_EDGES})"));
                    }
                    b.fc.edges.push(FlowEdge {
                        from: a.id.clone(),
                        to: z.id.clone(),
                        label: link.label.clone(),
                        kind: link.kind,
                        line,
                        stmt: stmt_no,
                        link: link_no,
                    });
                }
            }
        }
        links.push(link);
        groups.push(Group { span, nodes: refs });
    }
    Ok(StmtKind::Chain { groups, links })
}

fn first_word(s: &str) -> (&str, &str) {
    let end = s.find(char::is_whitespace).unwrap_or(s.len());
    (s.get(..end).unwrap_or(""), s.get(end..).unwrap_or("").trim_start())
}

/// Splits an id list (`A,B, C`) into references with spans.
fn id_list(text: &str, base: usize) -> Vec<NodeRef> {
    let mut out = Vec::new();
    let mut off = 0usize;
    for part in text.split(',') {
        let lead = part.len() - part.trim_start().len();
        let t = part.trim();
        if !t.is_empty() {
            let s = base + off + lead;
            out.push(NodeRef { id: t.to_string(), span: Span::new(s, s + t.len()), has_def: false });
        }
        off += part.len() + 1;
    }
    out
}

fn parse_subgraph_header(rest: &str) -> (String, String) {
    let rest = rest.trim();
    if let Some(b) = rest.find('[') {
        let id = rest.get(..b).unwrap_or("").trim();
        let inner = rest.get(b + 1..).unwrap_or("");
        let inner = inner.strip_suffix(']').unwrap_or(inner);
        let title = clean_label(inner);
        if !id.is_empty() && !id.contains(char::is_whitespace) {
            return (id.to_string(), title);
        }
    }
    let title = clean_label(rest);
    let id: String = rest.chars().filter(|c| is_id_char(*c) || *c == '-').collect();
    (if id.is_empty() { title.clone() } else { id }, title)
}

/// Prefix of the comment that stores moved node positions.
pub const LAYOUT_COMMENT: &str = "%% mermaidcraft:layout";

/// Reads ` A=12,-30 B=0,40`. Unknown or malformed entries are ignored.
pub fn parse_offsets(s: &str) -> std::collections::BTreeMap<String, (f32, f32)> {
    let mut m = std::collections::BTreeMap::new();
    for part in s.split_whitespace().take(MAX_NODES) {
        let Some((id, xy)) = part.split_once('=') else { continue };
        let Some((x, y)) = xy.split_once(',') else { continue };
        if let (Ok(x), Ok(y)) = (x.parse::<f32>(), y.parse::<f32>()) {
            if x.is_finite() && y.is_finite() && !id.is_empty() {
                m.insert(id.to_string(), (x.clamp(-20000.0, 20000.0), y.clamp(-20000.0, 20000.0)));
            }
        }
    }
    m
}

/// Parses a flowchart; `lines[first]` is the header line.
pub fn parse(lines: &[Line<'_>], first: usize, diags: &mut Vec<Diagnostic>) -> Flowchart {
    let mut b = Builder { fc: Flowchart::default(), index: HashMap::new(), stack: Vec::new(), diags: Vec::new() };
    let header = lines.get(first).copied().unwrap_or(Line { no: 1, start: 0, text: "" });
    // Header: `graph TD`, `flowchart LR;`
    {
        let code = strip_comment(header.text);
        let (lead, t) = trim_with_offset(code);
        let t_clean = t.trim_end_matches(';').trim_end();
        let (_kw, rest) = first_word(t_clean);
        let kw_len = t_clean.len() - rest.len();
        let kw_end = header.start + lead + t_clean.get(..kw_len).unwrap_or("").trim_end().len();
        b.fc.header_end = header.start + code.trim_end().trim_end_matches(';').trim_end().len().max(lead + kw_len);
        b.fc.direction_span = Span::new(kw_end, kw_end);
        if !rest.is_empty() {
            let dir_start = header.start + lead + kw_len;
            match Direction::parse(rest) {
                Some(d) => {
                    b.fc.direction = d;
                    b.fc.direction_span = Span::new(dir_start, dir_start + rest.len());
                }
                None => b.diags.push(Diagnostic::error(header.no, lead + kw_len + 1, format!("Unknown direction '{rest}': use TD, TB, BT, LR or RL"))),
            }
        }
    }
    let mut stmt_no = 0usize;
    for line in lines.iter().skip(first + 1) {
        if let Some(rest) = line.text.trim_start().strip_prefix(LAYOUT_COMMENT) {
            b.fc.offsets = parse_offsets(rest);
            continue;
        }
        let code = strip_comment(line.text);
        if code.trim().is_empty() {
            continue;
        }
        for (off, piece) in split_statements(code) {
            let (lead, text) = trim_with_offset(piece);
            if text.is_empty() {
                continue;
            }
            let base = line.start + off + lead;
            let span = Span::new(base, base + text.len());
            let col_base = off + lead;
            let (word, rest) = first_word(text);
            let rest_base = base + (text.len() - rest.len());
            let kind: Option<StmtKind> = match word {
                "subgraph" => {
                    if b.stack.len() >= MAX_DEPTH {
                        b.diags.push(Diagnostic::error(line.no, col_base + 1, "Subgraphs nested too deeply"));
                        None
                    } else {
                        let (id, title) = parse_subgraph_header(rest);
                        let idx = b.fc.subgraphs.len();
                        b.fc.subgraphs.push(Subgraph { id, title, parent: b.stack.last().copied(), direction: None, line: line.no });
                        b.stack.push(idx);
                        Some(StmtKind::Keyword { word: "subgraph".into(), ids: Vec::new() })
                    }
                }
                "end" if rest.is_empty() => {
                    if b.stack.pop().is_none() {
                        b.diags.push(Diagnostic::error(line.no, col_base + 1, "'end' without a matching 'subgraph'"));
                    }
                    Some(StmtKind::Keyword { word: "end".into(), ids: Vec::new() })
                }
                "direction" => {
                    match (Direction::parse(rest.trim()), b.stack.last().copied()) {
                        (Some(d), Some(s)) => {
                            if let Some(sg) = b.fc.subgraphs.get_mut(s) {
                                sg.direction = Some(d);
                            }
                        }
                        (None, _) => b.diags.push(Diagnostic::error(line.no, col_base + 1, format!("Unknown direction '{}'", rest.trim()))),
                        (_, None) => b.diags.push(Diagnostic::warning(line.no, col_base + 1, "'direction' is only used inside a subgraph")),
                    }
                    Some(StmtKind::Keyword { word: "direction".into(), ids: Vec::new() })
                }
                "style" => {
                    let (ids, props) = first_word(rest);
                    let refs = id_list(ids, rest_base);
                    let map = parse_props(props);
                    for r in &refs {
                        b.fc.node_styles.entry(r.id.clone()).or_default().extend(map.clone());
                    }
                    Some(StmtKind::Keyword { word: "style".into(), ids: refs })
                }
                "classDef" => {
                    let (names, props) = first_word(rest);
                    let map = parse_props(props);
                    for n in names.split(',') {
                        if !n.trim().is_empty() {
                            b.fc.class_defs.entry(n.trim().to_string()).or_default().extend(map.clone());
                        }
                    }
                    Some(StmtKind::Keyword { word: "classDef".into(), ids: Vec::new() })
                }
                "class" => {
                    let (ids, name) = first_word(rest);
                    let refs = id_list(ids, rest_base);
                    for r in &refs {
                        if let Some(i) = b.index.get(&r.id).copied() {
                            if let Some(n) = b.fc.nodes.get_mut(i) {
                                n.classes.push(name.trim().to_string());
                            }
                        }
                    }
                    Some(StmtKind::Keyword { word: "class".into(), ids: refs })
                }
                "click" | "linkStyle" | "accTitle" | "accDescr" | "title" => Some(StmtKind::Keyword { word: word.to_string(), ids: Vec::new() }),
                _ => match parse_chain(&mut b, text, base, line.no, stmt_no) {
                    Ok(k) => Some(k),
                    Err((col, msg)) => {
                        b.diags.push(Diagnostic::error(line.no, col_base + col, msg));
                        None
                    }
                },
            };
            if let Some(kind) = kind {
                b.fc.statements.push(Stmt { span, line: line.no, kind });
            } else {
                // Keep statement numbering aligned with `FlowEdge::stmt`.
                b.fc.statements.push(Stmt { span, line: line.no, kind: StmtKind::Keyword { word: "error".into(), ids: Vec::new() } });
            }
            stmt_no += 1;
        }
    }
    for idx in b.stack.iter().rev() {
        if let Some(sg) = b.fc.subgraphs.get(*idx) {
            b.diags.push(Diagnostic::error(sg.line, 1, format!("Subgraph '{}' is missing its 'end'", sg.id)));
        }
    }
    // An edge may only join known things.
    diags.append(&mut b.diags);
    b.fc
}
