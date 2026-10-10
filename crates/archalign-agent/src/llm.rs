//! The `LlmClient` seam (ADR 0021, ADR 0034): one model turn with tools.

use std::pin::Pin;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A boxed, sendable future.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// A tool call requested by the model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    /// Provider-assigned call ID (echoed in the tool result).
    pub id: String,
    /// Tool name.
    pub name: String,
    /// Raw JSON arguments as the model produced them.
    pub arguments: String,
}

/// One chat message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ChatMessage {
    /// System prompt.
    System(String),
    /// User text.
    User(String),
    /// An assistant reply, possibly with tool calls.
    Assistant {
        /// Text (often empty when tools are called).
        text: String,
        /// Tool calls.
        tool_calls: Vec<ToolCall>,
        /// Provider items sent back verbatim with this turn (OpenAI encrypted reasoning
        /// items, ADR 0034). Empty for other providers.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        replay: Vec<Value>,
    },
    /// A tool result.
    Tool {
        /// The call this answers.
        call_id: String,
        /// Result text (at most 24 KB).
        content: String,
    },
}

/// Token usage of one turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Usage {
    /// Prompt tokens.
    pub input_tokens: u64,
    /// Completion tokens.
    pub output_tokens: u64,
}

/// One completion request.
#[derive(Debug, Clone, PartialEq)]
pub struct TurnRequest {
    /// The conversation so far.
    pub messages: Vec<ChatMessage>,
    /// OpenAI function-tool schemas.
    pub tools: Value,
    /// `max_completion_tokens`.
    pub max_completion_tokens: u32,
    /// Passed through unchanged when set (FR-GEN-13).
    pub reasoning_effort: Option<String>,
}

/// The model's reply.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TurnResponse {
    /// Text content.
    pub text: String,
    /// Tool calls, in order.
    pub tool_calls: Vec<ToolCall>,
    /// Usage.
    pub usage: Usage,
    /// Provider items to send back with this turn (see [`ChatMessage::Assistant`]).
    pub replay: Vec<Value>,
}

/// Why a completion failed.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum LlmError {
    /// The provider answered with an HTTP error status.
    #[error("HTTP {status}: {message}")]
    Status {
        /// Status code.
        status: u16,
        /// How long the provider asked us to wait, when known.
        retry_after: Option<Duration>,
        /// Short message (never contains the key).
        message: String,
    },
    /// The request timed out.
    #[error("request timed out")]
    Timeout,
    /// Connection or transport failure.
    #[error("network error: {0}")]
    Transport(String),
    /// The response couldn't be understood.
    #[error("unexpected response: {0}")]
    Parse(String),
    /// Configuration problem (unknown provider, missing key, …).
    #[error("{0}")]
    Config(String),
}

impl LlmError {
    /// HTTP 429, HTTP 5xx and timeouts are retried (FR-GEN-08).
    pub fn is_retryable(&self) -> bool {
        match self {
            LlmError::Status { status, .. } => *status == 429 || (500..600).contains(status),
            LlmError::Timeout => true,
            _ => false,
        }
    }
}

/// Runs one completion. Implemented by the litellm adapter and by test fakes.
pub trait LlmClient: Send + Sync {
    /// One non-streaming completion with tools attached.
    fn complete(&self, request: TurnRequest) -> BoxFuture<'_, Result<TurnResponse, LlmError>>;
}

/// Parses an OpenAI-shaped `tool_calls` array:
/// `[{ "id", "type": "function", "function": { "name", "arguments": "<json string>" } }]`.
pub fn parse_tool_calls(value: &Value) -> Result<Vec<ToolCall>, LlmError> {
    let Some(items) = value.as_array() else {
        return if value.is_null() {
            Ok(Vec::new())
        } else {
            Err(LlmError::Parse("tool_calls is not an array".into()))
        };
    };
    items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let function = item.get("function").unwrap_or(&Value::Null);
            let name = function
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| LlmError::Parse(format!("tool_calls[{i}] has no function name")))?;
            let arguments = match function.get("arguments") {
                Some(Value::String(s)) => s.clone(),
                Some(Value::Null) | None => "{}".to_string(),
                Some(other) => other.to_string(),
            };
            let id = item
                .get("id")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| format!("call_{i}"));
            Ok(ToolCall {
                id,
                name: name.to_string(),
                arguments,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_openai_tool_calls() {
        let v = json!([
            { "id": "c1", "type": "function", "function": { "name": "list_dir", "arguments": "{\"path\":\"\"}" } },
            { "id": "c2", "type": "function", "function": { "name": "repo_meta" } }
        ]);
        let calls = parse_tool_calls(&v).unwrap();
        assert_eq!(calls[0].name, "list_dir");
        assert_eq!(calls[0].arguments, "{\"path\":\"\"}");
        assert_eq!(calls[1].arguments, "{}");
        assert!(parse_tool_calls(&Value::Null).unwrap().is_empty());
        assert!(parse_tool_calls(&json!({"x": 1})).is_err());
        assert!(parse_tool_calls(&json!([{ "id": "x" }])).is_err());
    }

    #[test]
    fn retryable_errors() {
        let s = |status| LlmError::Status {
            status,
            retry_after: None,
            message: String::new(),
        };
        assert!(s(429).is_retryable() && s(500).is_retryable() && s(503).is_retryable());
        assert!(!s(400).is_retryable() && !s(401).is_retryable());
        assert!(LlmError::Timeout.is_retryable());
        assert!(!LlmError::Transport("x".into()).is_retryable());
    }
}
