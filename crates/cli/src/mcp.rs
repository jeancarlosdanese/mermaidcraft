//! MCP server on stdio (newline-delimited JSON-RPC 2.0).

use std::io::{BufRead, Read, Write};

use mermaidcraft_engine as engine;
use serde_json::{Value, json};

use crate::{Format, info_json, load, ops};

/// Longest request line accepted, in bytes.
const MAX_LINE: usize = 8_000_000;

fn tools() -> Vec<Value> {
    let src_props = json!({
        "path": {"type": "string", "description": "Path of a .mmd, .mermaid or .md file."},
        "source": {"type": "string", "description": "Mermaid code, instead of a path."},
        "block": {"type": "integer", "description": "For .md files: which ```mermaid block (1-based)."}
    });
    vec![
        json!({"name": "diagram_create", "description": "Create a diagram file from Mermaid code, or from a starter template. Supported: flowchart/graph, sequenceDiagram, erDiagram. The code is validated first.",
            "inputSchema": {"type": "object", "required": ["path"], "properties": {
                "path": {"type": "string", "description": "File to create (.mmd, .mermaid or .md)."},
                "source": {"type": "string", "description": "Mermaid code. Leave out for a template."},
                "type": {"type": "string", "enum": ["flowchart", "sequence", "er"], "description": "Template type when no source is given."},
                "overwrite": {"type": "boolean"}, "append": {"type": "boolean", "description": "For .md: add the diagram as a new block."}}}}),
        json!({"name": "flowchart_create", "description": "Create a flowchart file from a structured description (nodes and links) without writing Mermaid by hand.",
            "inputSchema": {"type": "object", "required": ["path", "nodes"], "properties": {
                "path": {"type": "string"}, "direction": {"type": "string", "enum": ["TD", "BT", "LR", "RL"]},
                "nodes": {"type": "array", "items": {"type": "object", "required": ["id"], "properties": {"id": {"type": "string"}, "label": {"type": "string"}, "shape": {"type": "string"}}}},
                "edges": {"type": "array", "items": {"type": "object", "required": ["from", "to"], "properties": {"from": {"type": "string"}, "to": {"type": "string"}, "label": {"type": "string"}, "style": {"type": "string", "enum": ["solid", "dotted", "thick", "invisible"]}, "head_end": {"type": "string", "enum": ["none", "arrow", "circle", "cross"]}}}},
                "overwrite": {"type": "boolean"}}}}),
        json!({"name": "diagram_read", "description": "Read a diagram: its Mermaid code plus type, counts, structure and any errors.",
            "inputSchema": {"type": "object", "properties": src_props}}),
        json!({"name": "diagram_validate", "description": "Check the syntax of a diagram. Returns valid and the errors with line numbers.",
            "inputSchema": {"type": "object", "properties": src_props}}),
        json!({"name": "diagram_info", "description": "Describe a diagram's structure: nodes, edges (with index), subgraphs / participants / entities.",
            "inputSchema": {"type": "object", "properties": src_props}}),
        json!({"name": "diagram_write", "description": "Replace the code of a diagram file (or one ```mermaid block of a .md). The code is validated and rejected when it has errors.",
            "inputSchema": {"type": "object", "required": ["path", "source"], "properties": {
                "path": {"type": "string"}, "source": {"type": "string"}, "block": {"type": "integer"}, "append": {"type": "boolean"}}}}),
        json!({"name": "flowchart_edit", "description": "Edit a flowchart with a list of operations; the text is rewritten keeping comments and formatting. Operations: add_node{id?,label,shape?}, set_node{id,label?,shape?}, remove_node{id}, add_edge{from,to,label?,style?,head_end?}, set_edge{index|from+to,label?,style?,head_end?}, remove_edge{index|from+to}, set_direction{direction}. Without 'path' the new code is only returned.",
            "inputSchema": {"type": "object", "required": ["ops"], "properties": {
                "path": {"type": "string"}, "source": {"type": "string"}, "block": {"type": "integer"},
                "ops": {"type": "array", "items": {"type": "object", "required": ["op"], "properties": {"op": {"type": "string"}}}}}}}),
        json!({"name": "diagram_render", "description": "Draw a diagram to an SVG, PNG or PDF file.",
            "inputSchema": {"type": "object", "required": ["output_path"], "properties": {
                "path": {"type": "string"}, "source": {"type": "string"}, "block": {"type": "integer"},
                "output_path": {"type": "string", "description": "File to write; its extension (.svg, .png, .pdf) picks the format."},
                "format": {"type": "string", "enum": ["svg", "png", "pdf"]}, "scale": {"type": "number", "description": "PNG scale: 1, 2 or 3."}, "look": {"type": "string", "enum": ["sketch", "clean"], "description": "sketch (default): hand-drawn, colourful; clean: classic Mermaid look. The fields below change one part of it."},
                "font": {"type": "string", "enum": ["shantell", "patrick-hand", "kalam", "architects-daughter", "liberation"]},
                "colors": {"type": "string", "enum": ["group", "shape", "neutral", "per-node", "classic"], "description": "group: colour of the subgraph; shape: by the role of the shape (decision, start/end, data)."},
                "fill": {"type": "string", "enum": ["solid", "hachure", "cross-hatch"]},
                "stroke": {"type": "string", "enum": ["clean", "artist", "cartoonist"], "description": "How hand-drawn the lines are."},
                "arrows": {"type": "string", "enum": ["graphite", "source"], "description": "Edge colour: graphite, or the colour of the node it leaves from."}}}}),
        json!({"name": "markdown_blocks", "description": "List the ```mermaid blocks of a Markdown file.",
            "inputSchema": {"type": "object", "required": ["path"], "properties": {"path": {"type": "string"}}}}),
    ]
}

