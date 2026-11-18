//! sequenceDiagram parser.

use std::collections::HashMap;

use crate::model::*;
use crate::util::*;

const ARROWS: &[(&str, bool, SeqHead, bool)] = &[
    // token, dotted, head, both
    ("<<-->>", true, SeqHead::Filled, true),
    ("<<->>", false, SeqHead::Filled, true),
    ("-->>", true, SeqHead::Filled, false),
    ("->>", false, SeqHead::Filled, false),
    ("--x", true, SeqHead::Cross, false),
    ("-x", false, SeqHead::Cross, false),
    ("--)", true, SeqHead::Async, false),
    ("-)", false, SeqHead::Async, false),
    ("-->", true, SeqHead::None, false),
    ("->", false, SeqHead::None, false),
];

struct Builder {
    seq: Sequence,
    index: HashMap<String, usize>,
    depth: Vec<BlockKind>,
    diags: Vec<Diagnostic>,
}

impl Builder {
    fn participant(&mut self, id: &str, label: Option<&str>, actor: Option<bool>, line: usize) -> Option<usize> {
        if let Some(i) = self.index.get(id).copied() {
            if let Some(p) = self.seq.participants.get_mut(i) {
                if let Some(l) = label {
                    p.label = l.to_string();
                }
                if let Some(a) = actor {
                    p.actor = a;
                }
            }
            return Some(i);
        }
        if self.seq.participants.len() >= MAX_NODES {
            self.diags.push(Diagnostic::error(line, 0, format!("Too many participants (limit {MAX_NODES})")));
            return None;
        }
        let i = self.seq.participants.len();
        self.seq.participants.push(Participant { id: id.to_string(), label: label.unwrap_or(id).to_string(), actor: actor.unwrap_or(false) });
        self.index.insert(id.to_string(), i);
        Some(i)
    }

    fn push(&mut self, item: SeqItem, line: usize) {
        if self.seq.items.len() >= MAX_EDGES * 2 {
            self.diags.push(Diagnostic::error(line, 0, "Too many statements"));
            return;
        }
        self.seq.items.push(item);
    }
}

fn find_arrow(left: &str) -> Option<(usize, &'static (&'static str, bool, SeqHead, bool))> {
    let mut best: Option<(usize, &'static (&'static str, bool, SeqHead, bool))> = None;
    for a in ARROWS {
        if let Some(i) = left.find(a.0) {
            let better = match best {
                None => true,
                Some((bi, ba)) => i < bi || (i == bi && a.0.len() > ba.0.len()),
            };
            if better {
                best = Some((i, a));
            }
        }
    }
    best
}

