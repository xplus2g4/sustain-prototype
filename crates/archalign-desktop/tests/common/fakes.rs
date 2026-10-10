//! A scripted LLM for UI tests (no network).

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use archalign_agent::{BoxFuture, LlmClient, LlmError, ToolCall, TurnRequest, TurnResponse, Usage};
use archalign_desktop::agent_run::{LlmFactory, LlmSpec};
use archalign_desktop::keys::KeyEnv;
use serde_json::{Value, json};

pub enum Step {
    Reply(TurnResponse),
    Hang,
}

#[derive(Clone, Default)]
pub struct Scripted {
    steps: Arc<Mutex<VecDeque<Step>>>,
    pub requests: Arc<Mutex<Vec<TurnRequest>>>,
    /// Keys the factory was given.
    pub keys: Arc<Mutex<Vec<String>>>,
}

impl Scripted {
    pub fn new(steps: Vec<Step>) -> Self {
        Self {
            steps: Arc::new(Mutex::new(steps.into())),
            ..Default::default()
        }
    }

    pub fn calls(&self) -> usize {
        self.requests.lock().unwrap().len()
    }

    pub fn factory(&self) -> LlmFactory {
        let me = self.clone();
        Arc::new(move |spec: &LlmSpec| {
            me.keys.lock().unwrap().push(spec.key.expose().to_string());
            Ok(Arc::new(me.clone()) as Arc<dyn LlmClient>)
        })
    }
}

impl LlmClient for Scripted {
    fn complete(&self, request: TurnRequest) -> BoxFuture<'_, Result<TurnResponse, LlmError>> {
        self.requests.lock().unwrap().push(request);
        let step = self.steps.lock().unwrap().pop_front();
        Box::pin(async move {
            match step {
                Some(Step::Reply(r)) => Ok(r),
                Some(Step::Hang) => std::future::pending().await,
                None => Err(LlmError::Config("script exhausted".into())),
            }
        })
    }
}

pub fn tools(calls: Vec<(&str, Value)>) -> TurnResponse {
    TurnResponse {
        text: String::new(),
        tool_calls: calls
            .into_iter()
            .enumerate()
            .map(|(i, (name, args))| ToolCall {
                id: format!("c{i}"),
                name: name.into(),
                arguments: args.to_string(),
            })
            .collect(),
        usage: Usage {
            input_tokens: 1200,
            output_tokens: 80,
        },
        replay: Vec::new(),
    }
}

pub fn submit(diagram: Value) -> TurnResponse {
    tools(vec![("submit_diagram", json!({ "diagram": diagram }))])
}

/// A key environment with one variable.
pub fn key_env(var: &'static str, value: &'static str) -> KeyEnv {
    KeyEnv {
        vars: Arc::new(move |k| (k == var).then(|| value.to_string())),
        launch_dir: None,
        config_dir: None,
    }
}

/// A valid topology-only diagram for `fixtures::sample_repo()`.
pub fn topology() -> Value {
    json!({
        "meta": { "title": "Generated" },
        "components": [
            { "id": "api", "type": "backend", "label": "API", "sublabel": "request handlers",
              "sources": [{ "path": "src/api.rs", "line": 10, "end_line": 20 }] },
            { "id": "db", "type": "database", "label": "Store", "sources": [{ "path": "src/db/store.rs", "line": 1, "end_line": 5 }] },
            { "id": "ui", "type": "frontend", "label": "Docs", "sources": [{ "path": "README.md", "line": 1 }] }
        ],
        "connections": [
            { "from": "ui", "to": "api", "label": "calls" },
            { "from": "api", "to": "db", "label": "writes" }
        ]
    })
}
