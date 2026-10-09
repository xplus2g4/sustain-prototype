//! FR-LAY-07: auto-layout invariants over random graphs (up to 60 nodes, 120 edges and
//! 2-level boundaries).

mod common;

use archalign_layout::{FixedAdvance, SceneMode, layout_document};
use common::{check_invariants, doc};
use proptest::prelude::*;
use serde_json::{Value, json};

const TYPES: [&str; 7] = [
    "frontend",
    "backend",
    "database",
    "cloud",
    "security",
    "messagebus",
    "external",
];

/// A top-level `[start, end)` range of nodes with an optional nested sub-range.
type Group = (usize, usize, Option<(usize, usize)>);

#[derive(Debug, Clone)]
struct Spec {
    labels: Vec<(String, Option<String>, Option<String>, usize)>,
    edges: Vec<(usize, usize)>,
    groups: Vec<Group>,
}

fn spec() -> impl Strategy<Value = Spec> {
    (1usize..=60)
        .prop_flat_map(|n| {
            let label = (
                "[A-Za-z][A-Za-z ]{0,30}",
                proptest::option::of("[a-z ]{1,40}"),
                proptest::option::of("[a-z0-9]{1,8}"),
                0usize..7,
            );
            (
                proptest::collection::vec(label, n),
                proptest::collection::vec((0..n, 0..n), 0..=120),
                proptest::collection::vec((0..n, 1usize..12, any::<bool>()), 0..5),
            )
        })
        .prop_map(|(labels, edges, raw_groups)| {
            let n = labels.len();
            // Disjoint top-level ranges, in order; optional nested sub-range.
            let mut groups = Vec::new();
            let mut taken_until = 0usize;
            let mut starts: Vec<_> = raw_groups;
            starts.sort();
            for (start, len, nested) in starts {
                let start = start.max(taken_until);
                if start >= n {
                    break;
                }
                let end = (start + len).min(n);
                let sub = (nested && end - start >= 2).then(|| (start + 1, end));
                groups.push((start, end, sub));
                taken_until = end;
            }
            Spec {
                labels,
                edges,
                groups,
            }
        })
}

fn build(s: &Spec) -> Value {
    let components: Vec<Value> = s
        .labels
        .iter()
        .enumerate()
        .map(|(i, (label, sub, tag, ty))| {
            let mut c = json!({ "id": format!("n{i}"), "type": TYPES[*ty], "label": label });
            if let Some(sub) = sub {
                c["sublabel"] = json!(sub);
            }
            if let Some(tag) = tag {
                c["tag"] = json!(tag);
            }
            c
        })
        .collect();
    let connections: Vec<Value> = s
        .edges
        .iter()
        .map(|(a, b)| json!({ "from": format!("n{a}"), "to": format!("n{b}") }))
        .collect();
    let mut boundaries = Vec::new();
    for (k, (start, end, sub)) in s.groups.iter().enumerate() {
        let wraps: Vec<String> = (*start..*end).map(|i| format!("n{i}")).collect();
        boundaries.push(json!({ "kind": "region", "label": format!("G{k}"), "wraps": wraps }));
        if let Some((a, b)) = sub {
            let wraps: Vec<String> = (*a..*b).map(|i| format!("n{i}")).collect();
            boundaries.push(
                json!({ "kind": "security-group", "label": format!("S{k}"), "wraps": wraps }),
            );
        }
    }
    json!({
        "schema_version": 1, "diagram_type": "architecture",
        "meta": { "title": "random", "output": "r.html" },
        "components": components,
        "connections": connections,
        "boundaries": boundaries
    })
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, ..ProptestConfig::default() })]

    #[test]
    fn auto_layout_invariants_hold(s in spec()) {
        let d = doc(build(&s));
        let m = FixedAdvance::default();
        let scene = layout_document(&d, &m);
        prop_assert_eq!(scene.mode, SceneMode::Auto);
        prop_assert_eq!(scene.nodes.len(), s.labels.len());
        prop_assert_eq!(scene.edges.len(), s.edges.len());
        if let Err(e) = check_invariants(&scene) {
            prop_assert!(false, "{}", e);
        }
        // Every boundary contains its members.
        for b in &scene.boundaries {
            for id in &d.boundaries()[b.index].wraps {
                let node = scene.node(id).unwrap();
                prop_assert!(b.rect.contains_rect(&node.rect), "boundary {} misses {}", b.index, id);
            }
        }
        // Same input, same output.
        let again = layout_document(&d, &m);
        prop_assert_eq!(&scene, &again);
    }
}
