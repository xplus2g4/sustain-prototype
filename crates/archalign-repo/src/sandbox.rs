//! Root confinement (FR-SEC-01).
//!
//! Every file access — UI reads, agent tools, citation checks — resolves a [`RelPath`]
//! through a [`Sandbox`]. Resolution canonicalizes the joined path and requires the
//! canonical root as a prefix, so `..` escapes, absolute paths and symlinks that point
//! outside the root are all rejected.

use std::fmt;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Why a path was rejected or could not be resolved.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SandboxError {
    /// The path is malformed: absolute, contains `..`, a backslash or NUL. The payload
    /// is a full sentence.
    #[error("{0}")]
    InvalidPath(String),
    /// The path (payload) resolves outside the repository root.
    #[error("{0} is outside the repository folder")]
    Escapes(String),
    /// Nothing exists at the path (payload).
    #[error("{0} does not exist")]
    NotFound(String),
    /// Any other I/O failure; the payload names the path and the OS error.
    #[error("{0}")]
    Io(String),
}

impl SandboxError {
    fn from_io(err: &io::Error, what: &str) -> Self {
        match err.kind() {
            io::ErrorKind::NotFound => SandboxError::NotFound(what.to_string()),
            _ => SandboxError::Io(format!("{what}: {err}")),
        }
    }
}

/// A normalized repo-relative path: `/`-separated, no leading `/`, no empty, `.` or `..`
/// segments. The empty path is the root.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RelPath(String);

impl RelPath {
    /// The repository root (`""`).
    pub fn root() -> Self {
        RelPath(String::new())
    }

    /// Parses and normalizes a repo-relative path.
    ///
    /// Empty and `.` segments are dropped (`"./a//b/"` → `"a/b"`). Absolute paths
    /// (`/x`, `C:\x`, `C:/x`), `..` segments, backslashes and NUL bytes are rejected.
    pub fn parse(s: &str) -> Result<RelPath, SandboxError> {
        if s.contains('\0') {
            return Err(SandboxError::InvalidPath("path contains a NUL byte".into()));
        }
        if s.contains('\\') {
            return Err(SandboxError::InvalidPath(format!(
                "{s:?} contains a backslash; use `/`"
            )));
        }
        if s.starts_with('/') || has_drive_prefix(s) {
            return Err(SandboxError::InvalidPath(format!(
                "{s:?} is absolute; paths must be relative to the repository root"
            )));
        }
        let mut out = String::with_capacity(s.len());
        for seg in s.split('/') {
            match seg {
                "" | "." => {}
                ".." => {
                    return Err(SandboxError::InvalidPath(format!(
                        "{s:?} contains a `..` segment"
                    )));
                }
                seg => {
                    if !out.is_empty() {
                        out.push('/');
                    }
                    out.push_str(seg);
                }
            }
        }
        Ok(RelPath(out))
    }

    /// The path as a string (`""` for the root).
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether this is the root.
    pub fn is_root(&self) -> bool {
        self.0.is_empty()
    }

    /// Appends a relative path (one or more segments), with the same rules as
    /// [`RelPath::parse`].
    pub fn join(&self, name: &str) -> Result<RelPath, SandboxError> {
        let tail = RelPath::parse(name)?;
        Ok(if self.is_root() {
            tail
        } else if tail.is_root() {
            self.clone()
        } else {
            RelPath(format!("{}/{}", self.0, tail.0))
        })
    }

    /// The containing folder, or `None` for the root.
    pub fn parent(&self) -> Option<RelPath> {
        if self.is_root() {
            return None;
        }
        Some(match self.0.rfind('/') {
            Some(idx) => RelPath(self.0.get(..idx).unwrap_or_default().to_string()),
            None => RelPath::root(),
        })
    }

    /// The last segment, or `None` for the root.
    pub fn file_name(&self) -> Option<&str> {
        if self.is_root() {
            None
        } else {
            self.0.rsplit('/').next()
        }
    }

    /// The segments, outermost first (empty for the root).
    pub fn components(&self) -> impl Iterator<Item = &str> {
        self.0.split('/').filter(|s| !s.is_empty())
    }

