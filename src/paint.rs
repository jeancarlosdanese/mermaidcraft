//! Draws an engine scene with egui's painter and finds what is under the pointer.

use egui::epaint::PathShape;
use egui::{Align2, Color32, FontFamily, FontId, Painter, Pos2, Shape, Stroke, Vec2};
use mermaidcraft_engine::geom::dist_to_segment;
use mermaidcraft_engine::look::Font;
use mermaidcraft_engine::scene::{Anchor, PathCmd, PrimShape, Scene, Tag};

/// Where the scene is on screen: `screen = origin + scene * zoom`.
#[derive(Clone, Copy, Debug)]
pub struct View {
    pub origin: Pos2,
    pub zoom: f32,
}

impl View {
    pub fn to_screen(self, p: (f32, f32)) -> Pos2 {
        Pos2::new(self.origin.x + p.0 * self.zoom, self.origin.y + p.1 * self.zoom)
    }
    pub fn to_scene(self, p: Pos2) -> (f32, f32) {
        ((p.x - self.origin.x) / self.zoom, (p.y - self.origin.y) / self.zoom)
    }
}

fn col(c: [u8; 4]) -> Color32 {
    Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3])
}

/// The egui family of a diagram font (registered at start-up for every font of the look).
pub fn diagram_family(font: Font, bold: bool) -> FontFamily {
    FontFamily::Name(format!("d-{}{}", font.id(), if bold { "-b" } else { "" }).into())
}

/// Turns path commands into polylines (one per sub-path) with a flag for closed ones.
fn flatten(cmds: &[PathCmd], view: &View) -> Vec<(Vec<Pos2>, bool)> {
    let mut out: Vec<(Vec<Pos2>, bool)> = Vec::new();
    let mut cur: Vec<Pos2> = Vec::new();
    let mut last = (0.0f32, 0.0f32);
    for c in cmds {
        match *c {
            PathCmd::Move(x, y) => {
                if cur.len() > 1 {
                    out.push((std::mem::take(&mut cur), false));
                }
                cur.clear();
                cur.push(view.to_screen((x, y)));
                last = (x, y);
            }
            PathCmd::Line(x, y) => {
                cur.push(view.to_screen((x, y)));
                last = (x, y);
            }
            PathCmd::Cubic(a, b, c2, d, x, y) => {
                for k in 1..=14 {
                    let t = k as f32 / 14.0;
                    let u = 1.0 - t;
                    let px = u * u * u * last.0 + 3.0 * u * u * t * a + 3.0 * u * t * t * c2 + t * t * t * x;
                    let py = u * u * u * last.1 + 3.0 * u * u * t * b + 3.0 * u * t * t * d + t * t * t * y;
                    cur.push(view.to_screen((px, py)));
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

/// Draws every primitive of the scene.
pub fn draw(painter: &Painter, scene: &Scene, view: &View) {
    let z = view.zoom;
    for p in &scene.prims {
        let fill = p.fill.filter(|c| c[3] > 0).map(col);
        let stroke = p.stroke.filter(|c| c[3] > 0).map_or(Stroke::NONE, |c| Stroke::new((p.stroke_width * z).max(0.6), col(c)));
        match &p.shape {
            PrimShape::Rect { x, y, w, h, r } => {
                let rect = egui::Rect::from_min_size(view.to_screen((*x, *y)), Vec2::new(w * z, h * z));
                let rounding = (r * z).min(rect.height() / 2.0).min(rect.width() / 2.0);
                if let Some(f) = fill {
                    painter.rect_filled(rect, rounding, f);
                }
                if stroke != Stroke::NONE {
                    painter.rect_stroke(rect, rounding, stroke, egui::StrokeKind::Middle);
                }
            }
            PrimShape::Ellipse { cx, cy, rx, ry } => {
                let c = view.to_screen((*cx, *cy));
                let r = Vec2::new(rx * z, ry * z);
                if let Some(f) = fill {
                    painter.add(Shape::ellipse_filled(c, r, f));
                }
                if stroke != Stroke::NONE {
                    painter.add(Shape::ellipse_stroke(c, r, stroke));
                }
            }
            PrimShape::Polygon(pts) => {
                let pts: Vec<Pos2> = pts.iter().map(|q| view.to_screen(*q)).collect();
                if let Some(f) = fill {
                    painter.add(Shape::convex_polygon(pts.clone(), f, Stroke::NONE));
                }
                if stroke != Stroke::NONE {
                    painter.add(PathShape::closed_line(pts, stroke));
                }
            }
            PrimShape::Path(cmds) => {
                for (pts, closed) in flatten(cmds, view) {
                    if closed {
                        if let Some(f) = fill {
                            painter.add(Shape::convex_polygon(pts.clone(), f, Stroke::NONE));
                        }
                        if stroke != Stroke::NONE {
                            painter.add(PathShape::closed_line(pts, stroke));
                        }
                    } else if stroke != Stroke::NONE {
                        match p.dash {
                            Some((a, b)) => {
                                painter.extend(Shape::dashed_line(&pts, stroke, a * z, b * z));
                            }
                            None => {
                                painter.add(PathShape::line(pts, stroke));
                            }
                        }
                    }
                }
            }
            PrimShape::Text { x, y, text, size, bold, anchor, .. } => {
                let family = diagram_family(scene.font, *bold);
                let align = match anchor {
                    Anchor::Start => Align2::LEFT_CENTER,
                    Anchor::Middle => Align2::CENTER_CENTER,
                    Anchor::End => Align2::RIGHT_CENTER,
                };
                let pos = view.to_screen((*x, *y - size * 0.34));
                painter.text(pos, align, text, FontId::new(size * z, family), fill.unwrap_or(Color32::BLACK));
            }
        }
    }
}

/// What is under a point of the scene.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    Node(usize),
    Edge(usize),
    Cluster(usize),
    Nothing,
}

pub fn hit_test(scene: &Scene, p: (f32, f32), tol: f32) -> Hit {
    for (i, b) in scene.node_boxes.iter().enumerate().rev() {
        if b.contains(p.0, p.1) {
            return Hit::Node(i);
        }
    }
    let mut best: Option<(usize, f32)> = None;
    for (i, path) in scene.edge_paths.iter().enumerate() {
        for w in path.windows(2) {
            if let [a, b] = w {
                let d = dist_to_segment(p, *a, *b);
                if d <= tol && best.is_none_or(|(_, bd)| d < bd) {
                    best = Some((i, d));
                }
            }
        }
    }
    if let Some((i, _)) = best {
        return Hit::Edge(i);
    }
    // Subgraph frames: the smallest one containing the point.
    let mut cluster: Option<(usize, f32)> = None;
    for pr in &scene.prims {
        if let (Tag::Cluster(c), PrimShape::Rect { x, y, w, h, .. }) = (pr.tag, &pr.shape) {
            if p.0 >= *x && p.0 <= x + w && p.1 >= *y && p.1 <= y + h && cluster.is_none_or(|(_, a)| w * h < a) {
                cluster = Some((c, w * h));
            }
        }
    }
    cluster.map_or(Hit::Nothing, |(c, _)| Hit::Cluster(c))
}
