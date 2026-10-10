//! FR-WAT-01 (lifecycle), FR-WAT-02 (filtering), FR-WAT-07 (bursts), FR-WAT-08
//! (self-writes) and FR-WAT-09 (git refresh spacing).
//!
//! File names avoid patterns a developer's global gitignore is likely to contain.

use std::fs;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use archalign_repo::{
    BURST_PATHS, Changes, DEBOUNCE, GIT_REFRESH_INTERVAL, GitRefreshLimiter, RelPath, RepoWatcher,
    Sandbox, SelfWrites, WatchBatch, classify,
};

fn setup() -> (tempfile::TempDir, Sandbox) {
    let tmp = tempfile::tempdir().unwrap();
    fs::create_dir(tmp.path().join("repo")).unwrap();
    let sandbox = Sandbox::new(tmp.path().join("repo")).unwrap();
    (tmp, sandbox)
}

fn rel(s: &str) -> RelPath {
    RelPath::parse(s).unwrap()
}

fn rels(list: &[&str]) -> Vec<RelPath> {
    list.iter().map(|s| rel(s)).collect()
}

/// Absolute paths under the (canonical) sandbox root.
fn abs(sandbox: &Sandbox, list: &[&str]) -> Vec<PathBuf> {
    list.iter().map(|s| sandbox.root().join(s)).collect()
}

fn write(sandbox: &Sandbox, path: &str, body: &str) {
    let full = sandbox.root().join(path);
    fs::create_dir_all(full.parent().unwrap()).unwrap();
    fs::write(full, body).unwrap();
}

fn changes(batch: Option<WatchBatch>) -> Changes {
    match batch {
        Some(WatchBatch::Changes(c)) => c,
        other => panic!("expected Changes, got {other:?}"),
    }
}

fn run(sandbox: &Sandbox, list: &[&str], show_hidden: bool) -> Option<WatchBatch> {
    classify(
        sandbox,
        &abs(sandbox, list),
        &SelfWrites::default(),
        show_hidden,
    )
}

#[test]
fn outside_paths_are_dropped() {
    let (tmp, sandbox) = setup();
    fs::write(tmp.path().join("sibling.rs"), "x").unwrap();
    let outside = vec![
        tmp.path().join("sibling.rs"),
        tmp.path().join("gone.rs"),
        PathBuf::from("/definitely/not/here.rs"),
    ];
    assert_eq!(
        classify(&sandbox, &outside, &SelfWrites::default(), true),
        None
    );

    let mut mixed = outside;
    mixed.push(sandbox.root().join("in.rs"));
    assert_eq!(
        changes(classify(&sandbox, &mixed, &SelfWrites::default(), true)).paths,
        rels(&["in.rs"])
    );
}

#[test]
fn git_dir_keeps_only_head_and_index_as_git_flag() {
    let (_tmp, sandbox) = setup();
    write(&sandbox, ".git/HEAD", "ref: refs/heads/main\n");
    write(&sandbox, ".git/index", "idx");
    write(&sandbox, ".git/objects/ab/cdef", "obj");

    let noise = [
        ".git",
        ".git/objects/ab/cdef",
        ".git/refs/heads/main",
        ".git/HEAD.lock",
        ".git/index.lock",
    ];
    assert_eq!(run(&sandbox, &noise, true), None);

    for meta in [".git/HEAD", ".git/index"] {
        let c = changes(run(&sandbox, &[meta, ".git/objects/ab/cdef"], true));
        assert_eq!(
            c,
            Changes {
                git: true,
                ..Changes::default()
            },
            "{meta}"
        );
    }

    // Deleted `.git/index` (e.g. during a rewrite) still counts.
    fs::remove_file(sandbox.root().join(".git/index")).unwrap();
    assert!(changes(run(&sandbox, &[".git/index"], false)).git);
}

#[test]
fn archify_is_kept_even_when_gitignored_and_hidden() {
    let (_tmp, sandbox) = setup();
    write(&sandbox, ".gitignore", ".archify/\n*.json\n");
    write(&sandbox, ".archify/run-1/candidate.json", "{}");

    let c = changes(run(
        &sandbox,
        &[
            ".archify/run-1/candidate.json",
            ".archify/run-1",
            ".archify",
        ],
        false,
    ));
    assert_eq!(
        c.paths,
        rels(&[
            ".archify",
            ".archify/run-1",
            ".archify/run-1/candidate.json"
        ])
    );
    assert_eq!(c.dirs, rels(&["", ".archify", ".archify/run-1"]));
    assert!(c.archify);
    assert!(!c.git, "only .archify changed");
}

