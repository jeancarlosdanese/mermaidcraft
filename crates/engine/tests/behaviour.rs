#![allow(clippy::unwrap_used, clippy::panic, clippy::expect_used)]
use mermaidcraft_engine::edit;
use mermaidcraft_engine::markdown;
use mermaidcraft_engine::model::*;
use mermaidcraft_engine::*;

fn flow(src: &str) -> Flowchart {
    match parse(src).diagram {
        Some(Diagram::Flowchart(f)) => f,
        other => panic!("not a flowchart: {other:?}"),
    }
}

#[test]
fn flowchart_basics() {
    let f = flow("flowchart LR\n  A[Start] --> B{Ok?}\n  B -->|yes| C((Go))\n  B -- no --> D[(DB)]\n  C -.-> D\n  D ==> E[[Sub]]\n  E --- F\n  F --x G\n  G o--o H\n");
    assert_eq!(f.direction, Direction::LeftRight);
    assert_eq!(f.nodes.len(), 8);
    assert_eq!(f.edges.len(), 8);
    assert_eq!(f.edges[1].label.as_deref(), Some("yes"));
    assert_eq!(f.edges[2].label.as_deref(), Some("no"));
    assert_eq!(f.edges[3].kind.style, LineStyle::Dotted);
    assert_eq!(f.edges[4].kind.style, LineStyle::Thick);
    assert_eq!(f.edges[5].kind.head_end, Head::None);
    assert_eq!(f.edges[6].kind.head_end, Head::Cross);
    assert_eq!(f.edges[7].kind.head_start, Head::Circle);
    assert_eq!(f.nodes[1].shape, NodeShape::Diamond);
    assert_eq!(f.nodes[2].shape, NodeShape::Circle);
    assert_eq!(f.nodes[3].shape, NodeShape::Cylinder);
}

#[test]
fn flowchart_amp_and_subgraph() {
    let f = flow("graph TD\n A & B --> C & D\n subgraph one [My title]\n  C\n  subgraph inner\n   D\n  end\n end\n");
    assert_eq!(f.edges.len(), 4);
    assert_eq!(f.subgraphs.len(), 2);
    assert_eq!(f.subgraphs[0].title, "My title");
    assert_eq!(f.subgraphs[1].parent, Some(0));
}

#[test]
fn errors_have_lines() {
    let p = parse("flowchart TD\n  A --> B\n  B -->\n  C[unclosed\n");
    let lines: Vec<usize> = p.errors().map(|d| d.line).collect();
    assert!(lines.contains(&3), "{:?}", p.diagnostics);
    assert!(lines.contains(&4), "{:?}", p.diagnostics);
    // Still draws what it understood.
    assert!(p.diagram.is_some());
}

#[test]
fn malformed_input_never_panics() {
    let cases = [
        "", "\n\n", "flowchart", "flowchart XX", "graph TD\nA-->", "graph TD\n-->", "graph TD\nA[[[[[", "graph TD\nend", "graph TD\nsubgraph\nsubgraph\n",
        "sequenceDiagram\n->>:", "sequenceDiagram\nA->>B", "sequenceDiagram\nend\nelse", "erDiagram\nA ||--|| \nB {\n", "erDiagram\n}}}}", "erDiagram\nA ||--o{ B : \"unterminated",
        "classDiagram\nA", "\u{0}\u{1}\u{2}", "graph TD\nA[é\u{301}😀]-->B", "graph TD\nA-- text", "graph TD\nA-.", "graph TD\nA==", "graph TD\nA & ", "graph TD\nA:::",
    ];
    for c in cases {
        let _ = parse(c);
        let _ = render_source(c);
    }
    let big = "flowchart TD\n".to_string() + &"A --> B\n".repeat(20000);
    let _ = parse(&big);
    let deep = "flowchart TD\n".to_string() + &"subgraph s\n".repeat(500) + &"end\n".repeat(500);
    let _ = parse(&deep);
    let wide: String = "flowchart TD\n".to_string() + &(0..1500).map(|i| format!("N{i} --> N{}\n", i + 1)).collect::<String>();
    let (scene, _) = render_source(&wide).unwrap();
    assert!(scene.width > 0.0);
}

#[test]
fn cyclic_graph_lays_out() {
    let (scene, d) = render_source("flowchart TD\n A --> B --> C --> A\n A --> A\n").unwrap();
    assert!(d.is_empty());
    assert!(scene.width.is_finite() && scene.height.is_finite());
}

#[test]
fn sequence_parse() {
    let p = parse("sequenceDiagram\n participant A as Alice\n actor B\n A->>+B: hi\n B-->>-A: yo\n Note over A,B: n\n loop x\n A->B: z\n end\n");
    assert!(!p.has_errors(), "{:?}", p.diagnostics);
    let Some(Diagram::Sequence(s)) = p.diagram else { panic!() };
    assert_eq!(s.participants.len(), 2);
    assert_eq!(s.participants[0].label, "Alice");
    assert!(s.participants[1].actor);
    assert_eq!(s.items.iter().filter(|i| matches!(i, SeqItem::Message { .. })).count(), 3);
    assert!(parse("sequenceDiagram\nloop x\nA->>B: hi\n").has_errors());
}

#[test]
fn er_parse() {
    let p = parse("erDiagram\n A ||--o{ B : has\n B {\n int id PK\n string name \"n\"\n }\n");
    assert!(!p.has_errors(), "{:?}", p.diagnostics);
    let Some(Diagram::Er(e)) = p.diagram else { panic!() };
    assert_eq!(e.entities.len(), 2);
    assert_eq!(e.relations[0].card_b, Cardinality::ZeroOrMore);
    assert_eq!(e.entities[1].attributes.len(), 2);
    assert_eq!(e.entities[1].attributes[0].keys, vec!["PK"]);
}

#[test]
fn edit_add_nodes_and_edge_keeps_text() {
    let src = "%% keep me\nflowchart TD\n    A[Start] --> B\n";
    let (s1, id1) = edit::add_node(src, None, "First", NodeShape::Rect).unwrap();
    let (s2, id2) = edit::add_node(&s1, None, "Second", NodeShape::Round).unwrap();
    assert_eq!((id1.as_str(), id2.as_str()), ("C", "D"));
    let s3 = edit::add_edge(&s2, "C", "D", Some("go"), LinkKind::default()).unwrap();
    assert!(s3.starts_with("%% keep me\nflowchart TD\n    A[Start] --> B\n"));
    assert!(s3.contains("    C[First]\n    D(Second)\n    C -->|go| D\n"), "{s3}");
    assert!(!parse(&s3).has_errors());
}

#[test]
fn edit_rename_shape_direction() {
    let src = "flowchart TD\n    A[Start] --> B\n    B --> C{Go}\n";
    let s = edit::set_node(src, "A", Some("Begin (now)"), None).unwrap();
    assert_eq!(flow(&s).nodes[0].label, "Begin (now)");
    let s = edit::set_node(&s, "B", Some("Mid"), Some(NodeShape::Stadium)).unwrap();
    assert_eq!(flow(&s).nodes[1].shape, NodeShape::Stadium);
    assert_eq!(flow(&s).nodes[1].label, "Mid");
    let s = edit::set_direction(&s, Direction::LeftRight).unwrap();
    assert!(s.starts_with("flowchart LR\n"));
    let s2 = edit::set_direction("flowchart\n A-->B\n", Direction::BottomUp).unwrap();
    assert!(s2.starts_with("flowchart BT"));
}

#[test]
fn edit_remove_edge_and_node() {
    let src = "flowchart TD\n    A --> B --> C\n    %% note\n    C --> D\n    style D fill:#fff\n";
    let s = edit::remove_edge(src, 0).unwrap();
    let f = flow(&s);
    assert_eq!(f.edges.len(), 2);
    assert_eq!(f.nodes.len(), 4, "{s}");
    assert!(s.contains("%% note"));
    let s = edit::remove_node(src, "B").unwrap();
    let f = flow(&s);
    assert!(f.node_index("B").is_none());
    assert_eq!(f.edges.len(), 1);
    assert!(f.node_index("A").is_some());
    let s = edit::remove_node(src, "D").unwrap();
    assert!(!s.contains("style D"), "{s}");
    assert!(!parse(&s).has_errors());
}

#[test]
fn edit_edge_label_and_style() {
    let src = "flowchart TD\n    A --> B\n    B --> C\n";
    let s = edit::set_edge(src, 1, Some(Some("hello")), Some(LinkKind { style: LineStyle::Dotted, ..LinkKind::default() })).unwrap();
    let f = flow(&s);
    assert_eq!(f.edges[1].label.as_deref(), Some("hello"));
    assert_eq!(f.edges[1].kind.style, LineStyle::Dotted);
    assert!(s.contains("A --> B"));
}

#[test]
fn edit_rejects_bad_input() {
    assert!(edit::add_node("sequenceDiagram\nA->>B: x", None, "x", NodeShape::Rect).is_err());
    assert!(edit::add_node("flowchart TD\nA", Some("bad id"), "x", NodeShape::Rect).is_err());
    assert!(edit::add_node("flowchart TD\nA", Some("A"), "x", NodeShape::Rect).is_err());
    assert!(edit::remove_node("flowchart TD\nA", "Z").is_err());
    assert!(edit::remove_edge("flowchart TD\nA-->B", 9).is_err());
}

