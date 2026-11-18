//! erDiagram parser.

use std::collections::HashMap;

use crate::model::*;
use crate::util::*;

struct Builder {
    er: ErDiagram,
    index: HashMap<String, usize>,
    diags: Vec<Diagnostic>,
}

impl Builder {
    fn entity(&mut self, name: &str, line: usize) -> Option<usize> {
        if let Some(i) = self.index.get(name).copied() {
            return Some(i);
        }
        if self.er.entities.len() >= MAX_NODES {
            self.diags.push(Diagnostic::error(line, 0, format!("Too many entities (limit {MAX_NODES})")));
            return None;
        }
        let i = self.er.entities.len();
        self.er.entities.push(ErEntity { id: name.to_string(), label: name.to_string(), attributes: Vec::new(), line });
        self.index.insert(name.to_string(), i);
        Some(i)
    }
}

/// Reads an entity name (a word or a quoted string) from the start of `s`; returns it and the rest.
fn read_name(s: &str) -> Option<(String, &str)> {
    let s = s.trim_start();
    if let Some(r) = s.strip_prefix('"') {
        let q = r.find('"')?;
        return Some((r.get(..q)?.to_string(), r.get(q + 1..)?));
    }
    let end = s.find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '-' || c == '.')).unwrap_or(s.len());
    if end == 0 {
        return None;
    }
    Some((s.get(..end)?.to_string(), s.get(end..)?))
}

fn left_card(t: &str) -> Option<Cardinality> {
    match t {
        "||" => Some(Cardinality::ExactlyOne),
        "|o" => Some(Cardinality::ZeroOrOne),
        "}o" => Some(Cardinality::ZeroOrMore),
        "}|" => Some(Cardinality::OneOrMore),
        _ => None,
    }
}

fn right_card(t: &str) -> Option<Cardinality> {
    match t {
        "||" => Some(Cardinality::ExactlyOne),
        "o|" => Some(Cardinality::ZeroOrOne),
        "o{" => Some(Cardinality::ZeroOrMore),
        "|{" => Some(Cardinality::OneOrMore),
        _ => None,
    }
}

fn parse_attribute(line: &str) -> Option<ErAttribute> {
    let mut comment = String::new();
    let mut body = line.trim();
    if let Some(q) = body.find('"') {
        let tail = body.get(q + 1..).unwrap_or("");
        comment = tail.trim_end_matches('"').trim().to_string();
        body = body.get(..q).unwrap_or("").trim();
    }
    let mut parts = body.split_whitespace();
    let ty = parts.next()?.to_string();
    let name = parts.next()?.to_string();
    let mut keys = Vec::new();
    for p in parts {
        for k in p.split(',') {
            let k = k.trim();
            if !k.is_empty() {
                keys.push(k.to_string());
            }
        }
    }
    Some(ErAttribute { ty, name, keys, comment })
}

