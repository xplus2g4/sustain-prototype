//! Recursive repository watch (FR-WAT-01, FR-WAT-02, FR-WAT-07, FR-WAT-08, FR-WAT-09).
//!
//! [`RepoWatcher`] runs a `notify-debouncer-mini` watch on the sandbox root. Each
//! debounced batch goes through [`classify`], which applies the tree's ignore rules,
//! the `.git/` and `.archify/` rules, the self-write filter and the burst threshold.
//! [`GitRefreshLimiter`] spaces git-metadata refreshes at least
//! [`GIT_REFRESH_INTERVAL`] apart.

use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use notify_debouncer_mini::notify::{self, RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{DebounceEventResult, Debouncer, new_debouncer};

use crate::sandbox::{RelPath, Sandbox};
use crate::storage::{ARCHIFY_DIR, SelfWrites};
use crate::tree::{TreeOptions, is_ignored};

/// Debounce window for filesystem events (FR-WAT-02).
pub const DEBOUNCE: Duration = Duration::from_millis(300);

/// A batch with more relevant paths than this becomes [`WatchBatch::Full`] (FR-WAT-07).
pub const BURST_PATHS: usize = 500;

/// Minimum spacing between git-metadata refreshes (FR-WAT-09).
pub const GIT_REFRESH_INTERVAL: Duration = Duration::from_secs(2);

/// The git metadata files whose changes count (FR-WAT-02).
const GIT_META: [&str; 2] = [".git/HEAD", ".git/index"];

/// One debounced, filtered batch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchBatch {
    /// Relevant changed paths (deduplicated, sorted), classified.
    Changes(Changes),
    /// More than [`BURST_PATHS`] relevant paths: do one full refresh instead
    /// (FR-WAT-07).
    Full,
    /// The watcher failed after starting (message for a log/banner).
    Error(String),
}

/// The relevant paths of one batch and what they affect.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Changes {
    /// Every relevant changed path (files or folders, existing or deleted), sorted.
    pub paths: Vec<RelPath>,
    /// Parent folders of the changed paths (the folders to re-list if expanded),
    /// sorted and deduplicated. Includes the root as [`RelPath::root()`]; a change
    /// reported for the root itself also lists the root.
    pub dirs: Vec<RelPath>,
    /// Something under `.archify/` changed (refresh History / maybe reload the open
    /// diagram).
    pub archify: bool,
    /// `.git/HEAD` or `.git/index` changed, or any non-`.archify` working-tree path
    /// changed (git metadata may be stale; FR-WAT-09).
    pub git: bool,
}

/// Filters and classifies raw debounced event paths (FR-WAT-02, FR-WAT-07,
/// FR-WAT-08).
///
/// - Paths outside the root are dropped.
/// - Under `.git/`, only `.git/HEAD` and `.git/index` are kept; they set
///   [`Changes::git`] but are not listed in `paths`/`dirs`.
/// - Everything under `.archify/` is kept, whatever the ignore rules say.
/// - Other paths are dropped when [`is_ignored`] says so with `show_hidden`; deleted
///   paths are checked as files.
/// - Paths whose current length and mtime match [`SelfWrites::is_own`] are dropped.
/// - More than [`BURST_PATHS`] relevant paths give [`WatchBatch::Full`].
///
/// Returns `None` when nothing relevant is left.
pub fn classify(
    sandbox: &Sandbox,
    abs_paths: &[PathBuf],
    self_writes: &SelfWrites,
    show_hidden: bool,
) -> Option<WatchBatch> {
    let opts = TreeOptions { show_hidden };
    let mut paths = BTreeSet::new();
    let mut git = false;
    for abs in abs_paths {
        let Some(rel) = relative(sandbox, abs) else {
            continue;
        };
        match rel.components().next() {
            Some(".git") => {
                git |= GIT_META.contains(&rel.as_str());
                continue;
            }
            Some(ARCHIFY_DIR) => {}
            _ => {
                // Like the tree listing, symlinks are not followed.
                let is_dir = fs::symlink_metadata(abs).is_ok_and(|m| m.is_dir());
                if is_ignored(sandbox, &rel, is_dir, &opts) {
                    continue;
                }
            }
        }
        if self_writes.is_own(abs) {
            continue;
        }
        if paths.insert(rel) && paths.len() > BURST_PATHS {
            return Some(WatchBatch::Full);
        }
    }
    if paths.is_empty() && !git {
        return None;
    }
    let mut archify = false;
    let mut dirs = BTreeSet::new();
    for rel in &paths {
        if rel.components().next() == Some(ARCHIFY_DIR) {
            archify = true;
        } else {
            git = true;
        }
        dirs.insert(rel.parent().unwrap_or_else(RelPath::root));
    }
    Some(WatchBatch::Changes(Changes {
        paths: paths.into_iter().collect(),
        dirs: dirs.into_iter().collect(),
        archify,
        git,
    }))
}

