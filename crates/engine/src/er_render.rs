//! erDiagram → scene.

use crate::geom::*;
use crate::layout::{LEdge, LItem, Spacing, layered};
use crate::model::*;
use crate::scene::*;
use crate::text::{label_size, text_width};

const FONT: f32 = 14.0;
const HEAD_H: f32 = 32.0;
const ROW_H: f32 = 24.0;

struct Table {
    w: f32,
    h: f32,
    cols: [f32; 4],
}

fn table_of(e: &ErEntity) -> Table {
    let hw = text_width(&e.label, 15.0, true) + 32.0;
    let mut cols = [0f32; 4];
    for a in &e.attributes {
        cols[0] = cols[0].max(text_width(&a.ty, FONT, false));
        cols[1] = cols[1].max(text_width(&a.name, FONT, false));
        cols[2] = cols[2].max(text_width(&a.keys.join(", "), FONT, false));
        cols[3] = cols[3].max(text_width(&a.comment, FONT - 1.0, false));
    }
    let mut cw = 0.0;
    for c in &mut cols {
        if *c > 0.0 {
            *c += 18.0;
            cw += *c;
        }
    }
    let w = hw.max(cw + 8.0).max(110.0);
    if cw > 0.0 {
        // Give the extra width to the name column.
        let extra = w - cw - 8.0;
        if cols[1] > 0.0 {
            cols[1] += extra;
        } else if cols[0] > 0.0 {
            cols[0] += extra;
        }
    }
    let h = HEAD_H + if e.attributes.is_empty() { 10.0 } else { e.attributes.len() as f32 * ROW_H + 6.0 };
    Table { w, h, cols }
}

fn text(x: f32, y: f32, s: &str, size: f32, bold: bool, anchor: Anchor, color: Color) -> Prim {
    Prim::new(PrimShape::Text { x, y, text: s.to_string(), size, bold, italic: false, mono: false, anchor }).fill(color)
}

/// Cardinality marker at `p`; `d` points from the entity into the line.
fn marker(out: &mut Vec<Prim>, card: Cardinality, p: P, d: P, color: Color) {
    let perp = (-d.1, d.0);
    let at = |dist: f32, off: f32| add(add(p, scale(d, dist)), scale(perp, off));
    let bar = |dist: f32| {
        let a = at(dist, -7.0);
        let b = at(dist, 7.0);
        Prim::new(PrimShape::Path(vec![PathCmd::Move(a.0, a.1), PathCmd::Line(b.0, b.1)])).stroke(color, 1.6)
    };
    let foot = |out: &mut Vec<Prim>| {
        let tip = at(14.0, 0.0);
        for off in [-8.0, 0.0, 8.0] {
            let b = at(0.0, off);
            out.push(Prim::new(PrimShape::Path(vec![PathCmd::Move(tip.0, tip.1), PathCmd::Line(b.0, b.1)])).stroke(color, 1.6));
        }
    };
    let ring = |dist: f32| {
        let c = at(dist, 0.0);
        Prim::new(PrimShape::Ellipse { cx: c.0, cy: c.1, rx: 5.0, ry: 5.0 }).fill([255, 255, 255, 255]).stroke(color, 1.6)
    };
    match card {
        Cardinality::ExactlyOne => {
            out.push(bar(8.0));
            out.push(bar(13.0));
        }
        Cardinality::ZeroOrOne => {
            out.push(bar(8.0));
            out.push(ring(17.0));
        }
        Cardinality::OneOrMore => {
            foot(out);
            out.push(bar(18.0));
        }
        Cardinality::ZeroOrMore => {
            foot(out);
            out.push(ring(21.0));
        }
    }
}

