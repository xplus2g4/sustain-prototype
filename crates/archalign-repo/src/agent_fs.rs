//! Filesystem helpers behind the agent's tools and seed message (FR-GEN-03, FR-GEN-04).
//!
//! Everything is confined to the [`Sandbox`], applies the ignore rules of the Tree pane
//! with dotfiles shown, and leaves out deny-listed paths ([`DenyList`], FR-SEC-02).

use std::fs::{self, File};
use std::io::Read;
use std::time::{Duration, Instant};

use ignore::WalkBuilder;
use ignore::overrides::{Override, OverrideBuilder};
use regex::{Regex, RegexBuilder};

use crate::deny::DenyList;
use crate::sandbox::{RelPath, Sandbox, SandboxError};
use crate::text::{BINARY_SNIFF_BYTES, RawLoad, read_raw};
use crate::tree::{EntryKind, TreeOptions, is_ignored, list_dir};

/// Most entries `list_dir` returns.
pub const LIST_DIR_MAX: usize = 500;

/// Dotfiles are listed; the deny-list hides the sensitive ones.
const AGENT_TREE: TreeOptions = TreeOptions { show_hidden: true };

/// One level of a folder, formatted for the agent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Listing {
    /// `name/` for folders (and links to folders inside the root), `name (N bytes)` for
    /// files; folders first.
    pub lines: Vec<String>,
    /// Entries left out beyond the cap.
    pub omitted: usize,
}

/// One visible entry.
struct AgentEntry {
    name: String,
    rel: RelPath,
    is_dir: bool,
    is_link: bool,
    size: u64,
}

impl AgentEntry {
    fn line(&self) -> String {
        if self.is_dir {
            format!("{}/", self.name)
        } else {
            format!("{} ({} bytes)", self.name, self.size)
        }
    }
}

/// The entries of `dir` the agent may see, in tree order.
fn agent_entries(
    sandbox: &Sandbox,
    deny: &DenyList,
    dir: &RelPath,
) -> Result<Vec<AgentEntry>, SandboxError> {
    if deny.is_denied_in(sandbox, dir) {
        return Err(SandboxError::InvalidPath(format!(
            "{} is on the deny-list",
            shown(dir)
        )));
    }
    if is_ignored(sandbox, dir, true, &AGENT_TREE) {
        return Err(SandboxError::InvalidPath(format!(
            "{dir} is excluded by the ignore rules"
        )));
    }
    let mut out = Vec::new();
    for entry in list_dir(sandbox, dir, &AGENT_TREE)? {
        if deny.is_denied_in(sandbox, &entry.rel) {
            continue;
        }
        let (is_dir, is_link) = match entry.kind {
            EntryKind::Dir => (true, false),
            EntryKind::File => (false, false),
            EntryKind::Symlink {
                outside_root: true, ..
            } => continue,
            EntryKind::Symlink { target_is_dir, .. } => (target_is_dir, true),
        };
        let size = match entry.size {
            Some(size) => size,
            None if is_dir => 0,
            // Link to a file inside the root: report the target's size.
            None => match sandbox.resolve(&entry.rel).and_then(|abs| {
                abs.metadata()
                    .map_err(|e| SandboxError::Io(format!("{}: {e}", entry.rel)))
            }) {
                Ok(meta) => meta.len(),
                Err(_) => continue,
            },
        };
        out.push(AgentEntry {
            name: entry.name,
            rel: entry.rel,
            is_dir,
            is_link,
            size,
        });
    }
    Ok(out)
}

/// The path for messages.
fn shown(rel: &RelPath) -> &str {
    if rel.is_root() {
        "the repository folder"
    } else {
        rel.as_str()
    }
}

/// Lists one level of `dir` for the agent's `list_dir` tool: ignore rules applied,
/// dotfiles shown, deny-listed entries and links resolving outside the root left out,
/// folders first. At most `max` lines; the rest is counted in [`Listing::omitted`].
///
/// A denied or ignored `dir` is rejected with [`SandboxError::InvalidPath`].
pub fn list_for_agent(
    sandbox: &Sandbox,
    deny: &DenyList,
    dir: &RelPath,
    max: usize,
) -> Result<Listing, SandboxError> {
    let entries = agent_entries(sandbox, deny, dir)?;
    let omitted = entries.len().saturating_sub(max);
    let lines = entries.iter().take(max).map(AgentEntry::line).collect();
    Ok(Listing { lines, omitted })
}

/// The depth-limited tree of the seed message.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SeedTree {
    /// `name/` for folders, `name` for files, indented two spaces per level.
    pub lines: Vec<String>,
    /// Entries were left out because of the entry cap.
    pub truncated: bool,
}

/// One node of the seed tree, before printing.
struct SeedNode {
    line: String,
    children: Vec<usize>,
}

