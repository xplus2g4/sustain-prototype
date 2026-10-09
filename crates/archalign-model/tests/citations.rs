//! FR-VAL-06 and FR-VAL-07 with an in-memory `SourceProbe`.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use archalign_model::{
    ArchitectureDoc, FileProbe, Issue, IssueCode, Profile, Severity, SourceProbe, check_citations,
    citation_path_ok,
};
use serde_json::{Value, json};

#[derive(Default)]
struct FakeProbe {
    files: HashMap<String, FileProbe>,
    denied: HashSet<String>,
    calls: RefCell<Vec<String>>,
}

impl FakeProbe {
    fn with(files: &[(&str, FileProbe)]) -> Self {
        Self {
            files: files.iter().map(|(p, f)| (p.to_string(), *f)).collect(),
            ..Self::default()
        }
    }
}

impl SourceProbe for FakeProbe {
    fn probe(&self, path: &str) -> FileProbe {
        self.calls.borrow_mut().push(path.to_string());
        self.files.get(path).copied().unwrap_or(FileProbe::Missing)
    }

    fn is_denied(&self, path: &str) -> bool {
        self.denied.contains(path)
    }
}

fn doc(sources: Value) -> ArchitectureDoc {
    serde_json::from_value(json!({
        "schema_version": 1,
        "diagram_type": "architecture",
        "meta": { "title": "T", "output": "out/t.html" },
        "components": [
            { "id": "a", "type": "frontend", "label": "A", "sources": sources }
        ]
    }))
    .unwrap()
}

fn text(n: u64) -> FileProbe {
    FileProbe::Text { line_count: n }
}

fn run(sources: Value, probe: &FakeProbe, profile: Profile) -> Vec<Issue> {
    check_citations(&doc(sources), probe, profile, &HashSet::new())
}

fn one(issues: &[Issue]) -> &Issue {
    assert_eq!(issues.len(), 1, "{issues:#?}");
    &issues[0]
}

// ---------- FR-VAL-06: path syntax ----------

#[test]
fn path_syntax_accepts_plain_relative_paths() {
    for ok in [
        "a",
        "src/a.rs",
        "a/b/c.d",
        ".env.example",
        "dir/.hidden",
        "a..b/c",
    ] {
        assert!(citation_path_ok(ok), "{ok}");
    }
}

#[test]
fn path_syntax_rejects_absolute_backslash_dot_segments_and_nul() {
    for bad in [
        "",
        "/etc/passwd",
        "C:/x",
        "c:\\x",
        "src\\a.rs",
        "../a",
        "a/../b",
        "a/..",
        "./a",
        "a/./b",
        "a//b",
        "a/",
        "a\0b",
    ] {
        assert!(!citation_path_ok(bad), "{bad:?}");
    }
}

#[test]
fn valid_citation_has_no_issues() {
    let probe = FakeProbe::with(&[("src/a.rs", text(25))]);
    let issues = run(
        json!([{ "path": "src/a.rs", "line": 10, "end_line": 25 }]),
        &probe,
        Profile::Submission,
    );
    assert!(issues.is_empty(), "{issues:#?}");
}

#[test]
fn malformed_path_is_invalid_citation_path() {
    let probe = FakeProbe::default();
    let issues = run(
        json!([{ "path": "../secret.rs" }]),
        &probe,
        Profile::Submission,
    );
    let issue = one(&issues);
    assert_eq!(issue.code, IssueCode::InvalidCitationPath);
    assert_eq!(issue.pointer, "/components/0/sources/0/path");
    assert_eq!(issue.severity, Severity::Error);
    assert!(
        probe.calls.borrow().is_empty(),
        "malformed paths are not probed"
    );
}

#[test]
fn absolute_path_is_invalid_citation_path() {
    let probe = FakeProbe::default();
    let issues = run(
        json!([{ "path": "/etc/hosts" }]),
        &probe,
        Profile::Submission,
    );
    assert_eq!(one(&issues).code, IssueCode::InvalidCitationPath);
}

#[test]
fn backslash_path_is_invalid_citation_path() {
    let probe = FakeProbe::default();
    let issues = run(
        json!([{ "path": "src\\a.rs" }]),
        &probe,
        Profile::Submission,
    );
    assert_eq!(one(&issues).code, IssueCode::InvalidCitationPath);
}

