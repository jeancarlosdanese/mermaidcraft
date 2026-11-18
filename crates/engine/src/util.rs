//! Helpers shared by the parsers: lines, comments, labels, limits.

/// Largest source the engine accepts, in bytes.
pub const MAX_SOURCE_BYTES: usize = 2_000_000;
/// Most lines parsed.
pub const MAX_LINES: usize = 20_000;
/// Most nodes (or participants, entities) in one diagram.
pub const MAX_NODES: usize = 1_500;
/// Most edges (or messages, relations) in one diagram.
pub const MAX_EDGES: usize = 5_000;
/// Deepest nesting of subgraphs and blocks.
pub const MAX_DEPTH: usize = 24;
/// Longest label kept, in characters.
pub const MAX_LABEL_CHARS: usize = 2_000;

/// One line of the source.
#[derive(Clone, Copy, Debug)]
pub struct Line<'a> {
    /// 1-based line number.
    pub no: usize,
    /// Byte offset of the first character of the line.
    pub start: usize,
    /// The text without the line ending.
    pub text: &'a str,
}

/// Splits the source in lines, keeping byte offsets. Stops at `MAX_LINES`.
pub fn split_lines(src: &str) -> Vec<Line<'_>> {
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut no = 1usize;
    for raw in src.split_inclusive('\n') {
        let text = raw.strip_suffix('\n').unwrap_or(raw);
        let text = text.strip_suffix('\r').unwrap_or(text);
        out.push(Line { no, start, text });
        start += raw.len();
        no += 1;
        if out.len() >= MAX_LINES {
            break;
        }
    }
    out
}

/// Index of the first line that is not blank, a `%%` comment or the YAML front matter.
pub fn first_significant(lines: &[Line<'_>]) -> Option<usize> {
    let mut i = 0usize;
    // Front matter: `---` ... `---` at the very top.
    let first_non_blank = lines.iter().position(|l| !l.text.trim().is_empty())?;
    if lines.get(first_non_blank).is_some_and(|l| l.text.trim() == "---") {
        let close = lines.iter().skip(first_non_blank + 1).position(|l| l.text.trim() == "---");
        if let Some(c) = close {
            i = first_non_blank + 1 + c + 1;
        }
    }
    while let Some(l) = lines.get(i) {
        let t = l.text.trim();
        if t.is_empty() || t.starts_with("%%") {
            i += 1;
        } else {
            return Some(i);
        }
    }
    None
}

/// Removes a trailing `%%` comment (outside quotes and brackets) and returns the code part.
pub fn strip_comment(text: &str) -> &str {
    let mut depth = 0i32;
    let mut in_quote = false;
    let bytes = text.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes.get(i).copied().unwrap_or(0);
        match b {
            b'"' => in_quote = !in_quote,
            b'[' | b'(' | b'{' if !in_quote => depth += 1,
            b']' | b')' | b'}' if !in_quote => depth = (depth - 1).max(0),
            b'%' if !in_quote && depth == 0 && bytes.get(i + 1) == Some(&b'%') => {
                return text.get(..i).unwrap_or(text);
            }
            _ => {}
        }
        i += 1;
    }
    text
}

