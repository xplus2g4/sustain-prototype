//! FR-VAL-06, FR-VAL-07 and FR-SRC-08 end to end: `check_citations` over `RepoProbe`.

use std::collections::HashSet;
use std::fs;
use std::os::unix::fs::symlink;

use archalign_model::{
    ArchitectureDoc, FileProbe, IssueCode, Profile, Severity, SourceProbe, check_citations,
};
use archalign_repo::{MAX_VIEW_BYTES, RepoProbe, Sandbox};
use serde_json::json;

fn doc() -> ArchitectureDoc {
    serde_json::from_value(json!({
        "schema_version": 1,
        "diagram_type": "architecture",
        "meta": { "title": "T", "output": "out/t.html" },
        "components": [
            { "id": "ok", "type": "backend", "label": "OK", "sources": [
                { "path": "src/a.rs", "line": 2, "end_line": 3 },
                { "path": "src/dirty.rs" }
            ] },
            { "id": "bad", "type": "backend", "label": "Bad", "sources": [
                { "path": "src/missing.rs" },
                { "path": "src/a.rs", "line": 10, "end_line": 40 },
                { "path": "logo.png" }
            ] },
            { "id": "esc", "type": "backend", "label": "Esc", "sources": [
                { "path": "leak.txt" },
                { "path": "big.txt" },
                { "path": "src" }
            ] }
        ]
    }))
    .unwrap()
}

#[test]
fn repo_probe_end_to_end() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("repo");
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/a.rs"), "1\n2\n3\n4\n5\n").unwrap();
    fs::write(root.join("src/dirty.rs"), "x\n").unwrap();
    fs::write(root.join("logo.png"), b"\x89PNG\0\0\0").unwrap();
    fs::write(
        root.join("big.txt"),
        vec![b'a'; MAX_VIEW_BYTES as usize + 1],
    )
    .unwrap();
    fs::write(tmp.path().join("secret.txt"), "secret\n").unwrap();
    symlink(tmp.path().join("secret.txt"), root.join("leak.txt")).unwrap();
    let sandbox = Sandbox::new(&root).unwrap();
    let probe = RepoProbe::new(&sandbox);

    assert_eq!(probe.probe("src/a.rs"), FileProbe::Text { line_count: 5 });
    assert_eq!(probe.probe("src/missing.rs"), FileProbe::Missing);
    assert_eq!(probe.probe("logo.png"), FileProbe::Binary);
    assert_eq!(probe.probe("src"), FileProbe::NotAFile);
    assert_eq!(probe.probe("leak.txt"), FileProbe::OutsideRoot);
    assert_eq!(
        probe.probe("big.txt"),
        FileProbe::TooLarge {
            size: MAX_VIEW_BYTES + 1
        }
    );
    assert!(!probe.is_denied(".env"));

    let dirty: HashSet<String> = ["src/dirty.rs".to_string()].into();
    let issues = check_citations(&doc(), &probe, Profile::Submission, &dirty);
    let got: Vec<(&str, IssueCode, Severity)> = issues
        .iter()
        .map(|i| (i.pointer.as_str(), i.code, i.severity))
        .collect();
    assert_eq!(
        got,
        [
            (
                "/components/0/sources/1/path",
                IssueCode::ModifiedCitation,
                Severity::Warning
            ),
            (
                "/components/1/sources/0/path",
                IssueCode::BrokenCitation,
                Severity::Error
            ),
            (
                "/components/1/sources/1/line",
                IssueCode::BrokenCitation,
                Severity::Error
            ),
            (
                "/components/1/sources/2/path",
                IssueCode::BrokenCitation,
                Severity::Error
            ),
            (
                "/components/2/sources/0/path",
                IssueCode::InvalidCitationPath,
                Severity::Error
            ),
            (
                "/components/2/sources/1/path",
                IssueCode::BrokenCitation,
                Severity::Error
            ),
            (
                "/components/2/sources/2/path",
                IssueCode::BrokenCitation,
                Severity::Error
            ),
        ]
    );
    assert_eq!(
        issues[2].message,
        "Cited lines 10–40 are outside src/a.rs (5 lines)"
    );
    assert_eq!(issues[1].message, "src/missing.rs does not exist");
}

#[test]
fn citation_index_maps_files_folders_and_broken_markers() {
    use archalign_model::{Issue, IssueCode};
    use archalign_repo::citations::CitationIndex;
    let doc: archalign_model::ArchitectureDoc = serde_json::from_value(serde_json::json!({
        "schema_version": 1, "diagram_type": "architecture",
        "meta": { "title": "t", "output": "t.html" },
        "components": [
            { "id": "a", "type": "backend", "label": "A",
              "sources": [{ "path": "src/a/x.rs" }, { "path": "src/a/x.rs", "line": 3 }] },
            { "id": "b", "type": "backend", "label": "B",
              "sources": [{ "path": "src/a/x.rs" }, { "path": "src/b.rs", "line": 900 }] },
            { "id": "c", "type": "backend", "label": "C", "sources": [{ "path": "../escape.rs" }] },
            { "id": "d", "type": "backend", "label": "D" }
        ]
    }))
    .unwrap();
    let issues = vec![Issue::warning(
        IssueCode::BrokenCitation,
        "/components/1/sources/1/line",
        "out of range",
    )];
    let idx = CitationIndex::build(&doc, &issues);
    assert_eq!(idx.nodes_citing("src/a/x.rs"), &[0, 1]);
    assert_eq!(idx.nodes_citing("src/b.rs"), &[1]);
    assert!(
        idx.nodes_citing("../escape.rs").is_empty(),
        "invalid paths are not indexed"
    );
    assert_eq!(
        idx.folder_counts.get("src"),
        Some(&2),
        "distinct files below src"
    );
    assert_eq!(idx.folder_counts.get("src/a"), Some(&1));
    assert_eq!(
        idx.broken_files.iter().cloned().collect::<Vec<_>>(),
        ["src/b.rs"]
    );
}
