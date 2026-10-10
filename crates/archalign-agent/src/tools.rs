//! The agent's read-only tools (FR-GEN-04, FR-SEC-01/02, ADR 0025).

use archalign_repo::agent_fs::{self, GrepOptions};
use archalign_repo::text::{TextLoad, load_text};
use archalign_repo::{DenyList, RelPath, Sandbox};
use serde::Deserialize;
use serde_json::{Value, json};

/// Any tool result is capped at 24 KB (Appendix C, Contract).
pub const RESULT_CAP: usize = 24 * 1024;
/// `read_file` returns at most 400 lines per call (Appendix C, Contract).
pub const READ_MAX_LINES: usize = 400;
/// `repo_meta` lists at most 200 dirty paths.
pub const DIRTY_MAX: usize = 200;

/// Git facts the tools may report (credential-free).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RepoMeta {
    /// Whether git metadata is available.
    pub git: bool,
    /// HEAD.
    pub head: Option<String>,
    /// Sanitized origin.
    pub origin: Option<String>,
    /// Dirty paths.
    pub dirty: Vec<String>,
}

/// The outcome of one non-submit tool call.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolOutput {
    /// Text returned to the model (≤ 24 KB).
    pub content: String,
    /// Set when the call failed (also returned to the model inside `content`).
    pub error: Option<String>,
    /// Sanitized argument summary for `run.jsonl` (paths, lines, pattern, glob only).
    pub args: Value,
}

impl ToolOutput {
    fn ok(content: String, args: Value) -> Self {
        Self {
            content: cap(content),
            error: None,
            args,
        }
    }

    fn err(message: impl Into<String>, args: Value) -> Self {
        let message = message.into();
        Self {
            content: cap(format!("error: {message}")),
            error: Some(message),
            args,
        }
    }
}

