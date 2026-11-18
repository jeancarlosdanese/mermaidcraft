//! Finds ```mermaid blocks in a Markdown file and replaces one of them without touching the rest.

use crate::model::Span;

/// Most blocks listed in one file.
pub const MAX_BLOCKS: usize = 500;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MdBlock {
    /// 1-based line of the opening fence.
    pub fence_line: usize,
    /// The code between the fences (starts at the line after the opening fence and ends at the
    /// start of the closing fence line).
    pub body: Span,
}

fn fence_of(line: &str) -> Option<(char, usize, &str)> {
    let t = line.trim_start();
    let c = t.chars().next()?;
    if c != '`' && c != '~' {
        return None;
    }
    let n = t.chars().take_while(|x| *x == c).count();
    if n < 3 {
        return None;
    }
    Some((c, n, t.get(n..).unwrap_or("")))
}

/// Lists the mermaid blocks of a Markdown text, in order.
pub fn find_blocks(md: &str) -> Vec<MdBlock> {
    let mut blocks = Vec::new();
    let mut pos = 0usize;
    let mut open: Option<(char, usize, bool, usize, usize)> = None; // fence char, len, is_mermaid, line no, body start
    for (i, raw) in md.split_inclusive('\n').enumerate() {
        let line = raw.trim_end_matches(['\r', '\n']);
        let start = pos;
        pos += raw.len();
        match open {
            None => {
                if let Some((c, n, info)) = fence_of(line) {
                    let is_mermaid = info.trim().to_ascii_lowercase().split_whitespace().next().is_some_and(|w| w == "mermaid" || w.starts_with("mermaid{"));
                    open = Some((c, n, is_mermaid, i + 1, pos));
                }
            }
            Some((c, n, is_mermaid, fence_line, body_start)) => {
                if let Some((c2, n2, rest)) = fence_of(line) {
                    if c2 == c && n2 >= n && rest.trim().is_empty() {
                        if is_mermaid && blocks.len() < MAX_BLOCKS {
                            blocks.push(MdBlock { fence_line, body: Span::new(body_start, start) });
                        }
                        open = None;
                    }
                }
            }
        }
    }
    blocks
}

/// The code of a block, without the final line break.
pub fn block_text<'a>(md: &'a str, b: &MdBlock) -> &'a str {
    let t = md.get(b.body.start..b.body.end).unwrap_or("");
    t.strip_suffix('\n').map(|x| x.strip_suffix('\r').unwrap_or(x)).unwrap_or(t)
}

/// Replaces the code of a block. The rest of the file stays byte for byte the same.
pub fn replace_block(md: &str, b: &MdBlock, new_code: &str) -> Option<String> {
    let before = md.get(..b.body.start)?;
    let after = md.get(b.body.end..)?;
    let crlf = md.contains("\r\n");
    let mut code = new_code.replace("\r\n", "\n");
    if !code.is_empty() && !code.ends_with('\n') {
        code.push('\n');
    }
    if crlf {
        code = code.replace('\n', "\r\n");
    }
    Some(format!("{before}{code}{after}"))
}

/// A one-line preview of a block for lists.
pub fn block_title(md: &str, b: &MdBlock) -> String {
    let text = block_text(md, b);
    text.lines().map(str::trim).find(|l| !l.is_empty() && !l.starts_with("%%")).unwrap_or("(empty)").chars().take(48).collect()
}
