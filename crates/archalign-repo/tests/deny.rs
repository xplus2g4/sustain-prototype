//! FR-SEC-02: the agent deny-list.

use std::fs;
use std::os::unix::fs::symlink;

use archalign_repo::{DenyList, RelPath, Sandbox};

fn denied(path: &str) -> bool {
    DenyList::new().is_denied(&RelPath::parse(path).unwrap())
}

#[test]
fn git_and_archify_folders_are_denied_at_any_depth() {
    for path in [
        ".git",
        ".git/HEAD",
        "a/.git/HEAD",
        "vendor/lib/.git/config",
        ".archify",
        ".archify/architecture-x-20261010-093000/candidate.json",
        "sub/.archify/x",
    ] {
        assert!(denied(path), "{path}");
    }
    for path in [
        ".gitignore",
        ".github/workflows/ci.yml",
        "git/x",
        "archify/x",
        "a.git",
    ] {
        assert!(!denied(path), "{path}");
    }
}

#[test]
fn env_files_are_denied_except_examples() {
    for path in [
        ".env",
        ".env.local",
        ".env.production",
        "config/.env",
        "apps/web/.env.development",
    ] {
        assert!(denied(path), "{path}");
    }
    for path in [
        ".env.example",
        ".env.sample",
        "config/.env.example",
        "apps/web/.env.sample",
        ".envrc",
        "env",
        "src/env.rs",
    ] {
        assert!(!denied(path), "{path}");
    }
}

#[test]
fn key_extensions_are_denied_case_insensitively() {
    for path in [
        "server.pem",
        "keys/server.PEM",
        "tls/private.key",
        "a/b/cert.p12",
        "cert.PFX",
        "Cert.Key",
    ] {
        assert!(denied(path), "{path}");
    }
    for path in [
        "keys/README.md",
        "src/keyboard.rs",
        "pem.txt",
        "server.pem.txt",
        "key",
        "docs/p12.md",
    ] {
        assert!(!denied(path), "{path}");
    }
}

#[test]
fn ssh_key_names_are_denied() {
    for path in [
        "id_rsa",
        "id_rsa.pub",
        "home/.ssh/id_rsa_old",
        "id_ed25519",
        "deploy/id_ed25519.pub",
    ] {
        assert!(denied(path), "{path}");
    }
    for path in ["my_id_rsa", "docs/rsa.md", "id_ecdsa", "rsa_id"] {
        assert!(!denied(path), "{path}");
    }
}

#[test]
fn credential_rc_files_are_denied() {
    for path in [
        ".npmrc",
        ".pypirc",
        ".netrc",
        "packages/app/.npmrc",
        "x/.netrc",
    ] {
        assert!(denied(path), "{path}");
    }
    for path in ["npmrc", ".npmrc.md", ".yarnrc", "netrc.rs"] {
        assert!(!denied(path), "{path}");
    }
}

#[test]
fn root_and_ordinary_paths_are_allowed() {
    assert!(!DenyList::new().is_denied(&RelPath::root()));
    assert!(!denied("src/main.rs"));
}

#[test]
fn string_form_treats_unparsable_paths_as_denied() {
    let deny = DenyList::new();
    assert!(deny.is_denied_str(".env"));
    assert!(deny.is_denied_str("./config//.env"));
    assert!(deny.is_denied_str("../secret"));
    assert!(deny.is_denied_str("/etc/passwd"));
    assert!(deny.is_denied_str("a\\b"));
    assert!(deny.is_denied_str("a\0b"));
    assert!(!deny.is_denied_str("src/main.rs"));
    assert!(!deny.is_denied_str(""));
}

#[test]
fn links_to_denied_targets_are_denied() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("repo");
    fs::create_dir_all(root.join("config")).unwrap();
    fs::write(root.join("config/.env"), "TOKEN=1\n").unwrap();
    fs::write(root.join("notes.txt"), "hi\n").unwrap();
    symlink(root.join("config/.env"), root.join("innocent.txt")).unwrap();
    symlink(root.join("config"), root.join("cfg")).unwrap();
    let sandbox = Sandbox::new(&root).unwrap();
    let deny = DenyList::new();
    let rel = |s: &str| RelPath::parse(s).unwrap();
    assert!(deny.is_denied_in(&sandbox, &rel("innocent.txt")));
    assert!(deny.is_denied_in(&sandbox, &rel("cfg/.env")));
    assert!(!deny.is_denied_in(&sandbox, &rel("notes.txt")));
    assert!(!deny.is_denied_in(&sandbox, &rel("cfg")));
    assert!(!deny.is_denied_in(&sandbox, &rel("missing.txt")));
}
