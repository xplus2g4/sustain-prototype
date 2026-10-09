//! FR-VAL-01..03 against every vendored Archify fixture.

use std::path::{Path, PathBuf};

use archalign_model::{load_architecture, to_pretty_json};
use serde_json::Value;

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/archify")
}

fn fixtures() -> Vec<PathBuf> {
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
    assert_eq!(out.len(), 24, "all vendored fixtures are found");
    out
}

#[test]
fn every_fixture_loads() {
    for path in fixtures() {
        let bytes = std::fs::read(&path).unwrap();
        if let Err(failure) = load_architecture(&bytes) {
            panic!("{} failed: {:#?}", path.display(), failure.errors);
        }
    }
}

#[test]
fn load_save_round_trips_to_equal_json_value() {
    for path in fixtures() {
        let bytes = std::fs::read(&path).unwrap();
        let input: Value = serde_json::from_slice(&bytes).unwrap();
        let loaded = load_architecture(&bytes).unwrap();
        let saved = to_pretty_json(&loaded.doc);
        let output: Value = serde_json::from_slice(&saved).unwrap();
        assert_eq!(input, output, "{} round-trip differs", path.display());
    }
}

#[test]
fn saved_output_is_two_space_pretty_with_integer_numbers() {
    let path = fixture_root().join("examples/archify-repo.architecture.json");
    let loaded = load_architecture(&std::fs::read(path).unwrap()).unwrap();
    let text = String::from_utf8(to_pretty_json(&loaded.doc)).unwrap();
    assert!(text.starts_with("{\n  \"schema_version\": 1,\n  \"diagram_type\""));
    assert!(text.ends_with("}\n"));
    // Positions that were integers stay integers.
    assert!(!text.contains(".0,") && !text.contains(".0\n"), "{text}");
}
