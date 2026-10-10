//! LLM agent loop, tools, submission gate and run log (no GUI).
//!
//! - [`llm`]: the `LlmClient` seam; [`litellm`]: the litellm-rust adapter (ADR 0021).
//! - [`retry`]: 429/5xx/timeout retries (FR-GEN-08).
//! - [`tools`]: the confined, read-only tools (FR-GEN-04, ADR 0025).
//! - [`prompt`]: system prompt and seed message (ADR 0026, FR-GEN-03).
//! - [`gate`]: the submission gate (ADR 0027).
//! - [`runlog`]: `run.jsonl` (Appendix A).
//! - [`run`]: the loop with budgets (ADR 0028).

pub mod budgets;
pub mod gate;
pub mod litellm;
pub mod llm;
pub mod openai;
pub mod prompt;
pub mod retry;
pub mod run;
pub mod runlog;
pub mod tools;

pub use budgets::{Budgets, Cap};
pub use llm::{
    BoxFuture, ChatMessage, LlmClient, LlmError, ToolCall, TurnRequest, TurnResponse, Usage,
};
pub use run::{AgentEvent, EventSink, Finalize, RunConfig, RunOutcome, run_agent};
pub use runlog::{Outcome, RepoSummary, RunLog};
pub use tools::{RepoMeta, ToolSet};

/// The client for a `provider/model` string: `openai/…` uses the Responses API (ADR 0034),
/// every other provider goes through litellm-rust (ADR 0021).
pub fn client_for(
    model: &str,
    api_key: &str,
    request_timeout: std::time::Duration,
) -> Result<std::sync::Arc<dyn LlmClient>, LlmError> {
    let provider = litellm::provider_of(model)
        .ok_or_else(|| LlmError::Config(format!("Unknown provider in model `{model}`")))?;
    if provider.name == "openai" {
        let c =
            openai::OpenAiResponsesClient::new(model, api_key, request_timeout, provider.base_url)?;
        Ok(std::sync::Arc::new(c))
    } else {
        let c = litellm::LiteLlmClient::new(model, api_key, request_timeout)?;
        Ok(std::sync::Arc::new(c))
    }
}
