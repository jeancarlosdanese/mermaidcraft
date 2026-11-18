//! Operations shared by the MCP tools: edit operations from JSON and safe file writing.

use mermaidcraft_engine as engine;
use mermaidcraft_engine::edit;
use mermaidcraft_engine::model::{Direction, Head, LineStyle, LinkKind, NodeShape};
use serde_json::Value;

fn s<'a>(op: &'a Value, key: &str) -> Option<&'a str> {
    op.get(key).and_then(Value::as_str)
}

fn need<'a>(op: &'a Value, key: &str) -> Result<&'a str, String> {
    s(op, key).ok_or_else(|| format!("the operation needs '{key}'"))
}

fn shape_of(op: &Value) -> Result<Option<NodeShape>, String> {
    match s(op, "shape") {
        None => Ok(None),
        Some(n) => NodeShape::from_name(n).map(Some).ok_or_else(|| format!("unknown shape '{n}' (rect, round, stadium, subroutine, cylinder, circle, double-circle, diamond, hexagon, parallelogram, parallelogram-alt, trapezoid, trapezoid-alt, asymmetric)")),
    }
}

fn kind_of(op: &Value, base: LinkKind) -> Result<Option<LinkKind>, String> {
    let mut k = base;
    let mut touched = false;
    if let Some(n) = s(op, "style") {
        k.style = LineStyle::from_name(n).ok_or_else(|| format!("unknown style '{n}' (solid, dotted, thick, invisible)"))?;
        touched = true;
    }
    if let Some(n) = s(op, "head_end") {
        k.head_end = Head::from_name(n).ok_or_else(|| format!("unknown head '{n}' (none, arrow, circle, cross)"))?;
        touched = true;
    }
    if let Some(n) = s(op, "head_start") {
        k.head_start = Head::from_name(n).ok_or_else(|| format!("unknown head '{n}' (none, arrow, circle, cross)"))?;
        touched = true;
    }
    Ok(touched.then_some(k))
}

fn find_edge(src: &str, op: &Value) -> Result<usize, String> {
    if let Some(i) = op.get("index").and_then(Value::as_u64) {
        return usize::try_from(i).map_err(|_| "bad edge index".to_string());
    }
    let (from, to) = (need(op, "from")?, need(op, "to")?);
    let p = engine::parse(src);
    match p.diagram {
        Some(engine::Diagram::Flowchart(f)) => f.edges.iter().position(|e| e.from == from && e.to == to).ok_or_else(|| format!("there is no edge from '{from}' to '{to}'")),
        _ => Err("this edit only works on flowcharts".into()),
    }
}

/// Applies one edit operation (`op` names the kind) and returns the new source.
pub fn apply_op(src: &str, op: &Value) -> Result<String, String> {
    let kind = need(op, "op")?;
    let e = |x: edit::EditError| x.to_string();
    match kind {
        "add_node" => edit::add_node(src, s(op, "id"), s(op, "label").unwrap_or(""), shape_of(op)?.unwrap_or_default()).map(|r| r.0).map_err(e),
        "set_node" => edit::set_node(src, need(op, "id")?, s(op, "label"), shape_of(op)?).map_err(e),
        "remove_node" => edit::remove_node(src, need(op, "id")?).map_err(e),
        "add_edge" => {
            let k = kind_of(op, LinkKind::default())?.unwrap_or_default();
            edit::add_edge(src, need(op, "from")?, need(op, "to")?, s(op, "label"), k).map_err(e)
        }
        "set_edge" => {
            let idx = find_edge(src, op)?;
            let current = match engine::parse(src).diagram {
                Some(engine::Diagram::Flowchart(f)) => f.edges.get(idx).map(|x| x.kind).unwrap_or_default(),
                _ => LinkKind::default(),
            };
            let k = kind_of(op, current)?;
            let label = if op.get("label").is_some() { Some(s(op, "label")) } else { None };
            edit::set_edge(src, idx, label, k).map_err(e)
        }
        "remove_edge" => edit::remove_edge(src, find_edge(src, op)?).map_err(e),
        "set_direction" => {
            let d = need(op, "direction")?;
            let dir = Direction::parse(d).ok_or_else(|| format!("unknown direction '{d}' (TD, BT, LR, RL)"))?;
            edit::set_direction(src, dir).map_err(e)
        }
        other => Err(format!("unknown operation '{other}' (add_node, set_node, remove_node, add_edge, set_edge, remove_edge, set_direction)")),
    }
}

const WRITE_EXTENSIONS: &[&str] = &["mmd", "mermaid", "md", "markdown", "svg", "png", "pdf"];

/// Only diagram, Markdown and export files may be written.
pub fn check_write_path(path: &str) -> Result<(), String> {
    let lower = path.to_ascii_lowercase();
    let ext = lower.rsplit('.').next().unwrap_or("");
    if path.is_empty() || !lower.contains('.') || !WRITE_EXTENSIONS.contains(&ext) {
        return Err(format!("'{path}': only .mmd, .mermaid, .md, .svg, .png and .pdf files can be written"));
    }
    Ok(())
}

/// Writes Mermaid code to a file. For Markdown, `block` (1-based) replaces that block, and
/// `append` adds a new block at the end.
pub fn write_code(path: &str, code: &str, block: Option<usize>, append: bool) -> Result<String, String> {
    check_write_path(path)?;
    if crate::is_markdown(path) {
        let existing = if std::path::Path::new(path).exists() { crate::read_file(path)? } else { String::new() };
        let new = if append || existing.is_empty() {
            let mut t = existing;
            if !t.is_empty() && !t.ends_with('\n') {
                t.push('\n');
            }
            if !t.is_empty() {
                t.push('\n');
            }
            t.push_str("```mermaid\n");
            t.push_str(code.trim_end());
            t.push_str("\n```\n");
            t
        } else {
            let blocks = engine::markdown::find_blocks(&existing);
            let n = block.ok_or_else(|| format!("'{path}' is Markdown with {} mermaid block(s): say which block to replace, or append", blocks.len()))?;
            let b = n.checked_sub(1).and_then(|i| blocks.get(i)).ok_or_else(|| format!("block {n} does not exist"))?;
            engine::markdown::replace_block(&existing, b, code).ok_or("could not replace the block")?
        };
        std::fs::write(path, new).map_err(|e| format!("cannot write '{path}': {e}"))?;
    } else {
        let mut text = code.to_string();
        if !text.ends_with('\n') {
            text.push('\n');
        }
        std::fs::write(path, text).map_err(|e| format!("cannot write '{path}': {e}"))?;
    }
    Ok(path.to_string())
}
