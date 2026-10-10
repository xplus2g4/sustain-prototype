//! FR-GEN-03 (seed tree, file heads) and FR-GEN-04 (`list_dir` and `grep` tools) helpers,
//! with the FR-SEC-02 deny-list applied.
//!
//! File names avoid patterns a developer's global gitignore is likely to contain, except
//! where the file must be hidden anyway.

use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;
use std::time::Duration;

use archalign_repo::agent_fs::{
    GrepOptions, GrepResult, LIST_DIR_MAX, compile_pattern, grep, head_lines, list_for_agent,
    seed_tree,
};
use archalign_repo::{DenyList, RelPath, Sandbox, SandboxError};
use tempfile::TempDir;

fn write(root: &Path, rel: &str, bytes: impl AsRef<[u8]>) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

fn rel(s: &str) -> RelPath {
    RelPath::parse(s).unwrap()
}

/// A repo with code, dotfiles, ignored, denied and linked entries.
fn fixture() -> (TempDir, Sandbox) {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("repo");
    fs::create_dir(&root).unwrap();
    write(&root, ".gitignore", "build/\n*.log\n");
    write(&root, "README.md", "# Demo\nline2\n");
    write(&root, ".toolrc", "needle in a dotfile\n");
    write(&root, "src/main.rs", "fn main() {\n    // needle one\n}\n");
    write(&root, "src/lib.rs", "pub fn lib() {}\n");
    write(&root, "src/nested/deep.rs", "needle\r\n");
    write(&root, "docs/guide.md", "guide\n");
    write(&root, "config/app.toml", "name = \"x\"\n");
    // Denied (FR-SEC-02).
    write(&root, ".env", "needle=secret\n");
    write(&root, "config/.env", "needle=secret\n");
    write(&root, "keys/server.pem", "needle\n");
    write(&root, ".npmrc", "needle\n");
    write(&root, ".git/HEAD", "needle\n");
    write(&root, ".archify/old/candidate.json", "needle\n");
    // Ignored.
    write(&root, "build/out.txt", "needle\n");
    write(&root, "app.log", "needle\n");
    // Links.
    write(tmp.path(), "outside/secret.txt", "needle\n");
    symlink(tmp.path().join("outside"), root.join("link_out")).unwrap();
    symlink(root.join("src"), root.join("link_in")).unwrap();
    symlink(root.join("README.md"), root.join("file_link")).unwrap();
    symlink(root.join(".env"), root.join("env_link")).unwrap();
    let sandbox = Sandbox::new(&root).unwrap();
    (tmp, sandbox)
}

fn lines(sandbox: &Sandbox, dir: &str, max: usize) -> (Vec<String>, usize) {
    let listing = list_for_agent(sandbox, &DenyList::new(), &rel(dir), max).unwrap();
    (listing.lines, listing.omitted)
}

#[test]
fn list_dir_shows_folders_first_with_sizes_and_dotfiles() {
    let (_tmp, sandbox) = fixture();
    let (got, omitted) = lines(&sandbox, "", LIST_DIR_MAX);
    assert_eq!(
        got,
        [
            "config/",
            "docs/",
            "keys/",
            "link_in/",
            "src/",
            ".gitignore (13 bytes)",
            ".toolrc (20 bytes)",
            "file_link (13 bytes)",
            "README.md (13 bytes)",
        ]
    );
    assert_eq!(omitted, 0);
}

#[test]
fn list_dir_hides_denied_ignored_and_outside_entries() {
    let (_tmp, sandbox) = fixture();
    let (root, _) = lines(&sandbox, "", LIST_DIR_MAX);
    for hidden in [
        ".env", ".npmrc", ".git", ".archify", "build", "app.log", "link_out", "env_link",
    ] {
        assert!(
            !root
                .iter()
                .any(|l| l.starts_with(&format!("{hidden}/"))
                    || l.starts_with(&format!("{hidden} "))),
            "{hidden} listed: {root:?}"
        );
    }
    assert_eq!(lines(&sandbox, "config", 10).0, ["app.toml (11 bytes)"]);
    assert!(lines(&sandbox, "keys", 10).0.is_empty());
    assert_eq!(
        lines(&sandbox, "link_in", 10).0,
        ["nested/", "lib.rs (16 bytes)", "main.rs (32 bytes)"]
    );
}

#[test]
fn list_dir_rejects_denied_ignored_outside_and_missing_folders() {
    let (_tmp, sandbox) = fixture();
    let deny = DenyList::new();
    let list = |dir: &str| list_for_agent(&sandbox, &deny, &rel(dir), 10);
    assert!(matches!(list(".git"), Err(SandboxError::InvalidPath(m)) if m.contains("deny")));
    assert!(matches!(
        list(".archify"),
        Err(SandboxError::InvalidPath(_))
    ));
    assert!(matches!(list("build"), Err(SandboxError::InvalidPath(m)) if m.contains("ignore")));
    assert!(matches!(list("link_out"), Err(SandboxError::Escapes(_))));
    assert!(matches!(list("missing"), Err(SandboxError::NotFound(_))));
    assert!(matches!(
        list("README.md"),
        Err(SandboxError::InvalidPath(_))
    ));
}

