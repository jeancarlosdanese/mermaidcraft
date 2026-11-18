//! Flowchart → scene: sizes nodes, lays out each subgraph level, routes edges and draws shapes.

use std::collections::HashMap;

use crate::geom::*;
use crate::layout::{LEdge, LItem, LayoutResult, Spacing, layered};
use crate::look::{self, Arrows, ColorMode, Font};
use crate::model::*;
use crate::scene::*;
use crate::text::{label_size, split_lines};

const FONT: f32 = 15.0;
const LABEL_FONT: f32 = 13.5;
const TITLE_FONT: f32 = 14.0;
const CLUSTER_PAD: f32 = 18.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Item {
    Node(usize),
    Sub(usize),
}

struct NodeStyle {
    fill: Color,
    stroke: Color,
    stroke_width: f32,
    text: Color,
    dash: Option<(f32, f32)>,
}

fn resolve_style(fc: &Flowchart, n: &FlowNode, base: Option<(Color, Color)>) -> NodeStyle {
    let t = &DEFAULT_THEME;
    let (fill, stroke) = base.unwrap_or((t.node_fill, t.node_stroke));
    let mut st = NodeStyle { fill, stroke, stroke_width: if base.is_some() { 2.0 } else { 1.5 }, text: if base.is_some() { [30, 30, 30, 255] } else { t.text }, dash: None };
    let mut apply = |m: &StyleMap| {
        for (k, v) in m {
            match k.as_str() {
                "fill" => {
                    if let Some(c) = parse_color(v) {
                        st.fill = c;
                    }
                }
                "stroke" => {
                    if let Some(c) = parse_color(v) {
                        st.stroke = c;
                    }
                }
                "stroke-width" => {
                    if let Ok(w) = v.trim_end_matches("px").trim().parse::<f32>() {
                        st.stroke_width = w.clamp(0.0, 20.0);
                    }
                }
                "color" => {
                    if let Some(c) = parse_color(v) {
                        st.text = c;
                    }
                }
                "stroke-dasharray" => {
                    let nums: Vec<f32> = v.split([' ', ',']).filter_map(|x| x.trim().parse::<f32>().ok()).collect();
                    if let [a, b, ..] = nums.as_slice() {
                        st.dash = Some((a.clamp(0.5, 50.0), b.clamp(0.5, 50.0)));
                    } else if let [a] = nums.as_slice() {
                        st.dash = Some((a.clamp(0.5, 50.0), a.clamp(0.5, 50.0)));
                    }
                }
                _ => {}
            }
        }
    };
    if let Some(m) = fc.class_defs.get("default") {
        apply(m);
    }
    for c in &n.classes {
        if let Some(m) = fc.class_defs.get(c) {
            apply(m);
        }
    }
    if let Some(m) = fc.node_styles.get(&n.id) {
        apply(m);
    }
    st
}

fn node_size(n: &FlowNode, font: Font) -> (f32, f32) {
    let (tw, th) = label_size(&n.label, FONT, false, font);
    let (tw, th) = (tw.max(8.0), th.max(FONT * 1.25));
    let (w, h) = match n.shape {
        NodeShape::Rect | NodeShape::Round => (tw + 32.0, th + 22.0),
        NodeShape::Stadium => (tw + th + 28.0, th + 22.0),
        NodeShape::Subroutine => (tw + 48.0, th + 22.0),
        NodeShape::Cylinder => (tw + 32.0, th + 42.0),
        NodeShape::Circle => {
            let d = (tw * tw + th * th).sqrt() + 20.0;
            (d, d)
        }
        NodeShape::DoubleCircle => {
            let d = (tw * tw + th * th).sqrt() + 32.0;
            (d, d)
        }
        NodeShape::Diamond => (tw * 2.0 + 28.0, th * 2.0 + 24.0),
        NodeShape::Hexagon => (tw + 56.0, th + 24.0),
        NodeShape::Parallelogram | NodeShape::ParallelogramAlt | NodeShape::Trapezoid | NodeShape::TrapezoidAlt => (tw + 56.0, th + 22.0),
        NodeShape::Asymmetric => (tw + 44.0, th + 22.0),
    };
    (w.max(48.0), h.max(38.0))
}

