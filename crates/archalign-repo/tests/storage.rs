//! FR-STO-04 (history), FR-IMP-03 (newest valid diagram) and FR-STO-02 (atomic writes).

use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;

use archalign_repo::storage::folder_timestamp;
use archalign_repo::{HistoryStatus, RelPath, Sandbox, atomic_write, newest_valid, scan_history};
use serde_json::json;

fn valid(title: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schema_version": 1,
        "diagram_type": "architecture",
        "meta": { "title": title, "output": ".archify/x/x.html" },
        "components": [
            { "id": "a", "type": "backend", "label": "A", "sources": [{ "path": "a.rs" }] }
        ]
    }))
    .unwrap()
}

fn put(root: &Path, folder: &str, bytes: &[u8]) {
    let dir = root.join(".archify").join(folder);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("candidate.json"), bytes).unwrap();
}

fn setup() -> (tempfile::TempDir, Sandbox) {
    let tmp = tempfile::tempdir().unwrap();
    fs::create_dir(tmp.path().join("repo")).unwrap();
    let sandbox = Sandbox::new(tmp.path().join("repo")).unwrap();
    (tmp, sandbox)
}

#[test]
fn timestamps_parse_from_folder_suffix() {
    assert_eq!(
        folder_timestamp("architecture-api-20261010-093000").as_deref(),
        Some("20261010-093000")
    );
    assert_eq!(
        folder_timestamp("architecture-api-20261010-093000-2").as_deref(),
        Some("20261010-093000")
    );
    assert_eq!(folder_timestamp("hand-made"), None);
    assert_eq!(folder_timestamp("x-2026101-093000"), None);
    assert_eq!(folder_timestamp(""), None);
}

#[test]
fn history_is_newest_first_and_hides_folders_without_candidate() {
    let (_tmp, sandbox) = setup();
    let root = sandbox.root();
    put(root, "architecture-old-20250101-000000", &valid("Old"));
    put(root, "architecture-new-20261010-120000", &valid("New"));
    put(root, "architecture-new-20261010-120000-2", &valid("New 2"));
    put(root, "architecture-mid-20260601-080000", &valid("Mid"));
    put(root, "hand-made", &valid("Hand"));
    fs::create_dir_all(root.join(".archify/architecture-failed-20270101-000000")).unwrap();
    fs::write(
        root.join(".archify/architecture-failed-20270101-000000/run.jsonl"),
        "{}\n",
    )
    .unwrap();
    fs::write(root.join(".archify/stray.json"), valid("Stray")).unwrap();

    let entries = scan_history(&sandbox);
    let folders: Vec<&str> = entries.iter().map(|e| e.folder.as_str()).collect();
    assert_eq!(
        folders,
        [
            "architecture-new-20261010-120000-2",
            "architecture-new-20261010-120000",
            "architecture-mid-20260601-080000",
            "architecture-old-20250101-000000",
            "hand-made",
        ]
    );
    let first = &entries[0];
    assert_eq!(
        first.rel.as_str(),
        ".archify/architecture-new-20261010-120000-2/candidate.json"
    );
    assert_eq!(first.timestamp.as_deref(), Some("20261010-120000"));
    assert_eq!(first.title.as_deref(), Some("New 2"));
    assert!(first.modified.is_some());
    assert_eq!(entries[4].timestamp, None);
}

#[test]
fn status_detects_valid_invalid_and_unsupported() {
    let (_tmp, sandbox) = setup();
    let root = sandbox.root();
    put(root, "a-20260101-000003", b"{ not json");
    put(
        root,
        "a-20260101-000002",
        &serde_json::to_vec(&json!({
            "schema_version": 1,
            "diagram_type": "workflow",
            "meta": { "title": "Flow" }
        }))
        .unwrap(),
    );
    let mut broken: serde_json::Value = serde_json::from_slice(&valid("Broken")).unwrap();
    broken["components"][0]["type"] = json!("mainframe");
    put(
        root,
        "a-20260101-000001",
        &serde_json::to_vec(&broken).unwrap(),
    );
    put(root, "a-20260101-000000", &valid("Good"));

    let entries = scan_history(&sandbox);
    assert_eq!(entries.len(), 4);
    assert!(
        matches!(&entries[0].status, HistoryStatus::Invalid { first_error } if first_error.contains("Malformed JSON")),
        "{:?}",
        entries[0].status
    );
    assert_eq!(entries[0].title, None);
    assert_eq!(
        entries[1].status,
        HistoryStatus::Unsupported {
            diagram_type: "workflow".into()
        }
    );
    assert_eq!(entries[1].title.as_deref(), Some("Flow"));
    assert!(
        matches!(&entries[2].status, HistoryStatus::Invalid { first_error } if first_error.starts_with("/components/0/type")),
        "{:?}",
        entries[2].status
    );
    assert_eq!(entries[2].title.as_deref(), Some("Broken"));
    assert_eq!(entries[3].status, HistoryStatus::Valid);

    let newest = newest_valid(&entries).unwrap();
    assert_eq!(newest.title.as_deref(), Some("Good"));
}

#[test]
fn newer_valid_diagram_opens_and_none_without_archify() {
    let (_tmp, sandbox) = setup();
    assert!(scan_history(&sandbox).is_empty());
    assert!(newest_valid(&[]).is_none());
    put(
        sandbox.root(),
        "architecture-x-20260101-000000",
        &valid("Older"),
    );
    put(
        sandbox.root(),
        "architecture-x-20260202-000000",
        &valid("Newer"),
    );
    let entries = scan_history(&sandbox);
    assert_eq!(
        newest_valid(&entries).unwrap().title.as_deref(),
        Some("Newer")
    );
}

#[test]
fn history_ignores_gitignore_and_skips_escaping_links() {
    let (tmp, sandbox) = setup();
    let root = sandbox.root();
    fs::write(root.join(".gitignore"), ".archify/\n").unwrap();
    put(root, "kept-20260101-000000", &valid("Kept"));
    let outside = tmp.path().join("outside");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("candidate.json"), valid("Outside")).unwrap();
    symlink(&outside, root.join(".archify/linked-20990101-000000")).unwrap();
    let entries = scan_history(&sandbox);
    let folders: Vec<&str> = entries.iter().map(|e| e.folder.as_str()).collect();
    assert_eq!(folders, ["kept-20260101-000000"]);
}

#[test]
fn atomic_write_replaces_content_and_leaves_no_temp_files() {
    let (_tmp, sandbox) = setup();
    let dir = sandbox.root().join(".archify/run");
    fs::create_dir_all(&dir).unwrap();
    let rel = RelPath::parse(".archify/run/candidate.json").unwrap();
    atomic_write(&sandbox, &rel, b"first").unwrap();
    atomic_write(&sandbox, &rel, b"second").unwrap();
    assert_eq!(fs::read(dir.join("candidate.json")).unwrap(), b"second");
    let names: Vec<String> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(names, ["candidate.json"]);
}

#[test]
fn atomic_write_requires_parent_inside_root() {
    let (tmp, sandbox) = setup();
    assert!(atomic_write(&sandbox, &RelPath::parse("missing/x.json").unwrap(), b"x").is_err());
    assert!(atomic_write(&sandbox, &RelPath::root(), b"x").is_err());
    let outside = tmp.path().join("outside");
    fs::create_dir_all(&outside).unwrap();
    symlink(&outside, sandbox.root().join("out")).unwrap();
    assert!(atomic_write(&sandbox, &RelPath::parse("out/x.json").unwrap(), b"x").is_err());
    assert!(!outside.join("x.json").exists());
}