#[test]
fn list_dir_caps_entries_and_counts_the_rest() {
    let tmp = tempfile::tempdir().unwrap();
    for i in 0..7 {
        write(tmp.path(), &format!("f{i}.txt"), "x");
    }
    write(tmp.path(), "zdir/a", "x");
    let sandbox = Sandbox::new(tmp.path()).unwrap();
    let (got, omitted) = lines(&sandbox, "", 5);
    assert_eq!(
        got,
        [
            "zdir/",
            "f0.txt (1 bytes)",
            "f1.txt (1 bytes)",
            "f2.txt (1 bytes)",
            "f3.txt (1 bytes)"
        ]
    );
    assert_eq!(omitted, 3);
}

#[test]
fn seed_tree_is_depth_two_and_hides_denied_paths() {
    let (_tmp, sandbox) = fixture();
    let tree = seed_tree(&sandbox, &DenyList::new(), 2, 400);
    assert_eq!(
        tree.lines,
        [
            "config/",
            "  app.toml",
            "docs/",
            "  guide.md",
            "keys/",
            "link_in/",
            "src/",
            "  nested/",
            "  lib.rs",
            "  main.rs",
            ".gitignore",
            ".toolrc",
            "file_link",
            "README.md",
        ]
    );
    assert!(!tree.truncated);
    let joined = tree.lines.join("\n");
    for absent in [
        ".env",
        "server.pem",
        ".npmrc",
        ".git/",
        ".archify",
        "deep.rs",
        "build",
    ] {
        assert!(!joined.contains(absent), "{absent} in seed tree");
    }
}

#[test]
fn seed_tree_cap_keeps_shallow_levels_first() {
    let (_tmp, sandbox) = fixture();
    let tree = seed_tree(&sandbox, &DenyList::new(), 2, 6);
    assert_eq!(
        tree.lines,
        [
            "config/",
            "docs/",
            "keys/",
            "link_in/",
            "src/",
            ".gitignore"
        ]
    );
    assert!(tree.truncated);
}

#[test]
fn seed_tree_caps_at_400_entries() {
    let tmp = tempfile::tempdir().unwrap();
    for d in 0..10 {
        for f in 0..50 {
            write(tmp.path(), &format!("d{d:02}/f{f:02}.txt"), "x");
        }
    }
    let sandbox = Sandbox::new(tmp.path()).unwrap();
    let tree = seed_tree(&sandbox, &DenyList::new(), 2, 400);
    assert_eq!(tree.lines.len(), 400);
    assert!(tree.truncated);
    assert_eq!(tree.lines.first().map(String::as_str), Some("d00/"));
    assert_eq!(tree.lines.get(1).map(String::as_str), Some("  f00.txt"));

    let exact = seed_tree(&sandbox, &DenyList::new(), 2, 510);
    assert_eq!(exact.lines.len(), 510);
    assert!(!exact.truncated);
}

fn needle() -> regex::Regex {
    compile_pattern("needle").unwrap()
}

fn hits(result: &GrepResult) -> Vec<String> {
    result
        .matches
        .iter()
        .map(|m| format!("{}:{}: {}", m.path, m.line, m.text))
        .collect()
}

#[test]
fn grep_finds_lines_and_never_denied_ignored_or_outside_files() {
    let (_tmp, sandbox) = fixture();
    let result = grep(
        &sandbox,
        &DenyList::new(),
        &needle(),
        None,
        &GrepOptions::default(),
    )
    .unwrap();
    assert_eq!(
        hits(&result),
        [
            ".toolrc:1: needle in a dotfile",
            "src/main.rs:2:     // needle one",
            "src/nested/deep.rs:1: needle",
        ]
    );
    assert!(!result.truncated);
    assert!(!result.timed_out);
    assert_eq!(result.skipped, 0);
}

#[test]
fn grep_glob_filters_paths() {
    let (_tmp, sandbox) = fixture();
    let deny = DenyList::new();
    let opts = GrepOptions::default();
    let paths = |glob: &str| -> Vec<String> {
        grep(&sandbox, &deny, &needle(), Some(glob), &opts)
            .unwrap()
            .matches
            .into_iter()
            .map(|m| m.path)
            .collect()
    };
    assert_eq!(paths("*.rs"), ["src/main.rs", "src/nested/deep.rs"]);
    assert_eq!(paths("src/*.rs"), ["src/main.rs"]);
    assert_eq!(paths("src/**/*.rs"), ["src/main.rs", "src/nested/deep.rs"]);
    // A glob never re-admits denied or ignored files.
    assert!(paths("*.pem").is_empty());
    assert!(paths(".env").is_empty());
    assert!(paths("*.log").is_empty());
    assert!(matches!(
        grep(&sandbox, &deny, &needle(), Some("src/[a"), &opts),
        Err(SandboxError::InvalidPath(_))
    ));
}

