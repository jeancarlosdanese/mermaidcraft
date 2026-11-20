//! Visual edits of a flowchart expressed as changes to the source text, so comments, order and
//! formatting are kept as far as possible.

use std::collections::HashSet;

use crate::flowchart;
use crate::model::*;
use crate::util::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditError {
    NotFlowchart,
    NoSuchNode(String),
    NoSuchEdge(usize),
    InvalidId(String),
    DuplicateId(String),
    Invalid(String),
}

impl std::fmt::Display for EditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFlowchart => write!(f, "this edit only works on flowcharts"),
            Self::NoSuchNode(id) => write!(f, "there is no node '{id}'"),
            Self::NoSuchEdge(i) => write!(f, "there is no edge number {i}"),
            Self::InvalidId(id) => write!(f, "'{id}' is not a valid node name (use letters, digits and '_')"),
            Self::DuplicateId(id) => write!(f, "the name '{id}' is already used"),
            Self::Invalid(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for EditError {}

fn load(src: &str) -> Result<Flowchart, EditError> {
    load_with(src).map(|(fc, _)| fc)
}

/// Like `load`, but refuses a text that has syntax errors: structural edits of groups would
/// otherwise build on a parse that is wrong.
fn load_checked(src: &str) -> Result<Flowchart, EditError> {
    let (fc, diags) = load_with(src)?;
    match diags.iter().find(|d| d.severity == Severity::Error) {
        Some(d) => Err(EditError::Invalid(format!("fix the error on line {} first: {}", d.line, d.message))),
        None => Ok(fc),
    }
}

fn load_with(src: &str) -> Result<(Flowchart, Vec<Diagnostic>), EditError> {
    if src.len() > MAX_SOURCE_BYTES {
        return Err(EditError::Invalid("the diagram is too large".into()));
    }
    let lines = split_lines(src);
    let first = first_significant(&lines).ok_or(EditError::NotFlowchart)?;
    let head = lines.get(first).map_or("", |l| l.text.trim());
    let word = head.split(|c: char| c.is_whitespace() || c == ';').next().unwrap_or("");
    if !matches!(word, "graph" | "flowchart" | "flowchart-elk") {
        return Err(EditError::NotFlowchart);
    }
    let mut d = Vec::new();
    let fc = flowchart::parse(&lines, first, &mut d);
    Ok((fc, d))
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 100 && id.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') && !id.starts_with('-') && !id.ends_with('-') && id != "end"
}

/// A name that no node or subgraph uses yet: A, B, ... Z, A1, B1, ...
pub fn fresh_id(src: &str) -> String {
    let (nodes, subs): (HashSet<String>, HashSet<String>) = match load(src) {
        Ok(fc) => (fc.nodes.iter().map(|n| n.id.clone()).collect(), fc.subgraphs.iter().map(|s| s.id.clone()).collect()),
        Err(_) => (HashSet::new(), HashSet::new()),
    };
    for round in 0..200 {
        for c in 'A'..='Z' {
            let id = if round == 0 { c.to_string() } else { format!("{c}{round}") };
            if !nodes.contains(&id) && !subs.contains(&id) && id != "end" {
                return id;
            }
        }
    }
    "N".to_string()
}

fn indent_of(src: &str) -> String {
    for line in src.lines().rev() {
        let t = line.trim();
        if t.is_empty() || t.starts_with("%%") {
            continue;
        }
        if line.len() != line.trim_start().len() {
            return line.get(..line.len() - line.trim_start().len()).unwrap_or("    ").to_string();
        }
        break;
    }
    "    ".to_string()
}

fn append_line(src: &str, line: &str) -> String {
    let mut out = src.trim_end_matches([' ', '\t']).to_string();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(&indent_of(src));
    out.push_str(line);
    out.push('\n');
    out
}

fn shape_text(shape: NodeShape, label: &str) -> String {
    let (o, c) = shape.brackets();
    format!("{o}{}{c}", quote_if_needed(label))
}

fn apply(src: &str, mut edits: Vec<(Span, String)>) -> Result<String, EditError> {
    edits.sort_by_key(|e| std::cmp::Reverse(e.0.start));
    let mut out = src.to_string();
    let mut last_start = usize::MAX;
    for (span, text) in edits {
        if span.end > last_start || span.end > out.len() || !out.is_char_boundary(span.start) || !out.is_char_boundary(span.end) {
            return Err(EditError::Invalid("could not apply the edit to the text".into()));
        }
        out.replace_range(span.start..span.end, &text);
        last_start = span.start;
    }
    Ok(out)
}

/// Adds a node on a new line at the end. Returns the new text and the id used.
pub fn add_node(src: &str, id: Option<&str>, label: &str, shape: NodeShape) -> Result<(String, String), EditError> {
    let fc = load(src)?;
    let id = match id.map(str::trim).filter(|s| !s.is_empty()) {
        Some(i) => {
            if !valid_id(i) {
                return Err(EditError::InvalidId(i.to_string()));
            }
            if fc.node_index(i).is_some() || fc.subgraph_index(i).is_some() {
                return Err(EditError::DuplicateId(i.to_string()));
            }
            i.to_string()
        }
        None => fresh_id(src),
    };
    let label = if label.trim().is_empty() { id.as_str() } else { label.trim() };
    let line = format!("{id}{}", shape_text(shape, label));
    Ok((append_line(src, &line), id))
}

/// Changes the label and/or the shape of a node.
pub fn set_node(src: &str, id: &str, label: Option<&str>, shape: Option<NodeShape>) -> Result<String, EditError> {
    let fc = load(src)?;
    let idx = fc.node_index(id).ok_or_else(|| EditError::NoSuchNode(id.to_string()))?;
    let node = fc.nodes.get(idx).ok_or_else(|| EditError::NoSuchNode(id.to_string()))?;
    let new_label = label.map_or(node.label.clone(), |l| l.trim().to_string());
    let new_label = if new_label.is_empty() { node.id.clone() } else { new_label };
    let new_shape = shape.unwrap_or(node.shape);
    let text = shape_text(new_shape, &new_label);
    let span = match node.def_span {
        Some(s) => s,
        None => Span::new(node.id_span.end, node.id_span.end),
    };
    apply(src, vec![(span, text)])
}

/// Adds a link on a new line at the end.
pub fn add_edge(src: &str, from: &str, to: &str, label: Option<&str>, kind: LinkKind) -> Result<String, EditError> {
    let fc = load(src)?;
    for id in [from, to] {
        let known = fc.node_index(id).is_some() || fc.subgraph_index(id).is_some();
        if !known && !valid_id(id) {
            return Err(EditError::InvalidId(id.to_string()));
        }
        if !known {
            return Err(EditError::NoSuchNode(id.to_string()));
        }
    }
    let lab = label.map(str::trim).filter(|l| !l.is_empty()).map_or(String::new(), |l| format!("|{}|", l.replace('|', "/")));
    let line = format!("{from} {}{lab} {to}", kind.token());
    Ok(append_line(src, &line))
}

#[derive(Default)]
struct Removals {
    nodes: HashSet<String>,
    /// Edge indexes.
    edges: HashSet<usize>,
    /// When set, `nodes` are removed only from these statements (`style` and `class` lines stay).
    scope: Option<HashSet<usize>>,
}

fn rebuild(src: &str, fc: &Flowchart, rm: &Removals) -> Result<String, EditError> {
    let mut edits: Vec<(Span, String)> = Vec::new();
    let gone = |si: usize, id: &str| rm.nodes.contains(id) && rm.scope.as_ref().is_none_or(|s| s.contains(&si));
    for (si, st) in fc.statements.iter().enumerate() {
        let text_of = |s: Span| src.get(s.start..s.end).unwrap_or("").to_string();
        match &st.kind {
            StmtKind::Chain { groups, links } => {
                let touched = groups.iter().any(|g| g.nodes.iter().any(|n| gone(si, &n.id))) || fc.edges.iter().enumerate().any(|(ei, e)| e.stmt == si && rm.edges.contains(&ei));
                if !touched {
                    continue;
                }
                let remaining: Vec<Vec<&NodeRef>> = groups.iter().map(|g| g.nodes.iter().filter(|n| !gone(si, &n.id)).collect()).collect();
                let intact: Vec<bool> = groups.iter().zip(&remaining).map(|(g, r)| g.nodes.len() == r.len()).collect();
                let link_removed = |li: usize| -> bool {
                    fc.edges.iter().enumerate().any(|(ei, e)| e.stmt == si && e.link == li && rm.edges.contains(&ei))
                };
                let mut statements: Vec<String> = Vec::new();
                let mut mentioned: HashSet<String> = HashSet::new();
                let gtext = |i: usize| -> String { remaining.get(i).map(|r| r.iter().map(|n| text_of(n.span)).collect::<Vec<_>>().join(" & ")).unwrap_or_default() };
                let mut i = 0usize;
                while i < groups.len() {
                    let mut j = i;
                    while j + 1 < groups.len() && intact.get(j) == Some(&true) && intact.get(j + 1) == Some(&true) && !link_removed(j) {
                        j += 1;
                    }
                    if j > i {
                        let mut s = gtext(i);
                        for k in i..j {
                            s.push(' ');
                            s.push_str(&links.get(k).map_or(String::new(), |l| text_of(l.span)));
                            s.push(' ');
                            s.push_str(&gtext(k + 1));
                        }
                        statements.push(s);
                        for k in i..=j {
                            for n in remaining.get(k).map(Vec::as_slice).unwrap_or(&[]) {
                                mentioned.insert(n.id.clone());
                            }
                        }
                    }
                    // The link after the segment (if any) was cut: keep the edges that survive.
                    if j + 1 < groups.len() {
                        if let Some(l) = links.get(j) {
                            for a in remaining.get(j).map(Vec::as_slice).unwrap_or(&[]) {
                                for b in remaining.get(j + 1).map(Vec::as_slice).unwrap_or(&[]) {
                                    let dead = fc.edges.iter().enumerate().any(|(ei, e)| e.stmt == si && e.link == j && e.from == a.id && e.to == b.id && rm.edges.contains(&ei));
                                    if !dead {
                                        statements.push(format!("{} {} {}", text_of(a.span), text_of(l.span), text_of(b.span)));
                                        mentioned.insert(a.id.clone());
                                        mentioned.insert(b.id.clone());
                                    }
                                }
                            }
                        }
                    }
                    i = j + 1;
                }
                for r in &remaining {
                    for n in r {
                        if !mentioned.contains(&n.id) {
                            statements.push(text_of(n.span));
                            mentioned.insert(n.id.clone());
                        }
                    }
                }
                let text = if statements.is_empty() {
                    "\u{0}".to_string()
                } else {
                    let indent = line_indent(src, st.span.start);
                    let nl = if src.contains("\r\n") { "\r\n" } else { "\n" };
                    statements.join(&format!("{nl}{indent}"))
                };
                edits.push((st.span, text));
            }
            StmtKind::Keyword { word, ids } => {
                if rm.scope.is_some() || !matches!(word.as_str(), "style" | "class") || !ids.iter().any(|n| rm.nodes.contains(&n.id)) {
                    continue;
                }
                let keep: Vec<&NodeRef> = ids.iter().filter(|n| !rm.nodes.contains(&n.id)).collect();
                if keep.is_empty() {
                    edits.push((st.span, "\u{0}".to_string()));
                } else {
                    let first = ids.first().map_or(st.span.start, |n| n.span.start);
                    let last = ids.last().map_or(st.span.end, |n| n.span.end);
                    let prefix = src.get(st.span.start..first).unwrap_or("");
                    let suffix = src.get(last..st.span.end).unwrap_or("");
                    let list = keep.iter().map(|n| n.id.as_str()).collect::<Vec<_>>().join(",");
                    edits.push((st.span, format!("{prefix}{list}{suffix}")));
                }
            }
        }
    }
    let out = apply(src, edits)?;
    Ok(clean_markers(&out))
}

fn line_indent(src: &str, at: usize) -> String {
    let line_start = src.get(..at).map_or(0, |s| s.rfind('\n').map_or(0, |i| i + 1));
    let seg = src.get(line_start..at).unwrap_or("");
    seg.chars().take_while(|c| c.is_whitespace()).collect()
}

/// Removes the deletion marker and the lines or separators it leaves empty.
fn clean_markers(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for raw in s.split_inclusive('\n') {
        if !raw.contains('\u{0}') {
            out.push_str(raw);
            continue;
        }
        let ending = if raw.ends_with("\r\n") { "\r\n" } else if raw.ends_with('\n') { "\n" } else { "" };
        let body = raw.trim_end_matches(['\r', '\n']);
        let whole = body.replace('\u{0}', "");
        // A `%%` comment at the end of the line stays; the separators are cleaned before it.
        let code_len = strip_comment(&whole).len();
        let (code, comment) = (whole.get(..code_len).unwrap_or(""), whole.get(code_len..).unwrap_or("").trim());
        let mut cleaned = code.to_string();
        // Separators left behind: `; ;`, a leading or trailing `;`.
        while cleaned.contains(";;") || cleaned.contains("; ;") {
            cleaned = cleaned.replace("; ;", ";").replace(";;", ";");
        }
        let trimmed = cleaned.trim();
        let trimmed = trimmed.trim_start_matches(';').trim_end_matches(';').trim();
        if trimmed.is_empty() && comment.is_empty() {
            continue;
        }
        let indent: String = whole.chars().take_while(|c| c.is_whitespace()).collect();
        out.push_str(&indent);
        out.push_str(trimmed);
        if !trimmed.is_empty() && !comment.is_empty() {
            out.push(' ');
        }
        out.push_str(comment);
        out.push_str(ending);
    }
    out
}

fn remove_node_raw(src: &str, id: &str) -> Result<String, EditError> {
    let fc = load(src)?;
    if fc.node_index(id).is_none() {
        return Err(EditError::NoSuchNode(id.to_string()));
    }
    let mut rm = Removals::default();
    rm.nodes.insert(id.to_string());
    rebuild(src, &fc, &rm)
}

fn remove_edge_raw(src: &str, edge: usize) -> Result<String, EditError> {
    let fc = load(src)?;
    if edge >= fc.edges.len() {
        return Err(EditError::NoSuchEdge(edge));
    }
    let mut rm = Removals::default();
    rm.edges.insert(edge);
    rebuild(src, &fc, &rm)
}

/// Changes the label (`Some(None)` clears it) and/or the kind of an edge.
fn set_edge_raw(src: &str, edge: usize, label: Option<Option<&str>>, kind: Option<LinkKind>) -> Result<String, EditError> {
    let fc = load(src)?;
    let e = fc.edges.get(edge).ok_or(EditError::NoSuchEdge(edge))?;
    let new_label: Option<String> = match label {
        Some(l) => l.map(str::trim).filter(|s| !s.is_empty()).map(String::from),
        None => e.label.clone(),
    };
    let new_kind = kind.unwrap_or(e.kind);
    let lab = new_label.as_ref().map_or(String::new(), |l| format!("|{}|", l.replace('|', "/")));
    let token = format!("{}{lab}", new_kind.token());
    if let Some(Stmt { kind: StmtKind::Chain { groups, links }, .. }) = fc.statements.get(e.stmt) {
        let single = groups.get(e.link).is_some_and(|g| g.nodes.len() == 1) && groups.get(e.link + 1).is_some_and(|g| g.nodes.len() == 1);
        if single {
            if let Some(l) = links.get(e.link) {
                return apply(src, vec![(l.span, token)]);
            }
        }
    }
    // The link is shared by several edges (`A & B --> C`): move this edge to its own line.
    let (from, to) = (e.from.clone(), e.to.clone());
    let removed = remove_edge_raw(src, edge)?;
    Ok(append_line(&removed, &format!("{from} {token} {to}")))
}

/// Swaps the two ends of an edge and keeps its line as written: `A --> B` becomes `B --> A`,
/// `A x--> B` becomes `B x--> A`. Mermaid has no head on the start alone (`B <-- A` is an error),
/// so the heads are not swapped. A link alone on its statement is turned in place, with the node
/// definitions (`A[One] --> B[Two]` becomes `B[Two] --> A[One]`); a link of a chain or a shared
/// one (`A & B --> C`) moves to its own line. Returns the new text and the edge's new number.
fn reverse_edge_raw(src: &str, edge: usize) -> Result<(String, usize), EditError> {
    let fc = load(src)?;
    let e = fc.edges.get(edge).ok_or(EditError::NoSuchEdge(edge))?;
    if e.from == e.to {
        return Ok((src.to_string(), edge));
    }
    let Some(Stmt { kind: StmtKind::Chain { groups, links }, .. }) = fc.statements.get(e.stmt) else {
        return Err(EditError::NoSuchEdge(edge));
    };
    let link = links.get(e.link).ok_or(EditError::NoSuchEdge(edge))?;
    let text_of = |s: Span| src.get(s.start..s.end).unwrap_or("").to_string();
    let alone = |i: usize| groups.get(i).and_then(|g| if let [only] = g.nodes.as_slice() { Some(only) } else { None });
    if let (2, Some(a), Some(b)) = (groups.len(), alone(0), alone(1)) {
        let out = apply(src, vec![(a.span, text_of(b.span)), (b.span, text_of(a.span))])?;
        return Ok((out, edge));
    }
    let token = text_of(link.span);
    let (from, to) = (e.from.clone(), e.to.clone());
    let removed = remove_edge_raw(src, edge)?;
    // The new line goes where `insert_line` puts it: before the layout comment, else at the end.
    let mut at = removed.trim_end_matches([' ', '\t']).len();
    let mut seen = 0usize;
    for raw in removed.split_inclusive('\n') {
        if raw.trim_start().starts_with(flowchart::LAYOUT_COMMENT) {
            at = seen;
            break;
        }
        seen += raw.len();
    }
    let out = insert_line(&removed, &format!("{to} {token} {from}"));
    let fc = load(&out)?;
    let starts_after = |x: &FlowEdge| fc.statements.get(x.stmt).is_some_and(|st| st.span.start >= at);
    let index = fc.edges.iter().position(|x| x.from == to && x.to == from && starts_after(x)).ok_or(EditError::NoSuchEdge(edge))?;
    Ok((out, index))
}

/// The `style` statements that style this node alone (`style A ...`, not `style A,B ...`).
fn own_style_stmts<'a>(fc: &'a Flowchart, id: &str) -> Vec<&'a Stmt> {
    fc.statements.iter().filter(|st| matches!(&st.kind, StmtKind::Keyword { word, ids } if word == "style" && ids.len() == 1 && ids.first().is_some_and(|r| r.id == id))).collect()
}

