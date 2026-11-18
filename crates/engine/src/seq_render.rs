//! sequenceDiagram → scene.

use crate::geom::*;
use crate::model::*;
use crate::scene::*;
use crate::look;
use crate::text::{label_size, split_lines, text_width};

const FONT: f32 = 15.0;
const MSG_FONT: f32 = 14.0;
const BOX_H: f32 = 40.0;
const ACTOR_H: f32 = 62.0;

struct Frame {
    kind: BlockKind,
    label: String,
    start_y: f32,
    min_i: usize,
    max_i: usize,
    depth: usize,
    deepest: usize,
    elses: Vec<(f32, String)>,
}

fn text_prim(x: f32, y: f32, text: &str, size: f32, bold: bool, anchor: Anchor, color: Color) -> Prim {
    Prim::new(PrimShape::Text { x, y, text: text.to_string(), size, bold, italic: false, mono: false, anchor }).fill(color)
}

fn lines_block(out: &mut Vec<Prim>, cx: f32, top: f32, text: &str, size: f32, color: Color, anchor: Anchor) -> f32 {
    let lh = size * 1.25;
    let lines = split_lines(text);
    for (i, l) in lines.iter().enumerate() {
        if !l.is_empty() {
            out.push(text_prim(cx, top + lh * (i as f32 + 0.5) + size * 0.34, l, size, false, anchor, color));
        }
    }
    lh * lines.len() as f32
}

