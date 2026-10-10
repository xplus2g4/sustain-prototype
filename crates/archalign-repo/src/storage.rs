//! Diagram storage under `<repo>/.archify/` (FR-IMP-03, FR-STO-01, FR-STO-02, FR-STO-04,
//! FR-WAT-08, ADR 0015).

use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::SystemTime;

use archalign_model::document::parse_json;
use archalign_model::{DiagramType, load_architecture_value};
use serde_json::Value;

use crate::sandbox::{RelPath, Sandbox, SandboxError};

/// The diagram folder at the repo root.
pub const ARCHIFY_DIR: &str = ".archify";
/// The diagram file inside each run folder.
pub const CANDIDATE_FILE: &str = "candidate.json";

/// Whether a history entry can be opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistoryStatus {
    /// Passes the Import profile.
    Valid,
    /// Fails the Import profile.
    Invalid {
        /// The first error, prefixed by its JSON pointer when it has one.
        first_error: String,
    },
    /// A non-architecture Archify diagram.
    Unsupported {
        /// The `diagram_type` value.
        diagram_type: String,
    },
}

/// One `.archify/<folder>/` that contains a `candidate.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    /// Folder name.
    pub folder: String,
    /// `.archify/<folder>/candidate.json`.
    pub rel: RelPath,
    /// `YYYYMMDD-HHMMSS` from the folder-name suffix, if present.
    pub timestamp: Option<String>,
    /// Modification time of `candidate.json`.
    pub modified: Option<SystemTime>,
    /// `meta.title`, when the file has one.
    pub title: Option<String>,
    /// Load status.
    pub status: HistoryStatus,
}

/// Extracts `YYYYMMDD-HHMMSS` from a folder name ending in `-YYYYMMDD-HHMMSS`, optionally
/// followed by a collision suffix `-N`.
pub fn folder_timestamp(folder: &str) -> Option<String> {
    let digits = |s: &str, n: usize| s.len() == n && s.bytes().all(|b| b.is_ascii_digit());
    let mut segs = folder.rsplit('-');
    let last = segs.next()?;
    let prev = segs.next()?;
    let (date, time) = if digits(prev, 8) && digits(last, 6) {
        (prev, last)
    } else if !last.is_empty() && last.bytes().all(|b| b.is_ascii_digit()) {
        let date = segs.next()?;
        if digits(date, 8) && digits(prev, 6) {
            (date, prev)
        } else {
            return None;
        }
    } else {
        return None;
    };
    Some(format!("{date}-{time}"))
}

/// Loads one candidate file and classifies it.
fn classify(bytes: &[u8]) -> (Option<String>, HistoryStatus) {
    let value = match parse_json(bytes) {
        Ok(value) => value,
        Err(issue) => {
            return (
                None,
                HistoryStatus::Invalid {
                    first_error: issue.message,
                },
            );
        }
    };
    let title = value
        .pointer("/meta/title")
        .and_then(Value::as_str)
        .map(str::to_string);
    let ty = DiagramType::detect(&value);
    if !ty.is_supported() && ty != DiagramType::Unknown {
        return (
            title,
            HistoryStatus::Unsupported {
                diagram_type: ty.to_string(),
            },
        );
    }
    let status = match load_architecture_value(value) {
        Ok(_) => HistoryStatus::Valid,
        Err(failure) => {
            let first_error = failure
                .errors
                .first()
                .map(|i| {
                    if i.pointer.is_empty() {
                        i.message.clone()
                    } else {
                        format!("{}: {}", i.pointer, i.message)
                    }
                })
                .unwrap_or_else(|| "invalid diagram".to_string());
            HistoryStatus::Invalid { first_error }
        }
    };
    (title, status)
}

/// Lists `.archify/*/` folders that contain a `candidate.json`, whatever `.gitignore`
/// says, newest first: by timestamp suffix, then modification time, then folder name
/// (all descending). Folders without `candidate.json`, and anything resolving outside
/// the root, are skipped. A missing `.archify` gives an empty list. Status uses the
/// Import profile; citations are not checked.
pub fn scan_history(sandbox: &Sandbox) -> Vec<HistoryEntry> {
    let Ok(base) = RelPath::parse(ARCHIFY_DIR) else {
        return Vec::new();
    };
    let Ok(abs) = sandbox.resolve(&base) else {
        return Vec::new();
    };
    let Ok(read_dir) = fs::read_dir(&abs) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in read_dir.flatten() {
        let Ok(folder) = entry.file_name().into_string() else {
            continue;
        };
        let Ok(rel) = base.join(&folder).and_then(|f| f.join(CANDIDATE_FILE)) else {
            continue;
        };
        let Ok(path) = sandbox.resolve(&rel) else {
            continue;
        };
        let Ok(meta) = path.metadata() else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }
        let (title, status) = match fs::read(&path) {
            Ok(bytes) => classify(&bytes),
            Err(err) => (
                None,
                HistoryStatus::Invalid {
                    first_error: format!("{rel} cannot be read: {err}"),
                },
            ),
        };
        out.push(HistoryEntry {
            timestamp: folder_timestamp(&folder),
            modified: meta.modified().ok(),
            folder,
            rel,
            title,
            status,
        });
    }
    out.sort_by(|a, b| {
        (&b.timestamp, b.modified, &b.folder).cmp(&(&a.timestamp, a.modified, &a.folder))
    });
    out
}