pub fn parse(lines: &[Line<'_>], first: usize, diags: &mut Vec<Diagnostic>) -> Sequence {
    let mut b = Builder { seq: Sequence::default(), index: HashMap::new(), depth: Vec::new(), diags: Vec::new() };
    for line in lines.iter().skip(first + 1) {
        let code = strip_comment(line.text);
        let (lead, text) = trim_with_offset(code);
        if text.is_empty() {
            continue;
        }
        let col = lead + 1;
        let ln = line.no;
        let text = text.trim_end_matches(';').trim_end();
        let (word, rest) = match text.find(char::is_whitespace) {
            Some(i) => (text.get(..i).unwrap_or(""), text.get(i..).unwrap_or("").trim()),
            None => (text, ""),
        };
        match word {
            "autonumber" => b.seq.autonumber = true,
            "participant" | "actor" => {
                let actor = word == "actor";
                let rest = rest.split("@{").next().unwrap_or(rest).trim();
                if rest.is_empty() {
                    b.diags.push(Diagnostic::error(ln, col, format!("'{word}' needs a name")));
                    continue;
                }
                let (id, label) = match rest.split_once(" as ") {
                    Some((i, l)) => (i.trim(), Some(clean_label(l))),
                    None => (rest, None),
                };
                b.participant(id, label.as_deref(), Some(actor), ln);
            }
            "create" | "destroy" => {
                let r = rest.strip_prefix("participant").or_else(|| rest.strip_prefix("actor")).unwrap_or(rest).trim();
                if word == "create" && !r.is_empty() {
                    let (id, label) = match r.split_once(" as ") {
                        Some((i, l)) => (i.trim(), Some(clean_label(l))),
                        None => (r, None),
                    };
                    b.participant(id, label.as_deref(), None, ln);
                }
            }
            "activate" | "deactivate" => match b.participant(rest, None, None, ln) {
                Some(who) => {
                    let item = if word == "activate" { SeqItem::Activate { who } } else { SeqItem::Deactivate { who } };
                    b.push(item, ln);
                }
                None => b.diags.push(Diagnostic::error(ln, col, format!("'{word}' needs a participant"))),
            },
            "title" | "link" | "links" | "properties" | "details" => {}
            "box" => {
                if b.depth.len() >= MAX_DEPTH {
                    b.diags.push(Diagnostic::error(ln, col, "Blocks nested too deeply"));
                } else {
                    b.depth.push(BlockKind::Box);
                }
            }
            "loop" | "alt" | "opt" | "par" | "critical" | "break" | "rect" => {
                if b.depth.len() >= MAX_DEPTH {
                    b.diags.push(Diagnostic::error(ln, col, "Blocks nested too deeply"));
                    continue;
                }
                let kind = match word {
                    "loop" => BlockKind::Loop,
                    "alt" => BlockKind::Alt,
                    "opt" => BlockKind::Opt,
                    "par" => BlockKind::Par,
                    "critical" => BlockKind::Critical,
                    "break" => BlockKind::Break,
                    _ => BlockKind::Rect,
                };
                b.depth.push(kind);
                b.push(SeqItem::BlockStart { kind, label: clean_label(rest), line: ln }, ln);
            }
            "else" | "and" | "option" => {
                if b.depth.is_empty() {
                    b.diags.push(Diagnostic::error(ln, col, format!("'{word}' outside of a block")));
                } else {
                    b.push(SeqItem::BlockElse { label: clean_label(rest), line: ln }, ln);
                }
            }
            "end" if rest.is_empty() => match b.depth.pop() {
                Some(BlockKind::Box) => {}
                Some(_) => b.push(SeqItem::BlockEnd, ln),
                None => b.diags.push(Diagnostic::error(ln, col, "'end' without a matching block")),
            },
            "Note" | "note" => parse_note(&mut b, rest, ln, col),
            _ => parse_message(&mut b, text, ln, col),
        }
    }
    for k in b.depth.iter().rev() {
        if *k != BlockKind::Box {
            b.diags.push(Diagnostic::error(lines.last().map_or(1, |l| l.no), 1, format!("A '{}' block is missing its 'end'", k.word())));
        } else {
            b.diags.push(Diagnostic::error(lines.last().map_or(1, |l| l.no), 1, "A 'box' is missing its 'end'"));
        }
    }
    diags.append(&mut b.diags);
    b.seq
}

fn parse_note(b: &mut Builder, rest: &str, ln: usize, col: usize) {
    let lower = rest.to_ascii_lowercase();
    let (place, after) = if lower.starts_with("left of") {
        (NotePlace::LeftOf, rest.get(7..).unwrap_or(""))
    } else if lower.starts_with("right of") {
        (NotePlace::RightOf, rest.get(8..).unwrap_or(""))
    } else if lower.starts_with("over") {
        (NotePlace::Over, rest.get(4..).unwrap_or(""))
    } else {
        b.diags.push(Diagnostic::error(ln, col, "A note must be 'left of', 'right of' or 'over' a participant"));
        return;
    };
    let Some((who, text)) = after.split_once(':') else {
        b.diags.push(Diagnostic::error(ln, col, "A note needs ': text'"));
        return;
    };
    let mut targets = Vec::new();
    for w in who.split(',') {
        let w = w.trim();
        if w.is_empty() {
            continue;
        }
        if let Some(i) = b.participant(w, None, None, ln) {
            targets.push(i);
        }
    }
    if targets.is_empty() {
        b.diags.push(Diagnostic::error(ln, col, "A note needs a participant"));
        return;
    }
    b.push(SeqItem::Note { place, targets, text: clean_label(text), line: ln }, ln);
}

fn parse_message(b: &mut Builder, text: &str, ln: usize, col: usize) {
    let (left, msg) = match text.split_once(':') {
        Some((l, m)) => (l, Some(m)),
        None => (text, None),
    };
    let Some((pos, arrow)) = find_arrow(left) else {
        let shown: String = text.chars().take(30).collect();
        b.diags.push(Diagnostic::error(ln, col, format!("Unrecognised statement '{shown}': expected a message such as 'A->>B: hello'")));
        return;
    };
    let from = left.get(..pos).unwrap_or("").trim();
    let mut to = left.get(pos + arrow.0.len()..).unwrap_or("").trim();
    let mut activate = false;
    let mut deactivate = false;
    if let Some(t) = to.strip_prefix('+') {
        activate = true;
        to = t.trim();
    } else if let Some(t) = to.strip_prefix('-') {
        deactivate = true;
        to = t.trim();
    }
    if from.is_empty() || to.is_empty() {
        b.diags.push(Diagnostic::error(ln, col, "A message needs a participant on each side of the arrow"));
        return;
    }
    if msg.is_none() {
        b.diags.push(Diagnostic::warning(ln, col, "A message normally has ': text'"));
    }
    let (Some(f), Some(t)) = (b.participant(from, None, None, ln), b.participant(to, None, None, ln)) else {
        return;
    };
    let arrow = SeqArrow { dotted: arrow.1, head: arrow.2, both: arrow.3 };
    b.push(SeqItem::Message { from: f, to: t, text: clean_label(msg.unwrap_or("")), arrow, activate, deactivate, line: ln }, ln);
}
