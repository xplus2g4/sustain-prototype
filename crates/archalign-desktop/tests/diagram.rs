//! M4 diagram canvas and navigation through the UI harness (FR-DIA-01..09, 11; FR-TREE-06..07).

mod common;

use archalign_desktop::app::{Boot, Message};
use archalign_desktop::diagram::Selection;
use archalign_desktop::diagram::logic::Direction;
use archalign_desktop::ids;
use archalign_desktop::source::SourceContent;
use archalign_repo::RelPath;
use common::Harness;
use common::fixtures::{TempRepo, lines, sample_doc, sample_repo};
use iced::keyboard::{Key, Modifiers, key::Named};
use iced::{Point, mouse};
use serde_json::json;

const FOLDER: &str = "architecture-s-20261010-101010";

fn rel(s: &str) -> RelPath {
    s.parse().unwrap()
}

fn opened_with(repo: &TempRepo, doc: &serde_json::Value) -> Harness {
    repo.diagram(FOLDER, doc);
    let mut h = Harness::new(Boot::default());
    h.send(Message::OpenRepo(repo.root.clone()));
    assert!(h.app.diagram.loaded().is_some(), "{:?}", h.app.banners);
    // Let the canvas report its real size (as the first frame does).
    let c = h.bounds(ids::DIAGRAM_CANVAS).unwrap();
    h.move_to(Point::new(c.x + 1.0, c.y + 1.0));
    h
}

/// Window coordinates of a component's center.
fn node_point(h: &mut Harness, id: &str) -> Point {
    let canvas = h.bounds(ids::DIAGRAM_CANVAS).unwrap();
    let d = h.app.diagram.loaded().unwrap();
    let n = d.scene.node(id).unwrap();
    let c = n.rect.center();
    let p = h
        .app
        .diagram
        .camera
        .to_canvas(Point::new(c.x as f32, c.y as f32));
    Point::new(canvas.x + p.x, canvas.y + p.y)
}

fn click_node(h: &mut Harness, id: &str) {
    let p = node_point(h, id);
    h.click_at(p);
}

fn press(h: &mut Harness, c: &str) {
    h.key(Key::Character(c.into()), Modifiers::default());
}

// ---------- FR-DIA-01 (rendering, reviewed from screenshots) ----------

#[test]
fn fixture_screenshots_for_visual_review() {
    for (name, path) in [
        ("web-app", "skill-examples/web-app.architecture.json"),
        (
            "production-deployment",
            "skill-examples/production-deployment.architecture.json",
        ),
        (
            "archify-repo-grid",
            "examples/archify-repo-grid.architecture.json",
        ),
        (
            "archify-runtime",
            "skill-run/archify-runtime.architecture.json",
        ),
    ] {
        let repo = TempRepo::new();
        let bytes = std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../fixtures/archify")
                .join(path),
        )
        .unwrap();
        repo.write(&format!(".archify/{FOLDER}/candidate.json"), bytes);
        let mut h = Harness::new(Boot::default());
        h.send(Message::OpenRepo(repo.root.clone()));
        assert!(
            h.app.diagram.loaded().is_some(),
            "{name}: {:?}",
            h.app.banners
        );
        h.click(ids::CARDS_TOGGLE);
        h.screenshot(&common::smoke_dir("m4").join(format!("{name}.png")));
    }
}

// ---------- FR-DIA-02 ----------

#[test]
fn opening_fits_the_diagram_with_padding() {
    let repo = sample_repo();
    let h = opened_with(&repo, &sample_doc("S"));
    let size = h.app.diagram.canvas_size.expect("canvas reported its size");
    let d = h.app.diagram.loaded().unwrap().clone();
    for n in &d.scene.nodes {
        let tl = h
            .app
            .diagram
            .camera
            .to_canvas(Point::new(n.rect.x as f32, n.rect.y as f32));
        let br = h
            .app
            .diagram
            .camera
            .to_canvas(Point::new(n.rect.right() as f32, n.rect.bottom() as f32));
        assert!(tl.x >= 39.0 && tl.y >= 39.0, "{tl:?}");
        assert!(
            br.x <= size.width - 39.0 && br.y <= size.height - 39.0,
            "{br:?} in {size:?}"
        );
    }
}