#[test]
fn grep_skips_and_counts_binary_and_large_files() {
    let tmp = tempfile::tempdir().unwrap();
    write(tmp.path(), "a.txt", "needle\n");
    write(tmp.path(), "bin.dat", b"needle\0\x01\x02");
    write(
        tmp.path(),
        "big.txt",
        format!("needle\n{}", "x".repeat(100)),
    );
    let sandbox = Sandbox::new(tmp.path()).unwrap();
    let opts = GrepOptions {
        max_file_bytes: 64,
        ..GrepOptions::default()
    };
    let result = grep(&sandbox, &DenyList::new(), &needle(), None, &opts).unwrap();
    assert_eq!(hits(&result), ["a.txt:1: needle"]);
    assert_eq!(result.skipped, 2);
}

#[test]
fn grep_order_is_by_path_and_deterministic() {
    let tmp = tempfile::tempdir().unwrap();
    for f in ["b.rs", "a/z.rs", "a.rs", "A/c.rs"] {
        write(tmp.path(), f, "needle\n");
    }
    let sandbox = Sandbox::new(tmp.path()).unwrap();
    let run = || {
        grep(
            &sandbox,
            &DenyList::new(),
            &needle(),
            None,
            &GrepOptions::default(),
        )
        .unwrap()
    };
    let first = run();
    let paths: Vec<&str> = first.matches.iter().map(|m| m.path.as_str()).collect();
    let mut sorted = paths.clone();
    sorted.sort();
    assert_eq!(paths, sorted);
    assert_eq!(paths.len(), 4);
    assert_eq!(first, run());
}

#[test]
fn grep_stops_at_max_matches() {
    let tmp = tempfile::tempdir().unwrap();
    write(tmp.path(), "many.txt", "needle\n".repeat(10));
    let sandbox = Sandbox::new(tmp.path()).unwrap();
    let opts = |max_matches| GrepOptions {
        max_matches,
        ..GrepOptions::default()
    };
    let capped = grep(&sandbox, &DenyList::new(), &needle(), None, &opts(3)).unwrap();
    assert_eq!(capped.matches.len(), 3);
    assert_eq!(capped.matches.last().map(|m| m.line), Some(3));
    assert!(capped.truncated);
    let exact = grep(&sandbox, &DenyList::new(), &needle(), None, &opts(10)).unwrap();
    assert_eq!(exact.matches.len(), 10);
    assert!(!exact.truncated);
}

#[test]
fn grep_cuts_long_lines_on_char_boundaries() {
    let tmp = tempfile::tempdir().unwrap();
    write(
        tmp.path(),
        "wide.txt",
        format!("{}needle{}\r\n", "é".repeat(150), "日".repeat(150)),
    );
    let sandbox = Sandbox::new(tmp.path()).unwrap();
    let result = grep(
        &sandbox,
        &DenyList::new(),
        &needle(),
        None,
        &GrepOptions::default(),
    )
    .unwrap();
    let text = &result.matches[0].text;
    assert_eq!(text.chars().count(), 200);
    assert_eq!(
        *text,
        format!("{}needle{}", "é".repeat(150), "日".repeat(44))
    );
}

#[test]
fn grep_gives_up_at_the_time_limit() {
    let (_tmp, sandbox) = fixture();
    let opts = GrepOptions {
        time_limit: Duration::ZERO,
        ..GrepOptions::default()
    };
    let result = grep(&sandbox, &DenyList::new(), &needle(), None, &opts).unwrap();
    assert!(result.timed_out);
    assert!(result.matches.is_empty());
}

#[test]
fn compile_pattern_reports_errors_and_limits_size() {
    assert!(compile_pattern("fn\\s+main").is_ok());
    assert!(compile_pattern("(unclosed").is_err());
    assert!(compile_pattern("\\w{1000}\\w{1000}\\w{1000}").is_err());
}

#[test]
fn head_lines_reads_the_start_of_allowed_text_files() {
    let (tmp, sandbox) = fixture();
    let deny = DenyList::new();
    let root = sandbox.root().to_path_buf();
    write(&root, "five.txt", "1\n2\n3\n4\n5\n");
    write(&root, "logo.png", b"\x89PNG\0\0");
    write(&root, "huge.txt", vec![b'a'; 2 * 1024 * 1024 + 1]);
    let head = |p: &str, n| head_lines(&sandbox, &deny, &rel(p), n);
    assert_eq!(head("five.txt", 2).as_deref(), Some("1\n2\n"));
    assert_eq!(head("five.txt", 150).as_deref(), Some("1\n2\n3\n4\n5\n"));
    assert_eq!(head("file_link", 1).as_deref(), Some("# Demo\n"));
    assert_eq!(head("missing.txt", 5), None);
    assert_eq!(head(".env", 5), None);
    assert_eq!(head("env_link", 5), None);
    assert_eq!(head("keys/server.pem", 5), None);
    assert_eq!(head("logo.png", 5), None);
    assert_eq!(head("huge.txt", 5), None);
    assert_eq!(head("src", 5), None);
    assert_eq!(head("link_out/secret.txt", 5), None);
    drop(tmp);
}