/// The properties a node gets from its own `style <id> ...` lines. Lines shared with other nodes
/// and `class` are left out: the style panel only rewrites the node's own line.
pub fn own_node_style(src: &str, fc: &Flowchart, id: &str) -> StyleMap {
    let mut m = StyleMap::new();
    for st in own_style_stmts(fc, id) {
        let text = src.get(st.span.start..st.span.end).unwrap_or("");
        let rest = text.trim_start().strip_prefix("style").unwrap_or("").trim_start();
        let props = rest.find(char::is_whitespace).map_or("", |i| rest.get(i..).unwrap_or(""));
        m.extend(parse_props(props));
    }
    m
}

/// Adds a line before the `%% mermaidcraft:layout` comment (or at the end), with the file's own
/// line break.
fn insert_line(src: &str, line: &str) -> String {
    insert_line_with(src, line, &indent_of(src))
}

fn insert_line_with(src: &str, line: &str, indent: &str) -> String {
    let nl = if src.contains("\r\n") { "\r\n" } else { "\n" };
    let mut at = 0usize;
    for raw in src.split_inclusive('\n') {
        if raw.trim_start().starts_with(flowchart::LAYOUT_COMMENT) {
            return format!("{}{indent}{line}{nl}{}", src.get(..at).unwrap_or(""), src.get(at..).unwrap_or(""));
        }
        at += raw.len();
    }
    let mut out = src.trim_end_matches([' ', '\t']).to_string();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push_str(nl);
    }
    format!("{out}{indent}{line}{nl}")
}

