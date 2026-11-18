//! MermaidCraft command line and MCP server. The same functions serve the CLI commands and the
//! MCP tools, so what an agent does matches what a person gets at the terminal.

pub mod mcp;
pub mod ops;

use std::io::Read;

use mermaidcraft_engine as engine;
use serde_json::{Value, json};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Longest file read, in bytes.
pub const MAX_READ: u64 = 4_000_000;

/// The commands, for `commands` and the help text.
pub fn command_list() -> Value {
    json!([
        {"name": "info", "usage": "info <file|-> [--block N] [--detail]", "description": "Diagram type, node and edge counts, errors and warnings, as JSON."},
        {"name": "check", "usage": "check <file|-> [--block N] [--json]", "description": "Validate the syntax. Exit code 1 when there are errors."},
        {"name": "render", "usage": "render <file|-> -o <out.svg|png|pdf> [--format svg|png|pdf] [--scale 1|2|3] [--look sketch|clean] [--block N]", "description": "Draw the diagram to SVG, PNG or PDF."},
        {"name": "convert", "usage": "convert <in> <out> [--scale N] [--block N]", "description": "Convert between .mmd, .md (one block), .svg, .png and .pdf by file extension."},
        {"name": "new", "usage": "new <flowchart|sequence|er> [-o file.mmd]", "description": "Write a starter diagram."},
        {"name": "commands", "usage": "commands", "description": "List the commands and the MCP tools, as JSON."},
        {"name": "mcp", "usage": "mcp", "description": "Run the MCP server on stdio (tools to create, read, validate, edit and render diagrams)."}
    ])
}

/// An input read from a path, `-` (standard input) or text.
pub struct Loaded {
    /// The Mermaid code.
    pub code: String,
    /// For Markdown files: the whole file and the chosen block.
    pub markdown: Option<(String, engine::markdown::MdBlock)>,
}

/// Reads a file safely: size-limited and as UTF-8.
pub fn read_file(path: &str) -> Result<String, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("cannot read '{path}': {e}"))?;
    if !meta.is_file() {
        return Err(format!("'{path}' is not a file"));
    }
    if meta.len() > MAX_READ {
        return Err(format!("'{path}' is too large (limit {} KB)", MAX_READ / 1000));
    }
    let bytes = std::fs::read(path).map_err(|e| format!("cannot read '{path}': {e}"))?;
    String::from_utf8(bytes).map_err(|_| format!("'{path}' is not valid UTF-8 text"))
}

pub fn is_markdown(path: &str) -> bool {
    let l = path.to_ascii_lowercase();
    l.ends_with(".md") || l.ends_with(".markdown")
}

/// Loads Mermaid code from a path (`-` is standard input); Markdown needs `block` (1-based) when
/// it has more than one mermaid block.
pub fn load(path: &str, block: Option<usize>) -> Result<Loaded, String> {
    let text = if path == "-" {
        let mut s = String::new();
        std::io::stdin().take(MAX_READ + 1).read_to_string(&mut s).map_err(|e| format!("cannot read standard input: {e}"))?;
        if s.len() as u64 > MAX_READ {
            return Err("the input is too large".into());
        }
        s
    } else {
        read_file(path)?
    };
    if path != "-" && is_markdown(path) {
        let blocks = engine::markdown::find_blocks(&text);
        let idx = match (block, blocks.len()) {
            (_, 0) => return Err(format!("'{path}' has no ```mermaid blocks")),
            (Some(n), len) if n >= 1 && n <= len => n - 1,
            (Some(n), len) => return Err(format!("block {n} does not exist: the file has {len}")),
            (None, 1) => 0,
            (None, _) => {
                let list: Vec<String> = blocks.iter().enumerate().map(|(i, b)| format!("{}: {}", i + 1, engine::markdown::block_title(&text, b))).collect();
                return Err(format!("the file has {} mermaid blocks; choose one with --block N\n{}", blocks.len(), list.join("\n")));
            }
        };
        let b = blocks.get(idx).cloned().ok_or("block not found")?;
        let code = engine::markdown::block_text(&text, &b).to_string();
        return Ok(Loaded { code, markdown: Some((text, b)) });
    }
    Ok(Loaded { code: text, markdown: None })
}

