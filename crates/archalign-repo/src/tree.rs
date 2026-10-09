//! Lazy, ignore-aware directory listing for the Tree pane (FR-TREE-01, FR-TREE-02,
//! FR-TREE-08, NFR-PERF-01).

use std::cmp::Ordering;
use std::path::{Path, PathBuf};

use ignore::gitignore::{Gitignore, GitignoreBuilder};
use ignore::{Match, WalkBuilder};

use crate::sandbox::{RelPath, Sandbox, SandboxError};

/// Listing options.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TreeOptions {
    /// Show dotfiles ("Show hidden files" in Settings).
    pub show_hidden: bool,
}

/// What a listed entry is. Symlinks are never followed for listing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    /// A folder.
    Dir,
    /// A regular file (or anything else that is not a folder or symlink).
    File,
    /// A symbolic link.
    Symlink {
        /// The target is a folder (inside or outside the root).
        target_is_dir: bool,
        /// The target resolves outside the root, or the link is dangling: it can't be
        /// expanded or opened.
        outside_root: bool,
    },
}

impl EntryKind {
    /// Whether the entry sorts with folders.
    fn sorts_as_dir(self) -> bool {
        matches!(
            self,
            EntryKind::Dir
                | EntryKind::Symlink {
                    target_is_dir: true,
                    ..
                }
        )
    }
}

/// One listed entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntry {
    /// File name.
    pub name: String,
    /// Repo-relative path.
    pub rel: RelPath,
    /// Folder, file or symlink.
    pub kind: EntryKind,
    /// Size in bytes for regular files.
    pub size: Option<u64>,
}

/// Folders (and symlinks to folders) first, then files; each group case-insensitively,
/// ties broken by byte order.
fn entry_order(a: &DirEntry, b: &DirEntry) -> Ordering {
    b.kind
        .sorts_as_dir()
        .cmp(&a.kind.sorts_as_dir())
        .then_with(|| {
            let la = a.name.chars().flat_map(char::to_lowercase);
            let lb = b.name.chars().flat_map(char::to_lowercase);
            la.cmp(lb)
        })
        .then_with(|| a.name.cmp(&b.name))
}

/// Lists one level of `dir`.
///
/// Nested `.gitignore` files, `.git/info/exclude` and the global gitignore apply (also
/// outside a git repository); `.git` is never listed; dotfiles are hidden unless
/// `opts.show_hidden`. Entries whose names are not valid UTF-8 or contain a backslash
/// can't be represented as a [`RelPath`] and are skipped. Symlinks are never followed.
pub fn list_dir(
    sandbox: &Sandbox,
    dir: &RelPath,
    opts: &TreeOptions,
) -> Result<Vec<DirEntry>, SandboxError> {
    let abs = sandbox.resolve(dir)?;
    if !abs.is_dir() {
        return Err(SandboxError::InvalidPath(format!("{dir} is not a folder")));
    }
    let walker = WalkBuilder::new(&abs)
        .max_depth(Some(1))
        .follow_links(false)
        .hidden(!opts.show_hidden)
        .parents(true)
        .ignore(false)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .require_git(false)
        .filter_entry(|e| e.file_name() != ".git")
        .build();
    let mut out = Vec::new();
    for item in walker {
        let entry = match item {
            Ok(entry) => entry,
            Err(err) => {
                tracing::debug!(%err, "skipping unreadable tree entry");
                continue;
            }
        };
        if entry.depth() == 0 {
            continue;
        }
        let Some(name) = entry.file_name().to_str() else {
            tracing::debug!("skipping non-UTF-8 file name in {dir}");
            continue;
        };
        let Ok(rel) = dir.join(name) else {
            tracing::debug!("skipping unrepresentable file name in {dir}");
            continue;
        };
        let Some(file_type) = entry.file_type() else {
            continue;
        };
        let (kind, size) = if file_type.is_symlink() {
            let target = entry.path().canonicalize().ok();
            let outside_root = !target
                .as_ref()
                .is_some_and(|t| t.starts_with(sandbox.root()));
            let target_is_dir = target.as_ref().is_some_and(|t| t.is_dir());
            (
                EntryKind::Symlink {
                    target_is_dir,
                    outside_root,
                },
                None,
            )
        } else if file_type.is_dir() {
            (EntryKind::Dir, None)
        } else {
            (EntryKind::File, entry.metadata().ok().map(|m| m.len()))
        };
        out.push(DirEntry {
            name: name.to_string(),
            rel,
            kind,
            size,
        });
    }
    out.sort_by(entry_order);
    Ok(out)
}