    /// The path relative to `root` as a native path (empty for the root).
    fn to_native(&self, root: &Path) -> PathBuf {
        let mut path = root.to_path_buf();
        for seg in self.components() {
            path.push(seg);
        }
        path
    }

    /// The path for messages: the path itself, or "the repository folder" for the root.
    fn shown(&self) -> &str {
        if self.is_root() {
            "the repository folder"
        } else {
            &self.0
        }
    }
}

/// `C:`-style prefixes are absolute on Windows; reject them everywhere.
fn has_drive_prefix(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(
        (chars.next(), chars.next()),
        (Some(letter), Some(':')) if letter.is_ascii_alphabetic()
    )
}

impl fmt::Display for RelPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::str::FromStr for RelPath {
    type Err = SandboxError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        RelPath::parse(s)
    }
}

impl Serialize for RelPath {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for RelPath {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        RelPath::parse(&s).map_err(serde::de::Error::custom)
    }
}

/// The repository root, canonicalized. Cheap to clone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sandbox {
    root: Arc<PathBuf>,
}

impl Sandbox {
    /// Canonicalizes `root`, which must be an existing folder.
    pub fn new(root: impl AsRef<Path>) -> Result<Sandbox, SandboxError> {
        let given = root.as_ref();
        let shown = given.display().to_string();
        let canonical = given
            .canonicalize()
            .map_err(|e| SandboxError::from_io(&e, &shown))?;
        let meta = canonical
            .metadata()
            .map_err(|e| SandboxError::from_io(&e, &shown))?;
        if !meta.is_dir() {
            return Err(SandboxError::InvalidPath(format!(
                "{shown} is not a folder"
            )));
        }
        Ok(Sandbox {
            root: Arc::new(canonical),
        })
    }

    /// The canonical root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Resolves an existing entry to its canonical absolute path, following symlinks.
    /// Fails with [`SandboxError::Escapes`] when the result is outside the root and with
    /// [`SandboxError::NotFound`] when nothing exists (including dangling symlinks).
    pub fn resolve(&self, rel: &RelPath) -> Result<PathBuf, SandboxError> {
        let joined = rel.to_native(&self.root);
        let canonical = joined
            .canonicalize()
            .map_err(|e| SandboxError::from_io(&e, rel.shown()))?;
        if canonical.starts_with(&*self.root) {
            Ok(canonical)
        } else {
            Err(SandboxError::Escapes(rel.shown().to_string()))
        }
    }

    /// [`RelPath::parse`] followed by [`Sandbox::resolve`].
    pub fn resolve_str(&self, s: &str) -> Result<PathBuf, SandboxError> {
        self.resolve(&RelPath::parse(s)?)
    }

    /// The repo-relative path of an absolute path inside the root (canonical or not),
    /// or `None` when it is outside or not valid UTF-8.
    pub fn relativize(&self, abs: &Path) -> Option<RelPath> {
        if let Some(rel) = self.strip_root(abs) {
            return Some(rel);
        }
        let canonical = abs.canonicalize().ok()?;
        self.strip_root(&canonical)
    }

    fn strip_root(&self, abs: &Path) -> Option<RelPath> {
        let rest = abs.strip_prefix(&*self.root).ok()?;
        let mut out = String::new();
        for comp in rest.components() {
            match comp {
                Component::Normal(seg) => {
                    if !out.is_empty() {
                        out.push('/');
                    }
                    out.push_str(seg.to_str()?);
                }
                Component::CurDir => {}
                _ => return None,
            }
        }
        RelPath::parse(&out).ok()
    }

    /// Whether the entry at `rel` is a symlink whose target resolves outside the root,
    /// or is dangling. Plain files and folders return `false`.
    pub fn is_outside_link(&self, rel: &RelPath) -> bool {
        let joined = rel.to_native(&self.root);
        let is_link = joined
            .symlink_metadata()
            .is_ok_and(|m| m.file_type().is_symlink());
        is_link && !self.link_target_inside(&joined)
    }

    /// For a symlink at `abs`, whether its target exists inside the root.
    pub(crate) fn link_target_inside(&self, abs: &Path) -> bool {
        abs.canonicalize()
            .is_ok_and(|target| target.starts_with(&*self.root))
    }
}
