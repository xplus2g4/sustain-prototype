//! Diagram storage under `<repo>/.archify/` (FR-IMP-03, FR-STO-02, FR-STO-04, ADR 0015).

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
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