/// The repo-relative form of an event path. [`Sandbox::relativize`] handles canonical
/// and existing paths; a deleted path reported through a non-canonical alias (such as
/// `/var/...` for `/private/var/...` on macOS) is resolved through its nearest existing
/// ancestor.
fn relative(sandbox: &Sandbox, abs: &Path) -> Option<RelPath> {
    if let Some(rel) = sandbox.relativize(abs) {
        return Some(rel);
    }
    let mut missing = Vec::new();
    let mut cur = abs;
    loop {
        missing.push(cur.file_name()?);
        cur = cur.parent()?;
        if let Ok(mut canonical) = cur.canonicalize() {
            canonical.extend(missing.iter().rev());
            return sandbox.relativize(&canonical);
        }
    }
}

/// A running recursive watch of the sandbox root (FR-WAT-01). Dropping it stops the
/// watch; no batch is delivered once the drop has started.
pub struct RepoWatcher {
    root: PathBuf,
    stopped: Arc<AtomicBool>,
    _debouncer: Debouncer<RecommendedWatcher>,
}

/// The watcher could not start.
#[derive(Debug, thiserror::Error)]
pub enum WatchError {
    /// Creating the watcher or watching the root failed.
    #[error("Can't watch {path}: {message}")]
    Start {
        /// The root that could not be watched.
        path: String,
        /// The underlying error.
        message: String,
    },
}

impl RepoWatcher {
    /// Starts a recursive watch of the sandbox root with [`DEBOUNCE`]. Each debounced
    /// batch goes through [`classify`], and every `Some(batch)` is passed to
    /// `on_batch` on the watcher thread. Watcher errors become [`WatchBatch::Error`].
    pub fn start(
        sandbox: Sandbox,
        self_writes: SelfWrites,
        show_hidden: bool,
        on_batch: impl Fn(WatchBatch) + Send + 'static,
    ) -> Result<RepoWatcher, WatchError> {
        let root = sandbox.root().to_path_buf();
        let start_error = |err: notify::Error| WatchError::Start {
            path: root.display().to_string(),
            message: err.to_string(),
        };
        let stopped = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stopped);
        let mut debouncer = new_debouncer(DEBOUNCE, move |result: DebounceEventResult| {
            if flag.load(Ordering::Acquire) {
                return;
            }
            let batch = match result {
                Ok(events) => {
                    let paths: Vec<PathBuf> = events.into_iter().map(|e| e.path).collect();
                    classify(&sandbox, &paths, &self_writes, show_hidden)
                }
                Err(err) => Some(WatchBatch::Error(err.to_string())),
            };
            if let Some(batch) = batch {
                on_batch(batch);
            }
        })
        .map_err(start_error)?;
        debouncer
            .watcher()
            .watch(&root, RecursiveMode::Recursive)
            .map_err(start_error)?;
        Ok(RepoWatcher {
            root,
            stopped,
            _debouncer: debouncer,
        })
    }

    /// The watched (canonical) root.
    pub fn root(&self) -> &Path {
        &self.root
    }
}

impl Drop for RepoWatcher {
    fn drop(&mut self) {
        // Set before the debouncer is dropped, so a batch already being flushed is
        // discarded instead of delivered.
        self.stopped.store(true, Ordering::Release);
    }
}

impl fmt::Debug for RepoWatcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RepoWatcher")
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

/// Spaces git-metadata refreshes at least [`GIT_REFRESH_INTERVAL`] apart and
/// coalesces requests while one is scheduled (FR-WAT-09).
#[derive(Debug, Default, Clone)]
pub struct GitRefreshLimiter {
    last: Option<Instant>,
    pending: bool,
}

impl GitRefreshLimiter {
    /// Requests a refresh at `now`. `Some(Duration::ZERO)` means refresh now (counted
    /// as a refresh at `now`); a positive delay means schedule it after that delay.
    /// `None` means a refresh is already scheduled and this request is coalesced.
    pub fn request(&mut self, now: Instant) -> Option<Duration> {
        if self.pending {
            return None;
        }
        let delay = self.last.map_or(Duration::ZERO, |last| {
            GIT_REFRESH_INTERVAL.saturating_sub(now.saturating_duration_since(last))
        });
        if delay.is_zero() {
            self.last = Some(now);
        } else {
            self.pending = true;
        }
        Some(delay)
    }

    /// Records that a refresh ran at `now` and clears the scheduled flag.
    pub fn done(&mut self, now: Instant) {
        self.last = Some(now);
        self.pending = false;
    }
}
