//! Hand-drawn look: wobbly double strokes in the spirit of whiteboard sketches. Deterministic, so
//! the same diagram always looks the same.

use crate::geom::{P, add, len, norm, scale, smooth, sub};
use crate::scene::{PathCmd, Prim, PrimShape, Scene, Tag};

struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD1B5_4A32_D192_ED03)
    }
    /// A number in -1..1.
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        ((self.0 >> 40) as f32 / (1u64 << 24) as f32) * 2.0 - 1.0
    }
}

fn rounded_rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Vec<P> {
    let r = r.min(w / 2.0).min(h / 2.0);
    if r < 1.0 {
        return vec![(x, y), (x + w, y), (x + w, y + h), (x, y + h)];
    }
    let mut pts = Vec::new();
    let corners = [(x + w - r, y + r, -90.0f32), (x + w - r, y + h - r, 0.0), (x + r, y + h - r, 90.0), (x + r, y + r, 180.0)];
    for (cx, cy, start) in corners {
        for k in 0..=4 {
            let a = (start + 90.0 * k as f32 / 4.0).to_radians();
            pts.push((cx + r * a.cos(), cy + r * a.sin()));
        }
    }
    pts
}

fn ellipse_pts(cx: f32, cy: f32, rx: f32, ry: f32) -> Vec<P> {
    (0..28).map(|k| {
        let a = k as f32 / 28.0 * std::f32::consts::TAU;
        (cx + rx * a.cos(), cy + ry * a.sin())
    }).collect()
}

/// One hand-drawn pass over a closed outline: every side is a slightly bowed stroke.
fn closed_pass(pts: &[P], rng: &mut Rng, amp: f32) -> Vec<PathCmd> {
    let mut out = Vec::new();
    let n = pts.len();
    for i in 0..n {
        let a = pts.get(i).copied().unwrap_or_default();
        let b = pts.get((i + 1) % n.max(1)).copied().unwrap_or_default();
        let d = sub(b, a);
        let l = len(d);
        let short = l < 14.0;
        let nrm = norm(d);
        let perp = (-nrm.1, nrm.0);
        let j = if short { 0.15 } else { amp };
        let a2 = add(a, (rng.next() * j, rng.next() * j));
        let b2 = add(b, (rng.next() * j, rng.next() * j));
        let bow = if short { 0.0 } else { (0.6 + l * 0.006).min(2.2) * rng.next() };
        let c1 = add(add(a2, scale(d, 0.33)), scale(perp, bow));
        let c2 = add(add(a2, scale(d, 0.66)), scale(perp, bow * 0.8 + rng.next() * 0.4));
        out.push(PathCmd::Move(a2.0, a2.1));
        out.push(PathCmd::Cubic(c1.0, c1.1, c2.0, c2.1, b2.0, b2.1));
    }
    out
}

/// Flattens path commands to sub-paths of points (closed flag for `Close`).
fn flatten(cmds: &[PathCmd]) -> Vec<(Vec<P>, bool)> {
    let mut out = Vec::new();
    let mut cur: Vec<P> = Vec::new();
    let mut last = (0.0f32, 0.0f32);
    for c in cmds {
        match *c {
            PathCmd::Move(x, y) => {
                if cur.len() > 1 {
                    out.push((std::mem::take(&mut cur), false));
                }
                cur.clear();
                cur.push((x, y));
                last = (x, y);
            }
            PathCmd::Line(x, y) => {
                cur.push((x, y));
                last = (x, y);
            }
            PathCmd::Cubic(a, b, c2, d, x, y) => {
                for k in 1..=12 {
                    let t = k as f32 / 12.0;
                    let u = 1.0 - t;
                    cur.push((u * u * u * last.0 + 3.0 * u * u * t * a + 3.0 * u * t * t * c2 + t * t * t * x, u * u * u * last.1 + 3.0 * u * u * t * b + 3.0 * u * t * t * d + t * t * t * y));
                }
                last = (x, y);
            }
            PathCmd::Close => {
                if cur.len() > 2 {
                    out.push((std::mem::take(&mut cur), true));
                }
                cur.clear();
            }
        }
    }
    if cur.len() > 1 {
        out.push((cur, false));
    }
    out
}