#[test]
fn markdown_blocks_roundtrip() {
    let md = "# T\n\n```mermaid\nflowchart LR\n  A --> B\n```\n\ntext\n\n~~~mermaid\nsequenceDiagram\nA->>B: x\n~~~\n\n```rust\nfn main(){}\n```\n";
    let blocks = markdown::find_blocks(md);
    assert_eq!(blocks.len(), 2);
    assert_eq!(markdown::block_text(md, &blocks[0]), "flowchart LR\n  A --> B");
    let new = markdown::replace_block(md, &blocks[0], "flowchart TD\n  X --> Y").unwrap();
    assert!(new.starts_with("# T\n\n```mermaid\nflowchart TD\n  X --> Y\n```\n\ntext\n"));
    assert!(new.ends_with("```rust\nfn main(){}\n```\n"));
    let same = markdown::replace_block(md, &blocks[1], markdown::block_text(md, &blocks[1])).unwrap();
    assert_eq!(same, md);
}

#[test]
fn exports_work() {
    let (scene, _) = render_source(edit::template("flowchart")).unwrap();
    let svg = scene.to_svg();
    assert!(svg.starts_with("<svg"));
    assert!(export::svg_to_png(&svg, 3.0).unwrap().len() > 500);
    assert!(export::svg_to_pdf(&svg).unwrap().starts_with(b"%PDF"));
    for k in ["sequence", "er"] {
        let (s, _) = render_source(edit::template(k)).unwrap();
        assert!(export::svg_to_png(&s.to_svg(), 1.0).is_ok());
    }
}

#[test]
fn layout_has_no_overlaps_in_a_chain() {
    let (scene, _) = render_source("flowchart TD\n A --> B\n A --> C\n A --> D\n B --> E\n C --> E\n D --> E\n").unwrap();
    let b = &scene.node_boxes;
    for i in 0..b.len() {
        for j in (i + 1)..b.len() {
            let (x, y) = (&b[i], &b[j]);
            let overlap = x.x < y.x + y.w && y.x < x.x + x.w && x.y < y.y + y.h && y.y < x.y + x.h;
            assert!(!overlap, "nodes {i} and {j} overlap");
        }
    }
}

#[test]
fn moved_nodes_are_stored_in_a_comment_and_drawn() {
    let src = "flowchart TD\n    A --> B\n    B --> C\n";
    let mut map = std::collections::BTreeMap::new();
    map.insert("C".to_string(), (120.0, -30.0));
    let moved = edit::set_offsets(src, &map).unwrap();
    assert!(moved.contains("%% mermaidcraft:layout C=120,-30"));
    assert!(!parse(&moved).has_errors());
    assert_eq!(flow(&moved).offsets.get("C"), Some(&(120.0, -30.0)));
    let (a, _) = render_source_with(src, &RenderOptions::clean()).unwrap();
    let (b, _) = render_source_with(&moved, &RenderOptions::clean()).unwrap();
    assert!(b.node_boxes[2].x - b.node_boxes[1].x > a.node_boxes[2].x - a.node_boxes[1].x + 100.0);
    // Auto-arrange removes the comment; text edits leave it alone.
    assert_eq!(edit::set_offsets(&moved, &Default::default()).unwrap(), src);
    let more = edit::add_node(&moved, None, "x", NodeShape::Rect).unwrap().0;
    assert!(more.contains("mermaidcraft:layout"));
}

#[test]
fn looks_render() {
    for opts in [RenderOptions::default(), RenderOptions::clean()] {
        for k in ["flowchart", "sequence", "er"] {
            let (s, _) = render_source_with(edit::template(k), &opts).unwrap();
            assert!(export::svg_to_png(&s.to_svg(), 1.0).is_ok());
        }
    }
}

#[test]
fn default_look_is_proposal_a() {
    use look::*;
    let o = RenderOptions::default();
    assert_eq!((o.font, o.colors, o.fill, o.stroke, o.arrows), (Font::Shantell, ColorMode::Group, Fill::Solid, Stroke::Artist, Arrows::Graphite));
    let (s, _) = render_source(edit::template("flowchart")).unwrap();
    assert!(s.to_svg().contains("font-family=\"'Shantell Sans'"));
    let (c, _) = render_source_with(edit::template("flowchart"), &RenderOptions::clean()).unwrap();
    assert!(c.to_svg().contains("font-family=\"'Liberation Sans'"));
}

#[test]
fn every_look_option_renders_and_labels_fit() {
    use look::*;
    let src = "flowchart TD\n  A[Uma etiqueta bem comprida] --> B{Decisão?}\n  subgraph G [Grupo]\n    C([Ação]) --> D[(Dados)]\n  end\n  B -->|sim| C\n";
    for font in Font::ALL {
        for fill in Fill::ALL {
            let opts = RenderOptions { font: *font, fill: *fill, ..RenderOptions::default() };
            let (s, _) = render_source_with(src, &opts).unwrap();
            assert_eq!(s.font, *font);
            let w = text::text_width("Uma etiqueta bem comprida", 15.0, false, *font);
            assert!(s.node_boxes[0].w > w + 20.0, "{font:?}: box {} text {w}", s.node_boxes[0].w);
            assert!(export::svg_to_png(&s.to_svg(), 1.0).is_ok());
        }
    }
    for colors in ColorMode::ALL {
        for stroke in Stroke::ALL {
            for arrows in Arrows::ALL {
                let opts = RenderOptions { colors: *colors, stroke: *stroke, arrows: *arrows, ..RenderOptions::default() };
                assert!(render_source_with(src, &opts).is_ok());
            }
        }
    }
}

#[test]
fn colours_follow_group_or_shape() {
    use look::*;
    use scene::{PrimShape, Tag};
    let src = "flowchart TD\n  A{Ok?} --> B\n  subgraph G [Grupo]\n    B[Passo]\n  end\n";
    let fill_of = |opts: RenderOptions, node: usize| {
        let (s, _) = render_source_with(src, &RenderOptions { stroke: Stroke::Clean, ..opts }).unwrap();
        s.prims.iter().find(|p| p.tag == Tag::Node(node) && !matches!(p.shape, PrimShape::Text { .. }) && p.fill.is_some()).and_then(|p| p.fill).unwrap()
    };
    let group = RenderOptions::default();
    assert_eq!(fill_of(group, 0), YELLOW.light, "a loose decision follows its shape");
    assert_eq!(fill_of(group, 1), hue_at(0).light, "a node in a group takes the group colour");
    let shape = RenderOptions { colors: ColorMode::Shape, ..group };
    assert_eq!(fill_of(shape, 1), GRAY.light);
    let neutral = RenderOptions { colors: ColorMode::Neutral, ..group };
    assert_eq!(fill_of(neutral, 0), GRAY.light);
}

#[test]
fn hatching_adds_lines_and_is_deterministic() {
    use look::*;
    let src = edit::template("flowchart");
    let solid = render_source_with(src, &RenderOptions::default()).unwrap().0;
    let opts = RenderOptions { fill: Fill::CrossHatch, ..RenderOptions::default() };
    let a = render_source_with(src, &opts).unwrap().0;
    let b = render_source_with(src, &opts).unwrap().0;
    assert!(a.prims.len() > solid.prims.len() + 20);
    assert_eq!(a.to_svg(), b.to_svg());
}

#[test]
fn hatching_cost_is_bounded_with_hostile_input() {
    use look::*;
    use scene::PrimShape;
    // Many huge notes (the shape the review used to make the hatch blow up).
    let tall = vec!["a"; 300].join("<br>");
    let mut src = String::from("sequenceDiagram\n  participant A\n  participant B\n");
    for _ in 0..30 {
        src.push_str(&format!("  Note over A,B: {tall}\n"));
    }
    let opts = RenderOptions { fill: Fill::CrossHatch, stroke: Stroke::Cartoonist, ..RenderOptions::default() };
    let started = std::time::Instant::now();
    let (s, _) = render_source_with(&src, &opts).unwrap();
    let hatch = s.prims.iter().filter(|p| p.fill_line).count();
    assert!(hatch > 0 && hatch <= sketch::HATCH_BUDGET, "hatch lines: {hatch}");
    let cmds: usize = s.prims.iter().map(|p| if let PrimShape::Path(c) = &p.shape { c.len() } else { 1 }).sum();
    assert!(cmds < 200_000, "drawing commands: {cmds}");
    assert!(started.elapsed().as_secs() < 20);
}

#[test]
fn arrows_option_reaches_er_edges() {
    use look::*;
    let src = edit::template("er");
    let graphite = render_source(src).unwrap().0.to_svg();
    let source = render_source_with(src, &RenderOptions { arrows: Arrows::Source, ..RenderOptions::default() }).unwrap().0.to_svg();
    assert_ne!(graphite, source);
}

const LONG_FLOW: &str = include_str!("fixtures/fluxo-longo.mmd");

