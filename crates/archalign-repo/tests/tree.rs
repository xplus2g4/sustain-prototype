//! FR-TREE-01 (ordering), FR-TREE-02 (filtering) and FR-TREE-08 (symlinks).
//!
//! File names avoid patterns a developer's global gitignore is likely to contain.

use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;

use archalign_repo::{EntryKind, RelPath, Sandbox, TreeOptions, is_ignored, list_dir};
use tempfile::TempDir;

const SHOWN: TreeOptions = TreeOptions { show_hidden: false };
const HIDDEN_TOO: TreeOptions = TreeOptions { show_hidden: true };

fn setup() -> (TempDir, Sandbox) {
    let tmp = tempfile::tempdir().unwrap();
    fs::create_dir(tmp.path().join("repo")).unwrap();
    let sandbox = Sandbox::new(tmp.path().join("repo")).unwrap();
    (tmp, sandbox)
}

fn touch(root: &Path, rel: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, rel).unwrap();
}

fn names(sandbox: &Sandbox, dir: &str, opts: &TreeOptions) -> Vec<String> {
    list_dir(sandbox, &RelPath::parse(dir).unwrap(), opts)
        .unwrap()
        .into_iter()
        .map(|e| e.name)
        .collect()
}

fn rel(s: &str) -> RelPath {
    RelPath::parse(s).unwrap()
}

#[test]
fn folders_first_then_files_case_insensitively() {
    let (_tmp, sandbox) = setup();
    let root = sandbox.root();
    // macOS file systems are usually case-insensitive, so no case-only duplicates here;
    // the byte-order tie-break is unit-tested in `tree.rs`.
    for f in ["b.txt", "A.txt", "C.txt", "zeta/x", "Alpha/x", "beta/x"] {
        touch(root, f);
    }
    assert_eq!(
        names(&sandbox, "", &SHOWN),
        ["Alpha", "beta", "zeta", "A.txt", "b.txt", "C.txt"]
    );
    let entries = list_dir(&sandbox, &RelPath::root(), &SHOWN).unwrap();
    assert_eq!(entries[0].kind, EntryKind::Dir);
    assert_eq!(entries[0].rel.as_str(), "Alpha");
    assert_eq!(entries[0].size, None);
    let a = entries.iter().find(|e| e.name == "A.txt").unwrap();
    assert_eq!(a.kind, EntryKind::File);
    assert_eq!(a.size, Some(5));
}

#[test]
fn lists_one_level_of_a_subfolder() {
    let (_tmp, sandbox) = setup();
    touch(sandbox.root(), "src/core/deep.rs");
    touch(sandbox.root(), "src/main.rs");
    let entries = list_dir(&sandbox, &rel("src"), &SHOWN).unwrap();
    let rels: Vec<&str> = entries.iter().map(|e| e.rel.as_str()).collect();
    assert_eq!(rels, ["src/core", "src/main.rs"]);
}

#[test]
fn root_and_nested_gitignore_apply() {
    let (_tmp, sandbox) = setup();
    let root = sandbox.root();
    fs::write(root.join(".gitignore"), "dropped-root.qq\nbuildout/\n").unwrap();
    touch(root, "kept.qq");
    touch(root, "dropped-root.qq");
    touch(root, "buildout/artifact");
    touch(root, "sub/kept.qq");
    touch(root, "sub/dropped-nested.qq");
    touch(root, "sub/dropped-root.qq");
    fs::write(root.join("sub/.gitignore"), "dropped-nested.qq\n").unwrap();
    assert_eq!(names(&sandbox, "", &SHOWN), ["sub", "kept.qq"]);
    assert_eq!(names(&sandbox, "sub", &SHOWN), ["kept.qq"]);

    assert!(is_ignored(&sandbox, &rel("dropped-root.qq"), false, &SHOWN));
    assert!(is_ignored(
        &sandbox,
        &rel("sub/dropped-root.qq"),
        false,
        &SHOWN
    ));
    assert!(is_ignored(
        &sandbox,
        &rel("sub/dropped-nested.qq"),
        false,
        &SHOWN
    ));
    assert!(is_ignored(&sandbox, &rel("buildout"), true, &SHOWN));
    assert!(is_ignored(
        &sandbox,
        &rel("buildout/artifact"),
        false,
        &SHOWN
    ));
    assert!(!is_ignored(&sandbox, &rel("sub/kept.qq"), false, &SHOWN));
    assert!(!is_ignored(&sandbox, &rel("kept.qq"), false, &SHOWN));
}