/// Splits one line in statements at `;` (outside quotes and brackets). Returns (offset in line, text).
pub fn split_statements(code: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut in_quote = false;
    let mut start = 0usize;
    for (i, c) in code.char_indices() {
        match c {
            '"' => in_quote = !in_quote,
            '[' | '(' | '{' if !in_quote => depth += 1,
            ']' | ')' | '}' if !in_quote => depth = (depth - 1).max(0),
            ';' if !in_quote && depth == 0 => {
                out.push((start, code.get(start..i).unwrap_or("")));
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push((start, code.get(start..).unwrap_or("")));
    out
}

/// Trims a statement and returns (extra offset, trimmed text).
pub fn trim_with_offset(s: &str) -> (usize, &str) {
    let lead = s.len() - s.trim_start().len();
    (lead, s.trim())
}

/// Decodes `#quot;`, `#35;` style entities.
fn decode_entities(s: &str) -> String {
    if !s.contains('#') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0usize;
    while i < chars.len() {
        let c = chars.get(i).copied().unwrap_or(' ');
        if c == '#' {
            // Look for `;` within a short distance.
            let mut j = i + 1;
            let mut name = String::new();
            while j < chars.len() && j < i + 10 {
                let d = chars.get(j).copied().unwrap_or(' ');
                if d == ';' {
                    break;
                }
                name.push(d);
                j += 1;
            }
            if chars.get(j) == Some(&';') && !name.is_empty() {
                let decoded = match name.as_str() {
                    "quot" => Some('"'),
                    "amp" => Some('&'),
                    "lt" => Some('<'),
                    "gt" => Some('>'),
                    "nbsp" => Some(' '),
                    n => n.parse::<u32>().ok().and_then(char::from_u32),
                };
                if let Some(d) = decoded {
                    out.push(d);
                    i = j + 1;
                    continue;
                }
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

/// Cleans label text: quotes, markdown backticks, line breaks, simple HTML tags, entities.
pub fn clean_label(raw: &str) -> String {
    let mut t = raw.trim();
    if t.len() >= 2 && t.starts_with('"') && t.ends_with('"') {
        t = t.get(1..t.len() - 1).unwrap_or(t).trim();
    }
    if t.len() >= 4 && t.starts_with('`') && t.ends_with('`') {
        t = t.get(1..t.len() - 1).unwrap_or(t);
    }
    // Line breaks.
    let mut s = String::with_capacity(t.len());
    let lower = t.to_ascii_lowercase();
    let b = t.as_bytes();
    let mut i = 0usize;
    while i < b.len() {
        if b.get(i) == Some(&b'<') {
            // <br>, <br/>, <br />
            let rest = lower.get(i..).unwrap_or("");
            if let Some(end) = rest.find('>') {
                let inner = rest.get(1..end).unwrap_or("").trim().trim_end_matches('/').trim();
                if inner == "br" {
                    s.push('\n');
                    i += end + 1;
                    continue;
                }
                let name = inner.trim_start_matches('/');
                if matches!(name, "b" | "i" | "u" | "strong" | "em" | "small" | "sub" | "sup" | "span" | "code" | "p" | "div") && end < 60 {
                    i += end + 1;
                    continue;
                }
            }
        }
        // Copy one full char.
        let rest = t.get(i..).unwrap_or("");
        match rest.chars().next() {
            Some(c) => {
                s.push(c);
                i += c.len_utf8();
            }
            None => break,
        }
    }
    let s = s.replace("\\n", "\n");
    let s = decode_entities(&s);
    let s: String = s.chars().take(MAX_LABEL_CHARS).collect();
    s.trim().to_string()
}

/// Quotes a label when it contains characters that would break the syntax.
pub fn quote_if_needed(label: &str) -> String {
    let flat = label.replace('\n', "<br/>").replace('"', "#quot;");
    let special = flat.chars().any(|c| matches!(c, '[' | ']' | '(' | ')' | '{' | '}' | '|' | ';' | '%' | '"' | '<' | '>' | '/' | '\\' | '#' | '&' | ':' | ','));
    if special { format!("\"{flat}\"") } else { flat }
}

pub fn is_id_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Parses a CSS-like property list (`fill:#f9f,stroke:#333`).
pub fn parse_props(s: &str) -> std::collections::BTreeMap<String, String> {
    let mut m = std::collections::BTreeMap::new();
    for part in s.split(',') {
        if let Some((k, v)) = part.split_once(':') {
            let k = k.trim();
            let v = v.trim().trim_end_matches(';').trim();
            if !k.is_empty() && !v.is_empty() {
                m.insert(k.to_string(), v.to_string());
            }
        }
    }
    m
}
