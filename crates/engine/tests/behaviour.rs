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