#[test]
fn nested_gitignore_can_reinclude() {
    let (_tmp, sandbox) = setup();
    let root = sandbox.root();
    fs::write(root.join(".gitignore"), "*.gen\n").unwrap();
    fs::create_dir(root.join("sub")).unwrap();
    fs::write(root.join("sub/.gitignore"), "!keep.gen\n").unwrap();
    touch(root, "sub/keep.gen");
    touch(root, "sub/drop.gen");
    assert_eq!(names(&sandbox, "sub", &SHOWN), ["keep.gen"]);
    assert!(!is_ignored(&sandbox, &rel("sub/keep.gen"), false, &SHOWN));
    assert!(is_ignored(&sandbox, &rel("sub/drop.gen"), false, &SHOWN));
}

#[test]
fn git_info_exclude_applies() {
    let (_tmp, sandbox) = setup();
    let root = sandbox.root();
    fs::create_dir_all(root.join(".git/info")).unwrap();
    fs::write(root.join(".git/info/exclude"), "excluded.qq\n").unwrap();
    touch(root, "excluded.qq");
    touch(root, "kept.qq");
    assert_eq!(names(&sandbox, "", &SHOWN), ["kept.qq"]);
    assert!(is_ignored(&sandbox, &rel("excluded.qq"), false, &SHOWN));
}

#[test]
fn dot_git_is_never_listed() {
    let (_tmp, sandbox) = setup();
    let root = sandbox.root();
    fs::create_dir_all(root.join(".git/objects")).unwrap();
    touch(root, "kept.qq");
    assert_eq!(names(&sandbox, "", &HIDDEN_TOO), ["kept.qq"]);
    assert!(is_ignored(&sandbox, &rel(".git"), true, &HIDDEN_TOO));
    assert!(is_ignored(&sandbox, &rel(".git/HEAD"), false, &HIDDEN_TOO));
}

#[test]
fn dotfiles_follow_the_show_hidden_toggle() {
    let (_tmp, sandbox) = setup();
    let root = sandbox.root();
    touch(root, ".hidden.qq");
    touch(root, ".config/x.qq");
    touch(root, "kept.qq");
    assert_eq!(names(&sandbox, "", &SHOWN), ["kept.qq"]);
    assert_eq!(
        names(&sandbox, "", &HIDDEN_TOO),
        [".config", ".hidden.qq", "kept.qq"]
    );
    assert!(is_ignored(&sandbox, &rel(".hidden.qq"), false, &SHOWN));
    assert!(is_ignored(&sandbox, &rel(".config/x.qq"), false, &SHOWN));
    assert!(!is_ignored(
        &sandbox,
        &rel(".hidden.qq"),
        false,
        &HIDDEN_TOO
    ));
    assert!(!is_ignored(&sandbox, &RelPath::root(), true, &SHOWN));
}

#[test]
fn symlinks_are_marked_with_outside_flag() {
    let (tmp, sandbox) = setup();
    let root = sandbox.root();
    touch(root, "src/a.rs");
    touch(tmp.path(), "outside/secret.qq");
    symlink("src", root.join("in-dir")).unwrap();
    symlink("src/a.rs", root.join("in-file")).unwrap();
    symlink(tmp.path().join("outside"), root.join("out-dir")).unwrap();
    symlink(tmp.path().join("outside/secret.qq"), root.join("out-file")).unwrap();
    symlink("nowhere", root.join("zz-dangling")).unwrap();
    let entries = list_dir(&sandbox, &RelPath::root(), &SHOWN).unwrap();
    let kinds: Vec<(&str, EntryKind)> = entries.iter().map(|e| (e.name.as_str(), e.kind)).collect();
    let link = |target_is_dir, outside_root| EntryKind::Symlink {
        target_is_dir,
        outside_root,
    };
    assert_eq!(
        kinds,
        [
            ("in-dir", link(true, false)),
            ("out-dir", link(true, true)),
            ("src", EntryKind::Dir),
            ("in-file", link(false, false)),
            ("out-file", link(false, true)),
            ("zz-dangling", link(false, true)),
        ]
    );
    // An outside link can't be expanded: listing it fails.
    assert!(list_dir(&sandbox, &rel("out-dir"), &SHOWN).is_err());
    // An inside link to a folder lists under the link's path.
    let inner = list_dir(&sandbox, &rel("in-dir"), &SHOWN).unwrap();
    assert_eq!(inner[0].rel.as_str(), "in-dir/a.rs");
}

#[test]
fn listing_a_file_or_missing_folder_fails() {
    let (_tmp, sandbox) = setup();
    touch(sandbox.root(), "a.qq");
    assert!(list_dir(&sandbox, &rel("a.qq"), &SHOWN).is_err());
    assert!(list_dir(&sandbox, &rel("missing"), &SHOWN).is_err());
}