#[test]
fn zoom_and_pan_inputs_work_end_to_end() {
    let repo = sample_repo();
    let mut h = opened_with(&repo, &sample_doc("S"));
    let canvas = h.bounds(ids::DIAGRAM_CANVAS).unwrap();
    // Keys only act while the canvas has focus.
    let z0 = h.app.diagram.camera.zoom;
    press(&mut h, "+");
    assert_eq!(h.app.diagram.camera.zoom, z0, "no focus yet");
    // Focus by clicking empty canvas.
    h.click_at(Point::new(canvas.x + 5.0, canvas.y + 5.0));
    assert!(h.app.diagram.focused);
    press(&mut h, "+");
    assert!((h.app.diagram.camera.zoom - z0 * 1.25).abs() < 1e-4);
    press(&mut h, "-");
    assert!((h.app.diagram.camera.zoom - z0).abs() < 1e-4);
    press(&mut h, "0");
    assert!((h.app.diagram.camera.zoom - 1.0).abs() < 1e-4);
    // ⌘+scroll zooms around the cursor: the diagram point under it stays put.
    let at = Point::new(canvas.x + 300.0, canvas.y + 200.0);
    h.move_to(at);
    let local = Point::new(300.0, 200.0);
    let under = h.app.diagram.camera.to_diagram(local);
    h.simulate(vec![iced::Event::Keyboard(
        iced::keyboard::Event::ModifiersChanged(Modifiers::COMMAND),
    )]);
    h.wheel(120.0);
    h.simulate(vec![iced::Event::Keyboard(
        iced::keyboard::Event::ModifiersChanged(Modifiers::default()),
    )]);
    assert!(h.app.diagram.camera.zoom > 1.0);
    let back = h.app.diagram.camera.to_canvas(under);
    assert!((back.x - local.x).abs() < 0.5 && (back.y - local.y).abs() < 0.5);
    // Plain scroll pans.
    let pan = h.app.diagram.camera.pan;
    h.wheel(-50.0);
    assert!((h.app.diagram.camera.pan.y - (pan.y - 50.0)).abs() < 1e-3);
    // Dragging empty canvas pans.
    let pan = h.app.diagram.camera.pan;
    let start = Point::new(canvas.x + 8.0, canvas.y + canvas.height - 8.0);
    h.move_to(start);
    h.simulate(vec![iced::Event::Mouse(mouse::Event::ButtonPressed(
        mouse::Button::Left,
    ))]);
    h.move_to(Point::new(start.x + 30.0, start.y - 20.0));
    h.simulate(vec![iced::Event::Mouse(mouse::Event::ButtonReleased(
        mouse::Button::Left,
    ))]);
    assert!((h.app.diagram.camera.pan.x - (pan.x + 30.0)).abs() < 1e-3);
    assert!((h.app.diagram.camera.pan.y - (pan.y - 20.0)).abs() < 1e-3);
    // F fits again.
    press(&mut h, "f");
    assert!(h.app.diagram.fitted);
    assert!((h.app.diagram.camera.zoom - z0).abs() < 1e-4);
}

// ---------- FR-DIA-03 ----------

#[test]
fn clicking_a_node_opens_its_first_citation_and_reveals_the_file() {
    let repo = sample_repo();
    let mut h = opened_with(&repo, &sample_doc("S"));
    click_node(&mut h, "db");
    assert_eq!(h.app.diagram.selection, Selection::Node(1));
    assert_eq!(h.app.source.rel(), Some(&rel("src/db/store.rs")));
    assert_eq!(h.app.source.band, Some((3, 3)));
    assert!(
        h.app.tree.expanded.contains(&rel("src")) && h.app.tree.expanded.contains(&rel("src/db"))
    );
    assert_eq!(h.app.tree.selected, Some(rel("src/db/store.rs")));
    assert!(h.has(ids::tree_row(&rel("src/db/store.rs"))));
    // The strip lists every source; clicking one opens it.
    assert!(h.has_text("src/db/store.rs:3"));
    click_node(&mut h, "api");
    assert!(h.has_text("src/api.rs:10–20"));
    assert_eq!(h.app.source.band, Some((10, 20)));
    h.screenshot(&common::smoke_dir("m4").join("node-click.png"));
}

