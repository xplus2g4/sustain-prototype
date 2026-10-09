//! FR-LAY-04: every vendored fixture, with its geometry stripped, produces a reviewed
//! auto-layout geometry snapshot (fixed-advance measurement keeps it deterministic).

mod common;

use archalign_layout::{FixedAdvance, SceneMode, layout_document};
use common::{fixture_root, fixtures, load_fixture};

#[test]
fn auto_layout_geometry_snapshots() {
    let root = fixture_root();
    let paths = fixtures();
    assert_eq!(paths.len(), 24);
    for path in paths {
        let mut doc = load_fixture(&path);
        archalign_model::strip_geometry(&mut doc);
        let scene = layout_document(&doc, &FixedAdvance::default());
        assert_eq!(scene.mode, SceneMode::Auto);
        let name = path
            .strip_prefix(&root)
            .unwrap()
            .to_string_lossy()
            .trim_end_matches(".architecture.json")
            .replace('/', "__");
        insta::assert_json_snapshot!(name, scene);
    }
}
