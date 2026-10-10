//! The system prompt (ADR 0026) and the seed message (FR-GEN-02, FR-GEN-03).

use archalign_repo::agent_fs::{head_lines, seed_tree};
use archalign_repo::{DenyList, RelPath, Sandbox};

/// The system prompt, condensed from Archify (MIT); see `prompts/system.md`.
pub const SYSTEM_PROMPT: &str = include_str!("../prompts/system.md");

/// Seed limits (Appendix C, Tunable).
pub const SEED_TREE_DEPTH: usize = 2;
/// Seed limits (Appendix C, Tunable).
pub const SEED_TREE_ENTRIES: usize = 400;
/// Seed limits (Appendix C, Tunable).
pub const README_LINES: usize = 150;
/// Seed limits (Appendix C, Tunable).
pub const MANIFEST_LINES: usize = 120;
/// Seed limits (Appendix C, Tunable).
pub const SEED_MAX_BYTES: usize = 48 * 1024;

/// README candidates, first match wins.
pub const READMES: [&str; 4] = ["README.md", "README.rst", "README.txt", "README"];

/// Top-level manifests included when present.
pub const MANIFESTS: [&str; 16] = [
    "Cargo.toml",
    "package.json",
    "pyproject.toml",
    "setup.py",
    "requirements.txt",
    "go.mod",
    "pom.xml",
    "build.gradle",
    "build.gradle.kts",
    "settings.gradle",
    "settings.gradle.kts",
    "Gemfile",
    "composer.json",
    "Dockerfile",
    "docker-compose.yml",
    "docker-compose.yaml",
];

/// `Makefile` is listed separately in the spec; it is included too.
pub const MAKEFILE: &str = "Makefile";

/// Inputs to the seed message.
#[derive(Debug, Clone, Default)]
pub struct SeedInput {
    /// Focus text (may be empty).
    pub focus: String,
    /// Repository folder name.
    pub repo_name: String,
    /// Short HEAD, when git is available.
    pub short_head: Option<String>,
    /// Number of dirty paths.
    pub dirty_count: usize,
}

fn fenced(name: &str, body: &str) -> String {
    format!("### {name}\n```\n{body}\n```\n")
}

/// Builds the first user message (≤ 48 KB): Focus, repository name and git summary, a
/// depth-2 tree, the start of the README and the top-level manifests. Ignore and deny
/// rules apply to everything.
pub fn seed_message(sandbox: &Sandbox, input: &SeedInput) -> String {
    let deny = DenyList::new();
    let mut out = String::new();
    out.push_str("# Task\n\nCreate an Archify architecture diagram of this repository and submit it with `submit_diagram`.\n\n");
    if input.focus.trim().is_empty() {
        out.push_str("Focus: none given. Make a system overview led by the main user journey.\n\n");
    } else {
        out.push_str(&format!(
            "Focus (from the user): {}\n\n",
            input.focus.trim()
        ));
    }
    out.push_str(&format!("Repository folder: {}\n", input.repo_name));
    match &input.short_head {
        Some(h) => out.push_str(&format!(
            "Git: HEAD {h}, {} uncommitted path{}\n\n",
            input.dirty_count,
            if input.dirty_count == 1 { "" } else { "s" }
        )),
        None => out.push_str("Git: not available\n\n"),
    }
    let tree = seed_tree(sandbox, &deny, SEED_TREE_DEPTH, SEED_TREE_ENTRIES);
    let mut tree_text = tree.lines.join("\n");
    if tree.truncated {
        tree_text.push_str(&format!(
            "\n… (tree truncated at {SEED_TREE_ENTRIES} entries)"
        ));
    }
    out.push_str(&fenced("Tree (depth 2)", &tree_text));

    let mut sections = Vec::new();
    for name in READMES {
        let Ok(rel) = RelPath::parse(name) else {
            continue;
        };
        if let Some(text) = head_lines(sandbox, &deny, &rel, README_LINES) {
            sections.push(fenced(
                &format!("{name} (first {README_LINES} lines)"),
                &text,
            ));
            break;
        }
    }
    for name in MANIFESTS.iter().copied().chain(std::iter::once(MAKEFILE)) {
        let Ok(rel) = RelPath::parse(name) else {
            continue;
        };
        if let Some(text) = head_lines(sandbox, &deny, &rel, MANIFEST_LINES) {
            sections.push(fenced(
                &format!("{name} (first {MANIFEST_LINES} lines)"),
                &text,
            ));
        }
    }
    for s in sections {
        if out.len() + s.len() > SEED_MAX_BYTES {
            out.push_str("\n[more files omitted: the seed message is capped at 48 KB]\n");
            break;
        }
        out.push_str(&s);
    }
    if out.len() > SEED_MAX_BYTES {
        let mut end = SEED_MAX_BYTES;
        while !out.is_char_boundary(end) {
            end -= 1;
        }
        out.truncate(end);
    }
    out
}

/// The reminder sent after a reply without tool calls (FR-GEN-05).
pub const REMINDER: &str = "Use the tools to inspect the repository, then call `submit_diagram` with the complete diagram. Replies without a tool call don't make progress.";