/// Sets the style of one node in a single `style <id> key:value,...` line. With one line of its
/// own the node keeps it in place; with several they become one (before the layout comment).
/// Shared `style A,B ...` lines are kept. An empty map removes the node's own lines.
pub fn set_node_style(src: &str, id: &str, props: &StyleMap) -> Result<String, EditError> {
    let fc = load(src)?;
    if fc.node_index(id).is_none() && fc.subgraph_index(id).is_none() {
        return Err(EditError::NoSuchNode(id.to_string()));
    }
    let bad = |t: &str| t.trim().is_empty() || t.contains([',', ';', '\n', '\r']);
    if props.iter().any(|(k, v)| bad(k) || bad(v) || k.contains(':')) {
        return Err(EditError::Invalid("a style value cannot be empty or hold ',' or ';'".into()));
    }
    let own = own_style_stmts(&fc, id);
    let list: Vec<String> = props.iter().map(|(k, v)| format!("{}:{}", k.trim(), v.trim())).collect();
    let line = format!("style {id} {}", list.join(","));
    if let [only] = own.as_slice() {
        if !props.is_empty() {
            return apply(src, vec![(only.span, line)]);
        }
    }
    let out = clean_markers(&apply(src, own.iter().map(|st| (st.span, "\u{0}".to_string())).collect())?);
    if props.is_empty() { Ok(out) } else { Ok(insert_line(&out, &line)) }
}

