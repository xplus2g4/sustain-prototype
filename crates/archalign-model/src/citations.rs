//! Citation checks (FR-VAL-06, FR-VAL-07).
//!
//! The model does no file I/O: whatever needs the repository comes in through
//! [`SourceProbe`], which `archalign-repo` implements over its `Sandbox`.

use std::collections::{HashMap, HashSet};
use std::fmt;

use crate::checks::Profile;
use crate::issue::{Issue, IssueCode, Severity};
use crate::types::{ArchitectureDoc, SourceRef};

/// What a probe found at a repo-relative path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileProbe {
    /// A readable text file within the size limit.
    Text {
        /// Number of lines, not counting the phantom line after a trailing newline.
        line_count: u64,
    },
    /// Nothing exists at the path.
    Missing,
    /// The path exists but is not a regular file (a folder, a socket, …).
    NotAFile,
    /// The file looks binary (a NUL byte in its first 8 KB).
    Binary,
    /// The file is larger than the viewer limit (2 MB).
    TooLarge {
        /// Size in bytes.
        size: u64,
    },
    /// The file exists but could not be read.
    Unreadable,
    /// The path resolves outside the repository root (for example through a symlink).
    OutsideRoot,
}

/// File access for citation checks, implemented outside the model.
pub trait SourceProbe {
    /// Inspects a repo-relative path that already passed [`citation_path_ok`].
    fn probe(&self, path: &str) -> FileProbe;
    /// Whether the path is on the agent deny-list (FR-SEC-02).
    fn is_denied(&self, path: &str) -> bool;
}

/// The path-syntax rule of FR-VAL-06.
///
/// A valid citation path is non-empty, relative (no leading `/`, no `C:` drive prefix),
/// uses `/` separators only (no `\`), contains no NUL, and every `/`-separated segment is
/// non-empty and neither `.` nor `..`.
pub fn citation_path_ok(path: &str) -> bool {
    if path.is_empty() || path.contains('\0') || path.contains('\\') || path.starts_with('/') {
        return false;
    }
    if has_drive_prefix(path) {
        return false;
    }
    path.split('/')
        .all(|seg| !seg.is_empty() && seg != "." && seg != "..")
}

/// `C:`-style prefixes are absolute on Windows; reject them everywhere.
fn has_drive_prefix(path: &str) -> bool {
    let mut chars = path.chars();
    matches!(
        (chars.next(), chars.next()),
        (Some(letter), Some(':')) if letter.is_ascii_alphabetic()
    )
}

/// Formats a line count as "1 line" / "N lines".
struct Lines(u64);

impl fmt::Display for Lines {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0 == 1 {
            f.write_str("1 line")
        } else {
            write!(f, "{} lines", self.0)
        }
    }
}

/// A range problem: which field it concerns and the message.
fn range_problem(src: &SourceRef, line_count: u64) -> Option<(&'static str, String)> {
    let path = &src.path;
    if src.line == Some(0) {
        return Some((
            "line",
            format!("Cited line 0 in {path} is invalid; lines start at 1"),
        ));
    }
    match (src.line, src.end_line) {
        (_, Some(0)) => Some((
            "end_line",
            format!("Cited end line 0 in {path} is invalid; lines start at 1"),
        )),
        (Some(line), Some(end)) if end < line => Some((
            "end_line",
            format!("Cited range in {path} ends at line {end}, before its first line {line}"),
        )),
        (Some(line), Some(end)) if line > line_count => Some((
            "line",
            format!(
                "Cited lines {line}–{end} are outside {path} ({})",
                Lines(line_count)
            ),
        )),
        (Some(line), Some(end)) if end > line_count => Some((
            "end_line",
            format!(
                "Cited lines {line}–{end} are outside {path} ({})",
                Lines(line_count)
            ),
        )),
        (None, Some(end)) if end > line_count => Some((
            "end_line",
            format!(
                "Cited end line {end} is outside {path} ({})",
                Lines(line_count)
            ),
        )),
        (Some(line), None) if line > line_count => Some((
            "line",
            format!(
                "Cited line {line} is outside {path} ({})",
                Lines(line_count)
            ),
        )),
        _ => None,
    }
}

