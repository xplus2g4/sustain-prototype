//! Shared helpers: a scripted fake `LlmClient`, a recording sink, temp repos.
#![allow(dead_code)]

use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use archalign_agent::budgets::Budgets;
use archalign_agent::llm::{
    BoxFuture, LlmClient, LlmError, ToolCall, TurnRequest, TurnResponse, Usage,
};
use archalign_agent::retry::Sleeper;
use archalign_agent::run::{AgentEvent, EventSink, Finalize, RunConfig};
use archalign_agent::runlog::{RepoSummary, RunLog};
use archalign_agent::tools::{RepoMeta, ToolSet};
use archalign_model::{AppOwned, ArchitectureDoc};
use archalign_repo::storage::{atomic_write, create_run_folder};
use archalign_repo::{RelPath, Sandbox};
use serde_json::{Value, json};

pub fn block_on<F: Future>(f: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("runtime")
        .block_on(f)
}

/// One scripted step.
pub enum Step {
    Reply(TurnResponse),
    Fail(LlmError),
    /// Never answers (for cancel and timeout tests).
    Hang,
    /// Answers after a delay.
    Slow(Duration, TurnResponse),
}

#[derive(Clone, Default)]
pub struct Fake {
    pub steps: Arc<Mutex<VecDeque<Step>>>,
    pub requests: Arc<Mutex<Vec<TurnRequest>>>,
    /// Returned when the script runs out.
    pub fallback: Arc<Mutex<Option<TurnResponse>>>,
}

impl Fake {
    pub fn new(steps: Vec<Step>) -> Self {
        Self {
            steps: Arc::new(Mutex::new(steps.into())),
            ..Default::default()
        }
    }

    pub fn with_fallback(self, r: TurnResponse) -> Self {
        *self.fallback.lock().unwrap() = Some(r);
        self
    }

    pub fn calls(&self) -> usize {
        self.requests.lock().unwrap().len()
    }
}

impl LlmClient for Fake {
    fn complete(&self, request: TurnRequest) -> BoxFuture<'_, Result<TurnResponse, LlmError>> {
        self.requests.lock().unwrap().push(request);
        let step = self.steps.lock().unwrap().pop_front();
        let fallback = self.fallback.lock().unwrap().clone();
        Box::pin(async move {
            match step {
                Some(Step::Reply(r)) => Ok(r),
                Some(Step::Fail(e)) => Err(e),
                Some(Step::Hang) => std::future::pending().await,
                Some(Step::Slow(d, r)) => {
                    tokio::time::sleep(d).await;
                    Ok(r)
                }
                None => fallback.ok_or_else(|| LlmError::Config("script exhausted".into())),
            }
        })
    }
}

pub fn call(id: &str, name: &str, args: Value) -> ToolCall {
    ToolCall {
        id: id.into(),
        name: name.into(),
        arguments: args.to_string(),
    }
}

pub fn tools_reply(calls: Vec<ToolCall>, input_tokens: u64) -> TurnResponse {
    TurnResponse {
        text: String::new(),
        tool_calls: calls,
        usage: Usage {
            input_tokens,
            output_tokens: 10,
        },
        replay: Vec::new(),
    }
}

pub fn text_reply(text: &str) -> TurnResponse {
    TurnResponse {
        text: text.into(),
        tool_calls: Vec::new(),
        usage: Usage {
            input_tokens: 100,
            output_tokens: 5,
        },
        replay: Vec::new(),
    }
}

pub fn submit(diagram: Value) -> TurnResponse {
    tools_reply(
        vec![call("s", "submit_diagram", json!({ "diagram": diagram }))],
        100,
    )
}

/// A recording sink.
#[derive(Default, Clone)]
pub struct Sink(pub Arc<Mutex<Vec<AgentEvent>>>);

impl EventSink for Sink {
    fn emit(&mut self, event: AgentEvent) -> BoxFuture<'_, ()> {
        self.0.lock().unwrap().push(event);
        Box::pin(async {})
    }
}

/// Sleeps for real but at most 5 ms (keeps retry tests fast); records requested waits.
#[derive(Default, Clone)]
pub struct FastSleeper(pub Arc<Mutex<Vec<Duration>>>);

