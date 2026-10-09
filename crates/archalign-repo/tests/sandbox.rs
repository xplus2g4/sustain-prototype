//! FR-SEC-01: root confinement.

use std::fs;
use std::os::unix::fs::symlink;
use std::path::PathBuf;

use archalign_repo::{RelPath, Sandbox, SandboxError};
use tempfile::TempDir;

/// `<tmp>/repo` (the root, with `src/a.rs`) next to `<tmp>/outside` (`secret.txt`, `dir/`).
fn setup() -> (TempDir, Sandbox, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    let outside = tmp.path().join("outside");
    fs::create_dir_all(repo.join("src")).unwrap();
    fs::create_dir_all(outside.join("dir")).unwrap();
    fs::write(repo.join("src/a.rs"), "fn main() {}\n").unwrap();
    fs::write(outside.join("secret.txt"), "secret").unwrap();
    fs::write(outside.join("dir/inner.txt"), "inner").unwrap();
    let sandbox = Sandbox::new(&repo).unwrap();
    (tmp, sandbox, outside)
}

#[test]
fn parse_normalizes_and_rejects_bad_paths() {
    assert_eq!(RelPath::parse("").unwrap(), RelPath::root());
    assert_eq!(RelPath::parse("./src//a.rs/").unwrap().as_str(), "src/a.rs");
    for bad in [
        "../x",
        "a/../b",
        "..",
        "/etc/passwd",
        "C:\\x",
        "C:/x",
        "a\\b",
        "a\0b",
    ] {
        assert!(
            matches!(RelPath::parse(bad), Err(SandboxError::InvalidPath(_))),
            "{bad:?}"
        );
    }
}

#[test]
fn rel_path_helpers() {
    let p = RelPath::parse("src/core/lib.rs").unwrap();
    assert_eq!(p.file_name(), Some("lib.rs"));
    assert_eq!(p.parent().unwrap().as_str(), "src/core");
    assert_eq!(
        p.parent().unwrap().parent().unwrap().parent(),
        Some(RelPath::root())
    );
    assert_eq!(RelPath::root().parent(), None);
    assert_eq!(RelPath::root().file_name(), None);
    assert_eq!(
        p.components().collect::<Vec<_>>(),
        ["src", "core", "lib.rs"]
    );
    assert_eq!(RelPath::root().components().count(), 0);
    assert_eq!(RelPath::root().join("a/b").unwrap().as_str(), "a/b");
    assert_eq!(
        p.parent().unwrap().join("x.rs").unwrap().as_str(),
        "src/core/x.rs"
    );
    assert!(p.join("..").is_err());
    assert_eq!(p.to_string(), "src/core/lib.rs");
    let json = serde_json::to_string(&p).unwrap();
    assert_eq!(json, "\"src/core/lib.rs\"");
    assert_eq!(serde_json::from_str::<RelPath>(&json).unwrap(), p);
    assert!(serde_json::from_str::<RelPath>("\"../x\"").is_err());
}

#[test]
fn resolves_paths_inside_the_root() {
    let (_tmp, sandbox, _) = setup();
    let abs = sandbox.resolve_str("src/a.rs").unwrap();
    assert!(abs.starts_with(sandbox.root()));
    assert_eq!(sandbox.resolve(&RelPath::root()).unwrap(), sandbox.root());
    assert_eq!(sandbox.relativize(&abs).unwrap().as_str(), "src/a.rs");
}

#[test]
fn dot_dot_escape_is_rejected() {
    let (_tmp, sandbox, _) = setup();
    assert!(matches!(
        sandbox.resolve_str("../outside/secret.txt"),
        Err(SandboxError::InvalidPath(_))
    ));
    assert!(matches!(
        sandbox.resolve_str("src/../../outside/secret.txt"),
        Err(SandboxError::InvalidPath(_))
    ));
}

#[test]
fn absolute_path_is_rejected() {
    let (_tmp, sandbox, outside) = setup();
    let abs = outside.join("secret.txt");
    assert!(matches!(
        sandbox.resolve_str(abs.to_str().unwrap()),
        Err(SandboxError::InvalidPath(_))
    ));
    assert!(sandbox.relativize(&abs).is_none());
}

#[test]
fn symlink_to_file_outside_is_rejected() {
    let (_tmp, sandbox, outside) = setup();
    symlink(outside.join("secret.txt"), sandbox.root().join("leak.txt")).unwrap();
    let rel = RelPath::parse("leak.txt").unwrap();
    assert!(matches!(
        sandbox.resolve(&rel),
        Err(SandboxError::Escapes(_))
    ));
    assert!(sandbox.is_outside_link(&rel));
}

#[test]
fn symlink_to_folder_outside_is_rejected() {
    let (_tmp, sandbox, outside) = setup();
    symlink(outside.join("dir"), sandbox.root().join("leakdir")).unwrap();
    let dir = RelPath::parse("leakdir").unwrap();
    assert!(matches!(
        sandbox.resolve(&dir),
        Err(SandboxError::Escapes(_))
    ));
    assert!(matches!(
        sandbox.resolve_str("leakdir/inner.txt"),
        Err(SandboxError::Escapes(_))
    ));
    assert!(sandbox.is_outside_link(&dir));
}

#[test]
fn symlink_inside_root_is_accepted() {
    let (_tmp, sandbox, _) = setup();
    symlink("src/a.rs", sandbox.root().join("alias.rs")).unwrap();
    symlink("src", sandbox.root().join("srclink")).unwrap();
    let rel = RelPath::parse("alias.rs").unwrap();
    assert_eq!(
        sandbox.resolve(&rel).unwrap(),
        sandbox.root().join("src/a.rs")
    );
    assert!(!sandbox.is_outside_link(&rel));
    assert_eq!(
        sandbox.resolve_str("srclink/a.rs").unwrap(),
        sandbox.root().join("src/a.rs")
    );
}

#[test]
fn dangling_symlink_is_not_found_and_counts_as_outside() {
    let (_tmp, sandbox, _) = setup();
    symlink("nowhere.rs", sandbox.root().join("dangling.rs")).unwrap();
    let rel = RelPath::parse("dangling.rs").unwrap();
    assert!(matches!(
        sandbox.resolve(&rel),
        Err(SandboxError::NotFound(_))
    ));
    assert!(sandbox.is_outside_link(&rel));
}

#[test]
fn missing_path_is_not_found_and_root_must_be_a_folder() {
    let (_tmp, sandbox, outside) = setup();
    assert!(matches!(
        sandbox.resolve_str("src/missing.rs"),
        Err(SandboxError::NotFound(_))
    ));
    assert!(Sandbox::new(outside.join("secret.txt")).is_err());
    assert!(Sandbox::new(outside.join("nope")).is_err());
}

#[test]
fn sibling_with_shared_prefix_is_outside() {
    let tmp = tempfile::tempdir().unwrap();
    fs::create_dir_all(tmp.path().join("repo")).unwrap();
    fs::create_dir_all(tmp.path().join("repo2")).unwrap();
    fs::write(tmp.path().join("repo2/x"), "x").unwrap();
    symlink("../repo2/x", tmp.path().join("repo/x")).unwrap();
    let sandbox = Sandbox::new(tmp.path().join("repo")).unwrap();
    assert!(matches!(
        sandbox.resolve_str("x"),
        Err(SandboxError::Escapes(_))
    ));
}
