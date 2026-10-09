//! FR-IMP-04 (git metadata), FR-SEC-07 (origin sanitization), FR-SEC-08 (invocation).
//!
//! The repo tests run the real `git` binary.

use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::process::Command;

use archalign_model::{LinkMode, RepositoryProvider};
use archalign_repo::git::parse_porcelain_z;
use archalign_repo::{GitRead, GitStatus, collect, collect_with, read_argv, sanitize_origin};
use tempfile::TempDir;

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=Test", "-c", "user.email=test@example.com"])
        .args([
            "-c",
            "commit.gpgsign=false",
            "-c",
            "init.defaultBranch=main",
        ])
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .unwrap();
    assert!(status.status.success(), "git {args:?}: {status:?}");
}

/// A committed repo with `a.txt`, `b.txt` and `sub/c.txt`.
fn repo() -> TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    git(root, &["init", "-q"]);
    fs::write(root.join("a.txt"), "a\n").unwrap();
    fs::write(root.join("b.txt"), "b\n").unwrap();
    fs::create_dir(root.join("sub")).unwrap();
    fs::write(root.join("sub/c.txt"), "c\n").unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-q", "-m", "init"]);
    tmp
}

fn info(root: &Path) -> archalign_repo::GitInfo {
    match collect(root) {
        GitStatus::Repo(info) => info,
        other => panic!("{other:?}"),
    }
}

#[test]
fn clean_repo_reports_head_and_no_dirty_paths() {
    let tmp = repo();
    let info = info(tmp.path());
    assert_eq!(info.head.len(), 40);
    assert!(info.head.bytes().all(|b| b.is_ascii_hexdigit()));
    assert_eq!(info.short_head(), &info.head[..7]);
    assert!(info.dirty.is_empty(), "{:?}", info.dirty);
}

#[test]
fn dirty_repo_reports_modified_untracked_and_renamed_paths() {
    let tmp = repo();
    let root = tmp.path();
    fs::write(root.join("a.txt"), "changed\n").unwrap();
    fs::write(root.join("new file.txt"), "new\n").unwrap();
    git(root, &["mv", "b.txt", "sub/renamed.txt"]);
    let mut dirty = info(root).dirty;
    dirty.sort();
    assert_eq!(dirty, ["a.txt", "new file.txt", "sub/renamed.txt"]);
    assert!(info(root).dirty_set().contains("sub/renamed.txt"));
}

#[test]
fn repo_without_origin_has_no_repository_metadata() {
    let tmp = repo();
    let info = info(tmp.path());
    assert_eq!(info.origin, None);
    assert_eq!(info.repository(), None);
}

#[test]
fn origin_with_credentials_is_sanitized() {
    let tmp = repo();
    git(
        tmp.path(),
        &[
            "remote",
            "add",
            "origin",
            "https://user:s3cret@github.com/acme/widget.git",
        ],
    );
    let info = info(tmp.path());
    assert_eq!(
        info.origin.as_deref(),
        Some("https://github.com/acme/widget.git")
    );
    let repo = info.repository().unwrap();
    assert_eq!(repo.url, "https://github.com/acme/widget.git");
    assert_eq!(repo.revision, info.head);
    assert_eq!(repo.provider, Some(RepositoryProvider::Github));
    assert_eq!(repo.link_mode, None);
}

#[test]
fn non_hosted_origin_is_local_only() {
    let tmp = repo();
    git(
        tmp.path(),
        &["remote", "add", "origin", "git@example.org:acme/widget.git"],
    );
    let repo = info(tmp.path()).repository().unwrap();
    assert_eq!(repo.url, "git@example.org:acme/widget.git");
    assert_eq!(repo.provider, None);
    assert_eq!(repo.link_mode, Some(LinkMode::LocalOnly));
}