impl Sleeper for FastSleeper {
    fn sleep(&self, d: Duration) -> BoxFuture<'_, ()> {
        self.0.lock().unwrap().push(d);
        Box::pin(tokio::time::sleep(d.min(Duration::from_millis(5))))
    }
}

/// Saves `candidate.json` like the desktop does (minus layout).
pub struct SaveFinalize {
    pub sandbox: Sandbox,
    pub folder: RelPath,
}

impl Finalize for SaveFinalize {
    fn finalize(&self, doc: ArchitectureDoc) -> BoxFuture<'_, Result<RelPath, String>> {
        Box::pin(async move {
            let rel = self
                .folder
                .join("candidate.json")
                .map_err(|e| e.to_string())?;
            atomic_write(&self.sandbox, &rel, &archalign_model::to_pretty_json(&doc))
                .map_err(|e| e.to_string())?;
            Ok(rel)
        })
    }
}

pub struct Repo {
    _dir: tempfile::TempDir,
    pub root: PathBuf,
    pub sandbox: Sandbox,
}

impl Repo {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let sandbox = Sandbox::new(&root).unwrap();
        Self {
            _dir: dir,
            root,
            sandbox,
        }
    }

    pub fn write(&self, rel: &str, content: impl AsRef<[u8]>) -> &Self {
        let p = self.root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, content).unwrap();
        self
    }

    pub fn read(&self, rel: &str) -> String {
        std::fs::read_to_string(self.root.join(rel)).unwrap()
    }
}

pub fn lines(n: usize) -> String {
    (1..=n).map(|i| format!("line {i}\n")).collect()
}

/// A small repo with `src/main.rs` (50 lines) and `src/db.rs` (20 lines).
pub fn sample() -> Repo {
    let r = Repo::new();
    r.write("src/main.rs", lines(50))
        .write("src/db.rs", lines(20))
        .write("README.md", "# Sample\nA test repo.\n");
    r
}

/// A valid topology-only diagram for [`sample`].
pub fn good_diagram() -> Value {
    json!({
        "meta": { "title": "Sample" },
        "components": [
            { "id": "app", "type": "backend", "label": "App", "sources": [{ "path": "src/main.rs", "line": 1, "end_line": 10 }] },
            { "id": "db", "type": "database", "label": "DB", "sources": [{ "path": "src/db.rs", "line": 2, "end_line": 5 }] }
        ],
        "connections": [{ "from": "app", "to": "db", "label": "writes" }]
    })
}

/// Everything a run needs.
pub struct Setup {
    pub cfg: RunConfig,
    pub tools: ToolSet,
    pub finalize: SaveFinalize,
    pub log: Option<RunLog>,
    pub folder: RelPath,
}

pub fn setup(repo: &Repo, budgets: Budgets) -> Setup {
    let folder = create_run_folder(&repo.sandbox, "sample", "20261010-101010").unwrap();
    let app = AppOwned {
        output: format!(".archify/{}/sample.html", folder.name),
        repository: None,
    };
    let cfg = RunConfig::new(
        "openai/gpt-6-luna".into(),
        None,
        budgets,
        "payments".into(),
        "SEED".into(),
        RepoSummary {
            name: "sample".into(),
            head: None,
            dirty_count: 0,
        },
        app,
        HashSet::new(),
    );
    let log = RunLog::new(repo.sandbox.clone(), folder.rel.join("run.jsonl").unwrap());
    Setup {
        cfg,
        tools: ToolSet::new(repo.sandbox.clone(), RepoMeta::default()),
        finalize: SaveFinalize {
            sandbox: repo.sandbox.clone(),
            folder: folder.rel.clone(),
        },
        log: Some(log),
        folder: folder.rel,
    }
}

/// The parsed `run.jsonl` events.
pub fn events(repo: &Repo, folder: &RelPath) -> Vec<Value> {
    repo.read(&format!("{folder}/run.jsonl"))
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

pub fn event_names(evs: &[Value]) -> Vec<String> {
    evs.iter()
        .map(|e| e["event"].as_str().unwrap().to_string())
        .collect()
}
