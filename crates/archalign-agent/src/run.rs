//! The agent loop (FR-GEN-05..10, ADR 0020/0028).

use std::collections::HashSet;
use std::time::Duration;

use archalign_model::{AppOwned, ArchitectureDoc, Issue};
use archalign_repo::RelPath;
use serde_json::Value;

use crate::budgets::{Budgets, Cap};
use crate::gate::{self, Verdict};
use crate::llm::{BoxFuture, ChatMessage, LlmClient, LlmError, TurnRequest, Usage};
use crate::prompt::{REMINDER, SYSTEM_PROMPT};
use crate::retry::{RetryPolicy, Sleeper, with_retries};
use crate::runlog::{Outcome, RepoSummary, RunLog};
use crate::tools::{self, ToolSet};

/// Everything that configures one run.
#[derive(Debug, Clone)]
pub struct RunConfig {
    /// litellm `provider/model`.
    pub model: String,
    /// Passed through unchanged when set.
    pub reasoning_effort: Option<String>,
    /// Caps.
    pub budgets: Budgets,
    /// Overall time limit (normally `budgets.wall_clock()`).
    pub wall_clock: Duration,
    /// Per-request limit (normally `budgets.request_timeout()`).
    pub request_timeout: Duration,
    /// Retry policy.
    pub retry: RetryPolicy,
    /// Focus text.
    pub focus: String,
    /// The first user message.
    pub seed: String,
    /// For `run_started`.
    pub repo: RepoSummary,
    /// App-owned fields for the gate.
    pub app_owned: AppOwned,
    /// Dirty paths (citations of these get a warning, never an error).
    pub dirty: HashSet<String>,
}

impl RunConfig {
    /// A config with the budgets' own time limits and the default retry policy.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        model: String,
        reasoning_effort: Option<String>,
        budgets: Budgets,
        focus: String,
        seed: String,
        repo: RepoSummary,
        app_owned: AppOwned,
        dirty: HashSet<String>,
    ) -> Self {
        Self {
            model,
            reasoning_effort,
            wall_clock: budgets.wall_clock(),
            request_timeout: budgets.request_timeout(),
            budgets,
            retry: RetryPolicy::default(),
            focus,
            seed,
            repo,
            app_owned,
            dirty,
        }
    }
}

/// Progress events streamed to the UI (FR-GEN-09).
#[derive(Debug, Clone, PartialEq)]
pub enum AgentEvent {
    /// A turn started.
    TurnStarted {
        /// Turn number (1-based).
        n: u32,
        /// Turn budget.
        max: u32,
    },
    /// Token totals after a turn.
    Usage {
        /// Turn number.
        n: u32,
        /// Input tokens so far.
        input_tokens: u64,
        /// Output tokens so far.
        output_tokens: u64,
    },
    /// A request is being retried.
    Retry {
        /// Retry number.
        attempt: u32,
        /// Why.
        reason: String,
        /// Wait before retrying.
        wait: Duration,
    },
    /// A tool was called.
    ToolCalled {
        /// Tool name.
        name: String,
        /// Short argument summary (path, lines, pattern).
        summary: String,
    },
    /// A tool finished.
    ToolResult {
        /// Tool name.
        name: String,
        /// Result size in bytes.
        bytes: usize,
        /// Error, if any.
        error: Option<String>,
    },
    /// A submission was rejected.
    Rejected {
        /// Rejection number.
        attempt: u32,
        /// Number of errors.
        count: usize,
        /// The first few errors (`pointer: message`).
        first: Vec<String>,
    },
    /// The run ended.
    Finished(RunOutcome),
}

/// How a run ended.
#[derive(Debug, Clone, PartialEq)]
pub enum RunOutcome {
    /// The accepted diagram was saved here.
    Success {
        /// `.archify/<folder>/candidate.json`.
        candidate: RelPath,
    },
    /// The run failed; the reason names the cap or the error.
    Failed {
        /// User-facing reason.
        reason: String,
    },
}

/// Receives progress events.
pub trait EventSink: Send {
    /// Delivers one event (may wait for the receiver).
    fn emit(&mut self, event: AgentEvent) -> BoxFuture<'_, ()>;
}

/// Lays out and saves an accepted document; returns the candidate path (desktop-supplied).
pub trait Finalize: Send + Sync {
    /// Lays out, writes back integer geometry and saves `candidate.json` atomically.
    fn finalize(&self, doc: ArchitectureDoc) -> BoxFuture<'_, Result<RelPath, String>>;
}

