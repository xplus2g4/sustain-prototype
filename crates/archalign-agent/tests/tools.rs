//! FR-GEN-02/03 (seed), FR-GEN-04 (tools), FR-SEC-02 (deny-list in every tool).

mod common;

use archalign_agent::prompt::{SeedInput, seed_message};
use archalign_agent::tools::{RESULT_CAP, RepoMeta, ToolSet};
use common::*;
use serde_json::json;

fn tools(repo: &Repo) -> ToolSet {
    ToolSet::new(
        repo.sandbox.clone(),
        RepoMeta {
            git: true,
            head: Some("0123456789abcdef0123456789abcdef01234567".into()),
            origin: Some("https://github.com/o/r.git".into()),
            dirty: (0..250).map(|i| format!("f{i}.rs")).collect(),
        },
    )
}

fn exec(t: &ToolSet, name: &str, args: serde_json::Value) -> archalign_agent::tools::ToolOutput {
    t.execute(name, &args.to_string())
}

// ---------- FR-GEN-04: one success and one error per tool ----------

#[test]
fn list_dir_lists_folders_and_sizes() {
    let repo = sample();
    let t = tools(&repo);
    let out = exec(&t, "list_dir", json!({ "path": "" }));
    assert!(out.error.is_none(), "{out:?}");
    assert!(out.content.contains("src/"));
    assert!(
        out.content.contains("README.md (22 bytes)"),
        "{}",
        out.content
    );
    let out = exec(&t, "list_dir", json!({ "path": "nope" }));
    assert!(out.error.is_some());
    let out = exec(&t, "list_dir", json!({ "path": "../" }));
    assert!(out.error.is_some(), "escapes are rejected");
}

#[test]
fn read_file_numbers_lines_caps_at_400_and_points_to_the_next_start() {
    let repo = sample();
    repo.write("big.txt", lines(1000));
    let t = tools(&repo);
    let out = exec(
        &t,
        "read_file",
        json!({ "path": "src/db.rs", "start_line": 3, "end_line": 4 }),
    );
    assert_eq!(out.content, "3: line 3\n4: line 4\n[src/db.rs: 20 lines]");
    let out = exec(&t, "read_file", json!({ "path": "big.txt" }));
    assert!(out.content.starts_with("1: line 1\n"));
    assert!(out.content.contains("400: line 400\n"));
    assert!(!out.content.contains("401: line 401"));
    assert!(
        out.content
            .ends_with("[truncated: showing lines 1–400 of 1000; continue with start_line 401]")
    );
    let out = exec(
        &t,
        "read_file",
        json!({ "path": "src/db.rs", "start_line": 99 }),
    );
    assert_eq!(
        out.error.as_deref(),
        Some("start_line 99 is beyond the end of src/db.rs (20 lines)")
    );
    repo.write("img.bin", [0u8, 1, 2]);
    assert_eq!(
        exec(&t, "read_file", json!({ "path": "img.bin" }))
            .error
            .as_deref(),
        Some("img.bin is a binary file")
    );
    repo.write("huge.txt", vec![b'a'; 2 * 1024 * 1024 + 1]);
    assert!(
        exec(&t, "read_file", json!({ "path": "huge.txt" }))
            .error
            .unwrap()
            .contains("larger than 2 MB")
    );
}

#[test]
fn read_file_respects_the_24kb_cap() {
    let repo = sample();
    let long: String = (1..=300)
        .map(|i| format!("{i} {}\n", "x".repeat(200)))
        .collect();
    repo.write("wide.txt", long);
    repo.write("oneline.txt", "y".repeat(100_000));
    let t = tools(&repo);
    let out = exec(&t, "read_file", json!({ "path": "wide.txt" }));
    assert!(out.content.len() <= RESULT_CAP);
    assert!(
        out.content.contains("continue with start_line"),
        "{}",
        &out.content[out.content.len() - 120..]
    );
    let out = exec(&t, "read_file", json!({ "path": "oneline.txt" }));
    assert!(out.content.len() <= RESULT_CAP);
}