// ---------------------------------------------------------------------------------------------
// Groups (subgraph)
// ---------------------------------------------------------------------------------------------

/// The innermost group around each statement. The `subgraph` and `end` lines belong to the group
/// outside them.
fn owners(fc: &Flowchart) -> Vec<Option<usize>> {
    let mut stack: Vec<usize> = Vec::new();
    let mut next = 0usize;
    let mut out = Vec::with_capacity(fc.statements.len());
    for st in &fc.statements {
        match &st.kind {
            StmtKind::Keyword { word, .. } if word == "subgraph" => {
                out.push(stack.last().copied());
                stack.push(next);
                next += 1;
            }
            StmtKind::Keyword { word, .. } if word == "end" => {
                stack.pop();
                out.push(stack.last().copied());
            }
            _ => out.push(stack.last().copied()),
        }
    }
    out
}

/// Indentation of the statements that sit outside every group.
fn base_indent(src: &str, fc: &Flowchart, owners: &[Option<usize>]) -> String {
    let loose = fc.statements.iter().enumerate().find(|(si, _)| owners.get(*si).copied().flatten().is_none());
    loose.or_else(|| fc.statements.iter().enumerate().next()).map_or_else(|| "    ".to_string(), |(_, st)| line_indent(src, st.span.start))
}

/// The group named `id`. Two groups with the same id cannot be told apart in the text, so an edit
/// of either is refused.
fn group_index(fc: &Flowchart, id: &str) -> Result<usize, EditError> {
    if fc.subgraphs.iter().filter(|s| s.id == id).count() > 1 {
        return Err(EditError::Invalid(format!("two groups use the id '{id}': give each one an id of its own in the text (subgraph id[\"Title\"])")));
    }
    fc.subgraph_index(id).ok_or_else(|| EditError::Invalid(format!("there is no group '{id}'")))
}

/// `["Title"]` as written after the id in a `subgraph` line.
fn group_title_text(title: &str) -> Result<String, EditError> {
    let t = title.trim();
    if t.is_empty() {
        return Err(EditError::Invalid("a group needs a title".into()));
    }
    Ok(format!("[\"{}\"]", t.replace(['\n', '\r'], "<br/>").replace('"', "#quot;")))
}

/// Changes the title of a group and keeps its id (a header without an id of its own gets the one
/// the parser derived, so links and moved positions keep working).
pub fn rename_group(src: &str, id: &str, title: &str) -> Result<String, EditError> {
    let fc = load_checked(src)?;
    let sg = fc.subgraphs.get(group_index(&fc, id)?).ok_or_else(|| EditError::Invalid(format!("there is no group '{id}'")))?;
    apply(src, vec![(sg.header, format!("subgraph {id}{}", group_title_text(title)?))])
}