fn summarize_args(args: &Value) -> String {
    let mut parts = Vec::new();
    if let Some(Value::Object(m)) = Some(args) {
        for (k, v) in m {
            if v.is_null() {
                continue;
            }
            let s = match v {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            parts.push(format!("{k}={}", s.chars().take(80).collect::<String>()));
        }
    }
    parts.join(" ")
}

fn llm_failure(e: &LlmError, b: &Budgets) -> String {
    match e {
        LlmError::Timeout => Cap::RequestTimeout.message(b),
        other => format!("The model request failed: {other}"),
    }
}

/// Runs one agent conversation to completion. The caller may drop the future to cancel:
/// the `RunLog` then records `cancelled` and nothing is saved.
pub async fn run_agent(
    cfg: RunConfig,
    client: &dyn LlmClient,
    tools: ToolSet,
    finalize: &dyn Finalize,
    sink: &mut dyn EventSink,
    mut log: RunLog,
    sleeper: &dyn Sleeper,
) -> RunOutcome {
    let b = cfg.budgets;
    log.run_started(
        &cfg.model,
        cfg.reasoning_effort.as_deref(),
        &cfg.focus,
        &b,
        &cfg.repo,
    );
    let conversation = conversation(&cfg, client, &tools, finalize, sink, &mut log, sleeper);
    let outcome = match tokio::time::timeout(cfg.wall_clock, conversation).await {
        Ok(o) => o,
        Err(_) => Err(Cap::WallClock.message(&b)),
    };
    let outcome = match outcome {
        Ok(candidate) => {
            log.finished(Outcome::Success, "accepted", Some(&candidate));
            RunOutcome::Success { candidate }
        }
        Err(reason) => {
            log.finished(Outcome::Failed, &reason, None);
            RunOutcome::Failed { reason }
        }
    };
    sink.emit(AgentEvent::Finished(outcome.clone())).await;
    outcome
}

async fn conversation(
    cfg: &RunConfig,
    client: &dyn LlmClient,
    tools: &ToolSet,
    finalize: &dyn Finalize,
    sink: &mut dyn EventSink,
    log: &mut RunLog,
    sleeper: &dyn Sleeper,
) -> Result<RelPath, String> {
    let b = cfg.budgets;
    let schemas = tools::schemas();
    let mut messages = vec![
        ChatMessage::System(SYSTEM_PROMPT.to_string()),
        ChatMessage::User(cfg.seed.clone()),
    ];
    let mut turn = 0u32;
    let mut input_total = 0u64;
    let mut output_total = 0u64;
    let mut rejections = 0u32;
    loop {
        if turn >= b.max_turns {
            return Err(Cap::Turns.message(&b));
        }
        turn += 1;
        sink.emit(AgentEvent::TurnStarted {
            n: turn,
            max: b.max_turns,
        })
        .await;
        let request = TurnRequest {
            messages: messages.clone(),
            tools: schemas.clone(),
            max_completion_tokens: b.max_completion_tokens,
            reasoning_effort: cfg.reasoning_effort.clone(),
        };
        let mut notices = Vec::new();
        let response = with_retries(
            &cfg.retry,
            cfg.request_timeout,
            sleeper,
            || client.complete(request.clone()),
            |n| notices.push(n),
        )
        .await;
        for n in &notices {
            log.retry(turn, n.attempt, n.status, n.timeout, n.wait.as_millis());
            let reason = match n.status {
                Some(s) => format!("HTTP {s}"),
                None => "timeout".to_string(),
            };
            sink.emit(AgentEvent::Retry {
                attempt: n.attempt,
                reason,
                wait: n.wait,
            })
            .await;
        }
        let response = response.map_err(|e| llm_failure(&e, &b))?;
        let usage: Usage = response.usage;
        input_total = input_total.saturating_add(usage.input_tokens);
        output_total = output_total.saturating_add(usage.output_tokens);
        log.turn(turn, usage, response.tool_calls.len());
        sink.emit(AgentEvent::Usage {
            n: turn,
            input_tokens: input_total,
            output_tokens: output_total,
        })
        .await;
        if input_total > b.max_input_tokens {
            return Err(Cap::InputTokens.message(&b));
        }
        messages.push(ChatMessage::Assistant {
            text: response.text.clone(),
            tool_calls: response.tool_calls.clone(),
            replay: response.replay.clone(),
        });
        if response.tool_calls.is_empty() {
            messages.push(ChatMessage::User(REMINDER.to_string()));
            continue;
        }
        for call in &response.tool_calls {
            if call.name == tools::SUBMIT {
                let value = serde_json::from_str::<Value>(if call.arguments.trim().is_empty() {
                    "{}"
                } else {
                    &call.arguments
                })
                .ok()
                .and_then(|v| v.get("diagram").cloned());
                let Some(diagram) = value else {
                    let msg = "invalid arguments for submit_diagram: expected {\"diagram\": {…}}";
                    log.tool_call(
                        turn,
                        tools::SUBMIT,
                        Value::Object(Default::default()),
                        msg.len(),
                        Some(msg),
                    );
                    sink.emit(AgentEvent::ToolResult {
                        name: tools::SUBMIT.into(),
                        bytes: msg.len(),
                        error: Some(msg.into()),
                    })
                    .await;
                    messages.push(ChatMessage::Tool {
                        call_id: call.id.clone(),
                        content: format!("error: {msg}"),
                    });
                    continue;
                };
                let components = diagram
                    .get("components")
                    .and_then(Value::as_array)
                    .map(Vec::len)
                    .unwrap_or(0);
                sink.emit(AgentEvent::ToolCalled {
                    name: tools::SUBMIT.into(),
                    summary: format!("{components} components"),
                })
                .await;
                let sandbox = tools.sandbox().clone();
                let app = cfg.app_owned.clone();
                let dirty = cfg.dirty.clone();
                let result = tokio::task::spawn_blocking(move || {
                    gate::check(diagram, &app, &sandbox, &dirty)
                })
                .await
                .map_err(|e| format!("the submission check failed: {e}"))?;
                if !result.dropped.is_empty() {
                    log.fields_dropped(&result.dropped);
                }
                match result.verdict {
                    Verdict::Accepted(doc, _warnings) => {
                        log.tool_call(
                            turn,
                            tools::SUBMIT,
                            serde_json::json!({ "components": components }),
                            8,
                            None,
                        );
                        sink.emit(AgentEvent::ToolResult {
                            name: tools::SUBMIT.into(),
                            bytes: 8,
                            error: None,
                        })
                        .await;
                        return finalize
                            .finalize(*doc)
                            .await
                            .map_err(|e| format!("Saving the diagram failed: {e}"));
                    }
                    Verdict::Rejected(errors) => {
                        rejections += 1;
                        log.submission_rejected(rejections, &errors);
                        let text = gate::rejection_text(&errors, rejections, b.max_repairs);
                        log.tool_call(
                            turn,
                            tools::SUBMIT,
                            serde_json::json!({ "components": components }),
                            text.len(),
                            Some("rejected"),
                        );
                        sink.emit(AgentEvent::Rejected {
                            attempt: rejections,
                            count: errors.len(),
                            first: errors.iter().take(3).map(issue_line).collect(),
                        })
                        .await;
                        if rejections > b.max_repairs {
                            return Err(Cap::Repairs.message(&b));
                        }
                        messages.push(ChatMessage::Tool {
                            call_id: call.id.clone(),
                            content: tools::cap(text),
                        });
                    }
                }
                continue;
            }
            let tools_c = tools.clone();
            let (name, args) = (call.name.clone(), call.arguments.clone());
            let out = tokio::task::spawn_blocking(move || tools_c.execute(&name, &args))
                .await
                .map_err(|e| format!("a tool failed: {e}"))?;
            sink.emit(AgentEvent::ToolCalled {
                name: call.name.clone(),
                summary: summarize_args(&out.args),
            })
            .await;
            log.tool_call(
                turn,
                &call.name,
                out.args.clone(),
                out.content.len(),
                out.error.as_deref(),
            );
            sink.emit(AgentEvent::ToolResult {
                name: call.name.clone(),
                bytes: out.content.len(),
                error: out.error.clone(),
            })
            .await;
            messages.push(ChatMessage::Tool {
                call_id: call.id.clone(),
                content: out.content,
            });
        }
    }
}

fn issue_line(i: &Issue) -> String {
    format!(
        "{}: {}",
        if i.pointer.is_empty() {
            "/"
        } else {
            &i.pointer
        },
        i.message
    )
}