/// The first valid entry of a newest-first list.
pub fn newest_valid(entries: &[HistoryEntry]) -> Option<&HistoryEntry> {
    entries.iter().find(|e| e.status == HistoryStatus::Valid)
}

/// Writes `bytes` to `rel` atomically: a temporary file in the same folder, flushed,
/// then renamed over the target. The parent folder must exist inside the root.
pub fn atomic_write(sandbox: &Sandbox, rel: &RelPath, bytes: &[u8]) -> Result<(), SandboxError> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let (Some(parent), Some(name)) = (rel.parent(), rel.file_name()) else {
        return Err(SandboxError::InvalidPath(
            "cannot write to the repository root".into(),
        ));
    };
    let dir = sandbox.resolve(&parent)?;
    if !dir.is_dir() {
        return Err(SandboxError::InvalidPath(format!(
            "{parent} is not a folder"
        )));
    }
    let target = dir.join(name);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp = dir.join(format!(".{name}.tmp-{}-{n}", std::process::id()));
    let io_err = |e: std::io::Error| SandboxError::Io(format!("{rel}: {e}"));
    let write = || -> std::io::Result<()> {
        let mut file = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&tmp, &target)
    };
    write().map_err(|e| {
        let _ = fs::remove_file(&tmp);
        io_err(e)
    })
}

/// Longest run-folder slug (FR-STO-01).
pub const SLUG_MAX: usize = 40;

/// The run-folder slug (FR-STO-01): lowercase ASCII kebab-case (`[a-z0-9-]`) of the
/// Focus text's first words, or of the repo folder name when that gives nothing, or
/// `repo`. Every run of other characters (punctuation, spaces, non-ASCII letters)
/// becomes one `-`; leading and trailing dashes are dropped. Over [`SLUG_MAX`]
/// characters it is cut at the last word boundary that fits (or hard-cut when the
/// first word alone is longer).
pub fn slugify(focus: &str, repo_folder_name: &str) -> String {
    [focus, repo_folder_name]
        .into_iter()
        .map(kebab)
        .find(|slug| !slug.is_empty())
        .unwrap_or_else(|| "repo".to_string())
}

fn kebab(text: &str) -> String {
    let mut out = String::with_capacity(SLUG_MAX + 1);
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
        if out.len() > SLUG_MAX {
            break;
        }
    }
    // `out` is ASCII, so byte offsets are character offsets.
    if out.len() > SLUG_MAX {
        let cut = if out.as_bytes().get(SLUG_MAX) == Some(&b'-') {
            SLUG_MAX
        } else {
            out.get(..SLUG_MAX)
                .and_then(|head| head.rfind('-'))
                .unwrap_or(SLUG_MAX)
        };
        out.truncate(cut);
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

/// `YYYYMMDD-HHMMSS` of `now` in its own time zone (FR-STO-01).
pub fn run_timestamp(now: &jiff::Zoned) -> String {
    now.strftime("%Y%m%d-%H%M%S").to_string()
}

/// [`run_timestamp`] of the current local time.
pub fn now_run_timestamp() -> String {
    run_timestamp(&jiff::Zoned::now())
}

/// The current local time as RFC 3339 with the local UTC offset, to the second
/// (`2026-10-10T10:00:00+08:00`).
pub fn now_rfc3339() -> String {
    jiff::Zoned::now()
        .strftime("%Y-%m-%dT%H:%M:%S%:z")
        .to_string()
}

/// A newly created run folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunFolder {
    /// `architecture-<slug>-<timestamp>`, plus `-N` after a collision.
    pub name: String,
    /// `.archify/<name>`.
    pub rel: RelPath,
    /// The slug used.
    pub slug: String,
}

/// Most `-N` suffixes tried before giving up.
const MAX_COLLISIONS: u32 = 1000;