fn outline_of(shape: NodeShape) -> Outline {
    match shape {
        NodeShape::Circle | NodeShape::DoubleCircle | NodeShape::Stadium => Outline::Ellipse,
        NodeShape::Diamond => Outline::Diamond,
        _ => Outline::Box,
    }
}

fn push_text_lines(scene: &mut Scene, cx: f32, cy: f32, label: &str, size: f32, color: Color, bold: bool, anchor: Anchor, tag: Tag) {
    let lines = split_lines(label);
    let lh = size * 1.25;
    let total = lh * lines.len() as f32;
    for (i, l) in lines.iter().enumerate() {
        if l.is_empty() {
            continue;
        }
        let mid = cy - total / 2.0 + lh * (i as f32 + 0.5);
        let shape = PrimShape::Text { x: cx, y: mid + size * 0.34, text: (*l).to_string(), size, bold, italic: false, mono: false, anchor };
        scene.push(Prim::new(shape).fill(color).tag(tag));
    }
}

fn arc_top(x: f32, w: f32, cy: f32, ry: f32, up: bool) -> PathCmd {
    let k = if up { -4.0 / 3.0 * ry } else { 4.0 / 3.0 * ry };
    PathCmd::Cubic(x, cy + k, x + w, cy + k, x + w, cy)
}

fn draw_node(scene: &mut Scene, idx: usize, n: &FlowNode, c: P, size: (f32, f32), st: &NodeStyle) {
    let (w, h) = size;
    let (x, y) = (c.0 - w / 2.0, c.1 - h / 2.0);
    let tag = Tag::Node(idx);
    let base = |shape: PrimShape| {
        let mut p = Prim::new(shape).fill(st.fill).tag(tag);
        if st.stroke_width > 0.0 {
            p = p.stroke(st.stroke, st.stroke_width);
        }
        if let Some((a, b)) = st.dash {
            p = p.dash(a, b);
        }
        p
    };
    let line = |a: P, b: P| Prim::new(PrimShape::Path(vec![PathCmd::Move(a.0, a.1), PathCmd::Line(b.0, b.1)])).stroke(st.stroke, st.stroke_width.max(1.0)).tag(tag);
    let s = 16.0f32.min(w / 4.0);
    match n.shape {
        NodeShape::Rect => scene.push(base(PrimShape::Rect { x, y, w, h, r: 9.0 })),
        NodeShape::Round => scene.push(base(PrimShape::Rect { x, y, w, h, r: 16.0 })),
        NodeShape::Stadium => scene.push(base(PrimShape::Rect { x, y, w, h, r: h / 2.0 })),
        NodeShape::Subroutine => {
            scene.push(base(PrimShape::Rect { x, y, w, h, r: 6.0 }));
            scene.push(line((x + 9.0, y), (x + 9.0, y + h)));
            scene.push(line((x + w - 9.0, y), (x + w - 9.0, y + h)));
        }
        NodeShape::Cylinder => {
            let ry = 9.0;
            let body = vec![
                PathCmd::Move(x, y + ry),
                arc_top(x, w, y + ry, ry, true),
                PathCmd::Line(x + w, y + h - ry),
                PathCmd::Cubic(x + w, y + h - ry + 4.0 / 3.0 * ry, x, y + h - ry + 4.0 / 3.0 * ry, x, y + h - ry),
                PathCmd::Close,
            ];
            scene.push(base(PrimShape::Path(body)));
            let front = vec![PathCmd::Move(x, y + ry), PathCmd::Cubic(x, y + ry + 4.0 / 3.0 * ry, x + w, y + ry + 4.0 / 3.0 * ry, x + w, y + ry)];
            scene.push(Prim::new(PrimShape::Path(front)).stroke(st.stroke, st.stroke_width.max(1.0)).tag(tag));
        }
        NodeShape::Circle => scene.push(base(PrimShape::Ellipse { cx: c.0, cy: c.1, rx: w / 2.0, ry: h / 2.0 })),
        NodeShape::DoubleCircle => {
            scene.push(base(PrimShape::Ellipse { cx: c.0, cy: c.1, rx: w / 2.0, ry: h / 2.0 }));
            scene.push(Prim::new(PrimShape::Ellipse { cx: c.0, cy: c.1, rx: w / 2.0 - 5.0, ry: h / 2.0 - 5.0 }).stroke(st.stroke, st.stroke_width.max(1.0)).tag(tag));
        }
        NodeShape::Diamond => scene.push(base(PrimShape::Polygon(vec![(c.0, y), (x + w, c.1), (c.0, y + h), (x, c.1)]))),
        NodeShape::Hexagon => {
            let m = (h / 2.0).min(w / 4.0);
            scene.push(base(PrimShape::Polygon(vec![(x + m, y), (x + w - m, y), (x + w, c.1), (x + w - m, y + h), (x + m, y + h), (x, c.1)])));
        }
        NodeShape::Parallelogram => scene.push(base(PrimShape::Polygon(vec![(x + s, y), (x + w, y), (x + w - s, y + h), (x, y + h)]))),
        NodeShape::ParallelogramAlt => scene.push(base(PrimShape::Polygon(vec![(x, y), (x + w - s, y), (x + w, y + h), (x + s, y + h)]))),
        NodeShape::Trapezoid => scene.push(base(PrimShape::Polygon(vec![(x + s, y), (x + w - s, y), (x + w, y + h), (x, y + h)]))),
        NodeShape::TrapezoidAlt => scene.push(base(PrimShape::Polygon(vec![(x, y), (x + w, y), (x + w - s, y + h), (x + s, y + h)]))),
        NodeShape::Asymmetric => scene.push(base(PrimShape::Polygon(vec![(x, y), (x + w, y), (x + w, y + h), (x, y + h), (x + s * 0.8, c.1)]))),
    }
    push_text_lines(scene, c.0, c.1, &n.label, FONT, st.text, false, Anchor::Middle, tag);
}

