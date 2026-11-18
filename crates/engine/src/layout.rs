//! Layered (Sugiyama-style) graph layout: cycle removal, ranking, crossing reduction and
//! coordinate assignment. Works on boxes and edges only; callers draw the result.

use crate::model::Direction;

#[derive(Clone, Copy, Debug)]
pub struct LItem {
    pub w: f32,
    pub h: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct LEdge {
    pub from: usize,
    pub to: usize,
    pub min_len: usize,
    /// Size of the edge label, if any.
    pub label: Option<(f32, f32)>,
}

#[derive(Clone, Copy, Debug)]
pub struct Spacing {
    /// Between nodes of the same layer.
    pub node_sep: f32,
    /// Between layers (a label layer counts as half).
    pub rank_sep: f32,
    /// Between edges passing the same layer.
    pub edge_sep: f32,
}

impl Default for Spacing {
    fn default() -> Self {
        Self { node_sep: 50.0, rank_sep: 56.0, edge_sep: 18.0 }
    }
}

#[derive(Clone, Debug, Default)]
pub struct LayoutResult {
    /// Centre of every item.
    pub centers: Vec<(f32, f32)>,
    /// Bounding size of the items (and labels).
    pub width: f32,
    pub height: f32,
    /// Interior points of each edge, from `from` to `to`.
    pub edge_points: Vec<Vec<(f32, f32)>>,
    /// Where the label of each edge goes.
    pub label_pos: Vec<Option<(f32, f32)>>,
}

const MAX_DUMMIES: usize = 40_000;

struct G {
    /// Real nodes first, then dummies.
    w: Vec<f32>,
    h: Vec<f32>,
    layer: Vec<usize>,
    is_dummy: Vec<bool>,
    up: Vec<Vec<usize>>,
    down: Vec<Vec<usize>>,
}

fn inversions(pairs: &mut [(usize, usize)]) -> usize {
    // Count crossings between edges of two adjacent layers with a Fenwick tree.
    pairs.sort_unstable();
    let max_b = pairs.iter().map(|p| p.1).max().unwrap_or(0) + 2;
    let mut tree = vec![0usize; max_b + 1];
    let mut count = 0usize;
    let mut seen = 0usize;
    for &(_, b) in pairs.iter() {
        // number of earlier edges with a larger b
        let mut i = b + 1;
        let mut le = 0usize;
        while i > 0 {
            le += tree.get(i).copied().unwrap_or(0);
            i -= i & i.wrapping_neg();
        }
        count += seen - le;
        let mut j = b + 1;
        while j <= max_b {
            if let Some(t) = tree.get_mut(j) {
                *t += 1;
            }
            j += j & j.wrapping_neg();
        }
        seen += 1;
    }
    count
}

fn count_crossings(g: &G, layers: &[Vec<usize>]) -> usize {
    let mut pos = vec![0usize; g.w.len()];
    for l in layers {
        for (i, v) in l.iter().enumerate() {
            if let Some(p) = pos.get_mut(*v) {
                *p = i;
            }
        }
    }
    let mut total = 0usize;
    for l in layers {
        let mut pairs: Vec<(usize, usize)> = Vec::new();
        for v in l {
            let pv = pos.get(*v).copied().unwrap_or(0);
            for d in g.down.get(*v).map(Vec::as_slice).unwrap_or(&[]) {
                pairs.push((pv, pos.get(*d).copied().unwrap_or(0)));
            }
        }
        if pairs.len() > 1 {
            total += inversions(&mut pairs);
        }
    }
    total
}

/// Weighted isotonic regression: positions closest to `desired` that keep `min_gap` between
/// neighbours (order is preserved).
fn place_row(desired: &[f32], gaps: &[f32]) -> Vec<f32> {
    let m = desired.len();
    let mut offs = Vec::with_capacity(m);
    let mut acc = 0.0f32;
    for i in 0..m {
        offs.push(acc);
        acc += gaps.get(i).copied().unwrap_or(0.0);
    }
    // blocks: (sum, count, start index)
    let mut blocks: Vec<(f32, f32, usize)> = Vec::new();
    for i in 0..m {
        let t = desired.get(i).copied().unwrap_or(0.0) - offs.get(i).copied().unwrap_or(0.0);
        blocks.push((t, 1.0, i));
        while blocks.len() > 1 {
            let n = blocks.len();
            let (s2, c2, _) = blocks.get(n - 1).copied().unwrap_or((0.0, 1.0, 0));
            let (s1, c1, st1) = blocks.get(n - 2).copied().unwrap_or((0.0, 1.0, 0));
            if s1 / c1 > s2 / c2 {
                blocks.truncate(n - 2);
                blocks.push((s1 + s2, c1 + c2, st1));
            } else {
                break;
            }
        }
    }
    let mut out = vec![0.0f32; m];
    for (bi, (s, c, st)) in blocks.iter().enumerate() {
        let end = blocks.get(bi + 1).map_or(m, |b| b.2);
        let mean = s / c;
        for i in *st..end {
            if let Some(o) = out.get_mut(i) {
                *o = mean + offs.get(i).copied().unwrap_or(0.0);
            }
        }
    }
    out
}

/// Lays out the items. Sizes are given for a top-to-bottom layout in the caller's orientation:
/// this function swaps them itself for horizontal directions.
pub fn layered(items: &[LItem], edges: &[LEdge], dir: Direction, sp: Spacing) -> LayoutResult {
    let n = items.len();
    if n == 0 {
        return LayoutResult::default();
    }
    let horiz = dir.horizontal();
    let sw = |it: &LItem| if horiz { it.h } else { it.w };
    let sh = |it: &LItem| if horiz { it.w } else { it.h };

    // --- 1. cycle removal over non-loop edges -------------------------------------------------
    let mut es: Vec<(usize, usize, usize, usize, bool)> = Vec::new(); // from,to,minlen2,edge idx,reversed
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n]; // indexes into es
    for (ei, e) in edges.iter().enumerate() {
        if e.from == e.to || e.from >= n || e.to >= n {
            continue;
        }
        es.push((e.from, e.to, e.min_len.max(1) * 2, ei, false));
        if let Some(a) = adj.get_mut(e.from) {
            a.push(es.len() - 1);
        }
    }
    // iterative DFS
    let mut color = vec![0u8; n];
    let mut reverse = vec![false; es.len()];
    for s in 0..n {
        if color.get(s).copied() != Some(0) {
            continue;
        }
        let mut stack: Vec<(usize, usize)> = vec![(s, 0)];
        if let Some(c) = color.get_mut(s) {
            *c = 1;
        }
        while let Some(&(v, i)) = stack.last() {
            let out = adj.get(v).map(Vec::as_slice).unwrap_or(&[]);
            if let Some(&ei) = out.get(i) {
                if let Some(top) = stack.last_mut() {
                    top.1 += 1;
                }
                let to = es.get(ei).map_or(0, |e| e.1);
                match color.get(to).copied().unwrap_or(2) {
                    0 => {
                        if let Some(c) = color.get_mut(to) {
                            *c = 1;
                        }
                        stack.push((to, 0));
                    }
                    1 => {
                        if let Some(r) = reverse.get_mut(ei) {
                            *r = true;
                        }
                    }
                    _ => {}
                }
            } else {
                if let Some(c) = color.get_mut(v) {
                    *c = 2;
                }
                stack.pop();
            }
        }
    }
    for (i, e) in es.iter_mut().enumerate() {
        if reverse.get(i).copied().unwrap_or(false) {
            *e = (e.1, e.0, e.2, e.3, true);
        }
    }