/// Creates `.archify/` (if needed) and a fresh `.archify/architecture-<slug>-<timestamp>/`
/// (FR-STO-01). When the name is taken, `-2`, `-3`, … is appended. Fails when `.archify`
/// resolves outside the root or isn't a folder, when the folder can't be created, or
/// when `slug`/`timestamp` would not form a single path segment.
pub fn create_run_folder(
    sandbox: &Sandbox,
    slug: &str,
    timestamp: &str,
) -> Result<RunFolder, SandboxError> {
    let base_name = format!("architecture-{slug}-{timestamp}");
    if RelPath::parse(&base_name)?.components().count() != 1 {
        return Err(SandboxError::InvalidPath(format!(
            "{base_name:?} is not a valid folder name"
        )));
    }
    let archify = RelPath::parse(ARCHIFY_DIR)?;
    let archify_abs = ensure_archify(sandbox, &archify)?;
    for n in 1..=MAX_COLLISIONS {
        let name = if n == 1 {
            base_name.clone()
        } else {
            format!("{base_name}-{n}")
        };
        match fs::create_dir(archify_abs.join(&name)) {
            Ok(()) => {
                return Ok(RunFolder {
                    rel: archify.join(&name)?,
                    name,
                    slug: slug.to_string(),
                });
            }
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {}
            Err(err) => {
                return Err(SandboxError::Io(format!(
                    "can't create {ARCHIFY_DIR}/{name}: {err}"
                )));
            }
        }
    }
    Err(SandboxError::Io(format!(
        "can't create {ARCHIFY_DIR}/{base_name}: too many folders with that name"
    )))
}

/// Creates `.archify` at the root if missing and returns its canonical path, which must
/// be a folder inside the root.
fn ensure_archify(sandbox: &Sandbox, archify: &RelPath) -> Result<PathBuf, SandboxError> {
    match fs::create_dir(sandbox.root().join(ARCHIFY_DIR)) {
        Ok(()) => {}
        Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {}
        Err(err) => {
            return Err(SandboxError::Io(format!(
                "can't create {ARCHIFY_DIR}: {err}"
            )));
        }
    }
    let abs = sandbox.resolve(archify)?;
    if !abs.is_dir() {
        return Err(SandboxError::InvalidPath(format!(
            "{ARCHIFY_DIR} is not a folder"
        )));
    }
    Ok(abs)
}

/// Appends `line` and a `\n` to `rel` in one write, creating the file if needed. The
/// parent folder must exist inside the root; an existing file (or symlink) must resolve
/// inside the root.
pub fn append_line(sandbox: &Sandbox, rel: &RelPath, line: &str) -> Result<(), SandboxError> {
    let (Some(parent), Some(name)) = (rel.parent(), rel.file_name()) else {
        return Err(SandboxError::InvalidPath(
            "cannot write to the repository root".into(),
        ));
    };
    let dir = sandbox.resolve(&parent)?;
    if !dir.is_dir() {
        return Err(SandboxError::InvalidPath(format!(
            "{parent} is not a folder"
        )));
    }
    let candidate = dir.join(name);
    let target = if candidate.symlink_metadata().is_ok() {
        sandbox.resolve(rel)?
    } else {
        candidate
    };
    let mut buf = Vec::with_capacity(line.len() + 1);
    buf.extend_from_slice(line.as_bytes());
    buf.push(b'\n');
    OpenOptions::new()
        .append(true)
        .create(true)
        .open(&target)
        .and_then(|mut file| file.write_all(&buf))
        .map_err(|e| SandboxError::Io(format!("{rel}: {e}")))
}

/// The app's own writes (FR-WAT-08): path → (length, modification time) right after
/// the write. A watcher event whose file still has that length and mtime is ours and
/// is ignored. Cheap to clone; clones share the registry.
#[derive(Debug, Clone, Default)]
pub struct SelfWrites {
    entries: Arc<Mutex<HashMap<PathBuf, FileStamp>>>,
}

/// Length and modification time of a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileStamp {
    len: u64,
    modified: Option<SystemTime>,
}

impl FileStamp {
    fn of(path: &Path) -> Option<FileStamp> {
        let meta = fs::metadata(path).ok()?;
        Some(FileStamp {
            len: meta.len(),
            modified: meta.modified().ok(),
        })
    }
}

/// The registry key: the canonical path when it exists, else the path as given.
fn key(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

impl SelfWrites {
    /// Remembers the current length and mtime of `abs` (replacing an earlier record for
    /// the same path). Does nothing when the file can't be read.
    pub fn record(&self, abs: &Path) {
        let Some(stamp) = FileStamp::of(abs) else {
            return;
        };
        self.entries
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(key(abs), stamp);
    }

    /// Whether `abs` currently has the length and mtime recorded for it.
    pub fn is_own(&self, abs: &Path) -> bool {
        let Some(stamp) = FileStamp::of(abs) else {
            return false;
        };
        self.entries
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&key(abs))
            .is_some_and(|recorded| *recorded == stamp)
    }
}

/// [`atomic_write`], then [`SelfWrites::record`] of the written file.
pub fn atomic_write_tracked(
    sandbox: &Sandbox,
    rel: &RelPath,
    bytes: &[u8],
    writes: &SelfWrites,
) -> Result<(), SandboxError> {
    atomic_write(sandbox, rel, bytes)?;
    writes.record(&sandbox.resolve(rel)?);
    Ok(())
}
