//! NFR-PERF-02: opening a 2 MB text file (read, build the line index, highlight the first
//! visible window) in under 300 ms. Run with `cargo test --workspace --release -- --ignored perf_`.

use std::time::{Duration, Instant};

use archalign_desktop::source::{FileHighlighter, select_syntax};
use archalign_repo::text::{TextLoad, load_text};
use archalign_repo::{RelPath, Sandbox};

#[test]
#[ignore = "release-mode timing test; run with --release -- --ignored perf_"]
fn perf_open_2mb_file_read_index_and_highlight_first_window() {
    let dir = tempfile::tempdir().unwrap();
    let mut src = String::new();
    let mut i = 0;
    while src.len() < 2 * 1024 * 1024 - 200 {
        src.push_str(&format!(
            "pub fn function_{i}(x: u64) -> u64 {{ let y = x * {i}; // comment {i}\n    y + 1 }}\n"
        ));
        i += 1;
    }
    std::fs::write(dir.path().join("big.rs"), &src).unwrap();
    let sandbox = Sandbox::new(dir.path()).unwrap();
    let rel: RelPath = "big.rs".parse().unwrap();
    // Warm the syntax set once, as the app does at startup.
    let _ = select_syntax("x.rs", "");
    let mut times = Vec::new();
    for _ in 0..5 {
        let t = Instant::now();
        let TextLoad::Text { text, index, .. } = load_text(&sandbox, &rel).unwrap() else {
            panic!("text")
        };
        let syntax = select_syntax("big.rs", index.line(&text, 0)).unwrap();
        let mut h = FileHighlighter::new(syntax, archalign_desktop::palette::ThemeChoice::Dark);
        h.ensure(&text, &index, 0..80);
        assert!(h.line(79).is_some());
        times.push(t.elapsed());
    }
    times.sort();
    let median = times[2];
    println!("open 2 MB file (read + index + highlight 80 lines): median {median:?}");
    assert!(median < Duration::from_millis(300), "{median:?}");
}

/// NFR-PERF-04: building the canvas geometry (both layers) for a 100-node diagram < 8 ms.
#[test]
#[ignore = "release-mode timing test; run with --release -- --ignored perf_"]
fn perf_canvas_geometry_for_100_nodes_under_8ms() {
    use archalign_desktop::diagram::camera::{Bounds, Camera};
    use archalign_desktop::diagram::canvas::{DiagramCanvas, build_geometry};
    use archalign_desktop::diagram::logic;
    use iced_test::core::renderer::Headless;
    use std::collections::BTreeSet;

    archalign_desktop::fonts::ensure_loaded();
    let types = [
        "frontend",
        "backend",
        "database",
        "cloud",
        "security",
        "messagebus",
        "external",
    ];
    let components: Vec<_> = (0..100)
        .map(|i| {
            serde_json::json!({
                "id": format!("n{i}"), "type": types[i % 7], "label": format!("Service {i}"),
                "sublabel": "handles requests", "tag": format!("t{i}"),
                "sources": [{ "path": format!("src/m{i}.rs"), "line": 1 }]
            })
        })
        .collect();
    let connections: Vec<_> = (0..150)
        .map(|k| serde_json::json!({ "from": format!("n{}", k % 100), "to": format!("n{}", (k * 7 + 3) % 100), "label": format!("e{k}") }))
        .collect();
    let doc: archalign_model::ArchitectureDoc = serde_json::from_value(serde_json::json!({
        "schema_version": 1, "diagram_type": "architecture",
        "meta": { "title": "perf", "output": "p.html" },
        "components": components, "connections": connections,
        "boundaries": [{ "kind": "region", "label": "R", "wraps": (0..30).map(|i| format!("n{i}")).collect::<Vec<_>>() }]
    }))
    .unwrap();
    let scene = archalign_layout::layout_document(&doc, &archalign_desktop::fonts::IcedTextMeasure);
    let size = iced::Size::new(1000.0, 800.0);
    let b = scene.bounds;
    let camera = Camera::fit(
        Bounds {
            x: b.x as f32,
            y: b.y as f32,
            w: b.w as f32,
            h: b.h as f32,
        },
        size,
    );
    let badges = logic::badges(&doc, &[]);
    let legend = logic::legend(&doc);
    let reverse = BTreeSet::from([3, 4]);
    let reach = logic::reach(&doc, 0, logic::Direction::Downstream);
    let palette = archalign_desktop::palette::ThemeChoice::Dark.palette();
    let program = DiagramCanvas {
        doc: &doc,
        scene: &scene,
        badges: &badges,
        legend: &legend,
        camera,
        palette,
        theme: archalign_desktop::palette::ThemeChoice::Dark,
        version: 1,
        selected_node: Some(0),
        selected_edge: None,
        reach: Some(&reach),
        reverse: &reverse,
        hover: Some(5),
    };
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let renderer: iced::Renderer = rt
        .block_on(<iced::Renderer as Headless>::new(
            archalign_desktop::fonts::MONO,
            iced::Pixels(16.0),
            None,
        ))
        .expect("headless renderer");
    let _ = build_geometry(&renderer, size, &program);
    let mut times = Vec::new();
    for _ in 0..9 {
        let t = Instant::now();
        let g = build_geometry(&renderer, size, &program);
        times.push(t.elapsed());
        assert_eq!(g.len(), 2);
    }
    times.sort();
    let median = times[4];
    println!("canvas geometry, 100 nodes / 150 edges (both layers): median {median:?}");
    assert!(median < Duration::from_millis(8), "{median:?}");
}
