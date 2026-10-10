//! [`SourceProbe`] over the repository [`Sandbox`] (FR-VAL-06, FR-SRC-08).

use archalign_model::{FileProbe, SourceProbe};

use crate::deny::DenyList;
use crate::sandbox::{RelPath, Sandbox, SandboxError};
use crate::text::{RawLoad, count_lines, read_raw};

/// Probes citation targets through the sandbox, with the viewer's size limit and binary
/// sniff, so a citation is valid exactly when the Source panel can show it.
#[derive(Debug, Clone, Copy)]
pub struct RepoProbe<'a> {
    sandbox: &'a Sandbox,
}

impl<'a> RepoProbe<'a> {
    /// A probe over `sandbox`.
    pub fn new(sandbox: &'a Sandbox) -> Self {
        Self { sandbox }
    }
}

impl SourceProbe for RepoProbe<'_> {
    fn probe(&self, path: &str) -> FileProbe {
        let Ok(rel) = RelPath::parse(path) else {
            return FileProbe::OutsideRoot;
        };
        let abs = match self.sandbox.resolve(&rel) {
            Ok(abs) => abs,
            Err(SandboxError::NotFound(_)) => return FileProbe::Missing,
            Err(SandboxError::Escapes(_) | SandboxError::InvalidPath(_)) => {
                return FileProbe::OutsideRoot;
            }
            Err(SandboxError::Io(_)) => return FileProbe::Unreadable,
        };
        match abs.metadata() {
            Ok(meta) if meta.is_file() => {}
            Ok(_) => return FileProbe::NotAFile,
            Err(_) => return FileProbe::Unreadable,
        }
        match read_raw(self.sandbox, &rel) {
            Ok(RawLoad::Bytes(bytes, _)) => FileProbe::Text {
                line_count: count_lines(&bytes) as u64,
            },
            Ok(RawLoad::Binary(_)) => FileProbe::Binary,
            Ok(RawLoad::TooLarge(size)) => FileProbe::TooLarge { size },
            Err(SandboxError::NotFound(_)) => FileProbe::Missing,
            Err(SandboxError::Escapes(_)) => FileProbe::OutsideRoot,
            Err(_) => FileProbe::Unreadable,
        }
    }

    /// The agent deny-list (FR-SEC-02). Only the Submission profile consults it, so the
    /// UI is unaffected.
    fn is_denied(&self, path: &str) -> bool {
        DenyList::new().is_denied_str(path)
    }
}

/// File → citing nodes and folder → cited-file counts, for tree markers and reverse
/// highlighting (FR-TREE-06, FR-TREE-07).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CitationIndex {
    /// Repo-relative file path → indices of the components citing it (ascending, unique).
    pub by_file: std::collections::BTreeMap<String, Vec<usize>>,
    /// Folder path → number of distinct cited files below it (at any depth).
    pub folder_counts: std::collections::BTreeMap<String, usize>,
    /// Files with at least one broken citation.
    pub broken_files: std::collections::BTreeSet<String>,
}

impl CitationIndex {
    /// Builds the index. `issues` are the document's citation findings; a
    /// `BrokenCitation` at `/components/{i}/sources/{j}…` marks that source's file as broken.
    pub fn build(
        doc: &archalign_model::ArchitectureDoc,
        issues: &[archalign_model::Issue],
    ) -> Self {
        use archalign_model::IssueCode;
        let mut idx = CitationIndex::default();
        for (i, c) in doc.components.iter().enumerate() {
            for s in c.sources() {
                if !archalign_model::citation_path_ok(&s.path) {
                    continue;
                }
                let nodes = idx.by_file.entry(s.path.clone()).or_default();
                if nodes.last() != Some(&i) {
                    nodes.push(i);
                }
            }
        }
        for path in idx.by_file.keys() {
            let mut folder = String::new();
            let segments: Vec<&str> = path.split('/').collect();
            for seg in segments.iter().take(segments.len().saturating_sub(1)) {
                if !folder.is_empty() {
                    folder.push('/');
                }
                folder.push_str(seg);
                *idx.folder_counts.entry(folder.clone()).or_insert(0) += 1;
            }
        }
        for issue in issues {
            if issue.code != IssueCode::BrokenCitation {
                continue;
            }
            let mut parts = issue.pointer.split('/').skip(1);
            let (Some("components"), Some(i), Some("sources"), Some(j)) =
                (parts.next(), parts.next(), parts.next(), parts.next())
            else {
                continue;
            };
            let (Ok(i), Ok(j)) = (i.parse::<usize>(), j.parse::<usize>()) else {
                continue;
            };
            if let Some(s) = doc.components.get(i).and_then(|c| c.sources().get(j))
                && archalign_model::citation_path_ok(&s.path)
            {
                idx.broken_files.insert(s.path.clone());
            }
        }
        idx
    }

    /// Components citing `path`.
    pub fn nodes_citing(&self, path: &str) -> &[usize] {
        self.by_file.get(path).map(Vec::as_slice).unwrap_or(&[])
    }
}
