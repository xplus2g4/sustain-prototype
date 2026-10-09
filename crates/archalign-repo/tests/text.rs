//! FR-SRC-06 (limits), FR-SRC-08 (confinement) and the line index.

use std::fs;
use std::os::unix::fs::symlink;

use archalign_repo::{
    LineIndex, MAX_VIEW_BYTES, RelPath, Sandbox, SandboxError, TextLoad, load_text,
};
use tempfile::TempDir;

fn setup() -> (TempDir, Sandbox) {
    let tmp = tempfile::tempdir().unwrap();
    fs::create_dir(tmp.path().join("repo")).unwrap();
    let sandbox = Sandbox::new(tmp.path().join("repo")).unwrap();
    (tmp, sandbox)
}

fn write(sandbox: &Sandbox, rel: &str, bytes: &[u8]) -> RelPath {
    fs::write(sandbox.root().join(rel), bytes).unwrap();
    RelPath::parse(rel).unwrap()
}

#[test]
fn exactly_two_megabytes_is_text() {
    let (_tmp, sandbox) = setup();
    let rel = write(&sandbox, "big.txt", &vec![b'a'; MAX_VIEW_BYTES as usize]);
    match load_text(&sandbox, &rel).unwrap() {
        TextLoad::Text { size, index, .. } => {
            assert_eq!(size, MAX_VIEW_BYTES);
            assert_eq!(index.line_count(), 1);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn one_byte_over_two_megabytes_is_too_large() {
    let (_tmp, sandbox) = setup();
    let rel = write(
        &sandbox,
        "big.txt",
        &vec![b'a'; MAX_VIEW_BYTES as usize + 1],
    );
    assert_eq!(
        load_text(&sandbox, &rel).unwrap(),
        TextLoad::TooLarge {
            size: MAX_VIEW_BYTES + 1
        }
    );
}

#[test]
fn nul_in_first_8_kb_is_binary() {
    let (_tmp, sandbox) = setup();
    let mut bytes = vec![b'a'; 10_000];
    bytes[8 * 1024 - 1] = 0;
    let rel = write(&sandbox, "bin.dat", &bytes);
    assert_eq!(
        load_text(&sandbox, &rel).unwrap(),
        TextLoad::Binary { size: 10_000 }
    );
}

#[test]
fn nul_after_first_8_kb_is_text() {
    let (_tmp, sandbox) = setup();
    let mut bytes = vec![b'a'; 10_000];
    bytes[8 * 1024] = 0;
    let rel = write(&sandbox, "late.dat", &bytes);
    assert!(matches!(
        load_text(&sandbox, &rel).unwrap(),
        TextLoad::Text { lossy: false, .. }
    ));
}

#[test]
fn invalid_utf8_is_decoded_lossily() {
    let (_tmp, sandbox) = setup();
    let rel = write(&sandbox, "latin1.txt", b"caf\xe9\nok\n");
    match load_text(&sandbox, &rel).unwrap() {
        TextLoad::Text {
            text, lossy, index, ..
        } => {
            assert!(lossy);
            assert_eq!(index.line(&text, 0), "caf\u{FFFD}");
            assert_eq!(index.line(&text, 1), "ok");
        }
        other => panic!("{other:?}"),
    }
    let rel = write(&sandbox, "utf8.txt", "café\n".as_bytes());
    assert!(matches!(
        load_text(&sandbox, &rel).unwrap(),
        TextLoad::Text { lossy: false, .. }
    ));
}

#[test]
fn reads_go_through_the_sandbox() {
    let (tmp, sandbox) = setup();
    fs::write(tmp.path().join("secret.txt"), "secret").unwrap();
    symlink(
        tmp.path().join("secret.txt"),
        sandbox.root().join("leak.txt"),
    )
    .unwrap();
    assert!(matches!(
        load_text(&sandbox, &RelPath::parse("leak.txt").unwrap()),
        Err(SandboxError::Escapes(_))
    ));
    assert!(matches!(
        load_text(&sandbox, &RelPath::parse("missing.txt").unwrap()),
        Err(SandboxError::NotFound(_))
    ));
    assert!(
        load_text(&sandbox, &RelPath::root()).is_err(),
        "folders are not text"
    );
}

#[test]
fn line_count_semantics() {
    for (text, n) in [
        ("", 0),
        ("a", 1),
        ("a\n", 1),
        ("a\nb", 2),
        ("a\n\n", 2),
        ("\n", 1),
        ("a\r\nb\r\n", 2),
    ] {
        assert_eq!(LineIndex::new(text).line_count(), n, "{text:?}");
        assert_eq!(
            archalign_repo::text::count_lines(text.as_bytes()),
            n,
            "{text:?}"
        );
    }
}

#[test]
fn lines_strip_lf_and_crlf() {
    let text = "one\r\ntwo\nthree";
    let index = LineIndex::new(text);
    assert_eq!(index.line(text, 0), "one");
    assert_eq!(index.line(text, 1), "two");
    assert_eq!(index.line(text, 2), "three");
    assert_eq!(index.line(text, 3), "", "out of range is empty");
    assert_eq!(index.range(text, 0, 1), "one\r\ntwo");
    assert_eq!(index.range(text, 1, 99), "two\nthree", "last is clamped");
    assert_eq!(index.range(text, 2, 1), "");
    assert_eq!(index.range(text, 5, 9), "");
}

#[test]
fn empty_and_blank_lines() {
    let empty = LineIndex::new("");
    assert_eq!(empty.line("", 0), "");
    assert_eq!(empty.range("", 0, 0), "");

    let text = "a\n\nb\n";
    let index = LineIndex::new(text);
    assert_eq!(index.line_count(), 3);
    assert_eq!(index.line(text, 1), "");
    assert_eq!(index.line(text, 2), "b");
    assert_eq!(index.range(text, 0, 2), "a\n\nb");
}

#[test]
fn mismatched_text_never_panics() {
    let index = LineIndex::new("long line here\nsecond\n");
    assert_eq!(index.line("short", 1), "");
    assert_eq!(index.range("é", 0, 1), "");
}
