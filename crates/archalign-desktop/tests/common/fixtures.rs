//! Temporary repositories for UI tests.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};

pub struct TempRepo {
    _dir: tempfile::TempDir,
    pub root: PathBuf,
}

impl TempRepo {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().canonicalize().expect("canonical tempdir");
        Self { _dir: dir, root }
    }

    pub fn path(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    pub fn write(&self, rel: &str, bytes: impl AsRef<[u8]>) -> &Self {
        let p = self.root.join(rel);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).expect("mkdir");
        }
        std::fs::write(p, bytes).expect("write");
        self
    }

    pub fn mkdir(&self, rel: &str) -> &Self {
        std::fs::create_dir_all(self.root.join(rel)).expect("mkdir");
        self
    }

    /// Writes `.archify/<folder>/candidate.json`.
    pub fn diagram(&self, folder: &str, doc: &Value) -> &Self {
        self.write(
            &format!(".archify/{folder}/candidate.json"),
            serde_json::to_vec_pretty(doc).expect("json"),
        )
    }

    pub fn git(&self, args: &[&str]) {
        let ok = Command::new("git")
            .args(args)
            .current_dir(&self.root)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .expect("git runs")
            .status
            .success();
        assert!(ok, "git {args:?} failed");
    }

    /// `git init` + one commit of everything.
    pub fn git_commit_all(&self) {
        self.git(&["init", "-q", "-b", "main"]);
        self.git(&["config", "user.email", "t@example.com"]);
        self.git(&["config", "user.name", "T"]);
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", "init"]);
    }
}

/// `n` numbered lines.
pub fn lines(n: usize) -> String {
    (1..=n).map(|i| format!("line {i}\n")).collect()
}

/// A small valid architecture document with authored positions citing `src/` files.
pub fn sample_doc(title: &str) -> Value {
    json!({
        "schema_version": 1,
        "diagram_type": "architecture",
        "meta": { "title": title, "output": "out/x.html" },
        "components": [
            { "id": "api", "type": "backend", "label": "API", "pos": [40, 40], "size": [160, 70],
              "sources": [{ "path": "src/api.rs", "line": 10, "end_line": 20 }] },
            { "id": "db", "type": "database", "label": "DB", "pos": [400, 40], "size": [160, 70],
              "sources": [{ "path": "src/db/store.rs", "line": 3 }] },
            { "id": "ui", "type": "frontend", "label": "UI", "pos": [40, 260], "size": [160, 70],
              "sources": [{ "path": "README.md" }] }
        ],
        "connections": [
            { "id": "ui-api", "from": "ui", "to": "api", "label": "calls" },
            { "id": "api-db", "from": "api", "to": "db" }
        ]
    })
}

/// A repo with source files matching [`sample_doc`].
pub fn sample_repo() -> TempRepo {
    let r = TempRepo::new();
    r.write("src/api.rs", lines(120))
        .write("src/db/store.rs", lines(40))
        .write("README.md", "# Sample\n\nHello.\n")
        .write("Cargo.toml", "[package]\nname = \"sample\"\n");
    r
}

pub fn exists(p: &Path) -> bool {
    p.exists()
}