/// Builds the seed tree: `max_depth` levels and at most `max_entries` lines, with the
/// [`list_for_agent`] rules. Entries are admitted breadth-first (all of level 1, then
/// level 2 in order, …) so a cap keeps the shallow overview, and printed depth-first.
/// Links to folders are shown but not expanded; unreadable folders stay collapsed.
pub fn seed_tree(
    sandbox: &Sandbox,
    deny: &DenyList,
    max_depth: usize,
    max_entries: usize,
) -> SeedTree {
    let mut nodes: Vec<SeedNode> = Vec::new();
    let mut top: Vec<usize> = Vec::new();
    let mut truncated = false;
    // (folder, depth of its children, parent node)
    let mut level: Vec<(RelPath, usize, Option<usize>)> = vec![(RelPath::root(), 1, None)];
    'levels: while !level.is_empty() {
        let mut next = Vec::new();
        for (dir, depth, parent) in level {
            if depth > max_depth {
                continue;
            }
            let Ok(entries) = agent_entries(sandbox, deny, &dir) else {
                continue;
            };
            for entry in entries {
                if nodes.len() >= max_entries {
                    truncated = true;
                    break 'levels;
                }
                let indent = "  ".repeat(depth - 1);
                let line = if entry.is_dir {
                    format!("{indent}{}/", entry.name)
                } else {
                    format!("{indent}{}", entry.name)
                };
                let idx = nodes.len();
                nodes.push(SeedNode {
                    line,
                    children: Vec::new(),
                });
                match parent.and_then(|p| nodes.get_mut(p)) {
                    Some(parent) => parent.children.push(idx),
                    None => top.push(idx),
                }
                if entry.is_dir && !entry.is_link {
                    next.push((entry.rel, depth + 1, Some(idx)));
                }
            }
        }
        level = next;
    }
    let mut lines = Vec::with_capacity(nodes.len());
    let mut stack: Vec<usize> = top.into_iter().rev().collect();
    while let Some(idx) = stack.pop() {
        if let Some(node) = nodes.get_mut(idx) {
            lines.push(std::mem::take(&mut node.line));
            stack.extend(node.children.iter().rev());
        }
    }
    SeedTree { lines, truncated }
}

/// Limits of the `grep` tool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrepOptions {
    /// Most matches returned.
    pub max_matches: usize,
    /// Larger files are skipped.
    pub max_file_bytes: u64,
    /// The search gives up after this long.
    pub time_limit: Duration,
    /// Matched lines are cut to this many characters.
    pub line_chars: usize,
}

impl Default for GrepOptions {
    fn default() -> Self {
        Self {
            max_matches: 100,
            max_file_bytes: 1_048_576,
            time_limit: Duration::from_secs(5),
            line_chars: 200,
        }
    }
}

/// One matching line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrepMatch {
    /// Repo-relative file path.
    pub path: String,
    /// 1-based line number.
    pub line: u64,
    /// The line without its line ending, cut to [`GrepOptions::line_chars`] characters.
    pub text: String,
}

/// The outcome of a search.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GrepResult {
    /// Matches in path order, then line order.
    pub matches: Vec<GrepMatch>,
    /// More matches exist than [`GrepOptions::max_matches`].
    pub truncated: bool,
    /// Files skipped as binary, too large or unreadable.
    pub skipped: usize,
    /// The time limit was hit before all files were searched.
    pub timed_out: bool,
}

/// Compiled-program and lazy-DFA size limit for model-supplied patterns.
const PATTERN_SIZE_LIMIT: usize = 1024 * 1024;

/// Compiles a `grep` pattern (Rust `regex` syntax) with 1 MiB program and DFA limits, so
/// a model-supplied pattern can't blow up memory. The error is the regex error text.
pub fn compile_pattern(pattern: &str) -> Result<Regex, String> {
    RegexBuilder::new(pattern)
        .size_limit(PATTERN_SIZE_LIMIT)
        .dfa_size_limit(PATTERN_SIZE_LIMIT)
        .build()
        .map_err(|e| e.to_string())
}

/// Searches the repository line by line, in path order.
///
/// Ignore rules apply, dotfiles are searched, deny-listed paths never are, and
/// symlinks are not followed. `path_glob` is a gitignore-style glob relative to the
/// root (`*.py` matches at any depth, `src/**/*.rs` below `src`); an invalid glob gives
/// [`SandboxError::InvalidPath`]. Binary files (a NUL byte in the first 8 KB) and files
/// over `opts.max_file_bytes` are skipped and counted. Invalid UTF-8 is decoded lossily.
pub fn grep(
    sandbox: &Sandbox,
    deny: &DenyList,
    pattern: &Regex,
    path_glob: Option<&str>,
    opts: &GrepOptions,
) -> Result<GrepResult, SandboxError> {
    let started = Instant::now();
    let glob = path_glob
        .map(str::trim)
        .filter(|g| !g.is_empty())
        .map(|g| build_glob(sandbox, g))
        .transpose()?;
    let mut result = GrepResult::default();

    let files = match collect_files(sandbox, deny, glob.as_ref(), started, opts.time_limit) {
        Some(files) => files,
        None => {
            result.timed_out = true;
            return Ok(result);
        }
    };

    for rel in files {
        if started.elapsed() >= opts.time_limit {
            result.timed_out = true;
            break;
        }
        let text = match read_for_grep(sandbox, &rel, opts.max_file_bytes) {
            Some(text) => text,
            None => {
                result.skipped += 1;
                continue;
            }
        };
        for (idx, line) in text.split('\n').enumerate() {
            if idx % 256 == 255 && started.elapsed() >= opts.time_limit {
                result.timed_out = true;
                return Ok(result);
            }
            let line = line.strip_suffix('\r').unwrap_or(line);
            if !pattern.is_match(line) {
                continue;
            }
            if result.matches.len() >= opts.max_matches {
                result.truncated = true;
                return Ok(result);
            }
            result.matches.push(GrepMatch {
                path: rel.as_str().to_string(),
                line: idx as u64 + 1,
                text: cut_chars(line, opts.line_chars).to_string(),
            });
        }
    }
    Ok(result)
}