pub fn parse(lines: &[Line<'_>], first: usize, diags: &mut Vec<Diagnostic>) -> ErDiagram {
    let mut b = Builder { er: ErDiagram::default(), index: HashMap::new(), diags: Vec::new() };
    let mut open: Option<usize> = None;
    for line in lines.iter().skip(first + 1) {
        let code = strip_comment(line.text);
        let (lead, text) = trim_with_offset(code);
        if text.is_empty() {
            continue;
        }
        let col = lead + 1;
        let ln = line.no;
        let text = text.trim_end_matches(';').trim_end();
        if let Some(ent) = open {
            // Inside an attribute block.
            let (body, closes) = match text.find('}') {
                Some(i) => (text.get(..i).unwrap_or(""), true),
                None => (text, false),
            };
            if !body.trim().is_empty() {
                match parse_attribute(body) {
                    Some(a) => {
                        if let Some(e) = b.er.entities.get_mut(ent) {
                            if e.attributes.len() < 500 {
                                e.attributes.push(a);
                            }
                        }
                    }
                    None => b.diags.push(Diagnostic::error(ln, col, "An attribute needs a type and a name, for example 'string name'")),
                }
            }
            if closes {
                open = None;
            }
            continue;
        }
        if let Some(d) = text.strip_prefix("direction") {
            match Direction::parse(d.trim()) {
                Some(dir) => b.er.direction = dir,
                None => b.diags.push(Diagnostic::error(ln, col, "Unknown direction")),
            }
            continue;
        }
        if text.starts_with("title") || text.starts_with("accTitle") || text.starts_with("accDescr") || text.starts_with("style ") || text.starts_with("classDef ") || text.starts_with("class ") {
            continue;
        }
        let Some((name, rest)) = read_name(text) else {
            let shown: String = text.chars().take(30).collect();
            b.diags.push(Diagnostic::error(ln, col, format!("Unrecognised statement '{shown}'")));
            continue;
        };
        let rest_t = rest.trim_start();
        // Entity block: `NAME {` (optionally `NAME["alias"] {`), or an alias on its own.
        if rest_t.starts_with('{') || rest_t.starts_with('[') {
            let Some(idx) = b.entity(&name, ln) else { continue };
            let mut r = rest_t;
            if let Some(inner) = r.strip_prefix('[') {
                match inner.find(']') {
                    Some(e) => {
                        if let Some(en) = b.er.entities.get_mut(idx) {
                            en.label = clean_label(inner.get(..e).unwrap_or(""));
                        }
                        r = inner.get(e + 1..).unwrap_or("").trim_start();
                    }
                    None => {
                        b.diags.push(Diagnostic::error(ln, col, "Unclosed alias: expected ']'"));
                        continue;
                    }
                }
            }
            if let Some(body) = r.strip_prefix('{') {
                let (inside, closes) = match body.find('}') {
                    Some(i) => (body.get(..i).unwrap_or(""), true),
                    None => (body, false),
                };
                if !inside.trim().is_empty() {
                    // A one-line block holds a single attribute.
                    match parse_attribute(inside) {
                        Some(a) => {
                            if let Some(e) = b.er.entities.get_mut(idx) {
                                e.attributes.push(a);
                            }
                        }
                        None => b.diags.push(Diagnostic::error(ln, col, "An attribute needs a type and a name")),
                    }
                }
                if !closes {
                    open = Some(idx);
                }
            }
            continue;
        }
        if rest_t.is_empty() {
            b.entity(&name, ln);
            continue;
        }
        // Relationship: <card><line><card> ENTITY [: label]
        let t = rest_t;
        let (l, r_after) = (t.get(..2).unwrap_or(""), t.get(2..).unwrap_or(""));
        let line_tok = r_after.get(..2).unwrap_or("");
        let r_card = r_after.get(2..4).unwrap_or("");
        let (Some(ca), Some(cb)) = (left_card(l), right_card(r_card)) else {
            b.diags.push(Diagnostic::error(ln, col, "Invalid relationship: expected a pattern such as '||--o{'"));
            continue;
        };
        if line_tok != "--" && line_tok != ".." {
            b.diags.push(Diagnostic::error(ln, col, "A relationship line is '--' (identifying) or '..' (non-identifying)"));
            continue;
        }
        let after = r_after.get(4..).unwrap_or("");
        let Some((other, tail)) = read_name(after) else {
            b.diags.push(Diagnostic::error(ln, col, "A relationship needs the second entity"));
            continue;
        };
        let label = match tail.trim().strip_prefix(':') {
            Some(l) => clean_label(l),
            None => {
                if !tail.trim().is_empty() {
                    b.diags.push(Diagnostic::error(ln, col, "Expected ': label' after the second entity"));
                    continue;
                }
                String::new()
            }
        };
        if b.er.relations.len() >= MAX_EDGES {
            b.diags.push(Diagnostic::error(ln, col, format!("Too many relationships (limit {MAX_EDGES})")));
            continue;
        }
        let (Some(ia), Some(ib)) = (b.entity(&name, ln), b.entity(&other, ln)) else { continue };
        b.er.relations.push(ErRelation { a: ia, b: ib, card_a: ca, card_b: cb, identifying: line_tok == "--", label, line: ln });
    }
    if open.is_some() {
        b.diags.push(Diagnostic::error(lines.last().map_or(1, |l| l.no), 1, "An entity block is missing its closing '}'"));
    }
    diags.append(&mut b.diags);
    b.er
}