/// Truncates to [`RESULT_CAP`] bytes on a char boundary, with a notice.
pub fn cap(mut s: String) -> String {
    if s.len() <= RESULT_CAP {
        return s;
    }
    const NOTICE: &str = "\n[truncated: result exceeded 24 KB]";
    let mut end = RESULT_CAP - NOTICE.len();
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    s.truncate(end);
    s.push_str(NOTICE);
    s
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PathArgs {
    path: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadArgs {
    path: String,
    #[serde(default)]
    start_line: Option<u64>,
    #[serde(default)]
    end_line: Option<u64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GrepArgs {
    pattern: String,
    #[serde(default)]
    path_glob: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NoArgs {}

/// Tool names.
pub const LIST_DIR: &str = "list_dir";
/// Tool names.
pub const READ_FILE: &str = "read_file";
/// Tool names.
pub const GREP: &str = "grep";
/// Tool names.
pub const REPO_META: &str = "repo_meta";
/// Tool names.
pub const SUBMIT: &str = "submit_diagram";

/// OpenAI function-tool schemas sent with every turn.
pub fn schemas() -> Value {
    let f = |name: &str, description: &str, parameters: Value| json!({ "type": "function", "function": { "name": name, "description": description, "parameters": parameters } });
    json!([
        f(
            LIST_DIR,
            "List one folder of the repository. Folders end with '/', files show their size. Ignored and secret files are hidden.",
            json!({ "type": "object", "properties": { "path": { "type": "string", "description": "Repository-relative folder path; \"\" is the root." } }, "required": ["path"], "additionalProperties": false })
        ),
        f(
            READ_FILE,
            "Read a text file as numbered lines 'N: text'. At most 400 lines and 24 KB per call; a notice gives the next start_line when truncated.",
            json!({ "type": "object", "properties": {
              "path": { "type": "string", "description": "Repository-relative file path." },
              "start_line": { "type": "integer", "minimum": 1, "description": "First line (1-based, default 1)." },
              "end_line": { "type": "integer", "minimum": 1, "description": "Last line (inclusive)." } },
            "required": ["path"], "additionalProperties": false })
        ),
        f(
            GREP,
            "Search file contents with a Rust regex. Returns 'path:line: text', at most 100 matches. Skips binaries and files over 1 MB.",
            json!({ "type": "object", "properties": {
              "pattern": { "type": "string", "description": "Rust regex syntax (no look-around or backreferences)." },
              "path_glob": { "type": "string", "description": "Optional gitignore-style glob, e.g. 'src/**/*.rs'." } },
            "required": ["pattern"], "additionalProperties": false })
        ),
        f(
            REPO_META,
            "Git availability, HEAD, the credential-free origin and up to 200 dirty paths.",
            json!({ "type": "object", "properties": {}, "additionalProperties": false })
        ),
        f(
            SUBMIT,
            "Submit the complete Archify architecture diagram. Returns 'accepted' or a list of {pointer, message} errors to fix.",
            json!({ "type": "object", "properties": { "diagram": { "type": "object", "description": "The complete diagram object." } }, "required": ["diagram"], "additionalProperties": false })
        ),
    ])
}

/// The confined tool set over one repository.
#[derive(Debug, Clone)]
pub struct ToolSet {
    sandbox: Sandbox,
    deny: DenyList,
    meta: RepoMeta,
    grep: GrepOptions,
}

impl ToolSet {
    /// Tools over `sandbox`, reporting `meta` from `repo_meta`.
    pub fn new(sandbox: Sandbox, meta: RepoMeta) -> Self {
        Self {
            sandbox,
            deny: DenyList::new(),
            meta,
            grep: GrepOptions::default(),
        }
    }

    /// The sandbox.
    pub fn sandbox(&self) -> &Sandbox {
        &self.sandbox
    }

    /// Runs a non-submit tool with its raw JSON arguments. Blocking: call from
    /// `spawn_blocking`.
    pub fn execute(&self, name: &str, arguments: &str) -> ToolOutput {
        match name {
            LIST_DIR => match parse::<PathArgs>(name, arguments) {
                Ok(a) => self.list_dir(&a.path),
                Err(e) => e,
            },
            READ_FILE => match parse::<ReadArgs>(name, arguments) {
                Ok(a) => self.read_file(&a.path, a.start_line, a.end_line),
                Err(e) => e,
            },
            GREP => match parse::<GrepArgs>(name, arguments) {
                Ok(a) => self.grep(&a.pattern, a.path_glob.as_deref()),
                Err(e) => e,
            },
            REPO_META => match parse::<NoArgs>(name, arguments) {
                Ok(_) => self.repo_meta(),
                Err(e) => e,
            },
            other => ToolOutput::err(format!("unknown tool `{other}`"), json!({})),
        }
    }

    fn rel(&self, path: &str, args: &Value) -> Result<RelPath, ToolOutput> {
        let rel = RelPath::parse(path).map_err(|e| ToolOutput::err(e.to_string(), args.clone()))?;
        if self.deny.is_denied_in(&self.sandbox, &rel) {
            tracing::info!(target: "archalign::agent", "denied tool access to {rel}");
            return Err(ToolOutput::err(
                format!("denied: {rel} is not available to the agent"),
                args.clone(),
            ));
        }
        Ok(rel)
    }

    fn list_dir(&self, path: &str) -> ToolOutput {
        let args = json!({ "path": path });
        let rel = match self.rel(path, &args) {
            Ok(r) => r,
            Err(e) => return e,
        };
        match agent_fs::list_for_agent(&self.sandbox, &self.deny, &rel, agent_fs::LIST_DIR_MAX) {
            Ok(listing) => {
                let mut out = listing.lines.join("\n");
                if listing.lines.is_empty() {
                    out.push_str("(empty)");
                }
                if listing.omitted > 0 {
                    out.push_str(&format!(
                        "\n[truncated: {} more entries not shown; list subfolders or grep instead]",
                        listing.omitted
                    ));
                }
                ToolOutput::ok(out, args)
            }
            Err(e) => ToolOutput::err(e.to_string(), args),
        }
    }

    fn read_file(&self, path: &str, start: Option<u64>, end: Option<u64>) -> ToolOutput {
        let args = json!({ "path": path, "start_line": start, "end_line": end });
        let rel = match self.rel(path, &args) {
            Ok(r) => r,
            Err(e) => return e,
        };
        let (text, index) = match load_text(&self.sandbox, &rel) {
            Ok(TextLoad::Text { text, index, .. }) => (text, index),
            Ok(TextLoad::Binary { .. }) => {
                return ToolOutput::err(format!("{rel} is a binary file"), args);
            }
            Ok(TextLoad::TooLarge { size }) => {
                return ToolOutput::err(format!("{rel} is larger than 2 MB ({size} bytes)"), args);
            }
            Err(e) => return ToolOutput::err(e.to_string(), args),
        };
        let total = index.line_count();
        let start = start.unwrap_or(1).max(1) as usize;
        if total == 0 {
            return ToolOutput::ok(format!("({rel} is empty)"), args);
        }
        if start > total {
            return ToolOutput::err(
                format!("start_line {start} is beyond the end of {rel} ({total} lines)"),
                args,
            );
        }
        let requested_end = end.map(|e| e as usize).unwrap_or(total).min(total);
        if requested_end < start {
            return ToolOutput::err("end_line is before start_line", args);
        }
        let last = requested_end.min(start + READ_MAX_LINES - 1);
        let mut out = String::new();
        let budget = RESULT_CAP - 200;
        let mut shown_to = start - 1;
        for n in start..=last {
            let line = format!("{n}: {}\n", index.line(&text, n - 1));
            if out.len() + line.len() > budget {
                break;
            }
            out.push_str(&line);
            shown_to = n;
        }
        if shown_to == start - 1 {
            // A single line larger than the budget: cut it.
            let mut line = format!("{start}: {}", index.line(&text, start - 1));
            let mut end = budget.min(line.len());
            while !line.is_char_boundary(end) {
                end -= 1;
            }
            line.truncate(end);
            out.push_str(&line);
            out.push('\n');
            shown_to = start;
        }
        if shown_to < requested_end {
            out.push_str(&format!(
                "[truncated: showing lines {start}–{shown_to} of {total}; continue with start_line {}]",
                shown_to + 1
            ));
        } else {
            out.push_str(&format!("[{rel}: {total} lines]"));
        }
        ToolOutput::ok(out, args)
    }

    fn grep(&self, pattern: &str, glob: Option<&str>) -> ToolOutput {
        let args = json!({ "pattern": pattern, "path_glob": glob });
        let re = match agent_fs::compile_pattern(pattern) {
            Ok(r) => r,
            Err(e) => return ToolOutput::err(format!("invalid regex: {e}"), args),
        };
        match agent_fs::grep(&self.sandbox, &self.deny, &re, glob, &self.grep) {
            Ok(r) => {
                let mut out: Vec<String> = r
                    .matches
                    .iter()
                    .map(|m| format!("{}:{}: {}", m.path, m.line, m.text))
                    .collect();
                if out.is_empty() {
                    out.push("(no matches)".into());
                }
                if r.truncated {
                    out.push(format!(
                        "[truncated at {} matches; narrow the pattern or glob]",
                        self.grep.max_matches
                    ));
                }
                if r.timed_out {
                    out.push("[stopped after 5 s; results are partial]".into());
                }
                if r.skipped > 0 {
                    out.push(format!("[{} binary or large files skipped]", r.skipped));
                }
                ToolOutput::ok(out.join("\n"), args)
            }
            Err(e) => ToolOutput::err(e.to_string(), args),
        }
    }

    fn repo_meta(&self) -> ToolOutput {
        let m = &self.meta;
        let dirty: Vec<&String> = m.dirty.iter().take(DIRTY_MAX).collect();
        let v = json!({
            "git": m.git,
            "head": m.head,
            "origin": m.origin,
            "dirty_paths": dirty,
            "dirty_total": m.dirty.len(),
        });
        ToolOutput::ok(v.to_string(), json!({}))
    }
}

fn parse<T: for<'de> Deserialize<'de>>(name: &str, arguments: &str) -> Result<T, ToolOutput> {
    let raw = if arguments.trim().is_empty() {
        "{}"
    } else {
        arguments
    };
    serde_json::from_str(raw)
        .map_err(|e| ToolOutput::err(format!("invalid arguments for {name}: {e}"), json!({})))
}

/// Whether a tool output is an argument-parsing error (counts toward turns, not repairs).
pub fn is_invalid_args(out: &ToolOutput) -> bool {
    out.error
        .as_deref()
        .is_some_and(|e| e.starts_with("invalid arguments"))
}