pub fn diag_json(d: &engine::Diagnostic) -> Value {
    json!({"line": d.line, "column": d.column, "message": d.message})
}

/// `info`: type, counts and problems.
pub fn info_json(src: &str, detail: bool) -> Value {
    let p = engine::parse(src);
    let errors: Vec<Value> = p.diagnostics.iter().filter(|d| d.severity == engine::Severity::Error).map(diag_json).collect();
    let warnings: Vec<Value> = p.diagnostics.iter().filter(|d| d.severity == engine::Severity::Warning).map(diag_json).collect();
    let mut v = match &p.diagram {
        Some(d) => {
            let s = engine::stats(d);
            let mut v = json!({"type": s.kind, "nodes": s.nodes, "edges": s.edges});
            if let engine::Diagram::Flowchart(f) = d {
                v["direction"] = json!(f.direction.code());
                v["subgraphs"] = json!(f.subgraphs.len());
            }
            if detail {
                v["detail"] = detail_json(d);
            }
            v
        }
        None => json!({"type": null, "nodes": 0, "edges": 0}),
    };
    v["valid"] = json!(errors.is_empty());
    v["errors"] = json!(errors);
    v["warnings"] = json!(warnings);
    v
}

fn detail_json(d: &engine::Diagram) -> Value {
    match d {
        engine::Diagram::Flowchart(f) => json!({
            "nodes": f.nodes.iter().map(|n| json!({"id": n.id, "label": n.label, "shape": n.shape.name(), "line": n.line, "subgraph": n.parent.and_then(|p| f.subgraphs.get(p)).map(|s| s.id.clone())})).collect::<Vec<_>>(),
            "edges": f.edges.iter().enumerate().map(|(i, e)| json!({"index": i, "from": e.from, "to": e.to, "label": e.label, "style": e.kind.style.name(), "head_start": e.kind.head_start.name(), "head_end": e.kind.head_end.name(), "line": e.line})).collect::<Vec<_>>(),
            "subgraphs": f.subgraphs.iter().map(|s| json!({"id": s.id, "title": s.title, "line": s.line})).collect::<Vec<_>>(),
        }),
        engine::Diagram::Sequence(s) => json!({
            "participants": s.participants.iter().map(|p| json!({"id": p.id, "label": p.label, "actor": p.actor})).collect::<Vec<_>>(),
            "messages": s.items.iter().filter_map(|i| match i {
                engine::SeqItem::Message { from, to, text, line, .. } => Some(json!({"from": s.participants.get(*from).map(|p| p.id.clone()), "to": s.participants.get(*to).map(|p| p.id.clone()), "text": text, "line": line})),
                _ => None,
            }).collect::<Vec<_>>(),
        }),
        engine::Diagram::Er(e) => json!({
            "entities": e.entities.iter().map(|x| json!({"id": x.id, "label": x.label, "attributes": x.attributes.iter().map(|a| json!({"type": a.ty, "name": a.name, "keys": a.keys, "comment": a.comment})).collect::<Vec<_>>()})).collect::<Vec<_>>(),
            "relations": e.relations.iter().map(|r| json!({"from": e.entities.get(r.a).map(|x| x.id.clone()), "to": e.entities.get(r.b).map(|x| x.id.clone()), "label": r.label, "identifying": r.identifying, "line": r.line})).collect::<Vec<_>>(),
        }),
    }
}

/// Output formats of `render`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Svg,
    Png,
    Pdf,
}

impl Format {
    pub fn from_name(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "svg" => Some(Self::Svg),
            "png" => Some(Self::Png),
            "pdf" => Some(Self::Pdf),
            _ => None,
        }
    }
    pub fn from_path(p: &str) -> Option<Self> {
        let l = p.to_ascii_lowercase();
        let ext = l.rsplit('.').next()?;
        Self::from_name(ext)
    }
}

/// Draws source text to the bytes of a file with the default (hand-drawn, colourful) look.
pub fn render_bytes(src: &str, format: Format, scale: f32) -> Result<Vec<u8>, String> {
    render_bytes_with(src, format, scale, &engine::RenderOptions::default())
}