/// The ignore files that apply inside one folder.
struct DirRules {
    dir: PathBuf,
    gitignore: Option<Gitignore>,
    exclude: Option<Gitignore>,
}

impl DirRules {
    fn load(dir: &Path) -> Self {
        let read = |file: PathBuf| {
            if !file.is_file() {
                return None;
            }
            let mut builder = GitignoreBuilder::new(dir);
            if let Some(err) = builder.add(&file) {
                tracing::debug!(%err, "ignore file has errors");
            }
            builder.build().ok()
        };
        let git_dir = dir.join(".git");
        Self {
            dir: dir.to_path_buf(),
            gitignore: read(dir.join(".gitignore")),
            exclude: if git_dir.is_dir() {
                read(git_dir.join("info").join("exclude"))
            } else {
                None
            },
        }
    }
}

/// Applies the matcher stack to one path, like the walker does at that level: the
/// deepest `.gitignore` wins, then `.git/info/exclude` files, then the global gitignore.
fn ignored_by(rules: &[DirRules], global: &Gitignore, abs: &Path, is_dir: bool) -> bool {
    let applicable = || {
        rules
            .iter()
            .rev()
            .filter(|r| abs.parent().is_some_and(|p| p.starts_with(&r.dir)))
    };
    let decided = applicable()
        .filter_map(|r| r.gitignore.as_ref())
        .chain(applicable().filter_map(|r| r.exclude.as_ref()))
        .map(|gi| gi.matched(abs, is_dir))
        .chain(std::iter::once(global.matched(abs, is_dir)))
        .find(|m| !m.is_none());
    matches!(decided, Some(Match::Ignore(_)))
}

/// Whether [`list_dir`] would hide `rel` (or any of its ancestors). Used to filter
/// watcher events. Builds the matchers on every call.
pub fn is_ignored(sandbox: &Sandbox, rel: &RelPath, is_dir: bool, opts: &TreeOptions) -> bool {
    if rel.is_root() {
        return false;
    }
    let segments: Vec<&str> = rel.components().collect();
    if segments.contains(&".git") {
        return true;
    }
    if !opts.show_hidden && segments.iter().any(|s| s.starts_with('.')) {
        return true;
    }
    let root = sandbox.root();
    // Like `WalkBuilder::parents(true)`, folders above the root contribute rules too.
    let mut rules: Vec<DirRules> = root
        .ancestors()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(DirRules::load)
        .collect();
    let (global, err) = Gitignore::global();
    if let Some(err) = err {
        tracing::debug!(%err, "global gitignore has errors");
    }
    let mut abs = root.to_path_buf();
    let last = segments.len().saturating_sub(1);
    for (i, seg) in segments.iter().enumerate() {
        abs.push(seg);
        let seg_is_dir = i < last || is_dir;
        if ignored_by(&rules, &global, &abs, seg_is_dir) {
            return true;
        }
        if i < last {
            rules.push(DirRules::load(&abs));
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, kind: EntryKind) -> DirEntry {
        DirEntry {
            name: name.to_string(),
            rel: RelPath::root().join(name).unwrap(),
            kind,
            size: None,
        }
    }

    #[test]
    fn order_is_folders_first_case_insensitive_then_bytes() {
        let link_dir = EntryKind::Symlink {
            target_is_dir: true,
            outside_root: true,
        };
        let link_file = EntryKind::Symlink {
            target_is_dir: false,
            outside_root: false,
        };
        let mut entries = [
            entry("b", EntryKind::File),
            entry("a", EntryKind::File),
            entry("A", EntryKind::File),
            entry("Z", EntryKind::Dir),
            entry("m", link_dir),
            entry("c", link_file),
            entry("a", EntryKind::Dir),
        ];
        entries.sort_by(entry_order);
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["a", "m", "Z", "A", "a", "b", "c"]);
    }
}
