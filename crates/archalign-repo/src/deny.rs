//! The agent deny-list (FR-SEC-02, ADR 0025).
//!
//! Paths the agent tools never return: left out of listings, grep results and the seed
//! message, and refused on direct reads. The UI tree and Source panel don't consult it.
//!
//! Rules apply to every path segment, so `config/.env` and `a/.git/HEAD` are denied.
//! They match case-insensitively: on case-insensitive file systems (the macOS default)
//! `.ENV` opens the same file as `.env`.

use crate::sandbox::{RelPath, Sandbox};

/// Segment names denied outright (lowercase).
const DENIED_NAMES: [&str; 5] = [".git", ".archify", ".npmrc", ".pypirc", ".netrc"];
/// `.env.*` names that stay visible (lowercase).
const ENV_EXCEPTIONS: [&str; 2] = [".env.example", ".env.sample"];
/// Denied name prefixes (lowercase).
const DENIED_PREFIXES: [&str; 2] = ["id_rsa", "id_ed25519"];
/// Denied extensions (lowercase).
const DENIED_EXTENSIONS: [&str; 4] = ["pem", "key", "p12", "pfx"];

/// The fixed v1 deny-list.
#[derive(Debug, Clone, Copy, Default)]
pub struct DenyList;

impl DenyList {
    /// The deny-list.
    pub fn new() -> Self {
        DenyList
    }

    /// Whether any segment of `rel` matches a rule.
    pub fn is_denied(&self, rel: &RelPath) -> bool {
        rel.components().any(segment_denied)
    }

    /// [`DenyList::is_denied`] on a path string; paths that don't parse as a
    /// [`RelPath`] count as denied.
    pub fn is_denied_str(&self, path: &str) -> bool {
        RelPath::parse(path).map_or(true, |rel| self.is_denied(&rel))
    }

    /// Whether `rel` is denied either as written or through the in-root target a symlink
    /// on the way resolves to (`notes -> .env`). Targets that don't exist or resolve
    /// outside the root are not checked here; [`Sandbox::resolve`] rejects them.
    pub fn is_denied_in(&self, sandbox: &Sandbox, rel: &RelPath) -> bool {
        if self.is_denied(rel) {
            return true;
        }
        sandbox
            .resolve(rel)
            .ok()
            .and_then(|abs| sandbox.relativize(&abs))
            .is_some_and(|target| self.is_denied(&target))
    }
}

/// Whether one path segment matches a rule.
fn segment_denied(segment: &str) -> bool {
    let lower = segment.to_ascii_lowercase();
    let name = lower.as_str();
    if DENIED_NAMES.contains(&name) {
        return true;
    }
    if name == ".env" || (name.starts_with(".env.") && !ENV_EXCEPTIONS.contains(&name)) {
        return true;
    }
    if DENIED_PREFIXES.iter().any(|p| name.starts_with(p)) {
        return true;
    }
    name.rsplit_once('.')
        .is_some_and(|(_, ext)| DENIED_EXTENSIONS.contains(&ext))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_rules() {
        for denied in [
            ".git",
            ".archify",
            ".npmrc",
            ".pypirc",
            ".netrc",
            ".env",
            ".env.local",
            ".env.production",
            "server.pem",
            "tls.key",
            "cert.p12",
            "cert.pfx",
            "id_rsa",
            "id_rsa.pub",
            "id_ed25519",
            "id_ed25519_work.pub",
            ".ENV",
            "Server.PEM",
            ".GIT",
        ] {
            assert!(segment_denied(denied), "{denied} should be denied");
        }
        for allowed in [
            ".env.example",
            ".env.sample",
            ".ENV.Example",
            ".envrc",
            "env",
            "dotenv.rs",
            ".gitignore",
            ".github",
            "archify",
            "keys",
            "keyboard.rs",
            "key",
            "pem.rs",
            "my_id_rsa",
            "npmrc",
            "server.pem.txt",
        ] {
            assert!(!segment_denied(allowed), "{allowed} should be allowed");
        }
    }
}
