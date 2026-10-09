//! Filesystem and git boundary: sandbox, tree, storage, watcher.
//!
//! - [`sandbox`]: root confinement; every file access resolves through [`Sandbox`]
//!   (FR-SEC-01).
//! - [`tree`]: lazy, ignore-aware listing for the Tree pane (FR-TREE-01, 02, 08).
//! - [`text`]: size cap, binary sniff, lossy decoding and [`LineIndex`] (FR-SRC-06).
//! - [`git`]: HEAD, sanitized origin and dirty paths via the git CLI (FR-IMP-04,
//!   FR-SEC-07, FR-SEC-08).
//! - [`citations`]: [`archalign_model::SourceProbe`] over the sandbox (FR-VAL-06).
//! - [`storage`]: `.archify/` history and atomic writes (FR-IMP-03, FR-STO-04).

pub mod citations;
pub mod git;
pub mod sandbox;
pub mod storage;
pub mod text;
pub mod tree;

pub use citations::RepoProbe;
pub use git::{GitInfo, GitRead, GitStatus, collect, collect_with, read_argv, sanitize_origin};
pub use sandbox::{RelPath, Sandbox, SandboxError};
pub use storage::{HistoryEntry, HistoryStatus, atomic_write, newest_valid, scan_history};
pub use text::{LineIndex, MAX_VIEW_BYTES, TextLoad, load_text};
pub use tree::{DirEntry, EntryKind, TreeOptions, is_ignored, list_dir};
