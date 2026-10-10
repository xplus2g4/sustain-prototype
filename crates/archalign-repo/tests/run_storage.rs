//! FR-STO-01 (slug, timestamp, run folders), FR-STO-02 (atomic writes, appends) and
//! FR-WAT-08 (self-write registry).

use std::fs::{self, File};
use std::os::unix::fs::symlink;
use std::time::{Duration, SystemTime};

use archalign_repo::storage::folder_timestamp;
use archalign_repo::{
    RelPath, SLUG_MAX, Sandbox, SandboxError, SelfWrites, append_line, atomic_write_tracked,
    create_run_folder, now_rfc3339, now_run_timestamp, run_timestamp, slugify,
};
use jiff::civil::date;
use jiff::tz::{TimeZone, offset};

fn setup() -> (tempfile::TempDir, Sandbox) {
    let tmp = tempfile::tempdir().unwrap();
    fs::create_dir(tmp.path().join("repo")).unwrap();
    let sandbox = Sandbox::new(tmp.path().join("repo")).unwrap();
    (tmp, sandbox)
}

fn rel(s: &str) -> RelPath {
    RelPath::parse(s).unwrap()
}

#[test]
fn slug_from_focus_words() {
    assert_eq!(
        slugify("Payment service: retries & queues", "my-repo"),
        "payment-service-retries-queues"
    );
    assert_eq!(slugify("  Hello,   World!! ", "x"), "hello-world");
    assert_eq!(slugify("V2 API", "x"), "v2-api");
    assert_eq!(slugify("snake_case/and.dots", "x"), "snake-case-and-dots");
}

#[test]
fn slug_turns_unicode_into_dashes() {
    assert_eq!(slugify("Café déjà vu", "x"), "caf-d-j-vu");
    assert_eq!(slugify("日本 auth 服务", "x"), "auth");
}

#[test]
fn slug_is_capped_at_40_on_a_word_boundary() {
    let slug = slugify(
        "The quick brown fox jumps over the lazy dog again and again",
        "x",
    );
    assert_eq!(slug, "the-quick-brown-fox-jumps-over-the-lazy");
    let long_word = slugify(&"a".repeat(50), "x");
    assert_eq!(long_word, "a".repeat(SLUG_MAX));
    let exact = slugify(&format!("{} bbb", "a".repeat(40)), "x");
    assert_eq!(exact, "a".repeat(40));
    for slug in [slug, long_word, exact] {
        assert!(slug.len() <= SLUG_MAX);
        assert!(!slug.ends_with('-'));
        assert!(
            slug.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        );
    }
}

#[test]
fn slug_falls_back_to_folder_name_then_repo() {
    assert_eq!(slugify("", "My Repo!"), "my-repo");
    assert_eq!(slugify("   ", "archalign"), "archalign");
    assert_eq!(slugify("!!! ???", "Backend_API"), "backend-api");
    assert_eq!(slugify("", ""), "repo");
    assert_eq!(slugify("日本語", "中文"), "repo");
}

#[test]
fn timestamp_is_local_wall_clock() {
    let utc = date(2026, 10, 10)
        .at(9, 30, 5, 0)
        .to_zoned(TimeZone::UTC)
        .unwrap();
    assert_eq!(run_timestamp(&utc), "20261010-093005");
    let local = "2026-10-10T01:30:05Z"
        .parse::<jiff::Timestamp>()
        .unwrap()
        .to_zoned(TimeZone::fixed(offset(8)));
    assert_eq!(run_timestamp(&local), "20261010-093005");
    assert!(
        regex::Regex::new(r"^\d{8}-\d{6}$")
            .unwrap()
            .is_match(&now_run_timestamp())
    );
}

#[test]
fn now_rfc3339_has_offset_and_second_precision() {
    let now = now_rfc3339();
    let shape = regex::Regex::new(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}[+-]\d{2}:\d{2}$").unwrap();
    assert!(shape.is_match(&now), "{now}");
    assert!(now.parse::<jiff::Timestamp>().is_ok(), "{now}");
}

#[test]
fn run_folder_is_created_with_archify() {
    let (_tmp, sandbox) = setup();
    let run = create_run_folder(&sandbox, "api", "20261010-093000").unwrap();
    assert_eq!(run.name, "architecture-api-20261010-093000");
    assert_eq!(
        run.rel.as_str(),
        ".archify/architecture-api-20261010-093000"
    );
    assert_eq!(run.slug, "api");
    assert!(sandbox.root().join(".archify").is_dir());
    assert!(sandbox.root().join(run.rel.as_str()).is_dir());
}

#[test]
fn run_folder_collisions_get_numbered_suffixes() {
    let (_tmp, sandbox) = setup();
    let names: Vec<String> = (0..3)
        .map(|_| {
            create_run_folder(&sandbox, "api", "20261010-093000")
                .unwrap()
                .name
        })
        .collect();
    assert_eq!(
        names,
        [
            "architecture-api-20261010-093000",
            "architecture-api-20261010-093000-2",
            "architecture-api-20261010-093000-3",
        ]
    );
    for name in &names {
        assert!(sandbox.root().join(".archify").join(name).is_dir());
        assert_eq!(folder_timestamp(name).as_deref(), Some("20261010-093000"));
    }
}