#[test]
fn path_outside_root_is_invalid_citation_path() {
    let probe = FakeProbe::with(&[("link/a.rs", FileProbe::OutsideRoot)]);
    let issues = run(
        json!([{ "path": "link/a.rs" }]),
        &probe,
        Profile::Submission,
    );
    let issue = one(&issues);
    assert_eq!(issue.code, IssueCode::InvalidCitationPath);
    assert!(
        issue.message.contains("outside the repository root"),
        "{issue:?}"
    );
}

// ---------- FR-VAL-06: file exists, text, ≤ 2 MB ----------

#[test]
fn missing_file_is_broken() {
    let probe = FakeProbe::default();
    let issues = run(json!([{ "path": "src/x.rs" }]), &probe, Profile::Submission);
    let issue = one(&issues);
    assert_eq!(issue.code, IssueCode::BrokenCitation);
    assert_eq!(issue.pointer, "/components/0/sources/0/path");
    assert_eq!(issue.message, "src/x.rs does not exist");
}

#[test]
fn existing_text_file_without_lines_is_valid() {
    let probe = FakeProbe::with(&[("src/x.rs", text(3))]);
    assert!(run(json!([{ "path": "src/x.rs" }]), &probe, Profile::Submission).is_empty());
}

#[test]
fn non_text_files_are_broken() {
    for (probe_result, needle) in [
        (FileProbe::NotAFile, "is not a file"),
        (FileProbe::Binary, "binary"),
        (FileProbe::TooLarge { size: 3_000_000 }, "3000000 bytes"),
        (FileProbe::Unreadable, "cannot be read"),
    ] {
        let probe = FakeProbe::with(&[("f", probe_result)]);
        let issues = run(json!([{ "path": "f" }]), &probe, Profile::Submission);
        let issue = one(&issues);
        assert_eq!(issue.code, IssueCode::BrokenCitation, "{probe_result:?}");
        assert!(issue.message.contains(needle), "{issue:?}");
    }
}

// ---------- FR-VAL-06: line ranges ----------

#[test]
fn line_zero_is_broken() {
    let probe = FakeProbe::with(&[("a", text(5))]);
    let issues = run(
        json!([{ "path": "a", "line": 0 }]),
        &probe,
        Profile::Submission,
    );
    let issue = one(&issues);
    assert_eq!(issue.code, IssueCode::BrokenCitation);
    assert_eq!(issue.pointer, "/components/0/sources/0/line");
}

#[test]
fn line_one_is_valid() {
    let probe = FakeProbe::with(&[("a", text(1))]);
    assert!(
        run(
            json!([{ "path": "a", "line": 1 }]),
            &probe,
            Profile::Submission
        )
        .is_empty()
    );
}

#[test]
fn end_line_before_line_is_broken() {
    let probe = FakeProbe::with(&[("a", text(50))]);
    let issues = run(
        json!([{ "path": "a", "line": 10, "end_line": 9 }]),
        &probe,
        Profile::Submission,
    );
    let issue = one(&issues);
    assert_eq!(issue.code, IssueCode::BrokenCitation);
    assert_eq!(issue.pointer, "/components/0/sources/0/end_line");
}

#[test]
fn end_line_equal_to_line_is_valid() {
    let probe = FakeProbe::with(&[("a", text(50))]);
    let issues = run(
        json!([{ "path": "a", "line": 10, "end_line": 10 }]),
        &probe,
        Profile::Submission,
    );
    assert!(issues.is_empty(), "{issues:#?}");
}

#[test]
fn end_line_beyond_line_count_is_broken_with_specific_message() {
    let probe = FakeProbe::with(&[("src/a.rs", text(25))]);
    let issues = run(
        json!([{ "path": "src/a.rs", "line": 10, "end_line": 40 }]),
        &probe,
        Profile::Submission,
    );
    let issue = one(&issues);
    assert_eq!(issue.code, IssueCode::BrokenCitation);
    assert_eq!(issue.pointer, "/components/0/sources/0/end_line");
    assert_eq!(
        issue.message,
        "Cited lines 10–40 are outside src/a.rs (25 lines)"
    );
}

#[test]
fn end_line_at_line_count_is_valid() {
    let probe = FakeProbe::with(&[("a", text(40))]);
    let issues = run(
        json!([{ "path": "a", "line": 10, "end_line": 40 }]),
        &probe,
        Profile::Submission,
    );
    assert!(issues.is_empty(), "{issues:#?}");
}