pub fn render(seq: &Sequence, opts: &crate::RenderOptions) -> Scene {
    let font = opts.font;
    let open = opts.colorful();
    let t = &DEFAULT_THEME;
    let n = seq.participants.len();
    let has_actor = seq.participants.iter().any(|p| p.actor);
    let head_h = if has_actor { ACTOR_H } else { BOX_H };
    let box_w: Vec<f32> = seq.participants.iter().map(|p| (label_size(&p.label, FONT, false, font).0 + 32.0).max(100.0)).collect();
    // Column gaps.
    let mut gaps: Vec<f32> = (0..n.saturating_sub(1))
        .map(|i| {
            let a = box_w.get(i).copied().unwrap_or(100.0);
            let b = box_w.get(i + 1).copied().unwrap_or(100.0);
            ((a + b) / 2.0 + 30.0).max(150.0)
        })
        .collect();
    let mut reqs: Vec<(usize, usize, f32)> = Vec::new();
    for it in &seq.items {
        match it {
            SeqItem::Message { from, to, text, .. } => {
                let tw = label_size(text, MSG_FONT, false, font).0;
                if from == to {
                    reqs.push((*from, from + 1, tw + 70.0));
                } else {
                    let (a, b) = (*from.min(to), *from.max(to));
                    reqs.push((a, b, tw + 40.0));
                }
            }
            SeqItem::Note { place, targets, text, .. } => {
                let tw = label_size(text, MSG_FONT, false, font).0 + 24.0;
                let lo = targets.iter().copied().min().unwrap_or(0);
                let hi = targets.iter().copied().max().unwrap_or(0);
                match place {
                    NotePlace::Over if hi > lo => reqs.push((lo, hi, tw - 40.0)),
                    NotePlace::RightOf => reqs.push((hi, hi + 1, tw + 40.0)),
                    NotePlace::LeftOf if lo > 0 => reqs.push((lo - 1, lo, tw + 40.0)),
                    _ => {}
                }
            }
            _ => {}
        }
    }
    reqs.sort_by_key(|r| r.1.saturating_sub(r.0));
    for (a, b, need) in reqs {
        if b > n.saturating_sub(1) || a >= b {
            continue;
        }
        let have: f32 = gaps.get(a..b).map_or(0.0, |s| s.iter().sum());
        if have < need {
            if let Some(g) = gaps.get_mut(b - 1) {
                *g += need - have;
            }
        }
    }
    let mut xs: Vec<f32> = Vec::with_capacity(n);
    let mut cur = box_w.first().copied().unwrap_or(100.0) / 2.0;
    for i in 0..n {
        xs.push(cur);
        cur += gaps.get(i).copied().unwrap_or(0.0);
    }
    let x_of = |i: usize| xs.get(i).copied().unwrap_or(0.0);

    let mut scene = Scene { background: Some(t.background), font, ..Scene::default() };
    let mut back: Vec<Prim> = Vec::new();
    let mut mid: Vec<Prim> = Vec::new();
    let mut front: Vec<Prim> = Vec::new();

    let mut y = head_h + 24.0;
    let mut frames: Vec<Frame> = Vec::new();
    let mut done_frames: Vec<(Frame, f32)> = Vec::new();
    let mut act: Vec<Vec<f32>> = vec![Vec::new(); n];
    let mut bars: Vec<(usize, usize, f32, f32)> = Vec::new(); // participant, depth, y0, y1
    let mut counter = 0usize;
    let touch = |frames: &mut Vec<Frame>, idxs: &[usize]| {
        for f in frames.iter_mut() {
            for i in idxs {
                f.min_i = f.min_i.min(*i);
                f.max_i = f.max_i.max(*i);
            }
        }
    };
    for it in &seq.items {
        match it {
            SeqItem::Message { from, to, text, arrow, activate, deactivate, .. } => {
                touch(&mut frames, &[*from, *to]);
                let th = if text.is_empty() { 0.0 } else { label_size(text, MSG_FONT, false, font).1 };
                let (xa, xb) = (x_of(*from), x_of(*to));
                let sender = look::indexed(opts.colors, *from).map_or(t.line, |c| c.1);
                let col = look::edge_color(opts, t.line, sender);
                let dash = |p: Prim| if arrow.dotted { p.dash(5.0, 4.0) } else { p };
                counter += 1;
                if from == to {
                    let y0 = y + th + 6.0;
                    let r = xa + act.get(*from).map_or(0, Vec::len) as f32 * 5.0;
                    let cmds = vec![PathCmd::Move(r, y0), PathCmd::Line(r + 40.0, y0), PathCmd::Line(r + 40.0, y0 + 22.0), PathCmd::Line(r + 8.0, y0 + 22.0)];
                    front.push(dash(Prim::new(PrimShape::Path(cmds)).stroke(col, 1.6)));
                    front.extend(head_prims(arrow_head(arrow.head), (r, y0 + 22.0), (-1.0, 0.0), col, 1.6, Tag::None, open));
                    if !text.is_empty() {
                        lines_block(&mut front, r + 46.0, y, text, MSG_FONT, t.text, Anchor::Start);
                    }
                    if *activate {
                        if let Some(s) = act.get_mut(*to) {
                            s.push(y0 + 22.0);
                        }
                    }
                    if *deactivate {
                        pop_bar(&mut act, &mut bars, *from, y0);
                    }
                    y = y0 + 22.0 + 20.0;
                } else {
                    let dir = if xb > xa { 1.0 } else { -1.0 };
                    let y0 = y + th + 8.0;
                    let ds = act.get(*from).map_or(0, Vec::len);
                    let dt = act.get(*to).map_or(0, Vec::len);
                    let sa = xa + dir * if ds > 0 { 5.0 + (ds as f32 - 1.0) * 5.0 } else { 0.0 };
                    let sb = xb - dir * if dt > 0 || *activate { 5.0 + (dt as f32 - if *activate { 0.0 } else { 1.0 }).max(0.0) * 5.0 } else { 0.0 };
                    let head_len = if open && arrow.head != SeqHead::Cross { 1.0 } else { 8.0 };
                    let inset = if arrow.head == SeqHead::None { 0.0 } else { head_len };
                    let inset_a = if arrow.both { head_len } else { 0.0 };
                    let cmds = vec![PathCmd::Move(sa + dir * inset_a, y0), PathCmd::Line(sb - dir * inset, y0)];
                    front.push(dash(Prim::new(PrimShape::Path(cmds)).stroke(col, 1.6)));
                    front.extend(head_prims(arrow_head(arrow.head), (sb, y0), (dir, 0.0), col, 1.6, Tag::None, open));
                    if arrow.both {
                        front.extend(head_prims(arrow_head(arrow.head), (sa, y0), (-dir, 0.0), col, 1.6, Tag::None, open));
                    }
                    if !text.is_empty() {
                        lines_block(&mut front, (xa + xb) / 2.0, y, text, MSG_FONT, t.text, Anchor::Middle);
                    }
                    if seq.autonumber {
                        front.push(Prim::new(PrimShape::Ellipse { cx: sa + dir * 2.0, cy: y0, rx: 8.0, ry: 8.0 }).fill(sender));
                        front.push(text_prim(sa + dir * 2.0, y0 + 3.6, &counter.to_string(), 10.0, true, Anchor::Middle, [255, 255, 255, 255]));
                    }
                    if *activate {
                        if let Some(s) = act.get_mut(*to) {
                            s.push(y0);
                        }
                    }
                    if *deactivate {
                        pop_bar(&mut act, &mut bars, *from, y0);
                    }
                    y = y0 + 18.0;
                }
            }
            SeqItem::Note { place, targets, text, .. } => {
                touch(&mut frames, targets);
                let lo = targets.iter().copied().min().unwrap_or(0);
                let hi = targets.iter().copied().max().unwrap_or(0);
                let (tw, th) = label_size(text, MSG_FONT, false, font);
                let (x0, w) = match place {
                    NotePlace::Over => {
                        let span = (x_of(hi) - x_of(lo)) + 60.0;
                        let w = span.max(tw + 24.0).max(90.0);
                        ((x_of(lo) + x_of(hi)) / 2.0 - w / 2.0, w)
                    }
                    NotePlace::RightOf => (x_of(hi) + 14.0, (tw + 24.0).max(90.0)),
                    NotePlace::LeftOf => {
                        let w = (tw + 24.0).max(90.0);
                        (x_of(lo) - 14.0 - w, w)
                    }
                };
                let h = th + 16.0;
                front.push(Prim::new(PrimShape::Rect { x: x0, y, w, h, r: 2.0 }).fill(t.note_fill).stroke(t.note_stroke, 1.0));
                lines_block(&mut front, x0 + w / 2.0, y + 8.0, text, MSG_FONT, t.text, Anchor::Middle);
                y += h + 14.0;
            }
            SeqItem::Activate { who } => {
                if let Some(s) = act.get_mut(*who) {
                            s.push(y);
                        }
            }
            SeqItem::Deactivate { who } => pop_bar(&mut act, &mut bars, *who, y),
            SeqItem::BlockStart { kind, label, .. } => {
                let depth = frames.len();
                for f in frames.iter_mut() {
                    f.deepest = f.deepest.max(depth + 1);
                }
                frames.push(Frame { kind: *kind, label: label.clone(), start_y: y, min_i: usize::MAX, max_i: 0, depth, deepest: depth, elses: Vec::new() });
                y += if *kind == BlockKind::Rect { 8.0 } else { 30.0 };
            }
            SeqItem::BlockElse { label, .. } => {
                if let Some(f) = frames.last_mut() {
                    f.elses.push((y, label.clone()));
                }
                y += 28.0;
            }
            SeqItem::BlockEnd => {
                if let Some(f) = frames.pop() {
                    y += 6.0;
                    done_frames.push((f, y));
                    y += 12.0;
                }
            }
        }
    }
    for (p, stack) in act.iter().enumerate() {
        for (d, y0) in stack.iter().enumerate() {
            bars.push((p, d, *y0, y));
        }
    }
    let end_y = y + 6.0;
    // Frames.
    for (f, y1) in done_frames.iter().rev() {
        let (lo, hi) = if f.min_i == usize::MAX || f.max_i < f.min_i { (0, n.saturating_sub(1)) } else { (f.min_i, f.max_i) };
        let inner = f.deepest.saturating_sub(f.depth) as f32;
        let m = 24.0 + inner * 10.0;
        let x0 = x_of(lo) - m;
        let x1 = x_of(hi) + m;
        if f.kind == BlockKind::Rect {
            let c = parse_rgb(&f.label).unwrap_or([200, 220, 255, 255]);
            back.push(Prim::new(PrimShape::Rect { x: x0, y: f.start_y, w: x1 - x0, h: y1 - f.start_y, r: 0.0 }).fill(c));
            continue;
        }
        back.push(Prim::new(PrimShape::Rect { x: x0, y: f.start_y, w: x1 - x0, h: y1 - f.start_y, r: 2.0 }).stroke([140, 140, 160, 255], 1.2));
        let tag_w = text_width(f.kind.word(), 12.5, true, font) + 18.0;
        back.push(
            Prim::new(PrimShape::Polygon(vec![(x0, f.start_y), (x0 + tag_w, f.start_y), (x0 + tag_w, f.start_y + 10.0), (x0 + tag_w - 8.0, f.start_y + 19.0), (x0, f.start_y + 19.0)]))
                .fill([232, 232, 240, 255])
                .stroke([140, 140, 160, 255], 1.0),
        );
        back.push(text_prim(x0 + 9.0, f.start_y + 14.0, f.kind.word(), 12.5, true, Anchor::Start, t.text));
        if !f.label.is_empty() {
            back.push(text_prim(x0 + tag_w + 10.0, f.start_y + 14.5, &format!("[{}]", f.label), 12.5, false, Anchor::Start, t.text));
        }
        for (ey, el) in &f.elses {
            back.push(Prim::new(PrimShape::Path(vec![PathCmd::Move(x0, *ey), PathCmd::Line(x1, *ey)])).stroke([140, 140, 160, 255], 1.2).dash(6.0, 4.0));
            if !el.is_empty() {
                back.push(text_prim((x0 + x1) / 2.0, ey + 16.0, &format!("[{el}]"), 12.5, false, Anchor::Middle, t.text));
            }
        }
    }
    // Activation bars.
    for (p, d, y0, y1) in bars {
        mid.push(Prim::new(PrimShape::Rect { x: x_of(p) - 5.0 + d as f32 * 5.0, y: y0, w: 10.0, h: (y1 - y0).max(6.0), r: 0.0 }).fill([240, 240, 240, 255]).stroke([102, 102, 102, 255], 1.0));
    }
    // Participants (header and footer) and lifelines.
    let mut heads: Vec<Prim> = Vec::new();
    for (i, p) in seq.participants.iter().enumerate() {
        let x = x_of(i);
        back.insert(0, Prim::new(PrimShape::Path(vec![PathCmd::Move(x, head_h), PathCmd::Line(x, end_y + 14.0)])).stroke([153, 153, 153, 255], 1.0).dash(5.0, 4.0));
        for top in [0.0, end_y + 14.0] {
            if p.actor {
                let (fill, stroke) = look::indexed(opts.colors, i).unwrap_or((t.actor_fill, t.actor_stroke));
                actor_figure(&mut heads, x, top, &p.label, t, fill, stroke);
            } else {
                let w = box_w.get(i).copied().unwrap_or(100.0);
                let (fill, stroke) = look::indexed(opts.colors, i).unwrap_or((t.actor_fill, t.actor_stroke));
                heads.push(Prim::new(PrimShape::Rect { x: x - w / 2.0, y: top, w, h: BOX_H, r: 6.0 }).fill(fill).stroke(stroke, 1.8));
                let lh = label_size(&p.label, FONT, false, font).1;
                lines_block(&mut heads, x, top + (BOX_H - lh) / 2.0, &p.label, FONT, t.text, Anchor::Middle);
            }
        }
    }
    for p in back.into_iter().chain(mid).chain(heads).chain(front) {
        scene.push(p);
    }
    scene.finish(24.0);
    scene
}