#[test]
fn gitee_https_origin_has_provider() {
    let tmp = repo();
    git(
        tmp.path(),
        &["remote", "add", "origin", "https://gitee.com/acme/widget"],
    );
    let repo = info(tmp.path()).repository().unwrap();
    assert_eq!(repo.provider, Some(RepositoryProvider::Gitee));
}

#[test]
fn subfolder_of_a_work_tree_is_not_a_repo_root() {
    let tmp = repo();
    match collect(&tmp.path().join("sub")) {
        GitStatus::Subfolder { toplevel } => {
            assert_eq!(toplevel, tmp.path().canonicalize().unwrap());
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn plain_folder_is_not_a_repo() {
    let tmp = tempfile::tempdir().unwrap();
    // A folder under the temp dir, outside any work tree (unless the temp dir is in one).
    assert!(matches!(
        collect(tmp.path()),
        GitStatus::NotARepo | GitStatus::Subfolder { .. }
    ));
}

#[test]
fn repo_without_commits_is_not_a_repo() {
    let tmp = tempfile::tempdir().unwrap();
    git(tmp.path(), &["init", "-q"]);
    assert_eq!(collect(tmp.path()), GitStatus::NotARepo);
}

#[test]
fn missing_git_is_unavailable() {
    let tmp = repo();
    assert_eq!(
        collect_with(OsStr::new("archalign-no-such-git-binary"), tmp.path()),
        GitStatus::Unavailable
    );
}

// ---------- FR-SEC-07 ----------

#[test]
fn sanitize_strips_https_userinfo_and_keeps_the_rest() {
    let cases = [
        (
            "https://user:pass@github.com/a/b.git",
            "https://github.com/a/b.git",
        ),
        ("https://token@github.com/a/b", "https://github.com/a/b"),
        (
            "https://u:p@git.example.com:8443/team/repo.git",
            "https://git.example.com:8443/team/repo.git",
        ),
        (
            "http://u:p@host.local:8080/r.git",
            "http://host.local:8080/r.git",
        ),
        ("https://u:p%40x@host/path@v1", "https://host/path@v1"),
        ("https://github.com/a/b.git", "https://github.com/a/b.git"),
    ];
    for (input, expected) in cases {
        assert_eq!(sanitize_origin(input), expected, "{input}");
    }
}

#[test]
fn sanitize_leaves_ssh_and_scp_style_untouched() {
    for url in [
        "ssh://git@github.com/a/b.git",
        "git@github.com:a/b.git",
        "file:///srv/repo.git",
        "/srv/repo.git",
    ] {
        assert_eq!(sanitize_origin(url), url);
    }
}

// ---------- FR-SEC-08 ----------

#[test]
fn read_commands_are_fixed_argv_with_no_optional_locks_first() {
    for cmd in [
        GitRead::TopLevel,
        GitRead::Head,
        GitRead::Origin,
        GitRead::Status,
    ] {
        let argv = read_argv(cmd);
        assert_eq!(argv.first(), Some(&"--no-optional-locks"), "{cmd:?}");
        assert!(
            !argv
                .iter()
                .any(|a| *a == "sh" || *a == "-c" || a.contains(' ')),
            "{cmd:?}: {argv:?}"
        );
    }
    assert_eq!(
        read_argv(GitRead::Status),
        ["--no-optional-locks", "status", "--porcelain=v1", "-z"]
    );
    assert_eq!(
        read_argv(GitRead::Origin),
        [
            "--no-optional-locks",
            "config",
            "--get",
            "remote.origin.url"
        ]
    );
}

#[test]
fn porcelain_z_parsing_takes_the_new_path_of_renames() {
    let out = b" M a.txt\0R  new.txt\0old.txt\0?? dir/\0C  copy.txt\0orig.txt\0 D gone.txt\0";
    assert_eq!(
        parse_porcelain_z(out),
        ["a.txt", "new.txt", "dir/", "copy.txt", "gone.txt"]
    );
    assert!(parse_porcelain_z(b"").is_empty());
    assert!(parse_porcelain_z(b"X\0").is_empty());
}