#[test]
fn gitignored_paths_are_dropped() {
    let (_tmp, sandbox) = setup();
    write(&sandbox, ".gitignore", "*.qq\nbuildout/\n");
    write(&sandbox, "dropped.qq", "x");
    write(&sandbox, "buildout/out.rs", "x");
    write(&sandbox, "src/nested.qq", "x");
    write(&sandbox, "src/kept.rs", "x");

    let c = changes(run(
        &sandbox,
        &[
            "dropped.qq",
            "buildout",
            "buildout/out.rs",
            "src/nested.qq",
            "src/kept.rs",
        ],
        true,
    ));
    assert_eq!(c.paths, rels(&["src/kept.rs"]));
    assert_eq!(
        run(&sandbox, &["dropped.qq", "buildout/out.rs"], true),
        None
    );
}

#[test]
fn hidden_paths_follow_show_hidden() {
    let (_tmp, sandbox) = setup();
    write(&sandbox, ".hidden-note.qq", "x");
    write(&sandbox, ".config/tool.toml", "x");
    write(&sandbox, "visible.rs", "x");
    let list = [".hidden-note.qq", ".config/tool.toml", "visible.rs"];

    assert_eq!(
        changes(run(&sandbox, &list, false)).paths,
        rels(&["visible.rs"])
    );
    assert_eq!(
        changes(run(&sandbox, &list, true)).paths,
        rels(&[".config/tool.toml", ".hidden-note.qq", "visible.rs"])
    );
    assert_eq!(run(&sandbox, &[".config/tool.toml"], false), None);
}

#[test]
fn deleted_paths_are_kept_unless_ignored() {
    let (_tmp, sandbox) = setup();
    write(&sandbox, ".gitignore", "*.qq\n");
    let c = changes(run(
        &sandbox,
        &["src/removed.rs", "removed-dir", "gone.qq"],
        false,
    ));
    assert_eq!(c.paths, rels(&["removed-dir", "src/removed.rs"]));
    assert_eq!(c.dirs, rels(&["", "src"]));
    assert!(c.git);
    assert!(!c.archify);
}

#[test]
fn non_canonical_aliases_of_the_root_are_accepted() {
    // On macOS the temp dir is `/var/...` while the sandbox root and FSEvents use
    // `/private/var/...`; both spellings must map into the repo, existing or deleted.
    let (tmp, sandbox) = setup();
    write(&sandbox, "here.rs", "x");
    let alias = tmp.path().join("repo");
    let paths = vec![
        alias.join("here.rs"),
        alias.join("deleted.rs"),
        alias.join("gone-dir/deleted.rs"),
    ];
    let c = changes(classify(&sandbox, &paths, &SelfWrites::default(), false));
    assert_eq!(
        c.paths,
        rels(&["deleted.rs", "gone-dir/deleted.rs", "here.rs"])
    );
}

#[test]
fn burst_above_threshold_is_full_refresh() {
    let (_tmp, sandbox) = setup();
    let names: Vec<String> = (0..=BURST_PATHS).map(|i| format!("f{i:04}.rs")).collect();
    let all: Vec<&str> = names.iter().map(String::as_str).collect();

    let exact = changes(run(&sandbox, &all[..BURST_PATHS], false));
    assert_eq!(exact.paths.len(), BURST_PATHS);

    assert_eq!(run(&sandbox, &all, false), Some(WatchBatch::Full));

    // Duplicates and dropped paths don't count toward the threshold.
    let mut padded: Vec<&str> = all[..BURST_PATHS].to_vec();
    padded.extend_from_slice(&all[..10]);
    padded.extend([".git/objects/x", "/outside"]);
    assert_eq!(
        changes(run(&sandbox, &padded, false)).paths.len(),
        BURST_PATHS
    );
}