/// A group name nothing uses yet: G1, G2, ...
fn fresh_group_id(fc: &Flowchart) -> String {
    (1..)
        .map(|n| format!("G{n}"))
        .find(|id| fc.node_index(id).is_none() && fc.subgraph_index(id).is_none())
        .unwrap_or_else(|| "G".to_string())
}

/// Adds `line` just before the `end` of group `g`, one level deeper than the `end`.
fn insert_before_end(src: &str, fc: &Flowchart, owners: &[Option<usize>], g: usize, line: &str) -> Result<String, EditError> {
    let sg = fc.subgraphs.get(g).ok_or_else(|| EditError::Invalid("there is no such group".into()))?;
    let end = sg.end.ok_or_else(|| EditError::Invalid("the group is missing its 'end'".into()))?;
    let nl = if src.contains("\r\n") { "\r\n" } else { "\n" };
    let line_start = src.get(..end.start).map_or(0, |t| t.rfind('\n').map_or(0, |i| i + 1));
    let lead = src.get(line_start..end.start).unwrap_or("");
    if !lead.chars().all(char::is_whitespace) {
        // `end` shares its line with other statements.
        return apply(src, vec![(Span::new(end.start, end.start), format!("{line}; "))]);
    }
    let inner = fc
        .statements
        .iter()
        .enumerate()
        .find(|(si, st)| owners.get(*si).copied().flatten() == Some(g) && st.span.start >= sg.header.end && st.span.end <= end.start)
        .map(|(_, st)| line_indent(src, st.span.start))
        .filter(|i| i.len() > lead.len())
        .unwrap_or_else(|| format!("{lead}  "));
    apply(src, vec![(Span::new(line_start, line_start), format!("{inner}{line}{nl}"))])
}

/// Puts a node inside group `dest`, or outside every group when `dest` is None. The label, shape,
/// style and links of the node stay. Its lines inside groups are cut: a link in such a line goes to
/// a line of its own outside the groups, and the other nodes of the line stay where they were.
fn move_to_group_raw(src: &str, node: &str, dest: Option<&str>) -> Result<String, EditError> {
    let fc = load_checked(src)?;
    let ni = fc.node_index(node).ok_or_else(|| EditError::NoSuchNode(node.to_string()))?;
    let n = fc.nodes.get(ni).ok_or_else(|| EditError::NoSuchNode(node.to_string()))?;
    let dest_idx = match dest {
        Some(g) => Some(group_index(&fc, g)?),
        None => None,
    };
    let own = owners(&fc);
    // Mentioned inside some other group too: the move still has work to do.
    let elsewhere = fc.statements.iter().enumerate().any(|(si, st)| matches!(&st.kind, StmtKind::Chain { groups, .. } if groups.iter().any(|g| g.nodes.iter().any(|r| r.id == node))) && own.get(si).copied().flatten().is_some_and(|g| Some(g) != dest_idx));
    if n.parent == dest_idx && !elsewhere {
        return Ok(src.to_string());
    }
    let text_of = |s: Span| src.get(s.start..s.end).unwrap_or("").to_string();
    let mentions = |st: &Stmt| match &st.kind {
        StmtKind::Chain { groups, .. } => groups.iter().any(|g| g.nodes.iter().any(|r| r.id == node)),
        StmtKind::Keyword { .. } => false,
    };
    let in_group: HashSet<usize> = fc.statements.iter().enumerate().filter(|(si, st)| own.get(*si).copied().flatten().is_some() && mentions(st)).map(|(si, _)| si).collect();
    let refs = || fc.statements.iter().filter_map(|st| if let StmtKind::Chain { groups, .. } = &st.kind { Some(groups) } else { None }).flatten().flat_map(|g| g.nodes.iter()).filter(|r| r.id == node);
    // The last definition is the one that counts (`X[Raiz]` ... `X[Novo]` reads "Novo").
    let def = refs().rfind(|r| r.has_def).or_else(|| refs().next()).map_or_else(|| node.to_string(), |r| text_of(r.span));
    let mut readd: Vec<String> = Vec::new();
    for e in fc.edges.iter().filter(|e| (e.from == node || e.to == node) && in_group.contains(&e.stmt)) {
        if let Some(Stmt { kind: StmtKind::Chain { links, .. }, .. }) = fc.statements.get(e.stmt) {
            if let Some(l) = links.get(e.link) {
                readd.push(format!("{} {} {}", e.from, text_of(l.span), e.to));
            }
        }
    }
    let base = base_indent(src, &fc, &own);
    // Moving into a group: a loose line that holds only this node (`D` or `D[Label]`) is redundant.
    let mut scope = in_group.clone();
    if dest_idx.is_some() {
        let alone = |st: &Stmt| matches!(&st.kind, StmtKind::Chain { groups, .. } if matches!(groups.as_slice(), [g] if matches!(g.nodes.as_slice(), [r] if r.id == node)));
        scope.extend(fc.statements.iter().enumerate().filter(|(si, st)| !in_group.contains(si) && alone(st)).map(|(si, _)| si));
    }
    let mut text = src.to_string();
    if !scope.is_empty() {
        let rm = Removals { nodes: HashSet::from([node.to_string()]), edges: HashSet::new(), scope: Some(scope) };
        text = rebuild(src, &fc, &rm)?;
    }
    let fc2 = load_checked(&text)?;
    let mut root_lines: Vec<String> = Vec::new();
    match dest {
        Some(g) => {
            let own2 = owners(&fc2);
            let gi = group_index(&fc2, g)?;
            text = insert_before_end(&text, &fc2, &own2, gi, &def)?;
        }
        None => {
            let lost = match fc2.node_index(node).and_then(|i| fc2.nodes.get(i)) {
                None => true,
                Some(m) => n.def_span.is_some() && m.def_span.is_none(),
            };
            if lost {
                root_lines.push(def);
            }
        }
    }
    root_lines.extend(readd);
    let mut text = insert_root_lines(&text, &root_lines, &base)?;
    // The parser gives a class only to a node it has already met, so a `class` line that now comes
    // before the node (or a `:::class` on a mention that was cut) is said again after it.
    let fc3 = load_checked(&text)?;
    let kept = fc3.node_index(node).and_then(|i| fc3.nodes.get(i)).map(|m| m.classes.clone()).unwrap_or_default();
    for c in n.classes.iter().filter(|c| !kept.contains(c)) {
        text = insert_line_with(&text, &format!("class {node} {c}"), &base);
    }
    // A definition left elsewhere may now come last and give the node another label or shape:
    // say the one it had once more, at the end.
    let now = fc3.node_index(node).and_then(|i| fc3.nodes.get(i)).map(|m| (m.label.clone(), m.shape));
    if now != Some((n.label.clone(), n.shape)) {
        text = insert_line_with(&text, &format!("{node}{}", shape_text(n.shape, &n.label)), &base);
    }
    Ok(text)
}

