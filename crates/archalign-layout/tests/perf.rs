//! NFR-PERF-03: layout and routing of a 60-node diagram in under 100 ms (release build).
//! Run with `cargo test --workspace --release -- --ignored perf_`.

mod common;

use std::time::{Duration, Instant};

use archalign_layout::{FixedAdvance, SceneMode, layout_document};
use common::{check_invariants, doc};
use serde_json::json;

/// A deterministic 60-node, 120-edge graph with nested boundaries.
fn sixty_node_doc() -> archalign_model::ArchitectureDoc {
    let types = [
        "frontend",
        "backend",
        "database",
        "cloud",
        "security",
        "messagebus",
        "external",
    ];
    let components: Vec<_> = (0..60)
        .map(|i| {
            json!({
                "id": format!("n{i}"), "type": types[i % 7],
                "label": format!("Service number {i}"),
                "sublabel": if i % 3 == 0 { json!("handles requests and events") } else { json!(null) },
                "tag": format!("t{i}")
            })
        })
        .map(|mut c| {
            if c["sublabel"].is_null() {
                c.as_object_mut().unwrap().remove("sublabel");
            }
            c
        })
        .collect();
    // A linear congruential sequence keeps the edge set fixed.
    let mut x: u64 = 12345;
    let mut next = || {
        x = x
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (x >> 33) as usize
    };
    let connections: Vec<_> = (0..120)
        .map(|k| {
            let a = if k < 59 { k } else { next() % 60 };
            let b = if k < 59 { k + 1 } else { next() % 60 };
            json!({ "from": format!("n{a}"), "to": format!("n{b}"), "label": format!("e{k}") })
        })
        .collect();
    let wraps = |r: std::ops::Range<usize>| r.map(|i| format!("n{i}")).collect::<Vec<_>>();
    doc(json!({
        "schema_version": 1, "diagram_type": "architecture",
        "meta": { "title": "perf", "output": "p.html" },
        "components": components,
        "connections": connections,
        "boundaries": [
            { "kind": "region", "label": "A", "wraps": wraps(0..20) },
            { "kind": "security-group", "label": "A1", "wraps": wraps(5..12) },
            { "kind": "region", "label": "B", "wraps": wraps(25..45) },
            { "kind": "security-group", "label": "B1", "wraps": wraps(30..35) }
        ]
    }))
}

#[test]
#[ignore = "release-mode timing test; run with --release -- --ignored perf_"]
fn perf_layout_and_routing_of_60_nodes_under_100ms() {
    let d = sixty_node_doc();
    let m = FixedAdvance::default();
    // Warm-up (allocator, lazy statics).
    let scene = layout_document(&d, &m);
    assert_eq!(scene.mode, SceneMode::Auto);
    check_invariants(&scene).unwrap();
    let mut times: Vec<Duration> = (0..7)
        .map(|_| {
            let t = Instant::now();
            let s = layout_document(&d, &m);
            let el = t.elapsed();
            assert_eq!(s.edges.len(), 120);
            el
        })
        .collect();
    times.sort();
    let median = times[times.len() / 2];
    println!(
        "layout + routing, 60 nodes / 120 edges: median of 7 = {median:?} (max {:?})",
        times[6]
    );
    assert!(median < Duration::from_millis(100), "took {median:?}");
}