#[test]
fn a_node_without_sources_is_only_selected() {
    let repo = sample_repo();
    let mut doc = sample_doc("S");
    doc["components"][2]
        .as_object_mut()
        .unwrap()
        .remove("sources");
    let mut h = opened_with(&repo, &doc);
    click_node(&mut h, "ui");
    assert_eq!(h.app.diagram.selection, Selection::Node(2));
    assert!(matches!(h.app.source.content, SourceContent::Empty));
    assert!(h.has(ids::DETAIL_NO_SOURCE));
    assert!(h.has_text("No source"));
}

#[test]
fn strip_sources_open_their_citation() {
    let repo = sample_repo();
    let mut doc = sample_doc("S");
    doc["components"][0]["sources"] = json!([
        { "path": "src/api.rs", "line": 10, "end_line": 20 },
        { "path": "README.md", "line": 2 }
    ]);
    let mut h = opened_with(&repo, &doc);
    click_node(&mut h, "api");
    h.click(ids::detail_source(1));
    assert_eq!(h.app.source.rel(), Some(&rel("README.md")));
    assert_eq!(h.app.source.band, Some((2, 2)));
}

// ---------- FR-DIA-04 ----------

#[test]
fn hovering_a_node_shows_a_tooltip_after_400ms() {
    let repo = sample_repo();
    let mut doc = sample_doc("S");
    doc["components"][0]["sublabel"] = json!("REST gateway");
    doc["components"][0]["tag"] = json!("v2");
    let mut h = opened_with(&repo, &doc);
    let p = node_point(&mut h, "api");
    let t = std::time::Instant::now();
    h.move_to(p);
    assert!(
        t.elapsed() >= std::time::Duration::from_millis(400),
        "the delay task waited"
    );
    assert!(h.has(ids::DIAGRAM_TOOLTIP));
    for line in ["API", "REST gateway", "[v2]", "src/api.rs:10–20"] {
        assert!(h.has_text(line), "{line}");
    }
    h.screenshot(&common::smoke_dir("m4").join("tooltip.png"));
    // Moving off the node hides it.
    let canvas = h.bounds(ids::DIAGRAM_CANVAS).unwrap();
    h.move_to(Point::new(canvas.x + 2.0, canvas.y + canvas.height - 2.0));
    assert!(!h.has(ids::DIAGRAM_TOOLTIP));
}

// ---------- FR-DIA-05 ----------

#[test]
fn reach_keys_and_buttons_toggle_and_replace() {
    let repo = sample_repo();
    let mut h = opened_with(&repo, &sample_doc("S"));
    click_node(&mut h, "api");
    press(&mut h, "u");
    let (dir, set) = h.app.diagram.reach.clone().unwrap();
    assert_eq!(dir, Direction::Upstream);
    assert_eq!(set.nodes.iter().copied().collect::<Vec<_>>(), [0, 2]);
    press(&mut h, "d");
    let (dir, set) = h.app.diagram.reach.clone().unwrap();
    assert_eq!(dir, Direction::Downstream, "the other key replaces");
    assert_eq!(set.nodes.iter().copied().collect::<Vec<_>>(), [0, 1]);
    h.screenshot(&common::smoke_dir("m4").join("reach.png"));
    press(&mut h, "d");
    assert!(h.app.diagram.reach.is_none(), "the same key clears");
    h.click(ids::REACH_UP);
    assert_eq!(
        h.app.diagram.reach.as_ref().map(|r| r.0),
        Some(Direction::Upstream)
    );
    h.click(ids::REACH_UP);
    assert!(h.app.diagram.reach.is_none());
}

// ---------- FR-DIA-06 ----------

#[test]
fn clicking_a_connection_selects_it() {
    let repo = sample_repo();
    let mut h = opened_with(&repo, &sample_doc("S"));
    let canvas = h.bounds(ids::DIAGRAM_CANVAS).unwrap();
    let d = h.app.diagram.loaded().unwrap().clone();
    let e = d.scene.edges.iter().find(|e| e.index == 0).unwrap();
    // A point in the middle of the route's longest segment.
    let a = e.route.label_anchor;
    let p = h
        .app
        .diagram
        .camera
        .to_canvas(Point::new(a.x as f32, a.y as f32));
    h.click_at(Point::new(canvas.x + p.x, canvas.y + p.y));
    assert_eq!(h.app.diagram.selection, Selection::Edge(0));
    assert!(h.has(ids::DETAIL_EDGE));
    assert!(h.has_text("ui → api · calls · default"));
    h.screenshot(&common::smoke_dir("m4").join("edge-click.png"));
}