/// The look from a name: `sketch` (default) or `clean`.
pub fn look_from_name(name: &str) -> Result<engine::RenderOptions, String> {
    match name.to_ascii_lowercase().as_str() {
        "sketch" | "default" => Ok(engine::RenderOptions::default()),
        "clean" | "plain" => Ok(engine::RenderOptions::clean()),
        other => Err(format!("unknown look '{other}': use sketch or clean")),
    }
}

pub fn render_bytes_with(src: &str, format: Format, scale: f32, opts: &engine::RenderOptions) -> Result<Vec<u8>, String> {
    match engine::render_source_with(src, opts) {
        Ok((scene, diags)) => {
            if diags.iter().any(|d| d.severity == engine::Severity::Error) {
                let first = diags.iter().find(|d| d.severity == engine::Severity::Error);
                if let Some(d) = first {
                    return Err(format!("syntax error on line {}: {}", d.line, d.message));
                }
            }
            let svg = scene.to_svg();
            match format {
                Format::Svg => Ok(svg.into_bytes()),
                Format::Png => engine::export::svg_to_png(&svg, scale),
                Format::Pdf => engine::export::svg_to_pdf(&svg),
            }
        }
        Err(diags) => {
            let d = diags.first().map_or("nothing to draw".to_string(), |d| format!("line {}: {}", d.line, d.message));
            Err(d)
        }
    }
}

struct Args {
    positional: Vec<String>,
    output: Option<String>,
    format: Option<String>,
    scale: Option<f32>,
    block: Option<usize>,
    look: Option<String>,
    json: bool,
    detail: bool,
}

fn parse_args(args: &[String]) -> Result<Args, String> {
    let mut a = Args { positional: Vec::new(), output: None, format: None, scale: None, block: None, look: None, json: false, detail: false };
    let mut it = args.iter();
    while let Some(x) = it.next() {
        let mut value = |name: &str| it.next().cloned().ok_or_else(|| format!("{name} needs a value"));
        match x.as_str() {
            "-o" | "--output" => a.output = Some(value("-o")?),
            "--format" | "-f" => a.format = Some(value("--format")?),
            "--scale" | "-s" => {
                let v = value("--scale")?;
                a.scale = Some(v.trim_end_matches('x').parse::<f32>().map_err(|_| format!("'{v}' is not a number"))?);
            }
            "--block" | "-b" => {
                let v = value("--block")?;
                a.block = Some(v.parse::<usize>().map_err(|_| format!("'{v}' is not a block number"))?);
            }
            "--look" => a.look = Some(value("--look")?),
            "--json" => a.json = true,
            "--detail" => a.detail = true,
            s if s.starts_with('-') && s != "-" => return Err(format!("unknown option '{s}'")),
            _ => a.positional.push(x.clone()),
        }
    }
    Ok(a)
}

const HELP: &str = "mermaidcraft-cli: read, check, convert and render Mermaid diagrams

Usage: mermaidcraft-cli <command> [options]

Commands:
  info <file|->      type, nodes, edges and errors as JSON (--detail lists them)
  check <file|->     validate the syntax (exit 1 on errors; --json for JSON)
  render <file|->    draw to SVG, PNG or PDF: -o <out> [--format F] [--scale 1|2|3] [--look sketch|clean]
  convert <in> <out> convert by extension (.mmd .md .svg .png .pdf)
  new <type>         write a starter diagram (flowchart, sequence, er) [-o file]
  commands           list commands and MCP tools as JSON
  mcp                run the MCP server on stdio

