//! Small geometry helpers for edges and arrow heads.

use crate::scene::{PathCmd, Prim, PrimShape, Rect};

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

/// Side of a node where an edge meets it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    Top,
    Bottom,
    Left,
    Right,
}

/// The side of a `w`×`h` shape centred at `c` that faces `toward`.
pub fn side_toward(c: P, w: f32, h: f32, toward: P) -> Side {
    let d = sub(toward, c);
    if d.1.abs() * w.max(1.0) >= d.0.abs() * h.max(1.0) {
        if d.1 < 0.0 { Side::Top } else { Side::Bottom }
    } else if d.0 < 0.0 {
        Side::Left
    } else {
        Side::Right
    }
}

/// Unit vector pointing out of `side`.
pub fn side_normal(side: Side) -> P {
    match side {
        Side::Top => (0.0, -1.0),
        Side::Bottom => (0.0, 1.0),
        Side::Left => (-1.0, 0.0),
        Side::Right => (1.0, 0.0),
    }
}

/// Point on the outline of a shape, on `side`, `off` away from the middle of that side.
pub fn border_at(outline: Outline, c: P, w: f32, h: f32, side: Side, off: f32) -> P {
    let (a, b) = ((w / 2.0).max(1e-3), (h / 2.0).max(1e-3));
    // Along the side (`along`, half-length `half`) and out to the border (`reach`, half-depth `depth`).
    let (half, depth) = match side {
        Side::Top | Side::Bottom => (a, b),
        Side::Left | Side::Right => (b, a),
    };
    let along = off.clamp(-half * 0.8, half * 0.8);
    let k = match outline {
        Outline::Box => 1.0,
        Outline::Ellipse => (1.0 - (along / half).powi(2)).max(0.0).sqrt(),
        Outline::Diamond => (1.0 - along.abs() / half).max(0.0),
    };
    let reach = depth * k;
    match side {
        Side::Top => (c.0 + along, c.1 - reach),
        Side::Bottom => (c.0 + along, c.1 + reach),
        Side::Left => (c.0 - reach, c.1 + along),
        Side::Right => (c.0 + reach, c.1 + along),
    }
}

/// Does the segment `a`-`b` pass through the rectangle (Liang–Barsky clipping)?
pub fn segment_hits_rect(a: P, b: P, r: &Rect) -> bool {
    let d = sub(b, a);
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for (p, q) in [(-d.0, a.0 - r.x), (d.0, r.x + r.w - a.0), (-d.1, a.1 - r.y), (d.1, r.y + r.h - a.1)] {
        if p.abs() < 1e-6 {
            if q < 0.0 {
                return false;
            }
        } else {
            let t = q / p;
            if p < 0.0 {
                t0 = t0.max(t);
            } else {
                t1 = t1.min(t);
            }
            if t0 > t1 {
                return false;
            }
        }
    }
    true
}

/// A node an edge must not cross: its box and the shape inside it.
#[derive(Clone, Copy, Debug)]
pub struct Obstacle {
    pub rect: Rect,
    pub outline: Outline,
}

/// Do segments `a`-`b` and `c`-`d` cross? Touching at an end (two edges meeting at the same
/// node) does not count.
fn segments_cross(a: P, b: P, c: P, d: P) -> bool {
    let cross = |o: P, p: P, q: P| (p.0 - o.0) * (q.1 - o.1) - (p.1 - o.1) * (q.0 - o.0);
    let (d1, d2) = (cross(c, d, a), cross(c, d, b));
    let (d3, d4) = (cross(a, b, c), cross(a, b, d));
    d1 * d2 < 0.0 && d3 * d4 < 0.0
}

impl Obstacle {
    /// Does the segment pass through the shape grown by `m`?
    fn hit(&self, a: P, b: P, m: f32) -> bool {
        let r = Rect { x: self.rect.x - m, y: self.rect.y - m, w: self.rect.w + 2.0 * m, h: self.rect.h + 2.0 * m };
        if !segment_hits_rect(a, b, &r) {
            return false;
        }
        let (cx, cy) = r.center();
        let (hw, hh) = (r.w / 2.0, r.h / 2.0);
        let poly: Vec<P> = match self.outline {
            Outline::Box => return true,
            Outline::Diamond => vec![(cx, cy - hh), (cx + hw, cy), (cx, cy + hh), (cx - hw, cy)],
            Outline::Ellipse => (0..12)
                .map(|k| {
                    let t = k as f32 / 12.0 * std::f32::consts::TAU;
                    (cx + hw * t.cos(), cy + hh * t.sin())
                })
                .collect(),
        };
        let inside = |p: P| match self.outline {
            Outline::Diamond => (p.0 - cx).abs() / hw.max(1e-3) + (p.1 - cy).abs() / hh.max(1e-3) <= 1.0,
            _ => ((p.0 - cx) / hw.max(1e-3)).powi(2) + ((p.1 - cy) / hh.max(1e-3)).powi(2) <= 1.0,
        };
        inside(a) || inside(b) || (0..poly.len()).any(|i| match (poly.get(i), poly.get((i + 1) % poly.len())) {
            (Some(&p), Some(&q)) => segments_cross(a, b, p, q),
            _ => false,
        })
    }
}

/// Bounding box of some points.
pub fn bounds_of(pts: &[P]) -> Rect {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for p in pts {
        x0 = x0.min(p.0);
        y0 = y0.min(p.1);
        x1 = x1.max(p.0);
        y1 = y1.max(p.1);
    }
    if x0 > x1 { Rect::default() } else { Rect { x: x0, y: y0, w: x1 - x0, h: y1 - y0 } }
}