#[test]
fn long_labels_wrap() {
    let (s, _) = render_source(LONG_FLOW).unwrap();
    let f = flow(LONG_FLOW);
    for (n, b) in f.nodes.iter().zip(&s.node_boxes) {
        let max = if n.shape == NodeShape::Diamond { 360.0 } else { 300.0 };
        assert!(b.w <= max, "{} is {} wide", n.id, b.w);
    }
    // Explicit breaks stay.
    let (s, _) = render_source("flowchart TD\n  A[um<br>dois]\n").unwrap();
    assert!(s.to_svg().matches("<text").count() >= 2);
}

#[test]
fn edges_do_not_cross_other_nodes() {
    let f = flow(LONG_FLOW);
    let (s, _) = render_source(LONG_FLOW).unwrap();
    let index = |id: &str| f.nodes.iter().position(|n| n.id == id);
    for (ei, e) in f.edges.iter().enumerate() {
        let ends = [index(&e.from), index(&e.to)];
        for (ni, (n, b)) in f.nodes.iter().zip(&s.node_boxes).enumerate() {
            if ends.contains(&Some(ni)) {
                continue;
            }
            let (cx, cy) = (b.x + b.w / 2.0, b.y + b.h / 2.0);
            let inside = |p: &(f32, f32)| {
                let (dx, dy) = ((p.0 - cx).abs(), (p.1 - cy).abs());
                if n.shape == NodeShape::Diamond { dx / (b.w / 2.0) + dy / (b.h / 2.0) < 0.95 } else { dx < b.w / 2.0 - 2.0 && dy < b.h / 2.0 - 2.0 }
            };
            // Points every 2 px along the path, so straight stretches are checked too.
            let path = s.edge_paths.get(ei).unwrap();
            let dense: Vec<(f32, f32)> = path
                .windows(2)
                .flat_map(|w| {
                    let (a, b) = (w[0], w[1]);
                    let steps = ((b.0 - a.0).hypot(b.1 - a.1) / 2.0).ceil().max(1.0) as usize;
                    (0..=steps).map(move |k| {
                        let t = k as f32 / steps as f32;
                        (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
                    })
                })
                .collect();
            assert!(!dense.iter().any(inside), "edge {} -> {} crosses {}", e.from, e.to, n.id);
        }
    }
}

#[test]
fn edges_into_one_side_get_their_own_ports() {
    // Three edges converge on the top of D: each gets its own spot and comes in straight.
    let (s, _) = render_source("flowchart TD\n  A --> D\n  B --> D\n  C --> D\n").unwrap();
    let ends: Vec<(f32, f32)> = s.edge_paths.iter().map(|p| *p.last().unwrap()).collect();
    for (i, a) in ends.iter().enumerate() {
        for b in ends.iter().skip(i + 1) {
            assert!((a.0 - b.0).abs() > 8.0, "ends too close: {a:?} {b:?}");
        }
    }
    for p in &s.edge_paths {
        let (q, r) = (p[p.len() - 2], p[p.len() - 1]);
        assert!((r.0 - q.0).abs() < 0.3 * (r.1 - q.1).abs(), "comes in slanted: {q:?} -> {r:?}");
    }
}

#[test]
fn invisible_edges_leave_ports_alone() {
    // Off a shared port the edge runs straight from centre to centre; a port would bend it.
    let off_line = |src: &str| {
        let s = render_source(src).unwrap().0;
        let c = |i: usize| (s.node_boxes[i].x + s.node_boxes[i].w / 2.0, s.node_boxes[i].y + s.node_boxes[i].h / 2.0);
        let (a, b, p) = (c(0), c(1), s.edge_paths[0][0]);
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        ((p.0 - a.0) * dy - (p.1 - a.1) * dx).abs() / dx.hypot(dy)
    };
    assert!(off_line("flowchart TD\n  A --> B\n  A --> C\n") > 2.0, "two visible edges should share the side");
    assert!(off_line("flowchart TD\n  A --> B\n  A ~~~ C\n") < 1.0, "the hidden edge moved the visible one");
}

#[test]
fn straight_edge_label_sits_in_the_middle() {
    let src = "flowchart TD\n  subgraph S1[Um]\n    A\n  end\n  subgraph S2[Dois]\n    B\n  end\n  A -- envia pedido --> B\n";
    let (s, _) = render_source(src).unwrap();
    let path = &s.edge_paths[0];
    let (a, b) = (path[0], *path.last().unwrap());
    let svg = s.to_svg();
    // The label text is drawn between the two ends, away from both.
    let ys: Vec<f32> = svg.match_indices(">envia pedido<").filter_map(|(i, _)| {
        let head = &svg[..i];
        let tag = &head[head.rfind("<text")?..];
        let y = tag.split(" y=\"").nth(1)?.split('"').next()?;
        y.parse().ok()
    }).collect();
    assert_eq!(ys.len(), 1, "label not found");
    let lo = a.1.min(b.1) + 12.0;
    let hi = a.1.max(b.1) - 12.0;
    assert!(ys[0] > lo && ys[0] < hi, "label at {} not between {lo} and {hi}", ys[0]);
}

#[test]
fn oversized_png_is_refused_before_drawing() {
    let svg = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"90000\" height=\"20\" viewBox=\"0 0 90000 20\"></svg>";
    assert!(export::svg_to_png(svg, 1.0).is_err());
    let small = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"20\" height=\"20\" viewBox=\"0 0 20 20\"></svg>";
    assert!(export::svg_to_png(small, 1.0).is_ok());
}

fn frame_of(s: &scene::Scene, c: usize) -> scene::Rect {
    s.prims
        .iter()
        .find_map(|p| match (p.tag, &p.shape) {
            (scene::Tag::Cluster(k), scene::PrimShape::Rect { x, y, w, h, .. }) if k == c => Some(scene::Rect { x: *x, y: *y, w: *w, h: *h }),
            _ => None,
        })
        .unwrap()
}

fn inside(outer: &scene::Rect, inner: &scene::Rect) -> bool {
    inner.x >= outer.x && inner.y >= outer.y && inner.x + inner.w <= outer.x + outer.w && inner.y + inner.h <= outer.y + outer.h
}

#[test]
fn notes_sit_beside_their_node_inside_its_group() {
    let src = "flowchart TD\n  subgraph G[Etapa]\n    A[Início] --> B[Meio] --> C[Fim]\n  end\n  N[Nota sobre o meio, com um texto comprido] -.- B\n";
    let f = flow(src);
    let (s, _) = render_source_with(src, &RenderOptions::clean()).unwrap();
    let at = |id: &str| s.node_boxes[f.node_index(id).unwrap()];
    let (b, n) = (at("B"), at("N"));
    assert!(n.x > b.x + b.w, "note is not right of its node: {n:?} {b:?}");
    assert!(n.y < b.y + b.h && b.y < n.y + n.h, "note is not level with its node: {n:?} {b:?}");
    assert!(inside(&frame_of(&s, 0), &n), "note is outside the group");
    // The flow stays straight: A, B and C share one column.
    let cx = |r: scene::Rect| r.x + r.w / 2.0;
    assert!((cx(at("A")) - cx(b)).abs() < 1.0 && (cx(at("C")) - cx(b)).abs() < 1.0);
}

#[test]
fn plain_headless_links_are_not_notes() {
    // No group: an ordinary link, laid out as before.
    let src = "flowchart TD\n  A[Um] --- B[Dois]\n";
    let f = flow(src);
    let (s, _) = render_source(src).unwrap();
    let (a, b) = (s.node_boxes[f.node_index("A").unwrap()], s.node_boxes[f.node_index("B").unwrap()]);
    assert!(b.y > a.y + a.h, "B should stay below A");
    // A node declared in a group stays in it, even with one headless link to the outside; so do
    // two single-node groups linked to each other.
    for src in [
        "flowchart TD\n  API[Servidor]\n  subgraph Cloud[Nuvem]\n    DB[(Banco)]\n  end\n  API --- DB\n",
        "flowchart TD\n  subgraph G1[Um]\n    A[A]\n  end\n  subgraph G2[Dois]\n    B[B]\n  end\n  A --- B\n",
    ] {
        let f = flow(src);
        let (s, _) = render_source(src).unwrap();
        for (i, n) in f.nodes.iter().enumerate() {
            if let Some(g) = n.parent {
                assert!(inside(&s.cluster_boxes[g], &s.node_boxes[i]), "{} left its group in {src:?}", n.id);
            }
        }
    }
    // A labelled link is a real link, not a note.
    let src = "flowchart TD\n  subgraph G[Etapa]\n    B[Meio]\n  end\n  N[Nota] -.-|veja| B\n";
    let f = flow(src);
    let (s, _) = render_source(src).unwrap();
    assert!(!inside(&s.cluster_boxes[0], &s.node_boxes[f.node_index("N").unwrap()]));
}

#[test]
fn a_note_follows_its_node_when_moved() {
    let base = "flowchart TD\n  subgraph G[Etapa]\n    A[Início] --> B[Meio] --> C[Fim]\n  end\n  N[Nota] -.- B\n";
    let moved = format!("{base}%% mermaidcraft:layout B=-150,20\n");
    let f = flow(base);
    let gap = |src: &str| {
        let (s, _) = render_source(src).unwrap();
        let (b, n) = (s.node_boxes[f.node_index("B").unwrap()], s.node_boxes[f.node_index("N").unwrap()]);
        (n.x - b.x, n.y - b.y)
    };
    let (g0, g1) = (gap(base), gap(&moved));
    assert!((g0.0 - g1.0).abs() < 0.5 && (g0.1 - g1.1).abs() < 0.5, "{g0:?} -> {g1:?}");
}

#[test]
fn group_frames_are_known_in_every_look() {
    let src = "flowchart TD\n  subgraph G[Grupo]\n    A --> B\n  end\n";
    for opts in [RenderOptions::default(), RenderOptions::clean()] {
        let (s, _) = render_source_with(src, &opts).unwrap();
        assert_eq!(s.cluster_boxes.len(), 1);
        assert!(inside(&s.cluster_boxes[0], &s.node_boxes[0]));
    }
}

#[test]
fn moving_a_group_moves_everything_inside() {
    let base = "flowchart TD\n  O[Fora] --> A\n  subgraph G[Grupo]\n    A[Um] --> B[Dois]\n  end\n";
    let moved = format!("{base}%% mermaidcraft:layout sub:G=120,40\n");
    let f = flow(base);
    let (s0, _) = render_source_with(base, &RenderOptions::clean()).unwrap();
    let (s1, _) = render_source_with(&moved, &RenderOptions::clean()).unwrap();
    let rel = |s: &scene::Scene, id: &str| {
        let (o, r) = (s.node_boxes[f.node_index("O").unwrap()], s.node_boxes[f.node_index(id).unwrap()]);
        (r.x - o.x, r.y - o.y)
    };
    for id in ["A", "B"] {
        let (d0, d1) = (rel(&s0, id), rel(&s1, id));
        assert!((d1.0 - d0.0 - 120.0).abs() < 0.5 && (d1.1 - d0.1 - 40.0).abs() < 0.5, "{id}: {d0:?} -> {d1:?}");
    }
    let o0 = s0.node_boxes[f.node_index("O").unwrap()];
    let o1 = s1.node_boxes[f.node_index("O").unwrap()];
    let (g0, g1) = (frame_of(&s0, 0), frame_of(&s1, 0));
    assert!((g1.x - o1.x - (g0.x - o0.x) - 120.0).abs() < 0.5 && (g1.w - g0.w).abs() < 0.5, "frame did not move with its group");
    // The key round-trips through the layout comment.
    let mut map = std::collections::BTreeMap::new();
    map.insert(flowchart::group_offset_key("G"), (120.0, 40.0));
    let text = edit::set_offsets(base, &map).unwrap();
    assert!(text.contains("sub:G=120,40"));
    assert_eq!(flow(&text).offsets.get("sub:G"), Some(&(120.0, 40.0)));
}

#[test]
fn node_style_is_written_replaced_and_removed() {
    let src = "flowchart TD\n  A[Um] --> B[Dois]\n  style A fill:#fff,color:#333\n  style A,B stroke-width:3px\n  C --> D\n  %% mermaidcraft:layout B=10,0\n";
    let props = |pairs: &[(&str, &str)]| pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect::<StyleMap>();
    // The panel starts from the node's own line only, not from the shared one.
    let f = flow(src);
    let own = edit::own_node_style(src, &f, "A");
    assert_eq!(own, props(&[("fill", "#fff"), ("color", "#333")]));
    // One own line: replaced in place, nothing else moves.
    let mut p = own.clone();
    p.insert("fill".into(), "#d0ebff".into());
    p.insert("stroke".into(), "#1971c2".into());
    let out = edit::set_node_style(src, "A", &p).unwrap();
    assert_eq!(out, src.replace("style A fill:#fff,color:#333", "style A color:#333,fill:#d0ebff,stroke:#1971c2"));
    // A new line goes before the layout comment.
    let out2 = edit::set_node_style(&out, "B", &props(&[("stroke-dasharray", "5 5")])).unwrap();
    assert!(out2.ends_with("  C --> D\n  style B stroke-dasharray:5 5\n  %% mermaidcraft:layout B=10,0\n"), "{out2}");
    // Empty map: the own line goes (whole line), the shared one stays.
    let out3 = edit::set_node_style(&out2, "A", &StyleMap::new()).unwrap();
    assert!(!out3.contains("style A color") && out3.contains("style A,B stroke-width:3px"), "{out3}");
    assert_eq!(out3.lines().count(), out2.lines().count() - 1, "{out3}");
    // Two own lines become one; CRLF files keep CRLF.
    let crlf = "flowchart TD\r\n  A --> B\r\n  style A fill:#fff\r\n  style A stroke:#000\r\n";
    let out4 = edit::set_node_style(crlf, "A", &props(&[("fill", "#eee")])).unwrap();
    assert_eq!(out4, "flowchart TD\r\n  A --> B\r\n  style A fill:#eee\r\n");
    // Several statements on one line: the `;` between them goes with the removed ones.
    let fill = props(&[("fill", "#eee")]);
    let both = "flowchart TD\n  A --> B\n  style A fill:#fff; style A stroke:#000\n";
    assert_eq!(edit::set_node_style(both, "A", &fill).unwrap(), "flowchart TD\n  A --> B\n  style A fill:#eee\n");
    let tail = "flowchart TD\n  A --> B; style A fill:#fff\n  style A stroke:#000\n";
    assert_eq!(edit::set_node_style(tail, "A", &fill).unwrap(), "flowchart TD\n  A --> B\n  style A fill:#eee\n");
    let around = "flowchart TD\n  style A fill:#fff; C --> D; style A stroke:#000\n  A --> B\n";
    assert_eq!(edit::set_node_style(around, "A", &fill).unwrap(), "flowchart TD\n  C --> D\n  A --> B\n  style A fill:#eee\n");
    let noted = "flowchart TD\n  C --> D; style A fill:#fff %% nota\n  A --> B\n  style A stroke:#000\n";
    assert_eq!(edit::set_node_style(noted, "A", &fill).unwrap(), "flowchart TD\n  C --> D %% nota\n  A --> B\n  style A fill:#eee\n");
    // Bad input is refused.
    assert!(edit::set_node_style(src, "Z", &StyleMap::new()).is_err());
    assert!(edit::set_node_style(src, "A", &props(&[("fill", "#fff;evil")])).is_err());
    assert!(edit::set_node_style(src, "A", &props(&[("fill", "#fff\n  X --> Y")])).is_err());
}

#[test]
fn text_place_names_the_node_or_edge_under_it() {
    let src = "flowchart TD\n  A --> B\n  B[Dois] -- sim --> C\n  subgraph G\n    C\n  end\n  style C fill:#fff\n";
    let f = flow(src);
    let at = |needle: &str, off: usize, line: usize| edit::item_at(&f, src.find(needle).unwrap() + off, line);
    // The id under the cursor, wherever it is on the line.
    assert_eq!(at("A -->", 0, 2), Some(edit::TextItem::Node("A".into())));
    assert_eq!(at("B\n", 0, 2), Some(edit::TextItem::Node("B".into())));
    // The definition line of B selects B, even though B first shows up on line 2.
    assert_eq!(at("Dois", 0, 3), Some(edit::TextItem::Node("B".into())));
    // The link token selects its edge.
    let e = f.edges.iter().position(|e| e.from == "B" && e.to == "C").unwrap();
    assert_eq!(at("sim", 0, 3), Some(edit::TextItem::Edge(e)));
    // A lone id inside a group and a style line select that node.
    assert_eq!(at("    C", 4, 5), Some(edit::TextItem::Node("C".into())));
    assert_eq!(at("style C", 0, 7), Some(edit::TextItem::Node("C".into())));
    // Indentation before a statement falls back to its first node.
    assert_eq!(edit::item_at(&f, src.find("  A -->").unwrap(), 2), Some(edit::TextItem::Node("A".into())));
    // `end` names nothing.
    assert_eq!(at("end", 0, 6), None);
    // A group id names the group; a link drawn as two edges names neither.
    let src2 = "flowchart TD\n  subgraph G\n    X\n  end\n  A --> G\n  X & Y --> Z\n  style G fill:#fff\n";
    let f2 = flow(src2);
    let at2 = |needle: &str, off: usize, line: usize| edit::item_at(&f2, src2.find(needle).unwrap() + off, line);
    assert_eq!(at2("G\n  X &", 0, 5), Some(edit::TextItem::Group("G".into())));
    assert_eq!(at2("style G", 6, 7), Some(edit::TextItem::Group("G".into())));
    assert_eq!(at2("--> Z", 1, 6), None);
    assert_eq!(at2("Y -->", 0, 6), Some(edit::TextItem::Node("Y".into())));
}

#[test]
fn edit_reverse_edge_alone_on_its_line() {
    let src = "flowchart TD\n  A[Um] --> B[Dois]\n  C[Tres]\n";
    let (s, e) = edit::reverse_edge(src, 0).unwrap();
    assert_eq!(s, "flowchart TD\n  B[Dois] --> A[Um]\n  C[Tres]\n");
    assert_eq!(e, 0);
    let f = flow(&s);
    assert_eq!((f.edges[0].from.as_str(), f.edges[0].to.as_str()), ("B", "A"));
    assert_eq!(f.nodes.iter().find(|n| n.id == "A").unwrap().label, "Um");
}

#[test]
fn edit_reverse_edge_in_a_chain_and_shared() {
    let src = "flowchart TD\n  A --> B --> C\n";
    let (s, e) = edit::reverse_edge(src, 1).unwrap();
    let p = parse(&s);
    assert!(!p.has_errors(), "{s} {:?}", p.diagnostics);
    assert!(s.contains("A --> B\n"), "{s}");
    assert!(s.contains("C --> B\n"), "{s}");
    let f = flow(&s);
    assert_eq!(f.edges.len(), 2);
    assert_eq!((f.edges[e].from.as_str(), f.edges[e].to.as_str()), ("C", "B"));
    assert!(f.edges.iter().any(|x| x.from == "A" && x.to == "B"));

    let src = "flowchart TD\n  A & B --> C\n";
    let (s, e) = edit::reverse_edge(src, 0).unwrap();
    let f = flow(&s);
    assert_eq!(f.edges.len(), 2, "{s}");
    assert_eq!((f.edges[e].from.as_str(), f.edges[e].to.as_str()), ("C", "A"));
    assert!(f.edges.iter().any(|x| x.from == "B" && x.to == "C"), "{s}");
}

#[test]
fn edit_reverse_edge_keeps_label_line_and_heads() {
    let src = "flowchart TD\n  A x-.->|sim| B\n  C ---o D\n";
    let before = flow(src);
    let (s, _) = edit::reverse_edge(src, 0).unwrap();
    let (s, _) = edit::reverse_edge(&s, 1).unwrap();
    assert!(s.contains("B x-.->|sim| A"), "{s}");
    assert!(s.contains("D ---o C"), "{s}");
    let p = parse(&s);
    assert!(!p.has_errors(), "{s} {:?}", p.diagnostics);
    let f = flow(&s);
    assert_eq!(f.edges[0].kind, before.edges[0].kind);
    assert_eq!(f.edges[0].label.as_deref(), Some("sim"));
    assert_eq!(f.edges[1].kind, before.edges[1].kind);
}

#[test]
fn edit_reverse_edge_before_the_layout_comment() {
    let src = "flowchart TD\n  A --> B --> C\n%% mermaidcraft:layout A=10,0\n";
    let (s, _) = edit::reverse_edge(src, 0).unwrap();
    assert!(s.ends_with("B --> A\n%% mermaidcraft:layout A=10,0\n"), "{s}");
    let (same, e) = edit::reverse_edge("flowchart TD\n  A --> A\n", 0).unwrap();
    assert_eq!((same.as_str(), e), ("flowchart TD\n  A --> A\n", 0));
    assert!(edit::reverse_edge("flowchart TD\n  A --> B\n", 3).is_err());
    assert!(edit::reverse_edge("sequenceDiagram\n  A->>B: x\n", 0).is_err());
}

#[test]
fn edit_reverse_edge_finds_the_moved_edge_and_keeps_crlf() {
    // An equal edge written after the layout comment is not the one that moved.
    let src = "flowchart TD\n  A --> B --> C\n%% mermaidcraft:layout A=10,0\n  B --> A\n";
    let (s, e) = edit::reverse_edge(src, 0).unwrap();
    let f = flow(&s);
    assert_eq!(f.edges.len(), 3, "{s}");
    assert_eq!(f.edges[e].line, 4, "{s}");
    // A chain split in a CRLF file keeps CRLF.
    let (s, _) = edit::reverse_edge("flowchart TD\r\n  A --> B --> C\r\n", 1).unwrap();
    assert!(!s.replace("\r\n", "").contains('\n'), "{s:?}");
}

// ---- Grupos (subgraph) ----

/// `node -> group id` for every node of the text (None when the node is loose).
fn group_of(src: &str, node: &str) -> Option<String> {
    let f = flow(src);
    let n = &f.nodes[f.node_index(node).unwrap()];
    n.parent.map(|p| f.subgraphs[p].id.clone())
}

const GROUPED: &str = "flowchart TD\n  subgraph G1[\"Entrada\"]\n    A[Um] --> B[Dois]\n  end\n  C[Tres] --> G1\n  D[Quatro]\n";

#[test]
fn group_header_and_end_spans() {
    let src = "flowchart TD\n  subgraph Fora[\"Fora\"]\n    subgraph Dentro\n      A\n    end\n    B\n  end\n";
    let f = flow(src);
    let text = |s: Span| &src[s.start..s.end];
    assert_eq!(text(f.subgraphs[0].header), "subgraph Fora[\"Fora\"]");
    assert_eq!(text(f.subgraphs[1].header), "subgraph Dentro");
    assert_eq!(f.subgraphs[1].end.map(|s| s.start), src.find("    end\n").map(|i| i + 4));
    assert_eq!(f.subgraphs[0].end.map(|s| s.start), src.rfind("end"));
}

#[test]
fn edit_rename_group_keeps_the_id() {
    let out = edit::rename_group(GROUPED, "G1", "Chegada").unwrap();
    assert!(out.contains("subgraph G1[\"Chegada\"]"), "{out}");
    assert!(out.contains("C[Tres] --> G1"));
    assert_eq!(flow(&out).subgraphs[0].title, "Chegada");
    // A header without an id of its own gets the id the parser derived.
    let out = edit::rename_group("flowchart TD\n  subgraph Entrada\n    A\n  end\n  X --> Entrada\n", "Entrada", "Chegada").unwrap();
    assert!(out.contains("subgraph Entrada[\"Chegada\"]"), "{out}");
    let f = flow(&out);
    assert_eq!(f.subgraphs[0].id, "Entrada");
    assert_eq!(f.edges.len(), 1);
    // Special characters in the title do not break the line.
    let out = edit::rename_group(GROUPED, "G1", "A \"b\" [c]; d").unwrap();
    assert_eq!(flow(&out).subgraphs[0].title, "A \"b\" [c]; d");
    assert!(edit::rename_group(GROUPED, "G1", "  ").is_err());
    assert!(edit::rename_group(GROUPED, "nope", "x").is_err());
}

#[test]
fn edit_group_edits_refuse_a_text_with_errors() {
    let broken = "flowchart TD\n  subgraph G1[\"Entrada\"]\n    A --> B\n";
    assert!(edit::rename_group(broken, "G1", "x").is_err());
    assert!(edit::move_to_group(broken, "A", None).is_err());
    assert!(edit::ungroup(broken, "G1").is_err());
    assert!(edit::remove_group(broken, "G1").is_err());
}

#[test]
fn edit_move_a_loose_node_into_a_group() {
    let out = edit::move_to_group(GROUPED, "D", Some("G1")).unwrap();
    assert_eq!(group_of(&out, "D").as_deref(), Some("G1"), "{out}");
    assert_eq!(flow(&out).nodes[flow(&out).node_index("D").unwrap()].label, "Quatro");
    assert_eq!(group_of(&out, "A").as_deref(), Some("G1"));
    // Already there: nothing changes.
    assert_eq!(edit::move_to_group(&out, "D", Some("G1")).unwrap(), out);
    assert_eq!(edit::move_to_group(GROUPED, "D", None).unwrap(), GROUPED);
}

#[test]
fn edit_move_a_node_out_of_a_chain_keeps_the_others_and_the_link() {
    let out = edit::move_to_group(GROUPED, "B", None).unwrap();
    assert_eq!(group_of(&out, "B"), None, "{out}");
    assert_eq!(group_of(&out, "A").as_deref(), Some("G1"));
    let f = flow(&out);
    assert_eq!(f.nodes[f.node_index("B").unwrap()].label, "Dois");
    assert!(f.edges.iter().any(|e| e.from == "A" && e.to == "B"), "{out}");
    assert!(f.edges.iter().any(|e| e.from == "C" && e.to == "G1"));
    assert_eq!(f.edges.len(), 2);
}

#[test]
fn edit_move_between_groups_and_nested() {
    let src = "flowchart TD\n  subgraph G1[\"Um\"]\n    A[a] --> B[b]\n    subgraph G2[\"Dois\"]\n      C\n    end\n  end\n  B -->|x| D\n";
    let out = edit::move_to_group(src, "B", Some("G2")).unwrap();
    assert_eq!(group_of(&out, "B").as_deref(), Some("G2"), "{out}");
    assert_eq!(group_of(&out, "A").as_deref(), Some("G1"));
    assert_eq!(group_of(&out, "C").as_deref(), Some("G2"));
    let f = flow(&out);
    assert_eq!(f.nodes[f.node_index("B").unwrap()].label, "b");
    let labelled = f.edges.iter().find(|e| e.from == "B" && e.to == "D").unwrap();
    assert_eq!(labelled.label.as_deref(), Some("x"));
    assert!(f.edges.iter().any(|e| e.from == "A" && e.to == "B"), "{out}");
    // Out of everything, and the loose line keeps its link.
    let out2 = edit::move_to_group(&out, "B", None).unwrap();
    assert_eq!(group_of(&out2, "B"), None, "{out2}");
    assert_eq!(flow(&out2).edges.len(), 2);
}

#[test]
fn edit_move_keeps_style_and_shared_lines() {
    let src = "flowchart TD\n  subgraph G1[\"Um\"]\n    A & B --> C\n  end\n  style B fill:#f9f\n  class A,B k\n  classDef k stroke:#333\n";
    let out = edit::move_to_group(src, "B", None).unwrap();
    assert_eq!(group_of(&out, "B"), None, "{out}");
    let f = flow(&out);
    assert_eq!(f.edges.len(), 2);
    assert!(f.edges.iter().any(|e| e.from == "A" && e.to == "C"));
    assert!(f.edges.iter().any(|e| e.from == "B" && e.to == "C"));
    assert_eq!(f.node_styles.get("B").and_then(|m| m.get("fill")).map(String::as_str), Some("#f9f"));
    assert_eq!(f.nodes[f.node_index("B").unwrap()].classes, vec!["k".to_string()]);
}

#[test]
fn edit_new_group_wraps_a_node() {
    let (out, id) = edit::new_group(GROUPED, "D", "Grupo").unwrap();
    assert_eq!(id, "G2", "{out}");
    assert_eq!(group_of(&out, "D").as_deref(), Some("G2"));
    assert_eq!(flow(&out).subgraphs[1].title, "Grupo");
    assert_eq!(group_of(&out, "A").as_deref(), Some("G1"));
    // A node that was in a group leaves it.
    let (out, id) = edit::new_group(GROUPED, "B", "Novo").unwrap();
    assert_eq!(group_of(&out, "B").as_deref(), Some(id.as_str()), "{out}");
    assert_eq!(group_of(&out, "A").as_deref(), Some("G1"));
    assert!(flow(&out).edges.iter().any(|e| e.from == "A" && e.to == "B"));
}

#[test]
fn edit_ungroup_keeps_the_content_and_drops_what_named_the_group() {
    let src = "flowchart TD\n  subgraph G1[\"Entrada\"]\n    direction LR\n    A[Um] --> B[Dois]\n  end\n  C[Tres] --> G1\n  D --> E\n  style G1 fill:#fee\n  %% mermaidcraft:layout sub:G1=40,0 A=3,4\n";
    let (out, arrows) = edit::ungroup(src, "G1").unwrap();
    assert_eq!(arrows, 1);
    let f = flow(&out);
    assert!(f.subgraphs.is_empty(), "{out}");
    assert!(!out.contains("subgraph") && !out.contains("end") && !out.contains("direction") && !out.contains("style G1"), "{out}");
    assert_eq!(f.nodes.len(), 5, "G1 must not become a node: {out}");
    assert!(f.edges.iter().any(|e| e.from == "A" && e.to == "B"));
    assert_eq!(f.edges.len(), 2);
    assert!(!f.offsets.contains_key("sub:G1"));
    assert_eq!(f.offsets.get("A"), Some(&(3.0, 4.0)));
}

#[test]
fn edit_ungroup_moves_a_nested_group_up() {
    let src = "flowchart TD\n  subgraph G1\n    subgraph G2\n      A\n    end\n    B\n  end\n";
    let (out, arrows) = edit::ungroup(src, "G1").unwrap();
    assert_eq!(arrows, 0);
    let f = flow(&out);
    assert_eq!(f.subgraphs.len(), 1);
    assert_eq!(f.subgraphs[0].id, "G2");
    assert_eq!(f.subgraphs[0].parent, None);
    assert_eq!(group_of(&out, "A").as_deref(), Some("G2"));
    assert_eq!(group_of(&out, "B"), None);
}

#[test]
fn edit_remove_group_takes_the_content_with_it() {
    let src = "flowchart TD\n  subgraph G1[\"Entrada\"]\n    A --> B\n    subgraph G2\n      X\n    end\n  end\n  C --> A\n  C --> D\n  style G1 fill:#fee\n  %% mermaidcraft:layout sub:G1=40,0 sub:G2=1,1 D=3,4\n";
    let out = edit::remove_group(src, "G1").unwrap();
    let f = flow(&out);
    assert!(f.subgraphs.is_empty(), "{out}");
    let ids: Vec<&str> = f.nodes.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(ids, vec!["C", "D"], "{out}");
    assert_eq!(f.edges.len(), 1);
    assert!(!out.contains("style G1") && !out.contains("end") && !out.contains("sub:"), "{out}");
    assert_eq!(f.offsets.get("D"), Some(&(3.0, 4.0)));
}

#[test]
fn edit_group_edits_keep_crlf() {
    let src = GROUPED.replace('\n', "\r\n");
    for out in [edit::move_to_group(&src, "D", Some("G1")).unwrap(), edit::move_to_group(&src, "B", None).unwrap(), edit::new_group(&src, "D", "Novo").unwrap().0, edit::ungroup(&src, "G1").unwrap().0] {
        assert!(!out.replace("\r\n", "").contains('\n'), "a bare LF got in: {out:?}");
    }
}

#[test]
fn item_at_finds_a_group() {
    let f = flow(GROUPED);
    let header = GROUPED.find("subgraph").unwrap() + 3;
    assert_eq!(edit::item_at(&f, header, 2), Some(edit::TextItem::Group("G1".into())));
    let arrow_target = GROUPED.find("--> G1").unwrap() + 5;
    assert_eq!(edit::item_at(&f, arrow_target, 5), Some(edit::TextItem::Group("G1".into())));
    // A line of the group header names the group even with the cursor away from it.
    assert_eq!(edit::item_at(&f, header + 200, 2), Some(edit::TextItem::Group("G1".into())));
    // The nodes inside still win.
    let a = GROUPED.find("A[Um]").unwrap();
    assert_eq!(edit::item_at(&f, a, 3), Some(edit::TextItem::Node("A".into())));
}

#[test]
fn edit_group_moves_leave_tidy_text() {
    // Into a group: the loose line of the node is not left behind as a duplicate.
    let out = edit::move_to_group(GROUPED, "D", Some("G1")).unwrap();
    assert_eq!(out, "flowchart TD\n  subgraph G1[\"Entrada\"]\n    A[Um] --> B[Dois]\n    D[Quatro]\n  end\n  C[Tres] --> G1\n", "{out}");
    // Out of a group: the definition first, then the link.
    let out = edit::move_to_group(GROUPED, "B", None).unwrap();
    assert_eq!(out, "flowchart TD\n  subgraph G1[\"Entrada\"]\n    A[Um]\n  end\n  C[Tres] --> G1\n  D[Quatro]\n  B[Dois]\n  A --> B\n", "{out}");
    // Ungroup: the content goes one level to the left.
    let (out, _) = edit::ungroup(GROUPED, "G1").unwrap();
    assert_eq!(out, "flowchart TD\n  A[Um] --> B[Dois]\n  C[Tres]\n  D[Quatro]\n", "{out}");
}

fn frame_prim(s: &scene::Scene, c: usize) -> &scene::Prim {
    s.prims.iter().find(|p| p.tag == scene::Tag::Cluster(c) && matches!(p.shape, scene::PrimShape::Rect { .. })).unwrap()
}

#[test]
fn a_style_line_of_a_group_colours_its_frame() {
    let plain = "flowchart TD\n  subgraph G1[\"Um\"]\n    A\n  end\n";
    let (s0, _) = render_source_with(plain, &RenderOptions::clean()).unwrap();
    let base = frame_prim(&s0, 0).clone();
    let styled = format!("{plain}  style G1 fill:#ff0000,stroke:#00ff00,stroke-dasharray:0,color:#0000ff\n");
    let (s, _) = render_source_with(&styled, &RenderOptions::clean()).unwrap();
    let p = frame_prim(&s, 0);
    assert_eq!(p.fill, scene::parse_color("#ff0000"));
    assert_eq!(p.stroke, scene::parse_color("#00ff00"));
    assert_eq!(p.dash, None);
    assert_ne!(base.fill, p.fill);
    // The title takes `color`.
    let title = s.prims.iter().find(|p| p.tag == scene::Tag::Cluster(0) && matches!(p.shape, scene::PrimShape::Text { .. })).unwrap();
    assert_eq!(title.fill, scene::parse_color("#0000ff"));
    // A dash pattern is used as written, and a group without `style` is unchanged.
    let dashed = format!("{plain}  style G1 stroke-dasharray:3 4\n");
    let (s, _) = render_source_with(&dashed, &RenderOptions::clean()).unwrap();
    assert_eq!(frame_prim(&s, 0).dash, Some((3.0, 4.0)));
    let (s, _) = render_source_with(plain, &RenderOptions::clean()).unwrap();
    assert_eq!(frame_prim(&s, 0), &base);
}

#[test]
fn edit_set_node_style_takes_a_group_id() {
    let mut m = StyleMap::new();
    m.insert("fill".into(), "#fee".into());
    let out = edit::set_node_style(GROUPED, "G1", &m).unwrap();
    assert_eq!(flow(&out).node_styles.get("G1").and_then(|s| s.get("fill")).map(String::as_str), Some("#fee"), "{out}");
    assert_eq!(flow(&out).nodes.len(), 4, "the group must not become a node: {out}");
    let back = edit::set_node_style(&out, "G1", &StyleMap::new()).unwrap();
    assert_eq!(back, GROUPED);
}

/// Rectangle of the title of group `c`, from the text the scene drew for it.
fn title_rect(s: &scene::Scene, title: &str, c: usize, font: look::Font) -> scene::Rect {
    let t = s.prims.iter().find(|p| p.tag == scene::Tag::Cluster(c) && matches!(p.shape, scene::PrimShape::Text { .. })).unwrap();
    let scene::PrimShape::Text { x, y, anchor, .. } = &t.shape else { panic!("not text") };
    let (w, h) = text::label_size(title, 14.0, true, font);
    let left = if matches!(anchor, scene::Anchor::Middle) { x - w / 2.0 } else { *x };
    scene::Rect { x: left, y: y - h / 2.0, w, h }
}

/// Is the title of group `c` drawn over edge `ei`, on a patch of its own (the frame had no free
/// place for it)?
fn over_the_arrows(s: &scene::Scene, c: usize, ei: usize) -> bool {
    let patch = s.prims.iter().position(|p| is_patch(p, c));
    let last_edge = s.prims.iter().rposition(|p| p.tag == scene::Tag::Edge(ei));
    matches!((patch, last_edge), (Some(a), Some(b)) if a > b)
}

/// The patch under a title drawn over the arrows: a filled rectangle of the group with no outline
/// (the frame itself has one, in every look).
fn is_patch(p: &scene::Prim, c: usize) -> bool {
    p.tag == scene::Tag::Cluster(c) && p.stroke.is_none() && matches!(p.shape, scene::PrimShape::Rect { .. })
}

/// Titles that sit over the arrows, on a patch.
fn titles_over(src: &str, opts: &RenderOptions) -> usize {
    let f = flow(src);
    let (s, _) = render_source_with(src, opts).unwrap();
    (0..f.subgraphs.len()).filter(|c| s.prims.iter().any(|p| is_patch(p, *c))).count()
}

fn titles_crossed(src: &str, opts: &RenderOptions) -> Vec<String> {
    let f = flow(src);
    let (s, _) = render_source_with(src, opts).unwrap();
    let mut hits = Vec::new();
    for (c, sg) in f.subgraphs.iter().enumerate().filter(|(_, sg)| !sg.title.is_empty()) {
        let r = title_rect(&s, &sg.title, c, opts.font);
        for (ei, e) in f.edges.iter().enumerate() {
            // An edge that ends on the frame itself is not an arrow through its title.
            if f.subgraph_index(&e.from).is_some() || f.subgraph_index(&e.to).is_some() {
                continue;
            }
            if s.edge_paths[ei].windows(2).any(|w| geom::segment_hits_rect(w[0], w[1], &r)) && !over_the_arrows(&s, c, ei) {
                hits.push(format!("{} --> {} crosses the title of {}", e.from, e.to, sg.id));
            }
        }
    }
    hits
}

#[test]
fn no_arrow_crosses_the_title_of_a_group() {
    let small = "flowchart TD\n  X --> A\n  subgraph G[\"Titulo comprido do grupo inteiro\"]\n    A --> B\n  end\n";
    let up = "flowchart TD\n  subgraph G[\"Titulo comprido do grupo inteiro\"]\n    A --> B\n  end\n  A --> X\n";
    let long = include_str!("fixtures/fluxo-longo.mmd");
    let wide = "flowchart TD\n  S --> A & B & C & D\n  subgraph G[\"Titulo do grupo largo\"]\n    A\n    B\n    C\n    D\n  end\n";
    let nested = "flowchart TD\n  X --> A\n  subgraph G1[\"Externo com titulo comprido\"]\n    P --> Q\n    subgraph G2[\"Interno com titulo comprido\"]\n      A --> B\n    end\n  end\n  X --> P\n";
    for (name, src) in [("small", small), ("up", up), ("fluxo-longo", long), ("nested", nested), ("wide", wide)] {
        for opts in [RenderOptions::default(), RenderOptions::clean()] {
            let hits = titles_crossed(src, &opts);
            assert!(hits.is_empty(), "{name}: {hits:?}");
        }
    }
}

#[test]
fn group_moves_keep_classes_and_stay_at_the_root() {
    // `class` inside a group must not pull the node into that group.
    let out = edit::move_to_group("flowchart TD\n  subgraph G1\n    A\n    class A foo\n  end\n  subgraph G2\n    B\n  end\n", "B", None).unwrap();
    assert_eq!(group_of(&out, "B"), None, "{out}");
    assert_eq!(group_of(&out, "A").as_deref(), Some("G1"));
    // A loose node with a `class` line at the end keeps its class when it is wrapped in a group.
    let src = "flowchart TD\n  A[Lbl]\n  B --> C\n  classDef foo fill:#f00\n  class A foo\n";
    let (out, _) = edit::new_group(src, "A", "G").unwrap();
    let f = flow(&out);
    assert_eq!(f.nodes[f.node_index("A").unwrap()].classes, vec!["foo".to_string()], "{out}");
    assert_eq!(group_of(&out, "A").as_deref(), Some("G1"));
    // `:::class` on a mention without a shape survives the move.
    let src = "flowchart TD\n  subgraph G1\n    A:::foo\n  end\n  subgraph G2\n    B\n  end\n  A[Lbl]\n  classDef foo fill:#f00\n";
    let out = edit::move_to_group(src, "A", Some("G2")).unwrap();
    let f = flow(&out);
    assert_eq!(f.nodes[f.node_index("A").unwrap()].classes, vec!["foo".to_string()], "{out}");
    assert_eq!(group_of(&out, "A").as_deref(), Some("G2"));
    // A node named in two groups is moved out of both.
    let out = edit::move_to_group("flowchart TD\n  subgraph G1\n    A\n  end\n  subgraph G2\n    A\n  end\n  subgraph G3\n    B\n  end\n", "A", Some("G2")).unwrap();
    assert_eq!(group_of(&out, "A").as_deref(), Some("G2"));
    assert_eq!(out.matches('A').count(), 1, "{out}");
}

#[test]
fn a_title_detour_does_not_run_into_a_node_beside_the_title() {
    let src = "flowchart TD\n  subgraph G1[\"Muito muito muito longo titulo\"]\n    A\n  end\n  X --> A\n  Y --> A\n  A --> Z\n  A --> X\n";
    let f = flow(src);
    let (s, _) = render_source_with(src, &RenderOptions::clean()).unwrap();
    for (ei, e) in f.edges.iter().enumerate() {
        for (ni, n) in f.nodes.iter().enumerate().filter(|(_, n)| n.id != e.from && n.id != e.to) {
            let b = &s.node_boxes[ni];
            let r = scene::Rect { x: b.x + 2.0, y: b.y + 2.0, w: b.w - 4.0, h: b.h - 4.0 };
            assert!(!s.edge_paths[ei].windows(2).any(|w| geom::segment_hits_rect(w[0], w[1], &r)), "{} --> {} runs through {}", e.from, e.to, n.id);
        }
    }
}

#[test]
fn keeping_arrows_off_a_title_does_not_bend_them() {
    // The title moves out of the way; the arrows into the group stay as the layout drew them.
    let wide = "flowchart TD\n  S --> A & B & C & D\n  subgraph G[\"Titulo do grupo largo\"]\n    A\n    B\n    C\n    D\n  end\n";
    let nested = "flowchart TD\n  X --> A\n  subgraph G1[\"Externo com titulo comprido\"]\n    P --> Q\n    subgraph G2[\"Interno com titulo comprido\"]\n      A --> B\n    end\n  end\n  X --> P\n";
    for src in [wide, nested] {
        for opts in [RenderOptions::default(), RenderOptions::clean()] {
            let (s, _) = render_source_with(src, &opts).unwrap();
            let plain = src.replace("[\"Titulo do grupo largo\"]", "").replace("[\"Externo com titulo comprido\"]", "").replace("[\"Interno com titulo comprido\"]", "");
            let (p, _) = render_source_with(&plain, &opts).unwrap();
            let len = |path: &[(f32, f32)]| path.windows(2).map(|w| ((w[1].0 - w[0].0).powi(2) + (w[1].1 - w[0].1).powi(2)).sqrt()).sum::<f32>();
            let total = |sc: &scene::Scene| sc.edge_paths.iter().map(|path| len(path)).sum::<f32>();
            // Titled frames are a little taller; the routes must not grow much beyond that.
            assert!(total(&s) <= total(&p) * 1.25 + 40.0, "{} vs {} for {src}", total(&s), total(&p));
        }
    }
}

#[test]
fn a_title_sits_over_the_arrows_only_when_no_place_is_free() {
    let wide = "flowchart TD\n  S --> A & B & C & D\n  subgraph G[\"Titulo do grupo largo\"]\n    A\n    B\n    C\n    D\n  end\n";
    let narrow = "flowchart TD\n  X --> A\n  subgraph G[\"Titulo comprido do grupo inteiro\"]\n    A --> B\n  end\n";
    for opts in [RenderOptions::default(), RenderOptions::clean()] {
        // Four arrows spread over the top leave no gap as wide as the title: it sits over them.
        assert!(titles_crossed(wide, &opts).is_empty());
        assert_eq!(titles_over(narrow, &opts), 1);
    }
    // A group no arrow enters keeps its title in the usual place, without a patch.
    // One arrow down the middle of a wide frame: the title keeps its corner.
    let roomy = "flowchart TD\n  X --> B\n  subgraph G[\"Curto\"]\n    A --> B\n    C --> B\n    D --> B\n  end\n";
    assert_eq!(titles_over(roomy, &RenderOptions::clean()), 0);
    assert!(titles_crossed(roomy, &RenderOptions::clean()).is_empty());
    let alone = "flowchart TD\n  subgraph G[\"Sozinho\"]\n    A --> B\n  end\n";
    // (in the default look the usual place is the top-left corner; the classic one centres it)
    let (s, _) = render_source_with(alone, &RenderOptions::default()).unwrap();
    let f = flow(alone);
    let r = title_rect(&s, "Sozinho", 0, RenderOptions::default().font);
    assert!((r.x - (s.cluster_boxes[0].x + 12.0)).abs() < 0.5, "{} vs {}", r.x, s.cluster_boxes[0].x);
    assert_eq!(titles_over(alone, &RenderOptions::clean()), 0);
    assert_eq!(f.subgraphs.len(), 1);
}

#[test]
fn edit_groups_with_the_same_id_are_refused() {
    let src = "flowchart TD\n  subgraph Etapa\n    A[Primeira]\n  end\n  subgraph Etapa\n    B[Segunda]\n  end\n  C[Solto]\n";
    assert!(edit::remove_group(src, "Etapa").is_err());
    assert!(edit::ungroup(src, "Etapa").is_err());
    assert!(edit::rename_group(src, "Etapa", "x").is_err());
    assert!(edit::move_to_group(src, "C", Some("Etapa")).is_err());
    let msg = edit::remove_group(src, "Etapa").unwrap_err().to_string();
    assert!(msg.contains("two groups"), "{msg}");
}

#[test]
fn edit_move_keeps_the_label_that_counts() {
    // The last definition is the one shown.
    let src = "flowchart TD\n  X[Raiz] --> Y\n  subgraph G1\n    X[Novo]\n  end\n  subgraph G2\n    B\n  end\n";
    assert_eq!(flow(src).nodes[0].label, "Novo");
    for dest in [Some("G2"), None] {
        let out = edit::move_to_group(src, "X", dest).unwrap();
        let f = flow(&out);
        assert_eq!(f.nodes[f.node_index("X").unwrap()].label, "Novo", "{dest:?}: {out}");
        assert_eq!(group_of(&out, "X").as_deref(), dest, "{out}");
    }
    let src = "flowchart TD\n  subgraph G1\n    X[Um]\n  end\n  subgraph G2\n    X{Dois}\n  end\n  subgraph G3\n    B\n  end\n";
    let out = edit::move_to_group(src, "X", Some("G3")).unwrap();
    let f = flow(&out);
    let x = &f.nodes[f.node_index("X").unwrap()];
    assert_eq!((x.label.as_str(), x.shape), ("Dois", NodeShape::Diamond), "{out}");
}

#[test]
fn link_styles_follow_their_links() {
    // Moving X out sends A --> X to the end: its style goes with it.
    let src = "flowchart TD\n  subgraph G1\n    A --> X\n    B --> C\n  end\n  linkStyle 0 stroke:red\n  linkStyle 1 stroke:blue\n";
    let out = edit::move_to_group(src, "X", None).unwrap();
    let f = flow(&out);
    let ax = f.edges.iter().position(|e| e.from == "A" && e.to == "X").unwrap();
    let bc = f.edges.iter().position(|e| e.from == "B" && e.to == "C").unwrap();
    assert!(out.contains(&format!("linkStyle {ax} stroke:red")) && out.contains(&format!("linkStyle {bc} stroke:blue")), "{out}");
    // A removed link takes its number out of the list, and a list left empty goes.
    let src = "flowchart TD\n  A --> B\n  B --> C\n  C --> D\n  linkStyle 0,2 stroke:red\n  linkStyle 1 stroke:blue\n  linkStyle default stroke:#999\n";
    let out = edit::remove_edge(src, 1).unwrap();
    assert!(out.contains("linkStyle 0,1 stroke:red") && !out.contains("stroke:blue") && out.contains("linkStyle default stroke:#999"), "{out}");
    let out = edit::remove_node(src, "A").unwrap();
    assert!(out.contains("linkStyle 1 stroke:red") && out.contains("linkStyle 0 stroke:blue"), "{out}");
    // Reversing a link keeps its number on it.
    let src = "flowchart TD\n  A --> B --> C\n  D --> E\n  linkStyle 0 stroke:red\n  linkStyle 2 stroke:blue\n";
    let (out, n) = edit::reverse_edge(src, 0).unwrap();
    let f = flow(&out);
    let de = f.edges.iter().position(|e| e.from == "D").unwrap();
    assert!(out.contains(&format!("linkStyle {n} stroke:red")) && out.contains(&format!("linkStyle {de} stroke:blue")), "{out}");
    // Ungroup and removing a group drop the styles of the links that went.
    let src = "flowchart TD\n  subgraph G1\n    A --> B\n  end\n  C --> G1\n  C --> D\n  linkStyle 1 stroke:red\n  linkStyle 2 stroke:blue\n";
    let (out, _) = edit::ungroup(src, "G1").unwrap();
    assert!(!out.contains("stroke:red") && out.contains("linkStyle 1 stroke:blue"), "{out}");
    let out = edit::remove_group(src, "G1").unwrap();
    assert!(!out.contains("stroke:red") && out.contains("linkStyle 0 stroke:blue"), "{out}");
}

#[test]
fn link_styles_follow_twin_links_and_spaced_lists() {
    // Changing the label of one of two twin links moves no link: the numbers stay.
    let src = "flowchart TD\n  A -->|x| B\n  A -->|x| B\n  A --> B\n  linkStyle 0 stroke:red\n  linkStyle 2 stroke:blue\n";
    let out = edit::set_edge(src, 0, Some(None), None).unwrap();
    assert!(out.contains("linkStyle 0 stroke:red") && out.contains("linkStyle 2 stroke:blue"), "{out}");
    // Reversing in place keeps the number, even when the result is a twin of another link.
    let src = "flowchart TD\n  A --> B\n  B --> A\n  linkStyle 0 stroke:red\n  linkStyle 1 stroke:blue\n";
    let (out, n) = edit::reverse_edge(src, 0).unwrap();
    assert_eq!(n, 0);
    assert!(out.contains("linkStyle 0 stroke:red") && out.contains("linkStyle 1 stroke:blue"), "{out}");
    let src = "flowchart TD\n  A --> B\n  B --> C\n  C --> B\n  linkStyle 2 stroke:red\n";
    let (out, _) = edit::reverse_edge(src, 1).unwrap();
    assert!(out.contains("linkStyle 2 stroke:red"), "{out}");
    // Removing the first of two twins drops its style and moves the other's.
    let src = "flowchart TD\n  A --> B\n  A --> B\n  linkStyle 0 stroke:red\n  linkStyle 1 stroke:blue\n";
    let out = edit::remove_edge(src, 0).unwrap();
    assert!(!out.contains("stroke:red") && out.contains("linkStyle 0 stroke:blue"), "{out}");
    // A list written with spaces is renumbered too.
    let src = "flowchart TD\n  A --> B\n  B --> C\n  C --> D\n  linkStyle 1, 2 stroke:red\n";
    let out = edit::remove_edge(src, 0).unwrap();
    assert!(out.contains("linkStyle 0,1 stroke:red"), "{out}");
}

#[test]
fn a_title_over_a_see_through_frame_still_hides_the_arrows() {
    let src = "flowchart TD\n  X --> A\n  subgraph G[\"Titulo comprido do grupo inteiro\"]\n    A --> B\n  end\n  style G fill:none\n";
    let (s, _) = render_source_with(src, &RenderOptions::clean()).unwrap();
    let patch = s.prims.iter().find(|p| is_patch(p, 0)).expect("patch");
    assert_eq!(patch.fill.map(|c| c[3]), Some(255));
}

#[test]
fn removing_either_twin_keeps_the_other_ones_style() {
    let src = "flowchart TD\n  A --> B\n  A --> B\n  linkStyle 0 stroke:red\n  linkStyle 1 stroke:blue\n";
    let out = edit::remove_edge(src, 1).unwrap();
    assert!(out.contains("linkStyle 0 stroke:red") && !out.contains("stroke:blue"), "{out}");
    let out = edit::remove_edge(src, 0).unwrap();
    assert!(out.contains("linkStyle 0 stroke:blue") && !out.contains("stroke:red"), "{out}");
    // A number that named no link is left as written.
    let src = "flowchart TD\n  A --> B\n  B --> C\n  linkStyle 9 stroke:red\n";
    let out = edit::remove_edge(src, 0).unwrap();
    assert!(out.contains("linkStyle 9 stroke:red"), "{out}");
}