struct Level {
    items: Vec<Item>,
    layout: LayoutResult,
    /// Flow edge index of every layout edge.
    edges: Vec<usize>,
    origin: P,
}

fn sub_depth(fc: &Flowchart, mut s: usize) -> usize {
    let mut d = 0;
    while let Some(p) = fc.subgraphs.get(s).and_then(|x| x.parent) {
        d += 1;
        s = p;
        if d > 64 {
            break;
        }
    }
    d
}

/// Chain of containers from the outermost, ending with the thing itself.
fn path_of(fc: &Flowchart, it: Item) -> Vec<Item> {
    let mut chain = vec![it];
    let mut parent = match it {
        Item::Node(n) => fc.nodes.get(n).and_then(|x| x.parent),
        Item::Sub(s) => fc.subgraphs.get(s).and_then(|x| x.parent),
    };
    while let Some(p) = parent {
        chain.push(Item::Sub(p));
        parent = fc.subgraphs.get(p).and_then(|x| x.parent);
        if chain.len() > 80 {
            break;
        }
    }
    chain.reverse();
    chain
}

fn endpoint(fc: &Flowchart, id: &str) -> Option<Item> {
    if let Some(i) = fc.node_index(id) {
        return Some(Item::Node(i));
    }
    fc.subgraph_index(id).map(Item::Sub)
}

/// Colour slot of a node: the block (subgraph) it belongs to, or its own number when it is loose.
fn color_slot(fc: &Flowchart, n: usize) -> usize {
    match fc.nodes.get(n).and_then(|x| x.parent) {
        Some(sg) => sg,
        None => n + fc.subgraphs.len(),
    }
}

/// Colour role of a shape in the "by shape" mode.
fn role_hue(shape: NodeShape) -> look::Hue {
    match shape {
        NodeShape::Diamond => look::YELLOW,
        NodeShape::Stadium | NodeShape::Circle | NodeShape::DoubleCircle => look::GREEN,
        NodeShape::Cylinder => look::VIOLET,
        NodeShape::Subroutine => look::BLUE,
        NodeShape::Hexagon => look::ORANGE,
        NodeShape::Parallelogram | NodeShape::ParallelogramAlt | NodeShape::Trapezoid | NodeShape::TrapezoidAlt => look::CYAN,
        NodeShape::Rect | NodeShape::Round | NodeShape::Asymmetric => look::GRAY,
    }
}

/// Fill and stroke a node starts from (before `classDef` and `style`); `None` for the classic
/// Mermaid colours.
fn node_base(fc: &Flowchart, i: usize, mode: ColorMode) -> Option<(Color, Color)> {
    let n = fc.nodes.get(i)?;
    let h = match mode {
        ColorMode::Classic => return None,
        ColorMode::PerNode => return Some(palette(color_slot(fc, i))),
        ColorMode::Neutral => look::GRAY,
        ColorMode::Shape => role_hue(n.shape),
        ColorMode::Group => n.parent.map_or_else(|| role_hue(n.shape), look::hue_at),
    };
    Some((h.light, h.stroke))
}

