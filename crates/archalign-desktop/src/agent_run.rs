//! Starting, streaming and finishing an agent run from the UI (FR-GEN-01..12, ADR 0008).

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use archalign_agent::prompt::{SeedInput, seed_message};
use archalign_agent::retry::TokioSleeper;
use archalign_agent::run::{AgentEvent, EventSink, Finalize, RunConfig, RunOutcome, run_agent};
use archalign_agent::{BoxFuture, LlmClient, RepoMeta, RepoSummary, RunLog, ToolSet};
use archalign_layout::{layout_document, write_back};
use archalign_model::ArchitectureDoc;
use archalign_repo::{RelPath, Sandbox, SelfWrites, atomic_write_tracked};
use iced::futures::SinkExt;
use iced::futures::channel::mpsc;

use crate::app::Message;
use crate::fonts::IcedTextMeasure;
use crate::keys::ApiKey;

/// What the LLM client factory needs.
#[derive(Debug, Clone)]
pub struct LlmSpec {
    /// litellm `provider/model`.
    pub model: String,
    /// The resolved key.
    pub key: ApiKey,
    /// Per-request timeout.
    pub request_timeout: Duration,
}

/// Builds the LLM client for a run (litellm-rust in the app; scripted fakes in tests).
pub type LlmFactory = Arc<dyn Fn(&LlmSpec) -> Result<Arc<dyn LlmClient>, String> + Send + Sync>;

/// The production factory: [`archalign_agent::client_for`] (OpenAI Responses API for
/// `openai/…`, litellm-rust otherwise).
pub fn provider_factory() -> LlmFactory {
    Arc::new(|spec: &LlmSpec| {
        archalign_agent::client_for(&spec.model, spec.key.expose(), spec.request_timeout)
            .map_err(|e| e.to_string())
    })
}

/// Run-pane messages.
#[derive(Debug, Clone)]
pub enum RunMsg {
    /// A progress event from the agent.
    Event(AgentEvent),
    /// One-second tick while running (elapsed time).
    Tick,
    /// The consent modal was accepted.
    ConsentAccept,
    /// The consent modal was declined.
    ConsentDecline,
    /// Show or hide the run-log drawer.
    ToggleDrawer,
}

/// Forwards agent events into the iced stream.
struct ChannelSink(mpsc::Sender<Message>);

impl EventSink for ChannelSink {
    fn emit(&mut self, event: AgentEvent) -> BoxFuture<'_, ()> {
        Box::pin(async move {
            let _ = self.0.send(Message::Run(RunMsg::Event(event))).await;
        })
    }
}

/// Lays out an accepted document, writes integer geometry back and saves
/// `candidate.json` atomically (FR-GEN-06, FR-LAY-08, FR-STO-02).
pub struct DesktopFinalize {
    sandbox: Sandbox,
    folder: RelPath,
    writes: SelfWrites,
}

impl Finalize for DesktopFinalize {
    fn finalize(&self, doc: ArchitectureDoc) -> BoxFuture<'_, Result<RelPath, String>> {
        let sandbox = self.sandbox.clone();
        let folder = self.folder.clone();
        let writes = self.writes.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                let mut doc = doc;
                let scene = layout_document(&doc, &IcedTextMeasure);
                write_back(&mut doc, &scene);
                let rel = folder.join("candidate.json").map_err(|e| e.to_string())?;
                atomic_write_tracked(
                    &sandbox,
                    &rel,
                    &archalign_model::to_pretty_json(&doc),
                    &writes,
                )
                .map_err(|e| e.to_string())?;
                Ok(rel)
            })
            .await
            .map_err(|e| e.to_string())?
        })
    }
}

/// Everything prepared on the UI thread once the preconditions passed.
pub struct PreparedRun {
    /// Run configuration (seed filled in by the task).
    pub cfg: RunConfig,
    /// The client.
    pub client: Arc<dyn LlmClient>,
    /// Confined tools.
    pub sandbox: Sandbox,
    /// For `repo_meta`.
    pub meta: RepoMeta,
    /// `.archify/<folder>`.
    pub folder: RelPath,
    /// Self-write registry.
    pub writes: SelfWrites,
    /// Seed inputs.
    pub seed: SeedInput,
}

/// The run as an iced stream of messages: builds the seed off the UI thread, then runs the
/// agent. Dropping the stream (Cancel) drops the run; its `RunLog` records `cancelled`.
pub fn stream(p: PreparedRun) -> impl iced::futures::Stream<Item = Message> {
    iced::stream::channel(64, async move |out: mpsc::Sender<Message>| {
        let PreparedRun {
            mut cfg,
            client,
            sandbox,
            meta,
            folder,
            writes,
            seed,
        } = p;
        let log = match folder.join("run.jsonl") {
            Ok(rel) => RunLog::new(sandbox.clone(), rel),
            Err(_) => return,
        };
        let seed_sandbox = sandbox.clone();
        cfg.seed = tokio::task::spawn_blocking(move || seed_message(&seed_sandbox, &seed))
            .await
            .unwrap_or_default();
        let tools = ToolSet::new(sandbox.clone(), meta);
        let finalize = DesktopFinalize {
            sandbox,
            folder,
            writes,
        };
        let mut sink = ChannelSink(out);
        let _: RunOutcome = run_agent(
            cfg,
            client.as_ref(),
            tools,
            &finalize,
            &mut sink,
            log,
            &TokioSleeper,
        )
        .await;
    })
}

/// One line of the run-log drawer (FR-GEN-09).
pub fn event_line(e: &AgentEvent) -> String {
    match e {
        AgentEvent::TurnStarted { n, max } => format!("Turn {n}/{max}"),
        AgentEvent::Usage {
            input_tokens,
            output_tokens,
            ..
        } => format!("  tokens: {input_tokens} in, {output_tokens} out"),
        AgentEvent::Retry {
            attempt,
            reason,
            wait,
        } => {
            format!(
                "  ↻ retry {attempt} after {:.1} s ({reason})",
                wait.as_secs_f32()
            )
        }
        AgentEvent::ToolCalled { name, summary } => format!("  → {name} {summary}"),
        AgentEvent::ToolResult { name, bytes, error } => match error {
            Some(e) => format!("  ← {name}: error: {e}"),
            None => format!("  ← {name}: {bytes} bytes"),
        },
        AgentEvent::Rejected {
            attempt,
            count,
            first,
        } => format!(
            "  ✗ submission rejected (#{attempt}, {count} error{}): {}",
            if *count == 1 { "" } else { "s" },
            first.join("; ")
        ),
        AgentEvent::Finished(RunOutcome::Success { candidate }) => {
            format!("✓ success: {candidate}")
        }
        AgentEvent::Finished(RunOutcome::Failed { reason }) => format!("✗ failed: {reason}"),
    }
}

/// The repository facts for `run_started` and `repo_meta`.
pub fn repo_facts(
    git: &archalign_repo::git::GitStatus,
    name: &str,
) -> (RepoSummary, RepoMeta, HashSet<String>) {
    match git {
        archalign_repo::git::GitStatus::Repo(info) => (
            RepoSummary {
                name: name.to_string(),
                head: Some(info.head.clone()),
                dirty_count: info.dirty.len(),
            },
            RepoMeta {
                git: true,
                head: Some(info.head.clone()),
                origin: info.origin.clone(),
                dirty: info.dirty.clone(),
            },
            info.dirty_set(),
        ),
        _ => (
            RepoSummary {
                name: name.to_string(),
                head: None,
                dirty_count: 0,
            },
            RepoMeta::default(),
            HashSet::new(),
        ),
    }
}