    // --- 2. ranking (longest path) -----------------------------------------------------------
    let mut indeg = vec![0usize; n];
    let mut succ: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut pred: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (i, e) in es.iter().enumerate() {
        if let Some(d) = indeg.get_mut(e.1) {
            *d += 1;
        }
        if let Some(s) = succ.get_mut(e.0) {
            s.push(i);
        }
        if let Some(p) = pred.get_mut(e.1) {
            p.push(i);
        }
    }
    let mut rank = vec![0usize; n];
    let mut queue: std::collections::VecDeque<usize> = (0..n).filter(|v| indeg.get(*v) == Some(&0)).collect();
    let mut topo = Vec::with_capacity(n);
    let mut deg = indeg.clone();
    while let Some(v) = queue.pop_front() {
        topo.push(v);
        for &ei in succ.get(v).map(Vec::as_slice).unwrap_or(&[]) {
            if let Some(e) = es.get(ei) {
                let r = rank.get(v).copied().unwrap_or(0) + e.2;
                if let Some(rt) = rank.get_mut(e.1) {
                    *rt = (*rt).max(r);
                }
                if let Some(d) = deg.get_mut(e.1) {
                    *d = d.saturating_sub(1);
                    if *d == 0 {
                        queue.push_back(e.1);
                    }
                }
            }
        }
    }
    // Pull nodes without predecessors towards their successors.
    for &v in topo.iter().rev() {
        if pred.get(v).is_some_and(Vec::is_empty) {
            let mut best: Option<usize> = None;
            for &ei in succ.get(v).map(Vec::as_slice).unwrap_or(&[]) {
                if let Some(e) = es.get(ei) {
                    let cand = rank.get(e.1).copied().unwrap_or(0).saturating_sub(e.2);
                    best = Some(best.map_or(cand, |b| b.min(cand)));
                }
            }
            if let (Some(b), Some(r)) = (best, rank.get_mut(v)) {
                *r = b;
            }
        }
    }