/// Adds lines outside the groups, before the first `class` line when there is one (the parser
/// only gives a class to a node it has already met) and otherwise at the end.
fn insert_root_lines(src: &str, lines: &[String], indent: &str) -> Result<String, EditError> {
    if lines.is_empty() {
        return Ok(src.to_string());
    }
    let fc = load_checked(src)?;
    let nl = if src.contains("\r\n") { "\r\n" } else { "\n" };
    let own = owners(&fc);
    let class_at = fc.statements.iter().enumerate().find(|(si, st)| own.get(*si).copied().flatten().is_none() && matches!(&st.kind, StmtKind::Keyword { word, .. } if word == "class")).map(|(_, st)| st.span.start);
    if let Some(at) = class_at {
        let line_start = src.get(..at).map_or(0, |t| t.rfind('\n').map_or(0, |i| i + 1));
        if src.get(line_start..at).is_some_and(|lead| lead.chars().all(char::is_whitespace)) {
            let block: String = lines.iter().map(|l| format!("{indent}{l}{nl}")).collect();
            return apply(src, vec![(Span::new(line_start, line_start), block)]);
        }
    }
    Ok(lines.iter().fold(src.to_string(), |t, l| insert_line_with(&t, l, indent)))
}

/// Wraps a node in a new group. Returns the new text and the id of the group.
pub fn new_group(src: &str, node: &str, title: &str) -> Result<(String, String), EditError> {
    let fc = load_checked(src)?;
    if fc.node_index(node).is_none() {
        return Err(EditError::NoSuchNode(node.to_string()));
    }
    let id = fresh_group_id(&fc);
    let base = base_indent(src, &fc, &owners(&fc));
    let with = insert_line_with(src, &format!("subgraph {id}{}", group_title_text(title)?), &base);
    let with = insert_line_with(&with, "end", &base);
    Ok((move_to_group(&with, node, Some(&id))?, id))
}

/// Cuts the `subgraph` and `end` lines of a group and its own `direction`; what was inside stays.
fn strip_group(src: &str, id: &str) -> Result<String, EditError> {
    let fc = load_checked(src)?;
    let g = group_index(&fc, id)?;
    let sg = fc.subgraphs.get(g).ok_or_else(|| EditError::Invalid(format!("there is no group '{id}'")))?;
    let own = owners(&fc);
    let mut edits: Vec<(Span, String)> = vec![(sg.header, "\u{0}".to_string())];
    if let Some(end) = sg.end {
        edits.push((end, "\u{0}".to_string()));
    }
    for (si, st) in fc.statements.iter().enumerate() {
        if own.get(si).copied().flatten() == Some(g) && matches!(&st.kind, StmtKind::Keyword { word, .. } if word == "direction") {
            edits.push((st.span, "\u{0}".to_string()));
        }
    }
    // What was inside moves one level to the left, when the group sat on lines of its own.
    if let Some(end) = sg.end {
        let outer = line_indent(src, sg.header.start);
        let first = fc.statements.iter().enumerate().find(|(si, st)| own.get(*si).copied().flatten() == Some(g) && st.span.start >= sg.header.end && st.span.end <= end.start);
        let alone = |at: usize, before: bool| {
            let line = if before { src.get(..at).map_or("", |t| t.rsplit('\n').next().unwrap_or("")) } else { src.get(at..).map_or("", |t| t.split('\n').next().unwrap_or("")) };
            line.trim_matches([' ', '\t', '\r', ';']).is_empty()
        };
        if let (Some((_, first)), true, true) = (first, alone(sg.header.start, true) && alone(sg.header.end, false), alone(end.start, true) && alone(end.end, false)) {
            let inner = line_indent(src, first.span.start);
            if inner.len() > outer.len() && inner.starts_with(&outer) {
                let mut at = src.get(sg.header.end..).and_then(|t| t.find('\n')).map(|i| sg.header.end + i + 1);
                while let Some(ls) = at.filter(|ls| *ls < end.start) {
                    if src.get(ls..).is_some_and(|t| t.starts_with(&inner)) {
                        edits.push((Span::new(ls + outer.len(), ls + inner.len()), String::new()));
                    }
                    at = src.get(ls..).and_then(|t| t.find('\n')).map(|i| ls + i + 1);
                }
            }
        }
    }
    Ok(clean_markers(&apply(src, edits)?))
}

/// Forgets the moved positions of groups that no longer exist.
fn drop_group_offsets(src: &str, ids: &[String]) -> Result<String, EditError> {
    let fc = load(src)?;
    let mut offsets = fc.offsets.clone();
    let before = offsets.len();
    for id in ids {
        offsets.remove(&flowchart::group_offset_key(id));
    }
    if offsets.len() == before {
        return Ok(src.to_string());
    }
    set_offsets(src, &offsets)
}

/// Undoes a group: its lines go and what was inside moves up to the group outside. Links that
/// pointed at the group go too (without it the id would become a new node); the number of links
/// that went is returned.
fn ungroup_raw(src: &str, id: &str) -> Result<(String, usize), EditError> {
    let fc = load_checked(src)?;
    group_index(&fc, id)?;
    let arrows = fc.edges.iter().filter(|e| e.from == id || e.to == id).count();
    let rm = Removals { nodes: HashSet::from([id.to_string()]), ..Removals::default() };
    let text = strip_group(&rebuild(src, &fc, &rm)?, id)?;
    Ok((drop_group_offsets(&text, &[id.to_string()])?, arrows))
}