pub fn tool_summaries() -> Value {
    Value::Array(tools().into_iter().map(|t| json!({"name": t["name"], "description": t["description"]})).collect())
}

fn text_result(v: &Value, is_error: bool) -> Value {
    json!({"content": [{"type": "text", "text": serde_json::to_string_pretty(v).unwrap_or_else(|_| "{}".into())}], "isError": is_error})
}

fn err_result(msg: impl Into<String>) -> Value {
    text_result(&json!({"error": msg.into()}), true)
}

fn arg_str<'a>(a: &'a Value, k: &str) -> Option<&'a str> {
    a.get(k).and_then(Value::as_str)
}
fn arg_block(a: &Value) -> Option<usize> {
    a.get("block").and_then(Value::as_u64).and_then(|n| usize::try_from(n).ok())
}
fn arg_bool(a: &Value, k: &str) -> bool {
    a.get(k).and_then(Value::as_bool).unwrap_or(false)
}

/// Mermaid code from `source` or from `path` (+ `block`).
fn code_of(a: &Value) -> Result<String, String> {
    if let Some(s) = arg_str(a, "source") {
        if s.len() > engine::util::MAX_SOURCE_BYTES {
            return Err("the source is too large".into());
        }
        return Ok(s.to_string());
    }
    let path = arg_str(a, "path").ok_or("give 'path' or 'source'")?;
    Ok(load(path, arg_block(a))?.code)
}

fn first_error(src: &str) -> Option<String> {
    let p = engine::parse(src);
    p.errors().next().map(|d| format!("line {}: {}", d.line, d.message))
}

