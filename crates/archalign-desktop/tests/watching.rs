//! M6 watcher handling in the app (FR-WAT-03..10). File changes are real; the batches go
//! through the real filter (`archalign_repo::classify`) via `Harness::watch`. The OS watcher
//! itself is tested in `archalign-repo/tests/watch.rs`.

mod common;

use std::time::Duration;

use archalign_desktop::app::{Boot, Message, WatchStatus};
use archalign_desktop::ids;
use archalign_repo::{RelPath, WatchBatch};
use common::Harness;
use common::fixtures::{TempRepo, lines, sample_doc, sample_repo};

const FOLDER: &str = "architecture-sample-20260101-120000";

fn opened(repo: &TempRepo) -> Harness {
    repo.diagram(FOLDER, &sample_doc("Sample"));
    let mut h = Harness::new(Boot::default());
    h.send(Message::OpenRepo(repo.root.clone()));
    assert_eq!(h.app.diagram.title().as_deref(), Some("Sample"));
    h
}

fn rel(s: &str) -> RelPath {
    s.parse().unwrap()
}

fn open_text(h: &Harness) -> String {
    match &h.app.source.content {
        archalign_desktop::source::SourceContent::Text(t) => t.text.to_string(),
        other => panic!("{other:?}"),
    }
}

fn names(h: &Harness, dir: &str) -> Vec<String> {
    h.app
        .tree
        .children
        .get(&rel(dir))
        .map(|v| v.iter().map(|e| e.name.clone()).collect())
        .unwrap_or_default()
}

#[test]
fn tree_relists_only_affected_expanded_folders_and_keeps_state() {
    let repo = sample_repo();
    repo.write("docs/a.md", "a");
    let mut h = opened(&repo);
    h.click(ids::tree_row(&rel("src")));
    h.click(ids::tree_row(&rel("src/api.rs")));
    assert!(h.app.tree.expanded.contains(&rel("src")));
    let n = h.log.len();
    repo.write("src/new.rs", "fn main() {}\n");
    repo.write("docs/b.md", "b");
    h.watch(&[repo.path("src/new.rs"), repo.path("docs/b.md")]);
    assert!(names(&h, "src").contains(&"new.rs".to_string()));
    let listed: Vec<_> = h.log[n..]
        .iter()
        .filter(|l| l.starts_with("TreeListed("))
        .collect();
    assert_eq!(
        listed.len(),
        1,
        "docs/ is collapsed, so only src/ is re-listed: {listed:?}"
    );
    assert!(h.app.tree.expanded.contains(&rel("src")), "expansion kept");
    assert_eq!(
        h.app.tree.selected,
        Some(rel("src/api.rs")),
        "selection kept"
    );
    assert!(h.has_text("new.rs"));
    // Deleting a file at the root re-lists the root.
    std::fs::remove_file(repo.path("Cargo.toml")).unwrap();
    h.watch(&[repo.path("Cargo.toml")]);
    assert!(!names(&h, "").contains(&"Cargo.toml".to_string()));
}

#[test]
fn the_open_file_reloads_keeping_scroll_and_band_and_reports_deletion() {
    let repo = sample_repo();
    let mut h = opened(&repo);
    h.send(Message::OpenFile(
        rel("src/api.rs"),
        Some(archalign_desktop::source::Cite {
            line: Some(60),
            end_line: Some(64),
        }),
    ));
    let offset = h.app.source.scroll.offset_y;
    assert!(offset > 0.0);
    assert_eq!(h.app.source.band, Some((60, 64)));
    repo.write(
        "src/api.rs",
        lines(150).replace("line 61\n", "changed 61\n"),
    );
    h.watch(&[repo.path("src/api.rs")]);
    assert_eq!(h.app.source.line_count(), Some(150));
    assert!(open_text(&h).contains("changed 61\n"));
    h.screenshot(&common::smoke_dir("m6").join("reloaded.png"));
    assert_eq!(h.app.source.band, Some((60, 64)));
    assert_eq!(h.app.source.scroll.offset_y, offset);
    std::fs::remove_file(repo.path("src/api.rs")).unwrap();
    h.watch(&[repo.path("src/api.rs")]);
    assert!(
        h.app
            .banners
            .iter()
            .any(|b| b.text == "File deleted: src/api.rs")
    );
    assert!(h.has_text("File deleted: src/api.rs"));
}

#[test]
fn citations_of_changed_files_are_rechecked() {
    let repo = sample_repo();
    let mut h = opened(&repo);
    let api = 0;
    assert!(!h.app.diagram.view().unwrap().badges[api].broken);
    assert!(!h.app.tree.markers.broken.contains("src/api.rs"));
    let camera = h.app.diagram.camera;
    h.app.diagram.selection = archalign_desktop::diagram::Selection::Node(1);
    repo.write("src/api.rs", lines(5));
    h.watch(&[repo.path("src/api.rs")]);
    assert!(
        h.app.diagram.view().unwrap().badges[api].broken,
        "lines 10-20 no longer exist"
    );
    assert!(h.app.tree.markers.broken.contains("src/api.rs"));
    assert_eq!(h.app.diagram.camera, camera, "the view is kept");
    assert_eq!(
        h.app.diagram.selection,
        archalign_desktop::diagram::Selection::Node(1)
    );
    repo.write("src/api.rs", lines(120));
    h.watch(&[repo.path("src/api.rs")]);
    assert!(!h.app.diagram.view().unwrap().badges[api].broken);
}