/// Deletes a group with the groups, nodes and links inside it.
fn remove_group_raw(src: &str, id: &str) -> Result<String, EditError> {
    let fc = load_checked(src)?;
    let g = group_index(&fc, id)?;
    let inside = |mut p: Option<usize>| {
        while let Some(k) = p {
            if k == g {
                return true;
            }
            p = fc.subgraphs.get(k).and_then(|s| s.parent);
        }
        false
    };
    let subs: Vec<String> = fc.subgraphs.iter().enumerate().filter(|(k, _)| inside(Some(*k))).map(|(_, s)| s.id.clone()).collect();
    let mut rm = Removals::default();
    rm.nodes.extend(subs.iter().cloned());
    rm.nodes.extend(fc.nodes.iter().filter(|n| inside(n.parent)).map(|n| n.id.clone()));
    let mut text = rebuild(src, &fc, &rm)?;
    for sid in &subs {
        text = strip_group(&text, sid)?;
    }
    drop_group_offsets(&text, &subs)
}

/// What a place of the text names: a node or an edge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextItem {
    Node(String),
    Edge(usize),
    Group(String),
}

/// The node or edge at byte `at` of the text: the id or link under the cursor, else the first node
/// of a statement on `line` (1-based); a `subgraph` line or a group id names the group. A link drawn as several edges
/// (`A & B --> C`) names none of them.
pub fn item_at(fc: &Flowchart, at: usize, line: usize) -> Option<TextItem> {
    let inside = |sp: &Span| sp.start <= at && at <= sp.end;
    let node = |r: &NodeRef| match (fc.node_index(&r.id), fc.subgraph_index(&r.id)) {
        (Some(_), _) => Some(TextItem::Node(r.id.clone())),
        (None, Some(_)) => Some(TextItem::Group(r.id.clone())),
        (None, None) => None,
    };
    let header = |st: &Stmt| fc.subgraphs.iter().find(|s| s.header == st.span).map(|s| TextItem::Group(s.id.clone()));
    for (si, st) in fc.statements.iter().enumerate() {
        if !inside(&st.span) {
            continue;
        }
        match &st.kind {
            StmtKind::Chain { groups, links } => {
                if let Some(r) = groups.iter().flat_map(|g| g.nodes.iter()).find(|r| inside(&r.span)) {
                    return node(r);
                }
                if let Some(li) = links.iter().position(|l| inside(&l.span)) {
                    let mut drawn = fc.edges.iter().enumerate().filter(|(_, e)| e.stmt == si && e.link == li).map(|(i, _)| i);
                    return match (drawn.next(), drawn.next()) {
                        (Some(e), None) => Some(TextItem::Edge(e)),
                        _ => None,
                    };
                }
            }
            StmtKind::Keyword { word, ids } => {
                if word == "subgraph" {
                    return header(st);
                }
                if let Some(r) = ids.iter().find(|r| inside(&r.span)) {
                    return node(r);
                }
            }
        }
    }
    fc.statements.iter().filter(|st| st.line == line).find_map(|st| match &st.kind {
        StmtKind::Chain { groups, .. } => groups.first().and_then(|g| g.nodes.first()).and_then(node),
        StmtKind::Keyword { word, ids } if word == "style" || word == "class" => ids.first().and_then(node),
        StmtKind::Keyword { word, .. } if word == "subgraph" => header(st),
        StmtKind::Keyword { .. } => None,
    })
}

pub fn set_direction(src: &str, dir: Direction) -> Result<String, EditError> {
    let fc = load(src)?;
    let span = fc.direction_span;
    let text = if span.is_empty() { format!(" {}", dir.code()) } else { dir.code().to_string() };
    apply(src, vec![(span, text)])
}

/// Starter text for a new diagram of each supported type.
pub fn template(kind: &str) -> &'static str {
    match kind {
        "sequence" | "sequenceDiagram" => "sequenceDiagram\n    participant A as Alice\n    participant B as Bob\n    A->>B: Hello Bob, how are you?\n    B-->>A: Great!\n",
        "er" | "erDiagram" => "erDiagram\n    CUSTOMER ||--o{ ORDER : places\n    CUSTOMER {\n        string name\n        string email PK\n    }\n    ORDER {\n        int id PK\n        date created\n    }\n",
        _ => "flowchart TD\n    A[Start] --> B{Is it working?}\n    B -->|Yes| C[Great]\n    B -->|No| D[Fix it]\n    D --> B\n",
    }
}

/// Stores the moved-node offsets in a `%% mermaidcraft:layout` comment (replacing the old one);
/// an empty map removes the comment.
pub fn set_offsets(src: &str, offsets: &std::collections::BTreeMap<String, (f32, f32)>) -> Result<String, EditError> {
    load(src)?;
    let mut out = String::with_capacity(src.len() + 64);
    for raw in src.split_inclusive('\n') {
        if raw.trim_start().starts_with(flowchart::LAYOUT_COMMENT) {
            continue;
        }
        out.push_str(raw);
    }
    if offsets.is_empty() {
        return Ok(out);
    }
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    let items: Vec<String> = offsets.iter().filter(|(_, v)| v.0.abs() >= 0.5 || v.1.abs() >= 0.5).map(|(k, v)| format!("{k}={:.0},{:.0}", v.0, v.1)).collect();
    if items.is_empty() {
        return Ok(out);
    }
    out.push_str(&format!("{} {}\n", flowchart::LAYOUT_COMMENT, items.join(" ")));
    Ok(out)
}

// ---------------------------------------------------------------------------------------------
// `linkStyle` follows the links
// ---------------------------------------------------------------------------------------------

/// `linkStyle` names links by their number in the text. After an edit that moved, changed or
/// removed links, its numbers are brought to the links they named; a number whose link is gone is
/// dropped, and a line left with none goes.
fn keep_link_styles(old: &str, new: String) -> Result<String, EditError> {
    keep_link_styles_with(old, new, None)
}

