#![allow(clippy::unwrap_used, clippy::panic, clippy::expect_used)]
use mermaidcraft_engine::*;

fn example(name: &str) -> String {
    let p = format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(p).unwrap()
}

#[test]
fn dump_examples() {
    let out = std::env::var("MC_DUMP").ok();
    for n in ["flowchart.mmd", "sequence.mmd", "er.mmd"] {
        let src = example(n);
        let parsed = parse(&src);
        assert!(!parsed.has_errors(), "{n}: {:?}", parsed.diagnostics);
        let (scene, _) = render_source(&src).unwrap();
        let svg = scene.to_svg();
        assert!(svg.contains("<svg"));
        let png = export::svg_to_png(&svg, 2.0).unwrap();
        let pdf = export::svg_to_pdf(&svg).unwrap();
        assert!(png.starts_with(&[0x89, b'P', b'N', b'G']));
        assert!(pdf.starts_with(b"%PDF"));
        if let Some(dir) = &out {
            std::fs::create_dir_all(dir).unwrap();
            std::fs::write(format!("{dir}/{n}.svg"), &svg).unwrap();
            std::fs::write(format!("{dir}/{n}.png"), &png).unwrap();
            std::fs::write(format!("{dir}/{n}.pdf"), &pdf).unwrap();
        }
    }
}
