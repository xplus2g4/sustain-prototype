//! Git metadata through the git CLI (FR-IMP-04, FR-SEC-07, FR-SEC-08, ADR 0016).
//!
//! Git runs as a fixed argument vector via [`std::process::Command`], never through a
//! shell, and every read command passes `--no-optional-locks`.

use std::collections::HashSet;
use std::ffi::OsStr;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use archalign_model::{LinkMode, Repository, RepositoryProvider};

/// The read-only git commands ArchAlign runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitRead {
    /// `rev-parse --show-toplevel`.
    TopLevel,
    /// `rev-parse --verify HEAD`.
    Head,
    /// `config --get remote.origin.url`.
    Origin,
    /// `status --porcelain=v1 -z`.
    Status,
}

/// The fixed argument vector for a read command. Each starts with
/// `--no-optional-locks`.
pub fn read_argv(cmd: GitRead) -> Vec<&'static str> {
    let rest: &[&'static str] = match cmd {
        GitRead::TopLevel => &["rev-parse", "--show-toplevel"],
        GitRead::Head => &["rev-parse", "--verify", "--quiet", "HEAD"],
        GitRead::Origin => &["config", "--get", "remote.origin.url"],
        GitRead::Status => &["status", "--porcelain=v1", "-z"],
    };
    let mut argv = Vec::with_capacity(rest.len() + 1);
    argv.push("--no-optional-locks");
    argv.extend_from_slice(rest);
    argv
}

/// Metadata of a repository whose root is the top level of a work tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitInfo {
    /// HEAD commit, 40 lowercase hex characters.
    pub head: String,
    /// `remote.origin.url` with HTTP(S) userinfo removed ([`sanitize_origin`]).
    pub origin: Option<String>,
    /// Paths reported by `git status`, repo-relative with `/` separators. Untracked
    /// folders appear as git reports them (`dir/`).
    pub dirty: Vec<String>,
}

impl GitInfo {
    /// The first 7 characters of HEAD.
    pub fn short_head(&self) -> &str {
        self.head.get(..7).unwrap_or(&self.head)
    }

    /// The dirty paths as a set, for [`archalign_model::check_citations`].
    pub fn dirty_set(&self) -> HashSet<String> {
        self.dirty.iter().cloned().collect()
    }

    /// `meta.repository` (ADR 0016), or `None` without an origin. `provider` is set for
    /// HTTPS GitHub and Gitee origins; any other origin gets `link_mode: local-only`.
    pub fn repository(&self) -> Option<Repository> {
        let url = self.origin.clone()?;
        let provider = https_host(&url).and_then(|host| match host.as_str() {
            "github.com" => Some(RepositoryProvider::Github),
            "gitee.com" => Some(RepositoryProvider::Gitee),
            _ => None,
        });
        Some(Repository {
            url,
            link_mode: provider.is_none().then_some(LinkMode::LocalOnly),
            provider,
            revision: self.head.clone(),
        })
    }
}

/// The lowercase host (without port) of an `https://` URL.
fn https_host(url: &str) -> Option<String> {
    let (scheme, rest) = url.split_once("://")?;
    if !scheme.eq_ignore_ascii_case("https") {
        return None;
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host_port = authority.rsplit('@').next().unwrap_or_default();
    let host = host_port.split(':').next().unwrap_or_default();
    Some(host.to_ascii_lowercase())
}

/// What [`collect`] found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitStatus {
    /// `git` could not be run (not on PATH).
    Unavailable,
    /// Not inside a work tree, or the work tree has no valid HEAD commit yet.
    NotARepo,
    /// Inside a work tree but not at its top level: treated as non-git (info banner).
    Subfolder {
        /// The work tree's top level.
        toplevel: PathBuf,
    },
    /// The root is the top level of a work tree.
    Repo(GitInfo),
}

/// Strips HTTP(S) userinfo (`user:token@`) from a remote URL, keeping the scheme, host,
/// port, path and `.git` suffix. Other forms (ssh, scp-style, file) are returned as-is.
pub fn sanitize_origin(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else {
        return url.to_string();
    };
    if !(scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https")) {
        return url.to_string();
    }
    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(authority_end);
    let host = authority.rsplit('@').next().unwrap_or(authority);
    format!("{scheme}://{host}{tail}")
}

/// Collects git metadata for `root` using the `git` on PATH.
pub fn collect(root: &Path) -> GitStatus {
    collect_with(OsStr::new("git"), root)
}

enum RunError {
    /// The program could not be started.
    Spawn,
    /// It ran and exited unsuccessfully.
    Failed,
}

fn run(program: &OsStr, root: &Path, cmd: GitRead) -> Result<Vec<u8>, RunError> {
    let output = Command::new(program)
        .args(read_argv(cmd))
        .current_dir(root)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|err: io::Error| {
            tracing::info!(%err, ?cmd, "git could not be started");
            RunError::Spawn
        })?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(RunError::Failed)
    }
}

fn trimmed(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).trim().to_string()
}

/// [`collect`] with an explicit program (tests pass a nonexistent one).
pub fn collect_with(program: &OsStr, root: &Path) -> GitStatus {
    let toplevel = match run(program, root, GitRead::TopLevel) {
        Ok(out) => PathBuf::from(trimmed(&out)),
        Err(RunError::Spawn) => return GitStatus::Unavailable,
        Err(RunError::Failed) => return GitStatus::NotARepo,
    };
    let (Ok(top), Ok(canonical_root)) = (toplevel.canonicalize(), root.canonicalize()) else {
        return GitStatus::NotARepo;
    };
    if top != canonical_root {
        return GitStatus::Subfolder { toplevel: top };
    }
    let head = match run(program, root, GitRead::Head) {
        Ok(out) => trimmed(&out).to_ascii_lowercase(),
        Err(RunError::Spawn) => return GitStatus::Unavailable,
        Err(RunError::Failed) => return GitStatus::NotARepo,
    };
    if head.len() != 40 || !head.bytes().all(|b| b.is_ascii_hexdigit()) {
        tracing::info!("HEAD is not a 40-hex commit; treating the folder as non-git");
        return GitStatus::NotARepo;
    }
    let origin = run(program, root, GitRead::Origin)
        .ok()
        .map(|out| trimmed(&out))
        .filter(|url| !url.is_empty())
        .map(|url| sanitize_origin(&url));
    let dirty = match run(program, root, GitRead::Status) {
        Ok(out) => parse_porcelain_z(&out),
        Err(_) => {
            tracing::warn!("git status failed; no dirty paths");
            Vec::new()
        }
    };
    GitStatus::Repo(GitInfo {
        head,
        origin,
        dirty,
    })
}

/// Parses `git status --porcelain=v1 -z`. Renames and copies (`R`/`C` in either column)
/// are followed by the original path, which is skipped: the new path is reported.
pub fn parse_porcelain_z(out: &[u8]) -> Vec<String> {
    let mut paths = Vec::new();
    let mut fields = out.split(|b| *b == 0);
    while let Some(record) = fields.next() {
        let (Some(status), Some(path)) = (record.get(..2), record.get(3..)) else {
            continue;
        };
        if path.is_empty() {
            continue;
        }
        paths.push(String::from_utf8_lossy(path).into_owned());
        if status.iter().any(|c| *c == b'R' || *c == b'C') {
            fields.next();
        }
    }
    paths
}