/// Same, with the old → new link numbers when the edit knows them (removing one link of two twins
/// cannot be told from the text alone).
fn keep_link_styles_with(old: &str, new: String, known: Option<Vec<Option<usize>>>) -> Result<String, EditError> {
    if !new.contains("linkStyle") {
        return Ok(new);
    }
    let (a, b) = (load(old)?, load(&new)?);
    let key = |e: &FlowEdge| (e.from.clone(), e.to.clone(), e.kind, e.label.clone());
    let (ka, kb): (Vec<_>, Vec<_>) = (a.edges.iter().map(key).collect(), b.edges.iter().map(key).collect());
    let (n, m) = (ka.len(), kb.len());
    // The links that stayed, in order: the longest common run of equal links. Read from the end,
    // twins (`A --> B` twice) pair up by place, not by which comes first.
    let mut map: Vec<Option<usize>> = vec![None; n];
    let mut taken = vec![false; m];
    if let Some(k) = known.filter(|k| k.len() == n) {
        map = k;
        taken.fill(true);
    } else if n.saturating_mul(m) <= 4_000_000 {
        let w = m + 1;
        let mut dp = vec![0u32; (n + 1) * w];
        for x in 1..=n {
            for y in 1..=m {
                let v = if ka.get(x - 1) == kb.get(y - 1) { dp[(x - 1) * w + y - 1] + 1 } else { dp[(x - 1) * w + y].max(dp[x * w + y - 1]) };
                dp[x * w + y] = v;
            }
        }
        let (mut x, mut y) = (n, m);
        while x > 0 && y > 0 {
            if ka.get(x - 1) == kb.get(y - 1) && dp[x * w + y] == dp[(x - 1) * w + y - 1] + 1 {
                map[x - 1] = Some(y - 1);
                taken[y - 1] = true;
                x -= 1;
                y -= 1;
            } else if dp[(x - 1) * w + y] >= dp[x * w + y - 1] {
                x -= 1;
            } else {
                y -= 1;
            }
        }
    }
    // The rest moved or changed: an equal link elsewhere first, then (when as many are left on
    // both sides, as after a change of label, kind or direction) by order.
    for (x, slot) in map.iter_mut().enumerate() {
        if slot.is_none() {
            if let Some(y) = (0..m).find(|y| !taken[*y] && kb.get(*y) == ka.get(x)) {
                taken[y] = true;
                *slot = Some(y);
            }
        }
    }
    let left_old: Vec<usize> = (0..n).filter(|x| map[*x].is_none()).collect();
    let left_new: Vec<usize> = (0..m).filter(|y| !taken[*y]).collect();
    if left_old.len() == left_new.len() {
        for (x, y) in left_old.into_iter().zip(left_new) {
            map[x] = Some(y);
        }
    }
    let mut edits: Vec<(Span, String)> = Vec::new();
    for st in &b.statements {
        if !matches!(&st.kind, StmtKind::Keyword { word, .. } if word == "linkStyle") {
            continue;
        }
        let text = new.get(st.span.start..st.span.end).unwrap_or("");
        let rest = text.strip_prefix("linkStyle").unwrap_or("");
        let lead = rest.len() - rest.trim_start().len();
        // The numbers run up to the first property: `0,2`, `0, 2`.
        let body = rest.trim_start();
        let run = body.find(|c: char| !(c.is_ascii_digit() || c == ',' || c == ' ' || c == '\t')).unwrap_or(body.len());
        let list = body.get(..run).unwrap_or("").trim_end();
        let Ok(nums) = list.split(',').map(|n| n.trim().parse::<usize>()).collect::<Result<Vec<usize>, _>>() else { continue };
        // A number past the old links named none: it stays as written.
        let moved: Vec<usize> = nums.iter().filter_map(|k| if *k >= n { Some(*k) } else { map.get(*k).copied().flatten() }).collect();
        if moved == nums {
            continue;
        }
        let start = st.span.start + "linkStyle".len() + lead;
        if moved.is_empty() {
            edits.push((st.span, "\u{0}".to_string()));
        } else {
            let joined = moved.iter().map(usize::to_string).collect::<Vec<_>>().join(",");
            edits.push((Span::new(start, start + list.len()), joined));
        }
    }
    if edits.is_empty() {
        return Ok(new);
    }
    Ok(clean_markers(&apply(&new, edits)?))
}

pub fn remove_node(src: &str, id: &str) -> Result<String, EditError> {
    keep_link_styles(src, remove_node_raw(src, id)?)
}

pub fn remove_edge(src: &str, edge: usize) -> Result<String, EditError> {
    let out = remove_edge_raw(src, edge)?;
    let count = load(src)?.edges.len();
    let map = (0..count).map(|i| match i.cmp(&edge) {
        std::cmp::Ordering::Less => Some(i),
        std::cmp::Ordering::Equal => None,
        std::cmp::Ordering::Greater => Some(i - 1),
    });
    keep_link_styles_with(src, out, Some(map.collect()))
}

/// Changes the label (`Some(None)` clears it) and/or the kind of an edge.
pub fn set_edge(src: &str, edge: usize, label: Option<Option<&str>>, kind: Option<LinkKind>) -> Result<String, EditError> {
    keep_link_styles(src, set_edge_raw(src, edge, label, kind)?)
}

/// Reverses an edge; returns the new text and the edge's new number.
pub fn reverse_edge(src: &str, edge: usize) -> Result<(String, usize), EditError> {
    let (out, index) = reverse_edge_raw(src, edge)?;
    Ok((keep_link_styles(src, out)?, index))
}

/// See `move_to_group_raw`.
pub fn move_to_group(src: &str, node: &str, dest: Option<&str>) -> Result<String, EditError> {
    keep_link_styles(src, move_to_group_raw(src, node, dest)?)
}

/// See `ungroup_raw`.
pub fn ungroup(src: &str, id: &str) -> Result<(String, usize), EditError> {
    let (out, n) = ungroup_raw(src, id)?;
    Ok((keep_link_styles(src, out)?, n))
}

/// Deletes a group with the groups, nodes and links inside it.
pub fn remove_group(src: &str, id: &str) -> Result<String, EditError> {
    keep_link_styles(src, remove_group_raw(src, id)?)
}