#[test]
fn grep_finds_matches_and_reports_bad_patterns() {
    let repo = sample();
    repo.write("src/x.py", "def handler():\n    return 42\n");
    let t = tools(&repo);
    let out = exec(
        &t,
        "grep",
        json!({ "pattern": "line 1[0-2]$", "path_glob": "src/*.rs" }),
    );
    assert_eq!(
        out.content.lines().take(3).collect::<Vec<_>>(),
        [
            "src/db.rs:10: line 10",
            "src/db.rs:11: line 11",
            "src/db.rs:12: line 12"
        ]
    );
    let out = exec(&t, "grep", json!({ "pattern": "def \\w+" }));
    assert_eq!(out.content, "src/x.py:1: def handler():");
    let out = exec(&t, "grep", json!({ "pattern": "(unclosed" }));
    assert!(out.error.unwrap().starts_with("invalid regex"));
    let out = exec(&t, "grep", json!({ "pattern": "a", "unknown": 1 }));
    assert!(out.error.unwrap().starts_with("invalid arguments for grep"));
}

#[test]
fn grep_caps_at_100_matches() {
    let repo = sample();
    repo.write("many.txt", "hit\n".repeat(500));
    let out = exec(&tools(&repo), "grep", json!({ "pattern": "hit" }));
    assert_eq!(
        out.content
            .lines()
            .filter(|l| l.starts_with("many.txt:"))
            .count(),
        100
    );
    assert!(out.content.contains("[truncated at 100 matches"));
}

#[test]
fn repo_meta_reports_git_facts_with_at_most_200_dirty_paths() {
    let repo = sample();
    let out = exec(&tools(&repo), "repo_meta", json!({}));
    let v: serde_json::Value = serde_json::from_str(&out.content).unwrap();
    assert_eq!(v["git"], true);
    assert_eq!(v["origin"], "https://github.com/o/r.git");
    assert_eq!(v["dirty_paths"].as_array().unwrap().len(), 200);
    assert_eq!(v["dirty_total"], 250);
    let out = exec(&tools(&repo), "repo_meta", json!({ "x": 1 }));
    assert!(out.error.is_some());
}

#[test]
fn unknown_tools_are_errors() {
    let repo = sample();
    let out = exec(&tools(&repo), "shell", json!({ "cmd": "rm -rf /" }));
    assert_eq!(out.error.as_deref(), Some("unknown tool `shell`"));
}

#[test]
fn every_result_is_capped_at_24kb() {
    let repo = sample();
    for i in 0..400 {
        repo.write(&format!("d/{}{i:04}.txt", "a".repeat(60)), "");
    }
    let out = exec(&tools(&repo), "list_dir", json!({ "path": "d" }));
    assert!(out.content.len() <= RESULT_CAP, "{}", out.content.len());
    repo.write("matches.txt", format!("{}\n", "m".repeat(190)).repeat(150));
    let out = exec(&tools(&repo), "grep", json!({ "pattern": "m+" }));
    assert!(out.content.len() <= RESULT_CAP);
}

// ---------- FR-SEC-02: every pattern against every tool ----------

const DENIED: [&str; 14] = [
    ".git/config",
    ".archify/old/candidate.json",
    ".env",
    ".env.local",
    "config/.env.production",
    "certs/server.pem",
    "certs/tls.key",
    "certs/client.p12",
    "certs/store.pfx",
    "home/id_rsa",
    "home/id_ed25519.pub",
    ".npmrc",
    ".pypirc",
    ".netrc",
];