/// Fill, border, dashed border and title colour of a group frame.
fn cluster_colors(mode: ColorMode, s: usize) -> (Color, Color, bool, Color) {
    let t = &DEFAULT_THEME;
    match mode {
        ColorMode::Classic => (t.cluster_fill, t.cluster_stroke, false, t.text),
        ColorMode::PerNode => (tint(palette(s).0, 0.4), palette(s).1, true, t.text),
        ColorMode::Neutral | ColorMode::Group | ColorMode::Shape => {
            let h = if mode == ColorMode::Neutral { look::GRAY } else { look::hue_at(s) };
            (h.pale, look::mix(h.hatch, h.stroke, 0.4), true, h.stroke)
        }
    }
}

fn tint(c: Color, k: f32) -> Color {
    let m = |v: u8| (f32::from(v) * k + 255.0 * (1.0 - k)).round().clamp(0.0, 255.0) as u8;
    [m(c[0]), m(c[1]), m(c[2]), 255]
}

pub fn render(fc: &Flowchart, opts: &crate::RenderOptions) -> Scene {
    let t = &DEFAULT_THEME;
    let nn = fc.nodes.len();
    let ns = fc.subgraphs.len();
    let font = opts.font;
    let node_sizes: Vec<(f32, f32)> = fc.nodes.iter().map(|n| node_size(n, font)).collect();

    // Levels: 0 is the root, c + 1 is subgraph c.
    let mut members: Vec<Vec<Item>> = vec![Vec::new(); ns + 1];
    for (i, n) in fc.nodes.iter().enumerate() {
        let lvl = n.parent.map_or(0, |p| p + 1);
        if let Some(m) = members.get_mut(lvl) {
            m.push(Item::Node(i));
        }
    }
    for (i, s) in fc.subgraphs.iter().enumerate() {
        let lvl = s.parent.map_or(0, |p| p + 1);
        if let Some(m) = members.get_mut(lvl) {
            m.push(Item::Sub(i));
        }
    }
    // Edges per level.
    let mut level_edges: Vec<Vec<(usize, Item, Item)>> = vec![Vec::new(); ns + 1];
    for (ei, e) in fc.edges.iter().enumerate() {
        let (Some(a), Some(b)) = (endpoint(fc, &e.from), endpoint(fc, &e.to)) else { continue };
        if a == b {
            continue;
        }
        let (pa, pb) = (path_of(fc, a), path_of(fc, b));
        let k = pa.iter().zip(pb.iter()).take_while(|(x, y)| x == y).count();
        if k >= pa.len() || k >= pb.len() {
            continue;
        }
        let lvl = match k.checked_sub(1).and_then(|i| pa.get(i)) {
            Some(Item::Sub(s)) => s + 1,
            _ => 0,
        };
        if let (Some(ia), Some(ib), Some(l)) = (pa.get(k), pb.get(k), level_edges.get_mut(lvl)) {
            l.push((ei, *ia, *ib));
        }
    }
    // Does anything cross the boundary of subgraph c?
    let inside = |it: Item, s: usize| path_of(fc, it).contains(&Item::Sub(s));
    let crosses: Vec<bool> = (0..ns)
        .map(|s| {
            fc.edges.iter().any(|e| {
                let (Some(a), Some(b)) = (endpoint(fc, &e.from), endpoint(fc, &e.to)) else { return false };
                inside(a, s) != inside(b, s) && a != Item::Sub(s) && b != Item::Sub(s)
            })
        })
        .collect();
    let mut eff_dir: Vec<Direction> = vec![fc.direction; ns];
    let mut order: Vec<usize> = (0..ns).collect();
    order.sort_by_key(|s| sub_depth(fc, *s));
    for &s in &order {
        let parent_dir = fc.subgraphs.get(s).and_then(|x| x.parent).and_then(|p| eff_dir.get(p).copied()).unwrap_or(fc.direction);
        let own = fc.subgraphs.get(s).and_then(|x| x.direction);
        let d = if crosses.get(s).copied().unwrap_or(false) { parent_dir } else { own.unwrap_or(parent_dir) };
        if let Some(slot) = eff_dir.get_mut(s) {
            *slot = d;
        }
    }

    let mut sub_size: Vec<(f32, f32)> = vec![(0.0, 0.0); ns];
    let mut sub_title_h: Vec<f32> = vec![0.0; ns];
    let mut sub_inner: Vec<(f32, f32)> = vec![(0.0, 0.0); ns];
    let mut levels: Vec<Option<Level>> = (0..=ns).map(|_| None).collect();
    let mut by_depth: Vec<usize> = (0..ns).collect();
    by_depth.sort_by_key(|s| std::cmp::Reverse(sub_depth(fc, *s)));
    let mut process: Vec<usize> = by_depth.iter().map(|s| s + 1).collect();
    process.push(0);
    for lvl in process {
        let items = members.get(lvl).cloned().unwrap_or_default();
        let index: HashMap<Item, usize> = items.iter().enumerate().map(|(i, it)| (*it, i)).collect();
        let litems: Vec<LItem> = items
            .iter()
            .map(|it| match it {
                Item::Node(n) => {
                    let (w, h) = node_sizes.get(*n).copied().unwrap_or((50.0, 38.0));
                    LItem { w, h }
                }
                Item::Sub(s) => {
                    let (w, h) = sub_size.get(*s).copied().unwrap_or((50.0, 38.0));
                    LItem { w, h }
                }
            })
            .collect();
        let mut ledges = Vec::new();
        let mut flow_idx = Vec::new();
        for (ei, a, b) in level_edges.get(lvl).map(Vec::as_slice).unwrap_or(&[]) {
            let (Some(ia), Some(ib)) = (index.get(a), index.get(b)) else { continue };
            let label = fc.edges.get(*ei).and_then(|e| e.label.as_ref()).filter(|l| !l.is_empty()).map(|l| {
                let (w, h) = label_size(l, LABEL_FONT, false, font);
                (w + 12.0, h + 8.0)
            });
            let min_len = fc.edges.get(*ei).map_or(1, |e| e.kind.min_len);
            ledges.push(LEdge { from: *ia, to: *ib, min_len, label });
            flow_idx.push(*ei);
        }
        let dir = if lvl == 0 { fc.direction } else { eff_dir.get(lvl - 1).copied().unwrap_or(fc.direction) };
        let layout = layered(&litems, &ledges, dir, Spacing::default());
        if lvl > 0 {
            let s = lvl - 1;
            let title = fc.subgraphs.get(s).map(|x| x.title.as_str()).unwrap_or("");
            let (tw, th) = if title.is_empty() { (0.0, 0.0) } else { label_size(title, TITLE_FONT, true, font) };
            let th_total = if title.is_empty() { 0.0 } else { th + 10.0 };
            let (iw, ih) = if items.is_empty() { (40.0, 20.0) } else { (layout.width, layout.height) };
            if let Some(v) = sub_inner.get_mut(s) {
                *v = (iw, ih);
            }
            if let Some(v) = sub_title_h.get_mut(s) {
                *v = th_total;
            }
            if let Some(v) = sub_size.get_mut(s) {
                *v = ((iw + CLUSTER_PAD * 2.0).max(tw + CLUSTER_PAD * 2.0), ih + CLUSTER_PAD * 2.0 + th_total);
            }
        }
        if let Some(slot) = levels.get_mut(lvl) {
            *slot = Some(Level { items, layout, edges: flow_idx, origin: (0.0, 0.0) });
        }
    }

    // Absolute positions, outermost first.
    let mut node_c: Vec<P> = vec![(0.0, 0.0); nn];
    let mut sub_box: Vec<Rect> = vec![Rect::default(); ns];
    let mut place_order: Vec<usize> = vec![0];
    let mut by_depth_asc: Vec<usize> = (0..ns).collect();
    by_depth_asc.sort_by_key(|s| sub_depth(fc, *s));
    place_order.extend(by_depth_asc.iter().map(|s| s + 1));
    for lvl in place_order {
        let origin = if lvl == 0 {
            (0.0, 0.0)
        } else {
            let s = lvl - 1;
            let b = sub_box.get(s).copied().unwrap_or_default();
            let (iw, _) = sub_inner.get(s).copied().unwrap_or((0.0, 0.0));
            let th = sub_title_h.get(s).copied().unwrap_or(0.0);
            (b.x + CLUSTER_PAD + (b.w - CLUSTER_PAD * 2.0 - iw) / 2.0, b.y + CLUSTER_PAD + th)
        };
        let Some(Some(level)) = levels.get_mut(lvl) else { continue };
        level.origin = origin;
        for (i, it) in level.items.iter().enumerate() {
            let c = level.layout.centers.get(i).copied().unwrap_or((0.0, 0.0));
            let abs = (c.0 + origin.0, c.1 + origin.1);
            match it {
                Item::Node(n) => {
                    if let Some(slot) = node_c.get_mut(*n) {
                        *slot = abs;
                    }
                }
                Item::Sub(s) => {
                    let (w, h) = sub_size.get(*s).copied().unwrap_or((0.0, 0.0));
                    if let Some(slot) = sub_box.get_mut(*s) {
                        *slot = Rect { x: abs.0 - w / 2.0, y: abs.1 - h / 2.0, w, h };
                    }
                }
            }
        }
    }

    // Nodes the person moved by hand.
    let moved_node: Vec<bool> = fc.nodes.iter().map(|n| fc.offsets.get(&n.id).is_some_and(|o| o.0 != 0.0 || o.1 != 0.0)).collect();
    for (i, n) in fc.nodes.iter().enumerate() {
        if let (Some((dx, dy)), Some(c)) = (fc.offsets.get(&n.id), node_c.get_mut(i)) {
            c.0 += dx;
            c.1 += dy;
        }
    }
    let item_moved = |it: Item| -> bool {
        match it {
            Item::Node(n) => moved_node.get(n).copied().unwrap_or(false),
            Item::Sub(s) => fc.nodes.iter().enumerate().any(|(i, _)| moved_node.get(i).copied().unwrap_or(false) && path_of(fc, Item::Node(i)).contains(&Item::Sub(s))),
        }
    };
    // Frames grow to follow what was moved inside them (innermost first).
    let mut deepest_first: Vec<usize> = (0..ns).collect();
    deepest_first.sort_by_key(|s| std::cmp::Reverse(sub_depth(fc, *s)));
    for &s in &deepest_first {
        if !item_moved(Item::Sub(s)) {
            continue;
        }
        let mut bounds: Option<(f32, f32, f32, f32)> = None;
        let mut grow = |x0: f32, y0: f32, x1: f32, y1: f32| {
            bounds = Some(match bounds {
                None => (x0, y0, x1, y1),
                Some((a, b, c, d)) => (a.min(x0), b.min(y0), c.max(x1), d.max(y1)),
            });
        };
        for (i, n) in fc.nodes.iter().enumerate() {
            if n.parent == Some(s) {
                let (w, h) = node_sizes.get(i).copied().unwrap_or((0.0, 0.0));
                let c = node_c.get(i).copied().unwrap_or((0.0, 0.0));
                grow(c.0 - w / 2.0, c.1 - h / 2.0, c.0 + w / 2.0, c.1 + h / 2.0);
            }
        }
        for (j, sg) in fc.subgraphs.iter().enumerate() {
            if sg.parent == Some(s) {
                let b = sub_box.get(j).copied().unwrap_or_default();
                grow(b.x, b.y, b.x + b.w, b.y + b.h);
            }
        }
        if let Some((x0, y0, x1, y1)) = bounds {
            let th = sub_title_h.get(s).copied().unwrap_or(0.0);
            let old = sub_box.get(s).copied().unwrap_or_default();
            let w = (x1 - x0 + CLUSTER_PAD * 2.0).max(old.w.min(120.0));
            let cx = (x0 + x1) / 2.0;
            if let Some(slot) = sub_box.get_mut(s) {
                *slot = Rect { x: cx - w / 2.0, y: y0 - CLUSTER_PAD - th, w, h: y1 - y0 + CLUSTER_PAD * 2.0 + th };
            }
        }
    }

    // ---- drawing ------------------------------------------------------------------------
    let mut scene = Scene { background: Some(t.background), font, ..Scene::default() };
    scene.node_boxes = (0..nn)
        .map(|i| {
            let (w, h) = node_sizes.get(i).copied().unwrap_or((0.0, 0.0));
            let c = node_c.get(i).copied().unwrap_or((0.0, 0.0));
            Rect { x: c.0 - w / 2.0, y: c.1 - h / 2.0, w, h }
        })
        .collect();
    // Clusters, outermost first.
    for &s in &by_depth_asc {
        let b = sub_box.get(s).copied().unwrap_or_default();
        let tag = Tag::Cluster(s);
        let (fill, stroke, dashed, title_col) = cluster_colors(opts.colors, s);
        let classic = opts.colors == ColorMode::Classic;
        scene.push(Prim::new(PrimShape::Rect { x: b.x, y: b.y, w: b.w, h: b.h, r: if classic { 4.0 } else { 10.0 } }).fill(fill).stroke(stroke, 1.6).dash_if(dashed).tag(tag));
        if let Some(sg) = fc.subgraphs.get(s) {
            if !sg.title.is_empty() {
                let cy = b.y + 6.0 + label_size(&sg.title, TITLE_FONT, true, font).1 / 2.0;
                // The classic look centres the title; the others keep it in the top-left corner,
                // out of the way of edges that enter the frame from above.
                if classic {
                    push_text_lines(&mut scene, b.x + b.w / 2.0, cy, &sg.title, TITLE_FONT, title_col, true, Anchor::Middle, tag);
                } else {
                    push_text_lines(&mut scene, b.x + 12.0, cy, &sg.title, TITLE_FONT, title_col, true, Anchor::Start, tag);
                }
            }
        }
    }
    // Edges.
    let mut local: HashMap<usize, (usize, usize)> = HashMap::new();
    for (lvl, l) in levels.iter().enumerate() {
        if let Some(l) = l {
            for (k, ei) in l.edges.iter().enumerate() {
                local.insert(*ei, (lvl, k));
            }
        }
    }
    let center_of = |it: Item| -> (P, f32, f32, Outline) {
        match it {
            Item::Node(n) => {
                let (w, h) = node_sizes.get(n).copied().unwrap_or((0.0, 0.0));
                let shape = fc.nodes.get(n).map_or(NodeShape::Rect, |x| x.shape);
                (node_c.get(n).copied().unwrap_or((0.0, 0.0)), w, h, outline_of(shape))
            }
            Item::Sub(s) => {
                let b = sub_box.get(s).copied().unwrap_or_default();
                (b.center(), b.w, b.h, Outline::Box)
            }
        }
    };
    scene.edge_paths = vec![Vec::new(); fc.edges.len()];
    let mut label_prims: Vec<Prim> = Vec::new();
    for (ei, e) in fc.edges.iter().enumerate() {
        let (Some(a), Some(b)) = (endpoint(fc, &e.from), endpoint(fc, &e.to)) else { continue };
        let (ca, wa, ha, oa) = center_of(a);
        let (cb, wb, hb, ob) = center_of(b);
        let tag = Tag::Edge(ei);
        let ecol: Color = match (opts.colors, opts.arrows, a) {
            (ColorMode::Classic, _, _) => t.line,
            (_, Arrows::Graphite, _) => look::GRAPHITE,
            (mode, Arrows::Source, Item::Node(n)) => node_base(fc, n, mode).map_or(t.line, |c| c.1),
            (mode, Arrows::Source, Item::Sub(sg)) => cluster_colors(mode, sg).3,
        };
        let open = opts.colorful();
        let width = match e.kind.style {
            LineStyle::Thick => 3.5,
            _ => 1.8,
        };
        let mut label_at: Option<P> = None;
        let pts: Vec<P> = if a == b {
            // Self loop on the right side.
            let r = ca.0 + wa / 2.0;
            let top = ca.1 - ha / 4.0;
            let bot = ca.1 + ha / 4.0;
            let cmds = vec![PathCmd::Move(r, top), PathCmd::Cubic(r + 38.0, top - 16.0, r + 38.0, bot + 16.0, r - 1.0, bot)];
            let sampled = sample(&cmds);
            label_at = Some((r + 40.0 + label_size(e.label.as_deref().unwrap_or(""), LABEL_FONT, false, font).0 / 2.0, ca.1));
            if e.kind.style != LineStyle::Invisible {
                let mut p = Prim::new(PrimShape::Path(cmds)).stroke(ecol, width).tag(tag);
                if e.kind.style == LineStyle::Dotted {
                    p = p.dash(4.0, 3.5);
                }
                scene.push(p);
                for hp in head_prims(e.kind.head_end, (r, bot), (-1.0, 0.0), ecol, width, tag, open) {
                    scene.push(hp);
                }
            }
            if let Some(slot) = scene.edge_paths.get_mut(ei) {
                *slot = sampled;
            }
            Vec::new()
        } else {
            let mut interior: Vec<P> = Vec::new();
            if let (Some((lvl, k)), false) = (local.get(&ei), item_moved(a) || item_moved(b)) {
                if let Some(Some(l)) = levels.get(*lvl) {
                    interior = l.layout.edge_points.get(*k).map(|v| v.iter().map(|p| (p.0 + l.origin.0, p.1 + l.origin.1)).collect()).unwrap_or_default();
                    label_at = l.layout.label_pos.get(*k).copied().flatten().map(|p| (p.0 + l.origin.0, p.1 + l.origin.1));
                }
            }
            let mut pts = vec![ca];
            pts.extend(interior);
            pts.push(cb);
            let first_toward = pts.get(1).copied().unwrap_or(cb);
            let last_toward = pts.get(pts.len().saturating_sub(2)).copied().unwrap_or(ca);
            let n = pts.len();
            if let Some(p) = pts.first_mut() {
                *p = exit_point(oa, ca, wa, ha, first_toward);
            }
            if let Some(p) = pts.get_mut(n - 1) {
                *p = exit_point(ob, cb, wb, hb, last_toward);
            }
            pts
        };
        if pts.len() >= 2 {
            let mut pts = pts;
            let n = pts.len();
            let end_dir = norm(sub(pts.get(n - 1).copied().unwrap_or_default(), pts.get(n - 2).copied().unwrap_or_default()));
            let start_dir = norm(sub(pts.first().copied().unwrap_or_default(), pts.get(1).copied().unwrap_or_default()));
            let tip_end = pts.get(n - 1).copied().unwrap_or_default();
            let tip_start = pts.first().copied().unwrap_or_default();
            let inset_end = head_inset(e.kind.head_end, width, open);
            let inset_start = head_inset(e.kind.head_start, width, open);
            if let Some(p) = pts.get_mut(n - 1) {
                *p = sub(tip_end, scale(end_dir, inset_end));
            }
            if let Some(p) = pts.first_mut() {
                *p = sub(tip_start, scale(start_dir, inset_start));
            }
            let cmds = smooth(&pts);
            let sampled = sample(&cmds);
            if label_at.is_none() {
                label_at = sampled.get(sampled.len() / 2).copied();
            }
            if e.kind.style != LineStyle::Invisible {
                let mut p = Prim::new(PrimShape::Path(cmds)).stroke(ecol, width).tag(tag);
                if e.kind.style == LineStyle::Dotted {
                    p = p.dash(4.0, 3.5);
                }
                scene.push(p);
                for hp in head_prims(e.kind.head_end, tip_end, end_dir, ecol, width, tag, open) {
                    scene.push(hp);
                }
                for hp in head_prims(e.kind.head_start, tip_start, start_dir, ecol, width, tag, open) {
                    scene.push(hp);
                }
            }
            if let Some(slot) = scene.edge_paths.get_mut(ei) {
                *slot = sampled;
            }
        }
        if let (Some(label), Some(at)) = (e.label.as_ref().filter(|l| !l.is_empty()), label_at) {
            let (tw, th) = label_size(label, LABEL_FONT, false, font);
            label_prims.push(Prim::new(PrimShape::Rect { x: at.0 - tw / 2.0 - 5.0, y: at.1 - th / 2.0 - 3.0, w: tw + 10.0, h: th + 6.0, r: 3.0 }).fill(if opts.colorful() { [255, 255, 255, 255] } else { t.label_bg }).tag(tag));
            let mut tmp = Scene::default();
            let label_col = if opts.colorful() { look::GRAPHITE } else { t.text };
            push_text_lines(&mut tmp, at.0, at.1, label, LABEL_FONT, label_col, false, Anchor::Middle, tag);
            label_prims.append(&mut tmp.prims);
        }
    }
    for p in label_prims {
        scene.push(p);
    }
    // Nodes on top of the edge ends.
    for (i, n) in fc.nodes.iter().enumerate() {
        let st = resolve_style(fc, n, node_base(fc, i, opts.colors));
        draw_node(&mut scene, i, n, node_c.get(i).copied().unwrap_or((0.0, 0.0)), node_sizes.get(i).copied().unwrap_or((50.0, 38.0)), &st);
    }
    scene.finish(24.0);
    scene
}