    // --- 3. dummies ---------------------------------------------------------------------------
    let mut g = G {
        w: items.iter().map(&sw).collect(),
        h: items.iter().map(&sh).collect(),
        layer: rank.clone(),
        is_dummy: vec![false; n],
        up: vec![Vec::new(); n],
        down: vec![Vec::new(); n],
    };
    let mut chains: Vec<Option<(Vec<usize>, bool, Option<usize>)>> = vec![None; edges.len()];
    let mut dummies = 0usize;
    for e in &es {
        let (u, v, _, ei, rev) = *e;
        let lu = g.layer.get(u).copied().unwrap_or(0);
        let lv = g.layer.get(v).copied().unwrap_or(0);
        let span = lv.saturating_sub(lu);
        let mut chain = vec![u];
        let mut label_dummy = None;
        let label = edges.get(ei).and_then(|x| x.label);
        if span > 1 && dummies + span <= MAX_DUMMIES {
            let mid = span / 2;
            for k in 1..span {
                let id = g.w.len();
                let (mut w, mut h) = (1.0, 1.0);
                if k == mid {
                    if let Some((lw, lh)) = label {
                        let (a, b) = if horiz { (lh, lw) } else { (lw, lh) };
                        w = a;
                        h = b;
                    }
                    if label.is_some() {
                        label_dummy = Some(id);
                    }
                }
                g.w.push(w);
                g.h.push(h);
                g.layer.push(lu + k);
                g.is_dummy.push(true);
                g.up.push(Vec::new());
                g.down.push(Vec::new());
                chain.push(id);
                dummies += 1;
            }
        }
        chain.push(v);
        for pair in chain.windows(2) {
            if let [a, b] = pair {
                if g.layer.get(*b).copied().unwrap_or(0) == g.layer.get(*a).copied().unwrap_or(0) + 1 {
                    if let Some(d) = g.down.get_mut(*a) {
                        d.push(*b);
                    }
                    if let Some(u) = g.up.get_mut(*b) {
                        u.push(*a);
                    }
                }
            }
        }
        if let Some(c) = chains.get_mut(ei) {
            *c = Some((chain, rev, label_dummy));
        }
    }
    let total = g.w.len();
    let nlayers = g.layer.iter().copied().max().unwrap_or(0) + 1;

    // --- 4. initial order (DFS preorder) ------------------------------------------------------
    let mut layers: Vec<Vec<usize>> = vec![Vec::new(); nlayers];
    let mut seen = vec![false; total];
    let roots: Vec<usize> = (0..n).collect();
    for r in roots {
        if seen.get(r).copied().unwrap_or(true) {
            continue;
        }
        let mut stack = vec![r];
        while let Some(v) = stack.pop() {
            if seen.get(v).copied().unwrap_or(true) {
                continue;
            }
            if let Some(s) = seen.get_mut(v) {
                *s = true;
            }
            let l = g.layer.get(v).copied().unwrap_or(0);
            if let Some(lv) = layers.get_mut(l) {
                lv.push(v);
            }
            if let Some(d) = g.down.get(v) {
                for x in d.iter().rev() {
                    stack.push(*x);
                }
            }
        }
    }
    for v in 0..total {
        if !seen.get(v).copied().unwrap_or(true) {
            let l = g.layer.get(v).copied().unwrap_or(0);
            if let Some(lv) = layers.get_mut(l) {
                lv.push(v);
            }
        }
    }

