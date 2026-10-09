//! Shared helpers for layout tests.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use archalign_layout::{Rect, Scene};
use archalign_model::ArchitectureDoc;
use serde_json::Value;

pub fn doc(v: Value) -> ArchitectureDoc {
    serde_json::from_value(v).expect("test document parses")
}

pub fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/archify")
}

pub fn fixtures() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.to_string_lossy().ends_with(".architecture.json") {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(&fixture_root(), &mut out);
    out.sort();
    out
}

pub fn load_fixture(path: &Path) -> ArchitectureDoc {
    archalign_model::load_architecture(&std::fs::read(path).unwrap())
        .unwrap()
        .doc
}

/// Asserts the FR-LAY-07 geometric invariants (not determinism).
pub fn check_invariants(scene: &Scene) -> Result<(), String> {
    let n = &scene.nodes;
    for i in 0..n.len() {
        for j in (i + 1)..n.len() {
            if n[i].rect.overlaps(&n[j].rect) {
                return Err(format!("nodes {} and {} overlap", n[i].id, n[j].id));
            }
        }
    }
    for b in &scene.boundaries {
        for other in &scene.boundaries {
            if other.parent == Some(b.index) && !b.rect.contains_rect(&other.rect) {
                return Err(format!(
                    "boundary {} does not contain {}",
                    b.index, other.index
                ));
            }
        }
    }
    for e in &scene.edges {
        let pts = &e.route.points;
        if pts.len() < 2 {
            return Err(format!("edge {} has {} points", e.index, pts.len()));
        }
        for w in pts.windows(2) {
            let (a, b) = (w[0], w[1]);
            if (a.x - b.x).abs() > 1e-9 && (a.y - b.y).abs() > 1e-9 {
                return Err(format!(
                    "edge {} has a diagonal segment {a:?}→{b:?}",
                    e.index
                ));
            }
            for node in n {
                if node.index == e.from || node.index == e.to {
                    continue;
                }
                if node.rect.segment_crosses_interior(a, b) {
                    return Err(format!(
                        "edge {} segment {a:?}→{b:?} crosses node {}",
                        e.index, node.id
                    ));
                }
            }
        }
    }
    Ok(())
}

/// Whether `r` contains `p` on its border.
pub fn on_border(r: &Rect, x: f64, y: f64) -> bool {
    let eps = 1e-6;
    let on_v = ((x - r.x).abs() < eps || (x - r.right()).abs() < eps)
        && y >= r.y - eps
        && y <= r.bottom() + eps;
    let on_h = ((y - r.y).abs() < eps || (y - r.bottom()).abs() < eps)
        && x >= r.x - eps
        && x <= r.right() + eps;
    on_v || on_h
}