#[test]
fn deny_listed_paths_are_hidden_from_every_tool() {
    let repo = sample();
    for p in DENIED {
        repo.write(p, "SECRET-VALUE-XYZ\n");
    }
    repo.write(".env.example", "SECRET-VALUE-XYZ\n");
    repo.write(".env.sample", "SECRET-VALUE-XYZ\n");
    let t = tools(&repo);
    // grep never matches inside them.
    let out = exec(&t, "grep", json!({ "pattern": "SECRET-VALUE-XYZ" }));
    let hits: Vec<&str> = out.content.lines().filter(|l| l.contains(':')).collect();
    assert_eq!(hits.len(), 2, "only the allowed examples match: {hits:?}");
    assert!(
        hits.iter().any(|h| h.starts_with(".env.example:"))
            && hits.iter().any(|h| h.starts_with(".env.sample:"))
    );
    for p in DENIED {
        // read_file returns denied.
        let out = exec(&t, "read_file", json!({ "path": p }));
        assert!(
            out.error
                .as_deref()
                .is_some_and(|e| e.starts_with("denied")),
            "{p}: {out:?}"
        );
        assert!(!out.content.contains("SECRET-VALUE-XYZ"));
        // list_dir of the parent never shows it.
        let (parent, name) = p.rsplit_once('/').unwrap_or(("", p));
        let out = exec(&t, "list_dir", json!({ "path": parent }));
        assert!(
            !out.content.lines().any(|l| l == name
                || l.starts_with(&format!("{name} "))
                || l == format!("{name}/")),
            "{p} listed: {}",
            out.content
        );
    }
    let root = exec(&t, "list_dir", json!({ "path": "" })).content;
    assert!(!root.contains(".git/") && !root.contains(".archify/"));
    assert!(root.contains(".env.example"));
}

// ---------- FR-GEN-02 / FR-GEN-03: seed message ----------

#[test]
fn seed_contains_focus_tree_readme_and_manifests_but_no_denied_files() {
    let repo = sample();
    repo.write("README.md", lines(200));
    repo.write("pyproject.toml", lines(130));
    repo.write("Dockerfile", "FROM python:3.12\n");
    repo.write("Makefile", "all:\n\techo hi\n");
    repo.write("src/deep/er/file.rs", "x\n");
    repo.write(".env", "OPENAI_API_KEY=sk-should-not-appear\n");
    repo.write("id_rsa", "PRIVATE\n");
    let seed = seed_message(
        &repo.sandbox,
        &SeedInput {
            focus: "how payments are captured".into(),
            repo_name: "sample".into(),
            short_head: Some("0123456".into()),
            dirty_count: 2,
        },
    );
    assert!(seed.contains("Focus (from the user): how payments are captured"));
    assert!(seed.contains("Repository folder: sample"));
    assert!(seed.contains("Git: HEAD 0123456, 2 uncommitted paths"));
    assert!(seed.contains("src/"));
    assert!(seed.contains("  deep/"), "depth 2");
    assert!(!seed.contains("er/"), "depth 3 is not listed");
    assert!(seed.contains("### README.md (first 150 lines)"));
    assert!(seed.contains("line 150\n") && !seed.contains("line 151\n"));
    assert!(seed.contains("### pyproject.toml (first 120 lines)"));
    assert!(seed.contains("### Dockerfile") && seed.contains("### Makefile"));
    assert!(!seed.contains("sk-should-not-appear") && !seed.contains("PRIVATE"));
    assert!(!seed.contains(".env\n") && !seed.contains("id_rsa"));
    assert!(seed.len() <= 48 * 1024);
}

#[test]
fn seed_is_capped_at_48kb() {
    let repo = sample();
    let wide = format!("{}\n", "w".repeat(1000));
    repo.write("README.md", wide.repeat(150));
    repo.write("package.json", wide.repeat(120));
    let seed = seed_message(
        &repo.sandbox,
        &SeedInput {
            repo_name: "s".into(),
            ..SeedInput::default()
        },
    );
    assert!(seed.len() <= 48 * 1024);
    assert!(seed.contains("Focus: none given"));
}