/// Why a probed file cannot be cited, or `None` for text files.
fn probe_problem(path: &str, probe: FileProbe) -> Option<(IssueCode, String)> {
    let broken = IssueCode::BrokenCitation;
    match probe {
        FileProbe::Text { .. } => None,
        FileProbe::Missing => Some((broken, format!("{path} does not exist"))),
        FileProbe::NotAFile => Some((broken, format!("{path} is not a file"))),
        FileProbe::Binary => Some((broken, format!("{path} is a binary file"))),
        FileProbe::TooLarge { size } => Some((
            broken,
            format!("{path} is too large to cite ({size} bytes; the limit is 2 MB)"),
        )),
        FileProbe::Unreadable => Some((broken, format!("{path} cannot be read"))),
        FileProbe::OutsideRoot => Some((
            IssueCode::InvalidCitationPath,
            format!("{path} resolves outside the repository root"),
        )),
    }
}

/// Runs FR-VAL-06 and FR-VAL-07 over every `components[i].sources[j]`.
///
/// Pointers are `/components/{i}/sources/{j}/path` for path problems and
/// `.../line` or `.../end_line` for range problems.
///
/// - Bad syntax or a path resolving outside the root: [`IssueCode::InvalidCitationPath`].
/// - Missing, not a file, binary, too large, unreadable, or lines out of range:
///   [`IssueCode::BrokenCitation`].
/// - Under [`Profile::Submission`], a denied path: [`IssueCode::DeniedCitation`] (the
///   file is not probed).
/// - A path in `dirty` (exact repo-relative match): [`IssueCode::ModifiedCitation`],
///   always a warning.
///
/// Under [`Profile::Import`] everything is a warning; under [`Profile::Submission`]
/// everything except [`IssueCode::ModifiedCitation`] is an error. Each distinct path is
/// probed at most once per call.
pub fn check_citations(
    doc: &ArchitectureDoc,
    probe: &dyn SourceProbe,
    profile: Profile,
    dirty: &HashSet<String>,
) -> Vec<Issue> {
    let severity = match profile {
        Profile::Import => Severity::Warning,
        Profile::Submission => Severity::Error,
    };
    let mut cache: HashMap<&str, FileProbe> = HashMap::new();
    let mut out = Vec::new();
    for (i, component) in doc.components.iter().enumerate() {
        for (j, src) in component.sources().iter().enumerate() {
            let base = format!("/components/{i}/sources/{j}");
            let path = src.path.as_str();
            if !citation_path_ok(path) {
                out.push(Issue {
                    severity,
                    code: IssueCode::InvalidCitationPath,
                    pointer: format!("{base}/path"),
                    message: format!(
                        "`{path}` is not a valid repository-relative path (use `/` separators, no `.` or `..` segments, not absolute)"
                    ),
                });
                continue;
            }
            if profile == Profile::Submission && probe.is_denied(path) {
                out.push(Issue {
                    severity,
                    code: IssueCode::DeniedCitation,
                    pointer: format!("{base}/path"),
                    message: format!("{path} is on the agent deny-list and cannot be cited"),
                });
                continue;
            }
            let found = *cache.entry(path).or_insert_with(|| probe.probe(path));
            if let Some((code, message)) = probe_problem(path, found) {
                out.push(Issue {
                    severity,
                    code,
                    pointer: format!("{base}/path"),
                    message,
                });
            } else if let FileProbe::Text { line_count } = found
                && let Some((field, message)) = range_problem(src, line_count)
            {
                out.push(Issue {
                    severity,
                    code: IssueCode::BrokenCitation,
                    pointer: format!("{base}/{field}"),
                    message,
                });
            }
            if dirty.contains(path) {
                out.push(Issue::warning(
                    IssueCode::ModifiedCitation,
                    format!("{base}/path"),
                    format!("{path} has uncommitted changes; the cited lines may have moved"),
                ));
            }
        }
    }
    out
}
