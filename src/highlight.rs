//! Syntax highlighting for the text editor.

use std::sync::Arc;

use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontId};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Plain,
    Comment,
    Keyword,
    Link,
    Str,
    Number,
    Bracket,
    Op,
}

const KEYWORDS: &[&str] = &[
    "graph", "flowchart", "subgraph", "end", "direction", "style", "classDef", "class", "click", "linkStyle", "participant", "actor", "as", "autonumber", "activate", "deactivate", "Note",
    "note", "over", "left", "right", "of", "loop", "alt", "else", "opt", "par", "and", "critical", "option", "break", "rect", "sequenceDiagram", "erDiagram", "title", "box", "create",
    "destroy", "TD", "TB", "BT", "LR", "RL", "PK", "FK", "UK",
];

fn palette(k: Kind, dark: bool) -> Color32 {
    match (k, dark) {
        (Kind::Plain, false) => Color32::from_rgb(40, 44, 52),
        (Kind::Plain, true) => Color32::from_rgb(220, 223, 228),
        (Kind::Comment, false) => Color32::from_rgb(134, 142, 150),
        (Kind::Comment, true) => Color32::from_rgb(130, 138, 150),
        (Kind::Keyword, false) => Color32::from_rgb(124, 58, 237),
        (Kind::Keyword, true) => Color32::from_rgb(190, 150, 255),
        (Kind::Link, false) => Color32::from_rgb(11, 114, 133),
        (Kind::Link, true) => Color32::from_rgb(90, 200, 220),
        (Kind::Str, false) => Color32::from_rgb(43, 138, 62),
        (Kind::Str, true) => Color32::from_rgb(130, 210, 140),
        (Kind::Number, false) => Color32::from_rgb(217, 72, 15),
        (Kind::Number, true) => Color32::from_rgb(255, 160, 100),
        (Kind::Bracket, false) => Color32::from_rgb(73, 80, 87),
        (Kind::Bracket, true) => Color32::from_rgb(170, 178, 190),
        (Kind::Op, false) => Color32::from_rgb(201, 42, 42),
        (Kind::Op, true) => Color32::from_rgb(255, 120, 120),
    }
}

fn tokenize(line: &str) -> Vec<(usize, usize, Kind)> {
    let mut out: Vec<(usize, usize, Kind)> = Vec::new();
    let chars: Vec<(usize, char)> = line.char_indices().collect();
    let at = |i: usize| chars.get(i).map(|c| c.1);
    let byte = |i: usize| chars.get(i).map_or(line.len(), |c| c.0);
    let mut i = 0usize;
    while i < chars.len() {
        let c = at(i).unwrap_or(' ');
        let start = i;
        if c == '%' && at(i + 1) == Some('%') {
            out.push((byte(i), line.len(), Kind::Comment));
            return out;
        }
        if c == '"' {
            i += 1;
            while i < chars.len() && at(i) != Some('"') {
                i += 1;
            }
            i = (i + 1).min(chars.len());
            out.push((byte(start), byte(i), Kind::Str));
            continue;
        }
        if c.is_alphabetic() || c == '_' {
            while i < chars.len() && at(i).is_some_and(|x| x.is_alphanumeric() || x == '_') {
                i += 1;
            }
            let w = line.get(byte(start)..byte(i)).unwrap_or("");
            let kind = if KEYWORDS.contains(&w) { Kind::Keyword } else { Kind::Plain };
            out.push((byte(start), byte(i), kind));
            continue;
        }
        if c.is_ascii_digit() {
            while i < chars.len() && at(i).is_some_and(|x| x.is_ascii_digit()) {
                i += 1;
            }
            out.push((byte(start), byte(i), Kind::Number));
            continue;
        }
        // ER cardinalities (`||--o{`) and flowchart links (`-->`, `-.->`, `==>`, `~~~`).
        if matches!(c, '|' | '}' | '-' | '=' | '~' | '<' | '.') {
            let er = matches!(c, '|' | '}');
            let set: &str = if er { "|}{o-." } else { "-=.~<>" };
            while i < chars.len() && at(i).is_some_and(|x| set.contains(x)) {
                i += 1;
            }
            if !er && matches!(at(i), Some('x' | 'o')) && at(i + 1).is_none_or(char::is_whitespace) {
                i += 1;
            }
            let run = line.get(byte(start)..byte(i)).unwrap_or("");
            let is_link = if er { run.contains("--") || run.contains("..") } else { run.chars().count() >= 2 && run != ".." };
            out.push((byte(start), byte(i), if is_link { Kind::Link } else { Kind::Op }));
            if i == start {
                i += 1;
            }
            continue;
        }
        i += 1;
        let kind = match c {
            '[' | ']' | '(' | ')' | '{' | '}' | '>' => Kind::Bracket,
            ':' | ';' | '&' | '|' | ',' | '+' => Kind::Op,
            _ => Kind::Plain,
        };
        out.push((byte(start), byte(i), kind));
    }
    out
}

/// Builds the coloured text layout of the whole document.
pub fn layout(text: &str, dark: bool, font: FontId) -> Arc<LayoutJob> {
    let mut job = LayoutJob::default();
    job.wrap.max_width = f32::INFINITY;
    let fmt = |k: Kind| TextFormat { font_id: font.clone(), color: palette(k, dark), italics: k == Kind::Comment, ..TextFormat::default() };
    for raw in text.split_inclusive('\n') {
        let (line, nl) = match raw.strip_suffix('\n') {
            Some(l) => (l, "\n"),
            None => (raw, ""),
        };
        let mut pos = 0usize;
        for (s, e, k) in tokenize(line) {
            if s > pos {
                job.append(line.get(pos..s).unwrap_or(""), 0.0, fmt(Kind::Plain));
            }
            if e > s {
                job.append(line.get(s..e).unwrap_or(""), 0.0, fmt(k));
            }
            pos = pos.max(e);
        }
        if pos < line.len() {
            job.append(line.get(pos..).unwrap_or(""), 0.0, fmt(Kind::Plain));
        }
        if !nl.is_empty() {
            job.append(nl, 0.0, fmt(Kind::Plain));
        }
    }
    if text.is_empty() {
        job.append("", 0.0, fmt(Kind::Plain));
    }
    Arc::new(job)
}
