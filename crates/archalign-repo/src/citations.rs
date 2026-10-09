//! [`SourceProbe`] over the repository [`Sandbox`] (FR-VAL-06, FR-SRC-08).

use archalign_model::{FileProbe, SourceProbe};

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

    /// Always `false` for now: milestone M5 wires the agent deny-list (FR-SEC-02) here.
    fn is_denied(&self, _path: &str) -> bool {
        false
    }
}