A Markdown file is read through its ```mermaid blocks: pick one with --block N.
Supported diagrams: flowchart/graph, sequenceDiagram, erDiagram.";

fn write_output(path: &str, bytes: &[u8]) -> Result<(), String> {
    if path == "-" {
        use std::io::Write;
        std::io::stdout().write_all(bytes).map_err(|e| format!("cannot write: {e}"))
    } else {
        std::fs::write(path, bytes).map_err(|e| format!("cannot write '{path}': {e}"))
    }
}

/// Runs the CLI; returns the exit code.
pub fn run(argv: &[String]) -> i32 {
    match run_inner(argv) {
        Ok(code) => code,
        Err(msg) => {
            eprintln!("mermaidcraft-cli: {msg}");
            2
        }
    }
}

fn run_inner(argv: &[String]) -> Result<i32, String> {
    let Some(cmd) = argv.first() else {
        println!("{HELP}");
        return Ok(0);
    };
    let rest = argv.get(1..).unwrap_or(&[]);
    match cmd.as_str() {
        "-h" | "--help" | "help" => {
            println!("{HELP}");
            Ok(0)
        }
        "-V" | "--version" | "version" => {
            println!("mermaidcraft-cli {VERSION}");
            Ok(0)
        }
        "commands" => {
            let mut v = json!({"commands": command_list()});
            v["mcp_tools"] = mcp::tool_summaries();
            println!("{}", serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?);
            Ok(0)
        }
        "mcp" => mcp::serve(),
        "new" => {
            let a = parse_args(rest)?;
            let kind = a.positional.first().map_or("flowchart", String::as_str);
            let text = engine::edit::template(kind);
            match &a.output {
                Some(o) => write_output(o, text.as_bytes())?,
                None => print!("{text}"),
            }
            Ok(0)
        }
        "info" | "check" => {
            let a = parse_args(rest)?;
            let path = a.positional.first().ok_or("missing the file (or '-' for standard input)")?;
            let loaded = load(path, a.block)?;
            if cmd == "info" {
                let v = info_json(&loaded.code, a.detail);
                println!("{}", serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?);
                Ok(0)
            } else {
                let p = engine::parse(&loaded.code);
                let errs = p.errors().count();
                if a.json {
                    let v = info_json(&loaded.code, false);
                    println!("{}", serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?);
                } else if p.diagnostics.is_empty() {
                    println!("ok");
                } else {
                    for d in &p.diagnostics {
                        let kind = if d.severity == engine::Severity::Error { "error" } else { "warning" };
                        println!("{path}:{}:{}: {kind}: {}", d.line, d.column.max(1), d.message);
                    }
                }
                Ok(i32::from(errs > 0))
            }
        }
        "render" | "convert" => {
            let a = parse_args(rest)?;
            let (input, output) = if cmd == "convert" {
                (a.positional.first().ok_or("missing the input file")?.clone(), a.positional.get(1).cloned().or(a.output.clone()).ok_or("missing the output file")?)
            } else {
                (a.positional.first().ok_or("missing the file (or '-' for standard input)")?.clone(), a.output.clone().ok_or("missing -o <output file>")?)
            };
            let scale = a.scale.unwrap_or(1.0);
            // Output kind: --format, else the extension (`.mmd` / `.mermaid` writes the code).
            let lower_out = output.to_ascii_lowercase();
            let wants_code = lower_out.ends_with(".mmd") || lower_out.ends_with(".mermaid");
            let format = match &a.format {
                Some(f) => Some(Format::from_name(f).ok_or_else(|| format!("unknown format '{f}': use svg, png or pdf"))?),
                None => Format::from_path(&output),
            };
            // An SVG input is only rasterised.
            if input.to_ascii_lowercase().ends_with(".svg") && input != "-" {
                let svg = read_file(&input)?;
                let bytes = match format {
                    Some(Format::Png) => engine::export::svg_to_png(&svg, scale)?,
                    Some(Format::Pdf) => engine::export::svg_to_pdf(&svg)?,
                    Some(Format::Svg) => svg.into_bytes(),
                    None => return Err("the output must end in .png, .pdf or .svg".into()),
                };
                write_output(&output, &bytes)?;
                return Ok(0);
            }
            let loaded = load(&input, a.block)?;
            if wants_code {
                write_output(&output, loaded.code.as_bytes())?;
                return Ok(0);
            }
            let format = format.ok_or("cannot tell the output format: use -o file.svg|png|pdf or --format")?;
            let look = look_from_name(a.look.as_deref().unwrap_or("sketch"))?;
            let bytes = render_bytes_with(&loaded.code, format, scale, &look)?;
            write_output(&output, &bytes)?;
            Ok(0)
        }
        other => Err(format!("unknown command '{other}' (try --help)")),
    }
}