fn overlap(a: &Rect, b: &Rect) -> bool {
    a.x <= b.x + b.w && b.x <= a.x + a.w && a.y <= b.y + b.h && b.y <= a.y + a.h
}

/// Most detours added to one edge.
const MAX_DETOURS: usize = 16;

/// Bends a polyline around the shapes it crosses: each crossing becomes a detour along the side
/// of the shape's box nearest to where the line passes, `margin` away from it. Bounded number of
/// detours.
/// Among the two sides, the one with the shorter path wins, with each crossing of an `others`
/// line counting as a long detour.
pub fn avoid_boxes(pts: &mut Vec<P>, boxes: &[Obstacle], margin: f32, others: &[(Rect, &[P])]) {
    let hit_margin = margin * 0.5;
    let mut detours = 0;
    let mut i = 0;
    while i + 1 < pts.len() && detours < MAX_DETOURS {
        let (Some(&a), Some(&b)) = (pts.get(i), pts.get(i + 1)) else { break };
        let hit = boxes.iter().filter(|o| o.hit(a, b, hit_margin)).min_by(|p, q| len(sub(p.rect.center(), a)).total_cmp(&len(sub(q.rect.center(), a))));
        let Some(r) = hit.map(|o| o.rect) else {
            i += 1;
            continue;
        };
        detours += 1;
        let d = sub(b, a);
        let (x0, y0, x1, y1) = (r.x - margin, r.y - margin, r.x + r.w + margin, r.y + r.h + margin);
        let (cx, cy) = r.center();
        // Detour along one side; it stops where `b` is reached along the main direction, so the
        // line never turns back.
        let along = |side: f32| -> Vec<P> {
            if d.1.abs() >= d.0.abs() {
                let (enter, leave) = if d.1 >= 0.0 { (y0, y1) } else { (y1, y0) };
                let past = if d.1 >= 0.0 { b.1 >= leave } else { b.1 <= leave };
                if past { vec![(side, enter), (side, leave)] } else { vec![(side, enter)] }
            } else {
                let (enter, leave) = if d.0 >= 0.0 { (x0, x1) } else { (x1, x0) };
                let past = if d.0 >= 0.0 { b.0 >= leave } else { b.0 <= leave };
                if past { vec![(enter, side), (leave, side)] } else { vec![(enter, side)] }
            }
        };
        let (near_side, far_side) = if d.1.abs() >= d.0.abs() {
            let t = if d.1.abs() > 1e-3 { ((cy - a.1) / d.1).clamp(0.0, 1.0) } else { 0.5 };
            if a.0 + d.0 * t < cx { (x0, x1) } else { (x1, x0) }
        } else {
            let t = if d.0.abs() > 1e-3 { ((cx - a.0) / d.0).clamp(0.0, 1.0) } else { 0.5 };
            if a.1 + d.1 * t < cy { (y0, y1) } else { (y1, y0) }
        };
        let (near, far) = (along(near_side), along(far_side));
        // Only the edges near this detour can be crossed by it.
        let mut span_pts = vec![a, b];
        span_pts.extend_from_slice(&near);
        span_pts.extend_from_slice(&far);
        let span = bounds_of(&span_pts);
        let nearby: Vec<&[P]> = others.iter().filter(|(r, _)| overlap(r, &span)).map(|(_, o)| *o).collect();
        let cost = |via: &[P]| -> f32 {
            let mut path = vec![a];
            path.extend_from_slice(via);
            path.push(b);
            let length: f32 = path.windows(2).map(|w| match w {
                [p, q] => len(sub(*q, *p)),
                _ => 0.0,
            }).sum();
            let crossings = path.windows(2).map(|w| match w {
                [p, q] => nearby.iter().map(|o| o.windows(2).filter(|s| matches!(s, [c, e] if segments_cross(*p, *q, *c, *e))).count()).sum::<usize>(),
                _ => 0,
            }).sum::<usize>();
            length + 400.0 * crossings as f32
        };
        let detour = if cost(&far) < cost(&near) { far } else { near };
        for (k, p) in detour.into_iter().enumerate() {
            pts.insert(i + 1 + k, p);
        }
    }
    drop_reversals(pts);
}

/// Removes inner points where the line doubles back on itself (a turn sharper than ~150°).
pub fn drop_reversals(pts: &mut Vec<P>) {
    let mut i = 1;
    while i + 1 < pts.len() {
        let (Some(&p), Some(&q), Some(&r)) = (pts.get(i - 1), pts.get(i), pts.get(i + 1)) else { break };
        let (u, v) = (norm(sub(q, p)), norm(sub(r, q)));
        if u.0 * v.0 + u.1 * v.1 < -0.85 {
            pts.remove(i);
            i = i.saturating_sub(1).max(1);
        } else {
            i += 1;
        }
    }
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
        // Handles no longer than a third of the segment, so short segments do not loop.
        let limit = len(sub(p2, p1)) / 3.0;
        let clamp = |h: P| {
            let l = len(h);
            if l > limit && l > 1e-6 { scale(h, limit / l) } else { h }
        };
        let c1 = add(p1, clamp(scale(sub(p2, p0), 1.0 / 6.0)));
        let c2 = sub(p2, clamp(scale(sub(p3, p1), 1.0 / 6.0)));
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