fn arrow_head(h: SeqHead) -> Head {
    match h {
        SeqHead::Filled | SeqHead::Async => Head::Arrow,
        SeqHead::Cross => Head::Cross,
        SeqHead::None => Head::None,
    }
}

fn pop_bar(act: &mut [Vec<f32>], bars: &mut Vec<(usize, usize, f32, f32)>, who: usize, y: f32) {
    if let Some(stack) = act.get_mut(who) {
        if let Some(y0) = stack.pop() {
            bars.push((who, stack.len(), y0, y));
        }
    }
}

fn parse_rgb(s: &str) -> Option<Color> {
    let inner = s.trim().strip_prefix("rgb").or_else(|| s.trim().strip_prefix("rgba"))?;
    let inner = inner.trim_start_matches('a').trim().trim_start_matches('(').trim_end_matches(')');
    let v: Vec<u8> = inner.split(',').filter_map(|x| x.trim().parse::<f32>().ok()).map(|x| x.clamp(0.0, 255.0) as u8).collect();
    match v.as_slice() {
        [r, g, b, ..] => Some([*r, *g, *b, 255]),
        _ => None,
    }
}

fn actor_figure(out: &mut Vec<Prim>, x: f32, top: f32, label: &str, t: &Theme, fill: Color, c: Color) {
    out.push(Prim::new(PrimShape::Ellipse { cx: x, cy: top + 9.0, rx: 7.0, ry: 7.0 }).fill(fill).stroke(c, 1.8));
    let line = |a: P, b: P| Prim::new(PrimShape::Path(vec![PathCmd::Move(a.0, a.1), PathCmd::Line(b.0, b.1)])).stroke(c, 1.5);
    out.push(line((x, top + 16.0), (x, top + 34.0)));
    out.push(line((x - 11.0, top + 24.0), (x + 11.0, top + 24.0)));
    out.push(line((x, top + 34.0), (x - 9.0, top + 44.0)));
    out.push(line((x, top + 34.0), (x + 9.0, top + 44.0)));
    out.push(text_prim(x, top + 58.0, label, FONT, false, Anchor::Middle, t.text));
}