fn call(name: &str, a: &Value) -> Result<Value, String> {
    match name {
        "diagram_create" => {
            let path = arg_str(a, "path").ok_or("missing 'path'")?;
            let code = match arg_str(a, "source") {
                Some(s) => s.to_string(),
                None => engine::edit::template(arg_str(a, "type").unwrap_or("flowchart")).to_string(),
            };
            if let Some(e) = first_error(&code) {
                return Err(format!("the diagram has a syntax error, nothing was written ({e})"));
            }
            if std::path::Path::new(path).exists() && !arg_bool(a, "overwrite") && !arg_bool(a, "append") {
                return Err(format!("'{path}' already exists: pass overwrite=true to replace it"));
            }
            ops::write_code(path, &code, None, arg_bool(a, "append"))?;
            Ok(json!({"path": path, "info": info_json(&code, false)}))
        }
        "flowchart_create" => {
            let path = arg_str(a, "path").ok_or("missing 'path'")?;
            let dir = arg_str(a, "direction").unwrap_or("TD");
            if engine::Direction::parse(dir).is_none() {
                return Err(format!("unknown direction '{dir}'"));
            }
            let mut code = format!("flowchart {dir}\n");
            let nodes = a.get("nodes").and_then(Value::as_array).ok_or("'nodes' must be a list")?;
            if nodes.is_empty() || nodes.len() > engine::util::MAX_NODES {
                return Err("'nodes' must have between 1 and 1500 items".into());
            }
            for n in nodes {
                code = ops::apply_op(&code, &json!({"op": "add_node", "id": n.get("id"), "label": n.get("label").and_then(Value::as_str).unwrap_or(""), "shape": n.get("shape")}))?;
            }
            for e in a.get("edges").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]) {
                let mut op = e.clone();
                if let Some(o) = op.as_object_mut() {
                    o.insert("op".into(), json!("add_edge"));
                }
                code = ops::apply_op(&code, &op)?;
            }
            if let Some(e) = first_error(&code) {
                return Err(format!("the result has a syntax error ({e})"));
            }
            if std::path::Path::new(path).exists() && !arg_bool(a, "overwrite") {
                return Err(format!("'{path}' already exists: pass overwrite=true to replace it"));
            }
            ops::write_code(path, &code, None, false)?;
            Ok(json!({"path": path, "source": code, "info": info_json(&code, false)}))
        }
        "diagram_read" => {
            let code = code_of(a)?;
            let mut info = info_json(&code, true);
            info["source"] = json!(code);
            Ok(info)
        }
        "diagram_validate" => {
            let code = code_of(a)?;
            let v = info_json(&code, false);
            Ok(json!({"valid": v["valid"], "errors": v["errors"], "warnings": v["warnings"]}))
        }
        "diagram_info" => Ok(info_json(&code_of(a)?, true)),
        "diagram_write" => {
            let path = arg_str(a, "path").ok_or("missing 'path'")?;
            let code = arg_str(a, "source").ok_or("missing 'source'")?;
            if code.len() > engine::util::MAX_SOURCE_BYTES {
                return Err("the source is too large".into());
            }
            if let Some(e) = first_error(code) {
                return Err(format!("the diagram has a syntax error, nothing was written ({e})"));
            }
            ops::write_code(path, code, arg_block(a), arg_bool(a, "append"))?;
            Ok(json!({"path": path, "info": info_json(code, false)}))
        }
        "flowchart_edit" => {
            let mut code = code_of(a)?;
            let list = a.get("ops").and_then(Value::as_array).ok_or("'ops' must be a list")?;
            if list.len() > 500 {
                return Err("too many operations in one call (limit 500)".into());
            }
            for (i, op) in list.iter().enumerate() {
                code = ops::apply_op(&code, op).map_err(|e| format!("operation {} failed: {e}", i + 1))?;
            }
            let mut out = json!({"source": code, "info": info_json(&code, true)});
            if let Some(path) = arg_str(a, "path") {
                if arg_str(a, "source").is_none() {
                    if let Some(e) = first_error(&code) {
                        return Err(format!("the edit would leave a syntax error, nothing was written ({e})"));
                    }
                    ops::write_code(path, &code, arg_block(a), false)?;
                    out["written"] = json!(path);
                }
            }
            Ok(out)
        }
        "diagram_render" => {
            let code = code_of(a)?;
            let out_path = arg_str(a, "output_path").ok_or("missing 'output_path'")?;
            ops::check_write_path(out_path)?;
            let format = match arg_str(a, "format") {
                Some(f) => Format::from_name(f).ok_or("format must be svg, png or pdf")?,
                None => Format::from_path(out_path).ok_or("the output must end in .svg, .png or .pdf")?,
            };
            let scale = a.get("scale").and_then(Value::as_f64).unwrap_or(1.0) as f32;
            let s = |k: &str| arg_str(a, k).map(str::to_string);
            let look = crate::LookArgs { look: s("look"), font: s("font"), colors: s("colors"), fill: s("fill"), stroke: s("stroke"), arrows: s("arrows") }.options()?;
            let bytes = crate::render_bytes_with(&code, format, scale, &look)?;
            std::fs::write(out_path, &bytes).map_err(|e| format!("cannot write '{out_path}': {e}"))?;
            Ok(json!({"path": out_path, "bytes": bytes.len()}))
        }
        "markdown_blocks" => {
            let path = arg_str(a, "path").ok_or("missing 'path'")?;
            let text = crate::read_file(path)?;
            let blocks = engine::markdown::find_blocks(&text);
            Ok(json!({"blocks": blocks.iter().enumerate().map(|(i, b)| json!({"block": i + 1, "line": b.fence_line, "first_line": engine::markdown::block_title(&text, b), "type": engine::parse(engine::markdown::block_text(&text, b)).diagram.map(|d| d.kind_name())})).collect::<Vec<_>>()}))
        }
        other => Err(format!("unknown tool '{other}'")),
    }
}