#[test]
fn diagram_files_refresh_history_and_reload_the_open_diagram() {
    let repo = sample_repo();
    let mut h = opened(&repo);
    // A new run folder from outside (e.g. the Archify CLI) shows up in History.
    repo.diagram("architecture-other-20270101-000000", &sample_doc("Other"));
    h.watch(&[repo.path(".archify/architecture-other-20270101-000000/candidate.json")]);
    let history = &h.app.repo.as_ref().unwrap().history;
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].title.as_deref(), Some("Other"));
    // An external edit to the open file reloads it.
    let open = repo.path(&format!(".archify/{FOLDER}/candidate.json"));
    repo.diagram(FOLDER, &sample_doc("Edited outside"));
    h.watch(std::slice::from_ref(&open));
    assert_eq!(h.app.diagram.title().as_deref(), Some("Edited outside"));
    // An invalid edit keeps the last good version and says so.
    std::fs::write(&open, b"{ not json").unwrap();
    h.watch(std::slice::from_ref(&open));
    assert_eq!(h.app.diagram.title().as_deref(), Some("Edited outside"));
    assert!(
        h.app
            .banners
            .iter()
            .any(|b| b.text.contains("changed on disk and is now invalid"))
    );
}

#[test]
fn the_apps_own_relayout_write_is_not_reloaded() {
    let repo = sample_repo();
    let mut h = opened(&repo);
    h.click(ids::RELAYOUT);
    let open = repo.path(&format!(".archify/{FOLDER}/candidate.json"));
    let n = h.log.len();
    h.watch(std::slice::from_ref(&open));
    assert!(
        h.log[n..]
            .iter()
            .all(|l| !l.starts_with("DiagramReloaded(")),
        "{:?}",
        &h.log[n..]
    );
}

#[test]
fn a_burst_triggers_one_full_refresh() {
    let repo = sample_repo();
    let mut h = opened(&repo);
    h.click(ids::tree_row(&rel("src")));
    repo.write("src/burst.rs", "x");
    let n = h.log.len();
    h.send(Message::Watch(h.app.generation, WatchBatch::Full));
    assert!(names(&h, "src").contains(&"burst.rs".to_string()));
    let log = &h.log[n..];
    assert!(log.iter().any(|l| l.starts_with("HistoryScanned(")));
    assert!(log.iter().any(|l| l.starts_with("GitCollected(")));
    assert!(log.iter().any(|l| l.starts_with("CitationsRechecked(")));
}

#[test]
fn git_refreshes_are_rate_limited_to_one_per_two_seconds() {
    let repo = sample_repo();
    repo.git_commit_all();
    let mut h = opened(&repo);
    h.patience = Duration::from_millis(100);
    let n = h.log.len();
    let count = |h: &Harness| {
        h.log[n..]
            .iter()
            .filter(|l| l.starts_with("GitCollected("))
            .count()
    };
    repo.write("src/api.rs", lines(121));
    h.watch(&[repo.path("src/api.rs")]);
    assert_eq!(count(&h), 1, "the first change refreshes at once");
    let dirty = |h: &Harness| h.app.repo.as_ref().unwrap().dirty().contains("src/api.rs");
    assert!(dirty(&h));
    assert!(
        h.app.diagram.view().unwrap().badges[0].modified,
        "dirty paths update the badges"
    );
    h.watch(&[repo.path(".git/index")]);
    h.watch(&[repo.path("src/api.rs")]);
    assert_eq!(count(&h), 1, "later changes within 2 s wait and coalesce");
    h.pump(Duration::from_millis(2500));
    assert_eq!(count(&h), 2, "then exactly one more");
}

#[test]
fn cmd_r_refreshes_everything_without_a_watcher() {
    let repo = sample_repo();
    let mut h = opened(&repo);
    assert_eq!(h.app.watch, WatchStatus::ManualOnly);
    assert!(h.has_text("Manual refresh only"));
    h.send(Message::OpenFile(rel("README.md"), None));
    repo.write("NEW.md", "n");
    repo.write("README.md", "# Changed\n");
    repo.diagram("architecture-more-20270101-000000", &sample_doc("More"));
    repo.write("src/api.rs", lines(3));
    h.cmd("r");
    assert!(names(&h, "").contains(&"NEW.md".to_string()));
    assert_eq!(open_text(&h), "# Changed\n");
    assert_eq!(h.app.repo.as_ref().unwrap().history.len(), 2);
    assert!(h.app.diagram.view().unwrap().badges[0].broken);
    // Watcher status follows the watcher messages for the current repo only.
    let g = h.app.generation;
    h.send(Message::WatchStarted(g + 1));
    assert_eq!(h.app.watch, WatchStatus::ManualOnly);
    h.send(Message::WatchStarted(g));
    assert!(h.has_text("Watching"));
    h.send(Message::WatchFailed(g, "no FSEvents".into()));
    assert!(h.has_text("Manual refresh only"));
}

#[test]
fn the_subscription_stream_starts_a_watcher_and_delivers_batches() {
    use iced::futures::StreamExt;
    let repo = sample_repo();
    let key = archalign_desktop::watch::WatchKey {
        generation: 7,
        sandbox: archalign_repo::Sandbox::new(&repo.root).unwrap(),
        writes: archalign_repo::SelfWrites::default(),
        show_hidden: false,
    };
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let mut s = Box::pin(archalign_desktop::watch::stream(&key));
    let next = |s: &mut std::pin::Pin<Box<_>>| {
        rt.block_on(async {
            tokio::time::timeout(Duration::from_secs(5), StreamExt::next(s)).await
        })
    };
    assert!(matches!(next(&mut s), Ok(Some(Message::WatchStarted(7)))));
    repo.write("src/live.rs", "x");
    match next(&mut s) {
        Ok(Some(Message::Watch(7, WatchBatch::Changes(c)))) => {
            assert!(c.paths.contains(&rel("src/live.rs")), "{c:?}")
        }
        other => panic!("{other:?}"),
    }
}