#[test]
fn self_writes_are_dropped_but_later_external_changes_are_kept() {
    let (_tmp, sandbox) = setup();
    write(&sandbox, ".archify/run/candidate.json", "{\"v\":1}");
    write(&sandbox, "src/main.rs", "fn main() {}");
    let writes = SelfWrites::default();
    let candidate = sandbox.root().join(".archify/run/candidate.json");
    writes.record(&candidate);

    assert_eq!(
        classify(&sandbox, std::slice::from_ref(&candidate), &writes, false),
        None
    );
    let other = vec![candidate.clone(), sandbox.root().join("src/main.rs")];
    assert_eq!(
        changes(classify(&sandbox, &other, &writes, false)).paths,
        rels(&["src/main.rs"])
    );

    // An external edit changes the length: no longer ours.
    fs::write(&candidate, "{\"v\":2, \"external\":true}").unwrap();
    let c = changes(classify(
        &sandbox,
        std::slice::from_ref(&candidate),
        &writes,
        false,
    ));
    assert_eq!(c.paths, rels(&[".archify/run/candidate.json"]));
    assert!(c.archify);
    assert!(!c.git);
}

#[test]
fn flags_and_dirs_are_classified_and_sorted() {
    let (_tmp, sandbox) = setup();
    write(&sandbox, "src/main.rs", "x");
    write(&sandbox, "src/lib.rs", "x");
    write(&sandbox, "top.rs", "x");
    write(&sandbox, ".archify/x/candidate.json", "{}");
    write(&sandbox, ".git/HEAD", "x");

    let c = changes(run(
        &sandbox,
        &[
            "top.rs",
            "src/main.rs",
            ".archify/x/candidate.json",
            "src/lib.rs",
            "src/main.rs",
            ".git/HEAD",
        ],
        false,
    ));
    assert_eq!(
        c,
        Changes {
            paths: rels(&[
                ".archify/x/candidate.json",
                "src/lib.rs",
                "src/main.rs",
                "top.rs"
            ]),
            dirs: rels(&["", ".archify/x", "src"]),
            archify: true,
            git: true,
        }
    );

    let c = changes(run(&sandbox, &["src/main.rs"], false));
    assert_eq!(c.dirs, rels(&["src"]));
    assert!(c.git);
    assert!(!c.archify);

    assert_eq!(run(&sandbox, &[], false), None);
}

#[test]
fn git_refresh_limiter_spaces_and_coalesces() {
    let t0 = Instant::now();
    let at = |ms: u64| t0 + Duration::from_millis(ms);
    let mut limiter = GitRefreshLimiter::default();

    assert_eq!(
        limiter.request(t0),
        Some(Duration::ZERO),
        "first is immediate"
    );
    limiter.done(at(100));

    assert_eq!(
        limiter.request(at(600)),
        Some(GIT_REFRESH_INTERVAL - Duration::from_millis(500)),
        "within the interval: delayed by the remainder"
    );
    assert_eq!(limiter.request(at(900)), None, "coalesced while scheduled");
    assert_eq!(
        limiter.request(at(5000)),
        None,
        "still scheduled until done"
    );

    limiter.done(at(2100));
    assert_eq!(
        limiter.request(at(2100) + GIT_REFRESH_INTERVAL),
        Some(Duration::ZERO),
        "a full interval after the last refresh is immediate"
    );
    // That immediate refresh counts as the last one.
    assert_eq!(
        limiter.request(at(2100) + GIT_REFRESH_INTERVAL + Duration::from_millis(500)),
        Some(Duration::from_millis(1500))
    );
}

#[test]
fn real_watcher_reports_changes_and_stops_on_drop() {
    let (_tmp, sandbox) = setup();
    let (tx, rx) = mpsc::channel();
    let watcher = RepoWatcher::start(sandbox.clone(), SelfWrites::default(), false, move |b| {
        let _ = tx.send(b);
    })
    .unwrap();

    write(&sandbox, "hello.rs", "fn main() {}");
    let target = rel("hello.rs");
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut seen = false;
    while !seen {
        let left = deadline.saturating_duration_since(Instant::now());
        assert!(!left.is_zero(), "no batch with hello.rs within 5 s");
        match rx.recv_timeout(left) {
            Ok(WatchBatch::Changes(c)) => {
                seen = c.paths.contains(&target);
                if seen {
                    assert!(c.dirs.contains(&RelPath::root()));
                    assert!(c.git);
                }
            }
            Ok(WatchBatch::Full) => {}
            Ok(WatchBatch::Error(e)) => panic!("watcher error: {e}"),
            Err(e) => panic!("no batch with hello.rs within 5 s: {e}"),
        }
    }
    // Drain trailing batches for the same write.
    while rx.recv_timeout(DEBOUNCE * 2).is_ok() {}

    drop(watcher);
    write(&sandbox, "after.rs", "x");
    // Either nothing arrives or the handler (and its sender) is gone.
    assert!(rx.recv_timeout(DEBOUNCE * 4).is_err());
}