    // --- 5. crossing reduction ----------------------------------------------------------------
    let iterations = if total > 3000 { 4 } else if total > 800 { 8 } else { 24 };
    let mut best = layers.clone();
    let mut best_cross = if total > 6000 { usize::MAX } else { count_crossings(&g, &layers) };
    for it in 0..iterations {
        if best_cross == 0 {
            break;
        }
        let down_pass = it % 2 == 0;
        let mut pos = vec![0f32; total];
        for l in &layers {
            for (i, v) in l.iter().enumerate() {
                if let Some(p) = pos.get_mut(*v) {
                    *p = i as f32;
                }
            }
        }
        let order: Vec<usize> = if down_pass { (1..nlayers).collect() } else { (0..nlayers.saturating_sub(1)).rev().collect() };
        for li in order {
            let Some(layer) = layers.get(li).cloned() else { continue };
            let mut keyed: Vec<(f32, usize, usize)> = Vec::with_capacity(layer.len());
            for (i, v) in layer.iter().enumerate() {
                let nb = if down_pass { g.up.get(*v) } else { g.down.get(*v) };
                let nb = nb.map(Vec::as_slice).unwrap_or(&[]);
                let key = if nb.is_empty() {
                    i as f32
                } else {
                    nb.iter().map(|x| pos.get(*x).copied().unwrap_or(0.0)).sum::<f32>() / nb.len() as f32
                };
                keyed.push((key, i, *v));
            }
            // Nodes without neighbours keep their slot: sort only the ones with neighbours.
            let movable: Vec<usize> = keyed.iter().enumerate().filter(|(_, k)| {
                let nb = if down_pass { g.up.get(k.2) } else { g.down.get(k.2) };
                nb.is_some_and(|x| !x.is_empty())
            }).map(|(i, _)| i).collect();
            let mut vals: Vec<(f32, usize, usize)> = movable.iter().filter_map(|i| keyed.get(*i).copied()).collect();
            vals.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal).then(a.1.cmp(&b.1)));
            for (slot, val) in movable.iter().zip(vals) {
                if let Some(k) = keyed.get_mut(*slot) {
                    *k = val;
                }
            }
            let new_layer: Vec<usize> = keyed.iter().map(|k| k.2).collect();
            for (i, v) in new_layer.iter().enumerate() {
                if let Some(p) = pos.get_mut(*v) {
                    *p = i as f32;
                }
            }
            if let Some(l) = layers.get_mut(li) {
                *l = new_layer;
            }
        }
        if total <= 6000 {
            let c = count_crossings(&g, &layers);
            if c < best_cross {
                best_cross = c;
                best = layers.clone();
            }
        } else {
            best = layers.clone();
        }
    }
    let layers = best;

    // --- 6. coordinates -----------------------------------------------------------------------
    let gap = |a: usize, b: usize| -> f32 {
        let wa = g.w.get(a).copied().unwrap_or(0.0);
        let wb = g.w.get(b).copied().unwrap_or(0.0);
        let both_real = !g.is_dummy.get(a).copied().unwrap_or(false) && !g.is_dummy.get(b).copied().unwrap_or(false);
        (wa + wb) / 2.0 + if both_real { sp.node_sep } else { sp.edge_sep }
    };
    let mut x = vec![0f32; total];
    for l in &layers {
        let mut cursor = 0.0f32;
        let mut prev: Option<usize> = None;
        for v in l {
            if let Some(p) = prev {
                cursor += gap(p, *v);
            }
            if let Some(xv) = x.get_mut(*v) {
                *xv = cursor;
            }
            prev = Some(*v);
        }
    }
    let row_gaps: Vec<Vec<f32>> = layers.iter().map(|l| l.windows(2).map(|w| if let [a, b] = w { gap(*a, *b) } else { 0.0 }).collect()).collect();
    let place = |li: usize, use_up: bool, use_down: bool, x: &mut Vec<f32>| {
        let Some(l) = layers.get(li) else { return };
        let mut desired = Vec::with_capacity(l.len());
        for v in l {
            let mut sum = 0.0f32;
            let mut cnt = 0.0f32;
            if use_up {
                for u in g.up.get(*v).map(Vec::as_slice).unwrap_or(&[]) {
                    sum += x.get(*u).copied().unwrap_or(0.0);
                    cnt += 1.0;
                }
            }
            if use_down {
                for d in g.down.get(*v).map(Vec::as_slice).unwrap_or(&[]) {
                    sum += x.get(*d).copied().unwrap_or(0.0);
                    cnt += 1.0;
                }
            }
            desired.push(if cnt > 0.0 { sum / cnt } else { x.get(*v).copied().unwrap_or(0.0) });
        }
        let gaps = row_gaps.get(li).map(Vec::as_slice).unwrap_or(&[]);
        let placed = place_row(&desired, gaps);
        for (v, p) in l.iter().zip(placed) {
            if let Some(xv) = x.get_mut(*v) {
                *xv = p;
            }
        }
    };
    let rounds = if total > 3000 { 3 } else { 10 };
    for _ in 0..rounds {
        for li in 1..nlayers {
            place(li, true, false, &mut x);
        }
        for li in (0..nlayers.saturating_sub(1)).rev() {
            place(li, false, true, &mut x);
        }
    }
    for li in 0..nlayers {
        place(li, true, true, &mut x);
    }
    // vertical
    let mut layer_h = vec![0f32; nlayers];
    for v in 0..total {
        let l = g.layer.get(v).copied().unwrap_or(0);
        if let Some(h) = layer_h.get_mut(l) {
            *h = h.max(g.h.get(v).copied().unwrap_or(0.0));
        }
    }
    let half = sp.rank_sep / 2.0;
    let mut ly = vec![0f32; nlayers];
    let mut cur = 0.0f32;
    for l in 0..nlayers {
        let h = layer_h.get(l).copied().unwrap_or(0.0);
        if l == 0 {
            cur = h / 2.0;
        } else {
            let ph = layer_h.get(l - 1).copied().unwrap_or(0.0);
            cur += ph / 2.0 + half + h / 2.0;
        }
        if let Some(y) = ly.get_mut(l) {
            *y = cur;
        }
    }
    // --- 7. bounds and orientation ------------------------------------------------------------
    let mut min_x = f32::MAX;
    let mut max_x = f32::MIN;
    let mut min_y = f32::MAX;
    let mut max_y = f32::MIN;
    for v in 0..total {
        let dummy = g.is_dummy.get(v).copied().unwrap_or(false);
        let w = g.w.get(v).copied().unwrap_or(0.0);
        let h = g.h.get(v).copied().unwrap_or(0.0);
        if dummy && w <= 1.0 {
            continue;
        }
        let xv = x.get(v).copied().unwrap_or(0.0);
        let yv = ly.get(g.layer.get(v).copied().unwrap_or(0)).copied().unwrap_or(0.0);
        min_x = min_x.min(xv - w / 2.0);
        max_x = max_x.max(xv + w / 2.0);
        min_y = min_y.min(yv - h / 2.0);
        max_y = max_y.max(yv + h / 2.0);
    }
    if !min_x.is_finite() {
        return LayoutResult::default();
    }
    let (iw, ih) = (max_x - min_x, max_y - min_y);
    let tf = |xv: f32, yv: f32| -> (f32, f32) {
        let a = xv - min_x;
        let b = yv - min_y;
        match dir {
            Direction::TopDown => (a, b),
            Direction::BottomUp => (a, ih - b),
            Direction::LeftRight => (b, a),
            Direction::RightLeft => (ih - b, a),
        }
    };
    let point = |v: usize| -> (f32, f32) {
        let xv = x.get(v).copied().unwrap_or(0.0);
        let yv = ly.get(g.layer.get(v).copied().unwrap_or(0)).copied().unwrap_or(0.0);
        tf(xv, yv)
    };
    let centers: Vec<(f32, f32)> = (0..n).map(point).collect();
    let mut edge_points = vec![Vec::new(); edges.len()];
    let mut label_pos = vec![None; edges.len()];
    for (ei, c) in chains.iter().enumerate() {
        if let Some((chain, rev, label)) = c {
            let mut pts: Vec<(f32, f32)> = chain.iter().skip(1).take(chain.len().saturating_sub(2)).map(|v| point(*v)).collect();
            if *rev {
                pts.reverse();
            }
            if let Some(slot) = edge_points.get_mut(ei) {
                *slot = pts;
            }
            if let (Some(l), Some(slot)) = (label, label_pos.get_mut(ei)) {
                *slot = Some(point(*l));
            }
        }
    }
    let (width, height) = if horiz { (ih, iw) } else { (iw, ih) };
    LayoutResult { centers, width, height, edge_points, label_pos }
}