#[test]
fn run_folder_fails_when_archify_is_unusable() {
    let (tmp, sandbox) = setup();
    fs::create_dir(tmp.path().join("elsewhere")).unwrap();
    symlink(
        tmp.path().join("elsewhere"),
        sandbox.root().join(".archify"),
    )
    .unwrap();
    assert!(matches!(
        create_run_folder(&sandbox, "api", "20261010-093000"),
        Err(SandboxError::Escapes(_))
    ));

    let (_tmp2, file_sandbox) = setup();
    fs::write(file_sandbox.root().join(".archify"), "not a folder").unwrap();
    assert!(create_run_folder(&file_sandbox, "api", "20261010-093000").is_err());

    let (_tmp3, ok) = setup();
    assert!(matches!(
        create_run_folder(&ok, "a/b", "20261010-093000"),
        Err(SandboxError::InvalidPath(_))
    ));
    assert!(matches!(
        create_run_folder(&ok, "..", "x/../.."),
        Err(SandboxError::InvalidPath(_))
    ));
}

#[test]
fn append_line_creates_then_appends() {
    let (_tmp, sandbox) = setup();
    let run = create_run_folder(&sandbox, "api", "20261010-093000").unwrap();
    let log = run.rel.join("run.jsonl").unwrap();
    append_line(&sandbox, &log, r#"{"event":"run_started"}"#).unwrap();
    append_line(&sandbox, &log, r#"{"event":"run_finished"}"#).unwrap();
    assert_eq!(
        fs::read_to_string(sandbox.root().join(log.as_str())).unwrap(),
        "{\"event\":\"run_started\"}\n{\"event\":\"run_finished\"}\n"
    );
}

#[test]
fn append_line_stays_inside_the_root() {
    let (tmp, sandbox) = setup();
    assert!(matches!(
        append_line(&sandbox, &rel("missing/run.jsonl"), "x"),
        Err(SandboxError::NotFound(_))
    ));
    assert!(matches!(
        append_line(&sandbox, &RelPath::root(), "x"),
        Err(SandboxError::InvalidPath(_))
    ));
    fs::write(tmp.path().join("outside.txt"), "").unwrap();
    symlink(
        tmp.path().join("outside.txt"),
        sandbox.root().join("leak.txt"),
    )
    .unwrap();
    assert!(matches!(
        append_line(&sandbox, &rel("leak.txt"), "x"),
        Err(SandboxError::Escapes(_))
    ));
    assert_eq!(
        fs::read_to_string(tmp.path().join("outside.txt")).unwrap(),
        ""
    );
}

#[test]
fn atomic_write_tracked_writes_and_records() {
    let (_tmp, sandbox) = setup();
    let writes = SelfWrites::default();
    let target = rel("candidate.json");
    atomic_write_tracked(&sandbox, &target, b"{}", &writes).unwrap();
    let abs = sandbox.root().join("candidate.json");
    assert_eq!(fs::read(&abs).unwrap(), b"{}");
    assert!(writes.is_own(&abs));
    // Clones share the registry.
    assert!(writes.clone().is_own(&abs));
    // Replacing our own file is ours again.
    atomic_write_tracked(&sandbox, &target, b"{\"a\":1}", &writes).unwrap();
    assert!(writes.is_own(&abs));
    // Only regular files without leftovers in the folder.
    let names: Vec<_> = fs::read_dir(sandbox.root())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names, ["candidate.json"]);
    assert!(atomic_write_tracked(&sandbox, &rel("missing/x.json"), b"{}", &writes).is_err());
}

#[test]
fn self_writes_reject_later_external_changes() {
    let (_tmp, sandbox) = setup();
    let writes = SelfWrites::default();
    let abs = sandbox.root().join("candidate.json");
    atomic_write_tracked(&sandbox, &rel("candidate.json"), b"abc", &writes).unwrap();
    assert!(writes.is_own(&abs));

    // Different length.
    fs::write(&abs, b"abcdef").unwrap();
    assert!(!writes.is_own(&abs));

    // Same length, different mtime.
    atomic_write_tracked(&sandbox, &rel("candidate.json"), b"abc", &writes).unwrap();
    assert!(writes.is_own(&abs));
    fs::write(&abs, b"xyz").unwrap();
    let later = SystemTime::now() + Duration::from_secs(10);
    File::options()
        .write(true)
        .open(&abs)
        .unwrap()
        .set_modified(later)
        .unwrap();
    assert!(!writes.is_own(&abs));
}

#[test]
fn self_writes_ignore_unknown_and_missing_files() {
    let (_tmp, sandbox) = setup();
    let writes = SelfWrites::default();
    let abs = sandbox.root().join("other.txt");
    assert!(!writes.is_own(&abs));
    fs::write(&abs, "x").unwrap();
    assert!(!writes.is_own(&abs));
    writes.record(&abs);
    assert!(writes.is_own(&abs));
    fs::remove_file(&abs).unwrap();
    assert!(!writes.is_own(&abs));
}