fn respond(id: &Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

fn error(id: &Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

/// Handles one JSON-RPC message; `None` for notifications.
pub fn handle(msg: &Value) -> Option<Value> {
    let method = msg.get("method").and_then(Value::as_str)?;
    let id = msg.get("id").cloned();
    let Some(id) = id else {
        // Notification: nothing to answer.
        return None;
    };
    let params = msg.get("params").cloned().unwrap_or(Value::Null);
    Some(match method {
        "initialize" => {
            let wanted = params.get("protocolVersion").and_then(Value::as_str).unwrap_or("2025-03-26");
            let version = if ["2024-11-05", "2025-03-26", "2025-06-18"].contains(&wanted) { wanted } else { "2025-06-18" };
            respond(
                &id,
                json!({"protocolVersion": version, "capabilities": {"tools": {"listChanged": false}}, "serverInfo": {"name": "mermaidcraft", "version": crate::VERSION},
                    "instructions": "MermaidCraft: create, read, validate, edit and render Mermaid diagrams (flowchart, sequenceDiagram, erDiagram) without a browser."}),
            )
        }
        "ping" => respond(&id, json!({})),
        "tools/list" => respond(&id, json!({"tools": tools()})),
        "tools/call" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
            if !args.is_object() {
                return Some(respond(&id, err_result("'arguments' must be an object")));
            }
            let result = match call(name, &args) {
                Ok(v) => text_result(&v, false),
                Err(e) => err_result(e),
            };
            respond(&id, result)
        }
        "resources/list" => respond(&id, json!({"resources": []})),
        "prompts/list" => respond(&id, json!({"prompts": []})),
        _ => error(&id, -32601, "method not found"),
    })
}

/// Runs the server until standard input closes.
pub fn serve() -> Result<i32, String> {
    let stdin = std::io::stdin();
    let mut reader = stdin.lock();
    let stdout = std::io::stdout();
    let mut buf: Vec<u8> = Vec::new();
    loop {
        buf.clear();
        let mut limited = (&mut reader).take(MAX_LINE as u64 + 1);
        let n = limited.read_until(b'\n', &mut buf).map_err(|e| format!("cannot read: {e}"))?;
        if n == 0 {
            return Ok(0);
        }
        if buf.len() > MAX_LINE {
            // Drop the rest of the oversized line.
            while !buf.ends_with(b"\n") {
                buf.clear();
                let m = (&mut reader).take(1_000_000).read_until(b'\n', &mut buf).map_err(|e| format!("cannot read: {e}"))?;
                if m == 0 {
                    break;
                }
            }
            write_line(&stdout, &error(&Value::Null, -32600, "request too large"))?;
            continue;
        }
        let line = String::from_utf8_lossy(&buf);
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(line) {
            Ok(Value::Array(items)) => {
                let replies: Vec<Value> = items.iter().filter_map(handle).collect();
                if replies.is_empty() { None } else { Some(Value::Array(replies)) }
            }
            Ok(v) => handle(&v),
            Err(_) => Some(error(&Value::Null, -32700, "parse error")),
        };
        if let Some(r) = reply {
            write_line(&stdout, &r)?;
        }
    }
}

fn write_line(stdout: &std::io::Stdout, v: &Value) -> Result<(), String> {
    let mut out = stdout.lock();
    let text = serde_json::to_string(v).map_err(|e| e.to_string())?;
    out.write_all(text.as_bytes()).and_then(|()| out.write_all(b"\n")).and_then(|()| out.flush()).map_err(|e| format!("cannot write: {e}"))
}