/// A wobbly open line: samples every ~14 px, nudges sideways, and smooths.
fn open_pass(pts: &[P], rng: &mut Rng) -> Vec<PathCmd> {
    let mut dense: Vec<P> = Vec::new();
    for w in pts.windows(2) {
        let (Some(a), Some(b)) = (w.first().copied(), w.get(1).copied()) else { continue };
        let d = sub(b, a);
        let l = len(d);
        let n = ((l / 30.0) as usize).clamp(1, 60);
        let nrm = norm(d);
        let perp = (-nrm.1, nrm.0);
        for k in 0..n {
            let t = k as f32 / n as f32;
            let p = add(a, scale(d, t));
            dense.push(if k == 0 && dense.is_empty() { p } else { add(p, scale(perp, rng.next() * 0.5)) });
        }
    }
    if let Some(last) = pts.last() {
        dense.push(*last);
    }
    if dense.len() < 2 { Vec::new() } else { smooth(&dense) }
}

fn is_edge_part(t: Tag) -> bool {
    matches!(t, Tag::Edge(_))
}

/// Turns every outline of the scene into hand-drawn strokes. Arrow heads and label backgrounds
/// stay crisp so they remain readable.
pub fn roughen(scene: &mut Scene) {
    let old = std::mem::take(&mut scene.prims);
    for (i, p) in old.into_iter().enumerate() {
        let mut rng = Rng::new(i as u64 + 1);
        let tag = p.tag;
        let stroke_prim = |cmds: Vec<PathCmd>| -> Prim {
            let mut q = Prim::new(PrimShape::Path(cmds)).tag(tag);
            q.stroke = p.stroke;
            q.stroke_width = p.stroke_width;
            q.dash = p.dash;
            q
        };
        let fill_only = |shape: PrimShape| -> Option<Prim> { p.fill.filter(|c| c[3] > 0).map(|c| Prim::new(shape).fill(c).tag(tag)) };
        match &p.shape {
            PrimShape::Rect { x, y, w, h, r } if !is_edge_part(tag) && p.stroke.is_some() => {
                let pts = rounded_rect(*x, *y, *w, *h, *r);
                scene.prims.extend(fill_only(PrimShape::Polygon(pts.clone())));
                if p.dash.is_some() {
                    scene.prims.push(stroke_prim(closed_pass(&pts, &mut rng, 0.8)));
                } else {
                    scene.prims.push(stroke_prim(closed_pass(&pts, &mut rng, 1.0)));
                    scene.prims.push(stroke_prim(closed_pass(&pts, &mut rng, 1.3)));
                }
            }
            PrimShape::Ellipse { cx, cy, rx, ry } if !is_edge_part(tag) && p.stroke.is_some() => {
                let pts = ellipse_pts(*cx, *cy, *rx, *ry);
                scene.prims.extend(fill_only(PrimShape::Polygon(pts.clone())));
                let a = open_closed(&pts, &mut rng);
                scene.prims.push(stroke_prim(a));
                let b = open_closed(&pts, &mut rng);
                scene.prims.push(stroke_prim(b));
            }
            PrimShape::Polygon(pts) if !is_edge_part(tag) && p.stroke.is_some() => {
                scene.prims.extend(fill_only(PrimShape::Polygon(pts.clone())));
                scene.prims.push(stroke_prim(closed_pass(pts, &mut rng, 1.0)));
                scene.prims.push(stroke_prim(closed_pass(pts, &mut rng, 1.3)));
            }
            PrimShape::Path(cmds) if p.stroke.is_some() => {
                let subs = flatten(cmds);
                if subs.iter().any(|(_, c)| *c) {
                    // Closed body (cylinder): keep the fill, hand-draw the outline.
                    scene.prims.extend(fill_only(PrimShape::Path(cmds.clone())));
                    for (pts, closed) in &subs {
                        let c = if *closed { closed_pass(pts, &mut rng, 0.9) } else { open_pass(pts, &mut rng) };
                        scene.prims.push(stroke_prim(c));
                    }
                } else {
                    let mut all: Vec<PathCmd> = Vec::new();
                    for (pts, _) in &subs {
                        all.extend(open_pass(pts, &mut rng));
                    }
                    if all.is_empty() {
                        scene.prims.push(p);
                    } else {
                        scene.prims.push(stroke_prim(all));
                    }
                }
            }
            _ => scene.prims.push(p),
        }
    }
}

/// A closed smooth loop with a little wobble (for ellipses).
fn open_closed(pts: &[P], rng: &mut Rng) -> Vec<PathCmd> {
    let mut v: Vec<P> = pts.iter().map(|p| add(*p, (rng.next() * 0.7, rng.next() * 0.7))).collect();
    // Overshoot a little past the start, as a hand does.
    for k in 0..3 {
        if let Some(p) = v.get(k).copied() {
            v.push(add(p, (rng.next() * 0.9, rng.next() * 0.9)));
        }
    }
    smooth(&v)
}