// ---------- FR-DIA-07 / FR-DIA-08 ----------

#[test]
fn finder_searches_moves_and_picks() {
    let repo = sample_repo();
    let mut h = opened_with(&repo, &sample_doc("S"));
    let canvas = h.bounds(ids::DIAGRAM_CANVAS).unwrap();
    h.click_at(Point::new(canvas.x + 5.0, canvas.y + 5.0));
    h.cmd("f");
    assert!(h.app.diagram.finder.is_some());
    assert!(h.has(ids::FINDER_INPUT));
    h.type_text("i");
    let f = h.app.diagram.finder.clone().unwrap();
    assert_eq!(f.results, vec![0, 2], "API and UI match 'i' (label or id)");
    h.key(Key::Named(Named::ArrowDown), Modifiers::default());
    assert_eq!(h.app.diagram.finder.as_ref().unwrap().cursor, 1);
    h.key(Key::Named(Named::ArrowUp), Modifiers::default());
    h.key(Key::Named(Named::ArrowDown), Modifiers::default());
    h.key(Key::Named(Named::Enter), Modifiers::default());
    assert!(h.app.diagram.finder.is_none());
    assert_eq!(
        h.app.diagram.selection,
        Selection::Node(2),
        "acts like a click"
    );
    assert_eq!(h.app.source.rel(), Some(&rel("README.md")));
    // Centered.
    let size = h.app.diagram.canvas_size.unwrap();
    let c = h.app.diagram.loaded().unwrap().scene.nodes[2].rect.center();
    let at = h
        .app
        .diagram
        .camera
        .to_canvas(Point::new(c.x as f32, c.y as f32));
    assert!((at.x - size.width / 2.0).abs() < 1.0 && (at.y - size.height / 2.0).abs() < 1.0);
    // `/` also opens it; Esc closes the finder first, then clears.
    h.click_at(Point::new(canvas.x + 5.0, canvas.y + 5.0));
    press(&mut h, "/");
    assert!(h.app.diagram.finder.is_some());
    h.key(Key::Named(Named::Escape), Modifiers::default());
    assert!(h.app.diagram.finder.is_none());
    assert_eq!(
        h.app.diagram.selection,
        Selection::Node(2),
        "first Esc only closes the finder"
    );
}

#[test]
fn escape_clears_reach_selection_and_reverse_highlight() {
    let repo = sample_repo();
    let mut h = opened_with(&repo, &sample_doc("S"));
    h.send(Message::TreeActivate(rel("README.md")));
    assert_eq!(
        h.app.diagram.reverse.iter().copied().collect::<Vec<_>>(),
        [2]
    );
    click_node(&mut h, "api");
    press(&mut h, "d");
    assert!(h.app.diagram.reach.is_some());
    h.key(Key::Named(Named::Escape), Modifiers::default());
    assert!(h.app.diagram.reach.is_none());
    assert_eq!(h.app.diagram.selection, Selection::None);
    assert!(h.app.diagram.reverse.is_empty());
}

// ---------- FR-DIA-09 ----------

#[test]
fn problems_list_and_badges() {
    let repo = sample_repo();
    repo.git_commit_all();
    repo.write("README.md", "# changed\n");
    let mut doc = sample_doc("S");
    doc["components"][1]["sources"][0]["line"] = json!(999);
    doc["components"].as_array_mut().unwrap().push(json!(
        { "id": "orphan", "type": "external", "label": "Orphan", "pos": [400, 260], "size": [160, 70] }
    ));
    let mut h = opened_with(&repo, &doc);
    let v = h.app.diagram.view().unwrap().clone();
    assert!(v.badges[1].broken, "db cites line 999 of a 40-line file");
    assert!(v.badges[2].modified, "ui cites the modified README");
    assert!(!v.badges[0].broken && !v.badges[0].modified);
    let n = v.d.warnings.len();
    assert!(h.has_text(&format!("Problems ({n})")));
    h.click(ids::PROBLEMS_TOGGLE);
    assert!(
        h.find(|c: iced_test::selector::Candidate<'_>| match c {
            iced_test::selector::Candidate::Text { content, .. }
                if content.contains("has no sources") =>
                Some(()),
            _ => None,
        })
        .is_some()
    );
    h.screenshot(&common::smoke_dir("m4").join("problems.png"));
}

