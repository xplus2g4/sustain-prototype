//! `run.jsonl` (spec Appendix A, FR-STO-02, FR-SEC-06). One JSON object per line with `ts`
//! (RFC 3339, local offset) and `event`. Never file contents, tool output bodies or the key.

use archalign_model::Issue;
use archalign_repo::storage::{append_line, now_rfc3339};
use archalign_repo::{RelPath, Sandbox};
use serde_json::{Map, Value, json};

use crate::budgets::Budgets;
use crate::llm::Usage;

/// How a run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// A candidate was accepted and saved.
    Success,
    /// A cap, an error or the repair limit ended the run.
    Failed,
    /// The user cancelled.
    Cancelled,
}

impl Outcome {
    fn as_str(self) -> &'static str {
        match self {
            Outcome::Success => "success",
            Outcome::Failed => "failed",
            Outcome::Cancelled => "cancelled",
        }
    }
}

/// Repository facts for `run_started`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RepoSummary {
    /// Folder name.
    pub name: String,
    /// Full HEAD, when git is available.
    pub head: Option<String>,
    /// Number of dirty paths.
    pub dirty_count: usize,
}

/// Appends events to `.archify/<folder>/run.jsonl`. If dropped before `finished` (the run
/// future was cancelled), it writes `run_finished` with outcome `cancelled`.
#[derive(Debug)]
pub struct RunLog {
    sandbox: Sandbox,
    rel: RelPath,
    started: std::time::Instant,
    finished: bool,
    turns: u32,
    input_tokens: u64,
    output_tokens: u64,
}

impl RunLog {
    /// A log at `rel` (the folder must exist).
    pub fn new(sandbox: Sandbox, rel: RelPath) -> Self {
        Self {
            sandbox,
            rel,
            started: std::time::Instant::now(),
            finished: false,
            turns: 0,
            input_tokens: 0,
            output_tokens: 0,
        }
    }

    /// The file this log writes.
    pub fn rel(&self) -> &RelPath {
        &self.rel
    }

    fn write(&mut self, event: &str, fields: Value) {
        let mut obj = Map::new();
        obj.insert("ts".into(), Value::from(now_rfc3339()));
        obj.insert("event".into(), Value::from(event));
        if let Value::Object(f) = fields {
            obj.extend(f);
        }
        let line = Value::Object(obj).to_string();
        if let Err(e) = append_line(&self.sandbox, &self.rel, &line) {
            tracing::warn!(target: "archalign::runlog", "can't append to {}: {e}", self.rel);
        }
    }

    /// `run_started`.
    pub fn run_started(
        &mut self,
        model: &str,
        reasoning_effort: Option<&str>,
        focus: &str,
        budgets: &Budgets,
        repo: &RepoSummary,
    ) {
        self.write(
            "run_started",
            json!({
                "model": model,
                "reasoning_effort": reasoning_effort,
                "focus": focus,
                "budgets": budgets,
                "repo": { "name": repo.name, "head": repo.head, "dirty_count": repo.dirty_count },
            }),
        );
    }

    /// `turn`.
    pub fn turn(&mut self, n: u32, usage: Usage, tool_calls: usize) {
        self.turns = n;
        self.input_tokens = self.input_tokens.saturating_add(usage.input_tokens);
        self.output_tokens = self.output_tokens.saturating_add(usage.output_tokens);
        self.write(
            "turn",
            json!({
                "n": n,
                "usage": { "input_tokens": usage.input_tokens, "output_tokens": usage.output_tokens },
                "tool_calls": tool_calls,
            }),
        );
    }

    /// `tool_call`. `args` must already be a sanitized summary (paths, lines, pattern, glob).
    pub fn tool_call(
        &mut self,
        turn: u32,
        name: &str,
        args: Value,
        result_bytes: usize,
        error: Option<&str>,
    ) {
        self.write(
            "tool_call",
            json!({ "turn": turn, "name": name, "args": args, "result_bytes": result_bytes, "error": error }),
        );
    }

    /// `retry`.
    pub fn retry(
        &mut self,
        turn: u32,
        attempt: u32,
        status: Option<u16>,
        timeout: bool,
        wait_ms: u128,
    ) {
        let mut f = json!({ "turn": turn, "attempt": attempt, "wait_ms": wait_ms as u64 });
        match status {
            Some(s) => f["status"] = json!(s),
            None => f["timeout"] = json!(timeout),
        }
        self.write("retry", f);
    }

    /// `fields_dropped`.
    pub fn fields_dropped(&mut self, pointers: &[String]) {
        self.write("fields_dropped", json!({ "pointers": pointers }));
    }

    /// `submission_rejected`.
    pub fn submission_rejected(&mut self, attempt: u32, errors: &[Issue]) {
        let errors: Vec<Value> = errors
            .iter()
            .map(|e| json!({ "pointer": e.pointer, "message": e.message }))
            .collect();
        self.write(
            "submission_rejected",
            json!({ "attempt": attempt, "errors": errors }),
        );
    }

    /// `run_finished`.
    pub fn finished(&mut self, outcome: Outcome, reason: &str, candidate: Option<&RelPath>) {
        if self.finished {
            return;
        }
        self.finished = true;
        let elapsed_ms = self.started.elapsed().as_millis() as u64;
        self.write(
            "run_finished",
            json!({
                "outcome": outcome.as_str(),
                "reason": reason,
                "turns": self.turns,
                "input_tokens": self.input_tokens,
                "output_tokens": self.output_tokens,
                "elapsed_ms": elapsed_ms,
                "candidate": candidate.map(|c| c.to_string()),
            }),
        );
    }
}

impl Drop for RunLog {
    fn drop(&mut self) {
        if !self.finished {
            self.finished(Outcome::Cancelled, "cancelled by the user", None);
        }
    }
}