pub fn render(er: &ErDiagram, opts: &crate::RenderOptions) -> Scene {
    let t = &DEFAULT_THEME;
    let tables: Vec<Table> = er.entities.iter().map(table_of).collect();
    let items: Vec<LItem> = tables.iter().map(|tb| LItem { w: tb.w, h: tb.h }).collect();
    let edges: Vec<LEdge> = er
        .relations
        .iter()
        .map(|r| LEdge {
            from: r.a,
            to: r.b,
            min_len: 1,
            label: if r.label.is_empty() {
                None
            } else {
                let (w, h) = label_size(&r.label, FONT, false);
                Some((w + 14.0, h + 8.0))
            },
        })
        .collect();
    let layout = layered(&items, &edges, er.direction, Spacing { node_sep: 70.0, rank_sep: 90.0, edge_sep: 20.0 });
    let mut scene = Scene { background: Some(t.background), ..Scene::default() };
    scene.edge_paths = vec![Vec::new(); er.relations.len()];
    let center = |i: usize| layout.centers.get(i).copied().unwrap_or((0.0, 0.0));
    let mut tops: Vec<Prim> = Vec::new();
    for (ri, r) in er.relations.iter().enumerate() {
        let (Some(ta), Some(tb)) = (tables.get(r.a), tables.get(r.b)) else { continue };
        let (ca, cb) = (center(r.a), center(r.b));
        let tag = Tag::Edge(ri);
        let color = [90, 90, 110, 255];
        let pts: Vec<P>;
        let mut label_at: Option<P> = layout.label_pos.get(ri).copied().flatten();
        let (da, db);
        if r.a == r.b {
            let x = ca.0 + ta.w / 2.0;
            pts = vec![(x, ca.1 - 14.0), (x + 46.0, ca.1 - 20.0), (x + 46.0, ca.1 + 20.0), (x, ca.1 + 14.0)];
            da = (1.0, 0.0);
            db = (1.0, 0.0);
            label_at = Some((x + 46.0 + text_width(&r.label, FONT, false) / 2.0 + 8.0, ca.1));
        } else {
            let mut inner: Vec<P> = layout.edge_points.get(ri).cloned().unwrap_or_default();
            let first = inner.first().copied().unwrap_or(cb);
            let last = inner.last().copied().unwrap_or(ca);
            let s = exit_point(Outline::Box, ca, ta.w, ta.h, first);
            let e = exit_point(Outline::Box, cb, tb.w, tb.h, last);
            da = border_normal(s, ca, ta.w, ta.h);
            db = border_normal(e, cb, tb.w, tb.h);
            let mut v = vec![s];
            v.append(&mut inner);
            v.push(e);
            pts = v;
        }
        let n = pts.len();
        // The marker sits at the ends; keep the line itself running to the border.
        let cmds = smooth(&pts);
        let sampled = sample(&cmds);
        if label_at.is_none() {
            label_at = sampled.get(sampled.len() / 2).copied();
        }
        let mut p = Prim::new(PrimShape::Path(cmds)).stroke(color, 1.6).tag(tag);
        if !r.identifying {
            p = p.dash(6.0, 4.0);
        }
        scene.push(p);
        let (s, e) = (pts.first().copied().unwrap_or_default(), pts.get(n - 1).copied().unwrap_or_default());
        marker(&mut tops, r.card_a, s, da, color);
        marker(&mut tops, r.card_b, e, db, color);
        if let Some(slot) = scene.edge_paths.get_mut(ri) {
            *slot = sampled;
        }
        if let (false, Some(at)) = (r.label.is_empty(), label_at) {
            let (tw, th) = label_size(&r.label, FONT, false);
            tops.push(Prim::new(PrimShape::Rect { x: at.0 - tw / 2.0 - 6.0, y: at.1 - th / 2.0 - 3.0, w: tw + 12.0, h: th + 6.0, r: 3.0 }).fill([255, 255, 255, 255]).tag(tag));
            tops.push(text(at.0, at.1 + FONT * 0.34, &r.label, FONT, false, Anchor::Middle, t.text).tag(tag));
        }
    }
    for p in tops {
        scene.push(p);
    }
    for (i, e) in er.entities.iter().enumerate() {
        let Some(tb) = tables.get(i) else { continue };
        let c = center(i);
        let (x, y) = (c.0 - tb.w / 2.0, c.1 - tb.h / 2.0);
        let tag = Tag::Node(i);
        let (hfill, hstroke) = if opts.colorful { palette(i) } else { (t.node_fill, t.node_stroke) };
        scene.push(Prim::new(PrimShape::Rect { x, y, w: tb.w, h: tb.h, r: 0.0 }).fill([255, 255, 255, 255]).stroke(hstroke, 1.8).tag(tag));
        scene.push(Prim::new(PrimShape::Rect { x, y, w: tb.w, h: HEAD_H, r: 0.0 }).fill(hfill).stroke(hstroke, 1.8).tag(tag));
        scene.push(text(c.0, y + HEAD_H / 2.0 + 5.2, &e.label, 15.0, true, Anchor::Middle, t.text).tag(tag));
        for (ai, a) in e.attributes.iter().enumerate() {
            let ry = y + HEAD_H + ai as f32 * ROW_H;
            if ai % 2 == 1 {
                scene.push(Prim::new(PrimShape::Rect { x: x + 0.75, y: ry, w: tb.w - 1.5, h: ROW_H, r: 0.0 }).fill([244, 244, 252, 255]).tag(tag));
            }
            let base = ry + ROW_H / 2.0 + 4.8;
            let mut cx = x + 9.0;
            let cells = [a.ty.clone(), a.name.clone(), a.keys.join(", "), a.comment.clone()];
            for (ci, cell) in cells.iter().enumerate() {
                if !cell.is_empty() {
                    let size = if ci == 3 { FONT - 1.0 } else { FONT };
                    let col = if ci == 3 { [110, 110, 120, 255] } else { t.text };
                    scene.push(text(cx, base, cell, size, ci == 2, Anchor::Start, col).tag(tag));
                }
                cx += tb.cols.get(ci).copied().unwrap_or(0.0);
            }
        }
    }
    scene.node_boxes = (0..er.entities.len())
        .map(|i| {
            let c = center(i);
            let tb = tables.get(i);
            let (w, h) = tb.map_or((0.0, 0.0), |t| (t.w, t.h));
            Rect { x: c.0 - w / 2.0, y: c.1 - h / 2.0, w, h }
        })
        .collect();
    scene.finish(24.0);
    scene
}

/// Unit vector pointing out of a box at the border point `p`.
fn border_normal(p: P, c: P, w: f32, h: f32) -> P {
    let dx = (p.0 - c.0) / (w / 2.0).max(1.0);
    let dy = (p.1 - c.1) / (h / 2.0).max(1.0);
    if dy.abs() >= dx.abs() { (0.0, dy.signum()) } else { (dx.signum(), 0.0) }
}