// ---------- FR-DIA-11 / FR-LAY-08 ----------

#[test]
fn opening_never_writes_and_relayout_writes_integer_geometry_only() {
    let repo = sample_repo();
    let mut doc = sample_doc("Topology only");
    for c in doc["components"].as_array_mut().unwrap() {
        let c = c.as_object_mut().unwrap();
        c.remove("pos");
        c.remove("size");
    }
    let path = repo.path(&format!(".archify/{FOLDER}/candidate.json"));
    let mut h = opened_with(&repo, &doc);
    let before = std::fs::read(&path).unwrap();
    assert_eq!(
        h.app.diagram.loaded().unwrap().scene.mode,
        archalign_layout::SceneMode::Auto
    );
    assert_eq!(
        std::fs::read(&path).unwrap(),
        before,
        "opening never writes (FR-LAY-08)"
    );
    h.click(ids::RELAYOUT);
    let after: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    for c in after["components"].as_array().unwrap() {
        for k in ["pos", "size"] {
            assert!(
                c[k].as_array()
                    .unwrap()
                    .iter()
                    .all(|n| n.is_i64() || n.is_u64()),
                "{c}"
            );
        }
    }
    let mut stripped = after.clone();
    for c in stripped["components"].as_array_mut().unwrap() {
        let c = c.as_object_mut().unwrap();
        c.remove("pos");
        c.remove("size");
    }
    let original: serde_json::Value = serde_json::from_slice(&before).unwrap();
    assert_eq!(stripped, original, "non-geometry fields unchanged");
    assert_eq!(
        h.app.diagram.loaded().unwrap().scene.mode,
        archalign_layout::SceneMode::Authored
    );
    // ⌘⇧L does the same.
    let n = h.log.len();
    h.cmd_shift("l");
    assert!(h.log[n..].contains(&"Action(Relayout)".to_string()));
}

// ---------- FR-TREE-06 / FR-TREE-07 ----------

#[test]
fn tree_markers_show_cited_files_folder_counts_and_broken_files() {
    let repo = sample_repo();
    let mut doc = sample_doc("S");
    doc["components"][1]["sources"][0]["line"] = json!(999);
    let mut h = opened_with(&repo, &doc);
    let m = h.app.tree.markers.clone();
    assert!(m.cited.contains("README.md") && m.cited.contains("src/api.rs"));
    assert_eq!(m.folder_counts.get("src"), Some(&2));
    assert_eq!(m.folder_counts.get("src/db"), Some(&1));
    assert!(m.broken.contains("src/db/store.rs"));
    assert!(h.has_text("●"), "dot on a cited file");
    assert!(h.has_text("2"), "count on src");
    h.send(Message::TreeToggle(rel("src")));
    h.send(Message::TreeToggle(rel("src/db")));
    assert!(h.has_text("⚠"), "warning marker on the broken file");
    h.screenshot(&common::smoke_dir("m4").join("tree-markers.png"));
}

#[test]
fn selecting_a_file_highlights_the_nodes_citing_it() {
    let repo = sample_repo();
    let mut doc = sample_doc("S");
    doc["components"][1]["sources"] = json!([{ "path": "src/api.rs", "line": 1 }]);
    let mut h = opened_with(&repo, &doc);
    h.click(ids::tree_row(&rel("src")));
    h.click(ids::tree_row(&rel("src/api.rs")));
    assert_eq!(
        h.app.diagram.reverse.iter().copied().collect::<Vec<_>>(),
        [0, 1]
    );
    h.screenshot(&common::smoke_dir("m4").join("reverse-highlight.png"));
    h.click(ids::tree_row(&rel("Cargo.toml")));
    assert!(h.app.diagram.reverse.is_empty(), "another file clears it");
    let _ = lines(1);
}