/// Compiles a `path_glob`.
fn build_glob(sandbox: &Sandbox, glob: &str) -> Result<Override, SandboxError> {
    let invalid = |err: ignore::Error| {
        SandboxError::InvalidPath(format!("{glob:?} is not a valid path glob: {err}"))
    };
    let mut builder = OverrideBuilder::new(sandbox.root());
    builder.add(glob).map_err(invalid)?;
    builder.build().map_err(invalid)
}

/// Regular files to search, sorted by path, or `None` when time ran out while walking.
fn collect_files(
    sandbox: &Sandbox,
    deny: &DenyList,
    glob: Option<&Override>,
    started: Instant,
    time_limit: Duration,
) -> Option<Vec<RelPath>> {
    let root = sandbox.root().to_path_buf();
    let filter_root = root.clone();
    let filter_deny = *deny;
    let walker = WalkBuilder::new(&root)
        .follow_links(false)
        .hidden(false)
        .parents(true)
        .ignore(false)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .require_git(false)
        .filter_entry(move |e| {
            let Ok(rest) = e.path().strip_prefix(&filter_root) else {
                return false;
            };
            // The root itself: an empty path.
            let Some(rest) = rest.to_str() else {
                return false;
            };
            RelPath::parse(rest).is_ok_and(|rel| !filter_deny.is_denied(&rel))
        })
        .build();
    let mut files = Vec::new();
    for item in walker {
        if started.elapsed() >= time_limit {
            return None;
        }
        let entry = match item {
            Ok(entry) => entry,
            Err(err) => {
                tracing::debug!(%err, "grep skipping unreadable entry");
                continue;
            }
        };
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        if glob.is_some_and(|g| g.matched(entry.path(), false).is_ignore()) {
            continue;
        }
        let Some(rel) = sandbox.relativize(entry.path()) else {
            continue;
        };
        if deny.is_denied(&rel) {
            continue;
        }
        files.push(rel);
    }
    files.sort();
    Some(files)
}

/// Reads a file for searching; `None` when it is binary, too large, unreadable or no
/// longer a regular file inside the root.
fn read_for_grep(sandbox: &Sandbox, rel: &RelPath, max_bytes: u64) -> Option<String> {
    let abs = sandbox.root().join(rel.as_str());
    let meta = fs::symlink_metadata(&abs).ok()?;
    if !meta.is_file() || meta.len() > max_bytes {
        return None;
    }
    let file = File::open(&abs).ok()?;
    let mut bytes = Vec::with_capacity(usize::try_from(meta.len()).unwrap_or(0));
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() as u64 > max_bytes {
        return None;
    }
    let head = bytes.get(..BINARY_SNIFF_BYTES).unwrap_or(&bytes);
    if head.contains(&0) {
        return None;
    }
    Some(match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(err) => String::from_utf8_lossy(err.as_bytes()).into_owned(),
    })
}

/// The first `max` characters of `s`.
fn cut_chars(s: &str, max: usize) -> &str {
    match s.char_indices().nth(max) {
        Some((idx, _)) => s.get(..idx).unwrap_or(s),
        None => s,
    }
}

/// The first `max_lines` lines of a repo file (line endings kept), for the seed
/// message. `None` when the file is missing, denied, not a regular file, binary or over
/// 2 MB.
pub fn head_lines(
    sandbox: &Sandbox,
    deny: &DenyList,
    rel: &RelPath,
    max_lines: usize,
) -> Option<String> {
    if deny.is_denied_in(sandbox, rel) {
        return None;
    }
    let RawLoad::Bytes(bytes, _) = read_raw(sandbox, rel).ok()? else {
        return None;
    };
    let text = String::from_utf8_lossy(&bytes);
    Some(text.split_inclusive('\n').take(max_lines).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cut_chars_respects_char_boundaries() {
        assert_eq!(cut_chars("héllo", 2), "hé");
        assert_eq!(cut_chars("héllo", 10), "héllo");
        assert_eq!(cut_chars("", 3), "");
        assert_eq!(cut_chars("日本語", 0), "");
    }
}
