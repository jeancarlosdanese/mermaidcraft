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