#[test]
fn line_beyond_line_count_without_end_line_is_broken() {
    let probe = FakeProbe::with(&[("a", text(5))]);
    let issues = run(
        json!([{ "path": "a", "line": 6 }]),
        &probe,
        Profile::Submission,
    );
    let issue = one(&issues);
    assert_eq!(issue.pointer, "/components/0/sources/0/line");
    assert_eq!(issue.message, "Cited line 6 is outside a (5 lines)");
}

#[test]
fn line_in_empty_file_is_broken() {
    let probe = FakeProbe::with(&[("a", text(0))]);
    let issues = run(
        json!([{ "path": "a", "line": 1 }]),
        &probe,
        Profile::Submission,
    );
    assert_eq!(one(&issues).code, IssueCode::BrokenCitation);
}

// ---------- FR-VAL-06: deny-list ----------

#[test]
fn denied_path_is_an_error_under_submission() {
    let mut probe = FakeProbe::with(&[(".env", text(3))]);
    probe.denied.insert(".env".into());
    let issues = run(json!([{ "path": ".env" }]), &probe, Profile::Submission);
    let issue = one(&issues);
    assert_eq!(issue.code, IssueCode::DeniedCitation);
    assert_eq!(issue.severity, Severity::Error);
    assert!(
        probe.calls.borrow().is_empty(),
        "denied paths are not probed"
    );
}

#[test]
fn denied_path_is_not_flagged_under_import() {
    let mut probe = FakeProbe::with(&[(".env", text(3))]);
    probe.denied.insert(".env".into());
    assert!(run(json!([{ "path": ".env" }]), &probe, Profile::Import).is_empty());
}

#[test]
fn non_denied_path_is_not_flagged_under_submission() {
    let mut probe = FakeProbe::with(&[(".env.example", text(3))]);
    probe.denied.insert(".env".into());
    assert!(
        run(
            json!([{ "path": ".env.example" }]),
            &probe,
            Profile::Submission
        )
        .is_empty()
    );
}

// ---------- Severity per profile ----------

#[test]
fn import_profile_downgrades_everything_to_warnings() {
    let probe = FakeProbe::with(&[("a", text(5))]);
    let issues = run(
        json!([
            { "path": "../x" },
            { "path": "missing" },
            { "path": "a", "line": 9 }
        ]),
        &probe,
        Profile::Import,
    );
    assert_eq!(issues.len(), 3, "{issues:#?}");
    assert!(issues.iter().all(|i| i.severity == Severity::Warning));
    let pointers: Vec<&str> = issues.iter().map(|i| i.pointer.as_str()).collect();
    assert_eq!(
        pointers,
        [
            "/components/0/sources/0/path",
            "/components/0/sources/1/path",
            "/components/0/sources/2/line"
        ]
    );
}

#[test]
fn each_distinct_path_is_probed_once() {
    let probe = FakeProbe::with(&[("a", text(5))]);
    let issues = run(
        json!([{ "path": "a", "line": 1 }, { "path": "a", "line": 2 }, { "path": "b" }]),
        &probe,
        Profile::Import,
    );
    assert_eq!(issues.len(), 1);
    assert_eq!(*probe.calls.borrow(), ["a", "b"]);
}

// ---------- FR-VAL-07: dirty flags ----------

#[test]
fn dirty_cited_path_is_a_warning_under_both_profiles() {
    let probe = FakeProbe::with(&[("src/a.rs", text(10))]);
    let dirty: HashSet<String> = ["src/a.rs".to_string()].into();
    for profile in [Profile::Import, Profile::Submission] {
        let issues = check_citations(
            &doc(json!([{ "path": "src/a.rs", "line": 2 }])),
            &probe,
            profile,
            &dirty,
        );
        let issue = one(&issues);
        assert_eq!(issue.code, IssueCode::ModifiedCitation);
        assert_eq!(issue.severity, Severity::Warning, "{profile:?}");
        assert_eq!(issue.pointer, "/components/0/sources/0/path");
    }
}

#[test]
fn clean_cited_path_is_not_flagged() {
    let probe = FakeProbe::with(&[("src/a.rs", text(10))]);
    let dirty: HashSet<String> = ["src/b.rs".to_string(), "src".to_string()].into();
    let issues = check_citations(
        &doc(json!([{ "path": "src/a.rs" }])),
        &probe,
        Profile::Submission,
        &dirty,
    );
    assert!(issues.is_empty(), "{issues:#?}");
}
