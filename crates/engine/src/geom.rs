//! Small geometry helpers for edges and arrow heads.

use crate::scene::{PathCmd, Prim, PrimShape};

pub type P = (f32, f32);

pub fn sub(a: P, b: P) -> P {
    (a.0 - b.0, a.1 - b.1)
}
pub fn add(a: P, b: P) -> P {
    (a.0 + b.0, a.1 + b.1)
}
pub fn scale(a: P, k: f32) -> P {
    (a.0 * k, a.1 * k)
}
pub fn len(a: P) -> f32 {
    (a.0 * a.0 + a.1 * a.1).sqrt()
}
pub fn norm(a: P) -> P {
    let l = len(a);
    if l < 1e-4 { (0.0, 1.0) } else { (a.0 / l, a.1 / l) }
}

/// Outline kinds for clipping an edge to a node border.
#[derive(Clone, Copy, Debug)]
pub enum Outline {
    Box,
    Ellipse,
    Diamond,
}

/// Point where the ray from the centre `c` of a `w`×`h` shape towards `toward` leaves it.
pub fn exit_point(outline: Outline, c: P, w: f32, h: f32, toward: P) -> P {
    let d = sub(toward, c);
    if len(d) < 1e-3 {
        return c;
    }
    let (a, b) = (w / 2.0, h / 2.0);
    let t = match outline {
        Outline::Box => {
            let tx = if d.0.abs() < 1e-6 { f32::MAX } else { a / d.0.abs() };
            let ty = if d.1.abs() < 1e-6 { f32::MAX } else { b / d.1.abs() };
            tx.min(ty)
        }
        Outline::Ellipse => {
            let q = (d.0 / a.max(1e-3)).powi(2) + (d.1 / b.max(1e-3)).powi(2);
            1.0 / q.sqrt().max(1e-6)
        }
        Outline::Diamond => {
            let q = d.0.abs() / a.max(1e-3) + d.1.abs() / b.max(1e-3);
            1.0 / q.max(1e-6)
        }
    };
    add(c, scale(d, t.min(1.0)))
}

/// Cubic Bézier segments through `pts` (Catmull-Rom), as path commands starting with a move.
pub fn smooth(pts: &[P]) -> Vec<PathCmd> {
    let mut out = Vec::new();
    let Some(first) = pts.first() else { return out };
    out.push(PathCmd::Move(first.0, first.1));
    if pts.len() == 2 {
        if let Some(l) = pts.get(1) {
            out.push(PathCmd::Line(l.0, l.1));
        }
        return out;
    }
    for i in 0..pts.len().saturating_sub(1) {
        let p1 = pts.get(i).copied().unwrap_or((0.0, 0.0));
        let p2 = pts.get(i + 1).copied().unwrap_or(p1);
        let p0 = if i == 0 { p1 } else { pts.get(i - 1).copied().unwrap_or(p1) };
        let p3 = pts.get(i + 2).copied().unwrap_or(p2);
        let c1 = (p1.0 + (p2.0 - p0.0) / 6.0, p1.1 + (p2.1 - p0.1) / 6.0);
        let c2 = (p2.0 - (p3.0 - p1.0) / 6.0, p2.1 - (p3.1 - p1.1) / 6.0);
        out.push(PathCmd::Cubic(c1.0, c1.1, c2.0, c2.1, p2.0, p2.1));
    }
    out
}

/// Samples a path made by [`smooth`] into a polyline (for hit testing).
pub fn sample(cmds: &[PathCmd]) -> Vec<P> {
    let mut out = Vec::new();
    let mut cur = (0.0f32, 0.0f32);
    for c in cmds {
        match *c {
            PathCmd::Move(x, y) | PathCmd::Line(x, y) => {
                cur = (x, y);
                out.push(cur);
            }
            PathCmd::Cubic(a, b, c2, d, x, y) => {
                for k in 1..=10 {
                    let t = k as f32 / 10.0;
                    let u = 1.0 - t;
                    let px = u * u * u * cur.0 + 3.0 * u * u * t * a + 3.0 * u * t * t * c2 + t * t * t * x;
                    let py = u * u * u * cur.1 + 3.0 * u * u * t * b + 3.0 * u * t * t * d + t * t * t * y;
                    out.push((px, py));
                }
                cur = (x, y);
            }
            PathCmd::Close => {}
        }
    }
    out
}

/// Distance from point `p` to the segment `a`-`b`.
pub fn dist_to_segment(p: P, a: P, b: P) -> f32 {
    let ab = sub(b, a);
    let l2 = ab.0 * ab.0 + ab.1 * ab.1;
    if l2 < 1e-6 {
        return len(sub(p, a));
    }
    let t = (((p.0 - a.0) * ab.0 + (p.1 - a.1) * ab.1) / l2).clamp(0.0, 1.0);
    len(sub(p, add(a, scale(ab, t))))
}

/// An arrow head or marker at `tip` pointing along `dir` (unit vector). Returns primitives.
/// Shapes of an edge head. `open` draws the arrow as an open "V" instead of a filled triangle;
/// renderers ask for it in every colour mode except the classic Mermaid one.
pub fn head_prims(kind: crate::model::Head, tip: P, dir: P, color: [u8; 4], width: f32, tag: crate::scene::Tag, open: bool) -> Vec<Prim> {
    use crate::model::Head;
    let perp = (-dir.1, dir.0);
    match kind {
        Head::None => Vec::new(),
        Head::Arrow if open => {
            let l = 11.0 + width;
            let w = 5.5 + width * 0.5;
            let back = sub(tip, scale(dir, l));
            let a = add(back, scale(perp, w));
            let b = sub(back, scale(perp, w));
            vec![Prim::new(PrimShape::Path(vec![PathCmd::Move(a.0, a.1), PathCmd::Line(tip.0, tip.1), PathCmd::Line(b.0, b.1)])).stroke(color, width.max(1.6)).tag(tag)]
        }
        Head::Arrow => {
            let l = 10.0 + width;
            let w = 4.5 + width * 0.6;
            let back = sub(tip, scale(dir, l));
            let a = add(back, scale(perp, w));
            let b = sub(back, scale(perp, w));
            vec![Prim::new(PrimShape::Polygon(vec![tip, a, b])).fill(color).stroke(color, 1.0).tag(tag)]
        }
        Head::Circle => {
            let r = 5.0;
            let c = sub(tip, scale(dir, r));
            vec![Prim::new(PrimShape::Ellipse { cx: c.0, cy: c.1, rx: r, ry: r }).fill([255, 255, 255, 255]).stroke(color, 1.5).tag(tag)]
        }
        Head::Cross => {
            let c = sub(tip, scale(dir, 6.0));
            let a = scale(add(dir, perp), 5.0);
            let b = scale(sub(dir, perp), 5.0);
            vec![
                Prim::new(PrimShape::Path(vec![PathCmd::Move(c.0 - a.0, c.1 - a.1), PathCmd::Line(c.0 + a.0, c.1 + a.1)])).stroke(color, 2.0).tag(tag),
                Prim::new(PrimShape::Path(vec![PathCmd::Move(c.0 - b.0, c.1 - b.1), PathCmd::Line(c.0 + b.0, c.1 + b.1)])).stroke(color, 2.0).tag(tag),
            ]
        }
    }
}

/// How far the line should stop before the tip for a head.
pub fn head_inset(kind: crate::model::Head, width: f32, open: bool) -> f32 {
    use crate::model::Head;
    match kind {
        Head::None => 0.0,
        Head::Arrow if open => width * 0.6,
        Head::Arrow => 9.0 + width,
        Head::Circle => 10.0,
        Head::Cross => 8.0,
    }
}
