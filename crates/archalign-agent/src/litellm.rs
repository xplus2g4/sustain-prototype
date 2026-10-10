//! The only module that touches `litellm-rust` (ADR 0021, ADR 0033).

use std::time::Duration;

use litellm_rust::{
    ChatMessage as LChatMessage, ChatMessageContent, ChatRequest, LiteLLM, LiteLLMError,
    ProviderConfig, ProviderKind,
};
use serde_json::{Value, json};

use crate::llm::{
    BoxFuture, ChatMessage, LlmClient, LlmError, TurnRequest, TurnResponse, Usage, parse_tool_calls,
};

/// A provider litellm-rust knows by model prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Provider {
    /// Prefix in `provider/model`.
    pub name: &'static str,
    /// Environment variable holding its key.
    pub key_env: &'static str,
    /// API base URL.
    pub base_url: &'static str,
    /// Wire protocol.
    pub anthropic: bool,
    /// Gemini wire protocol.
    pub gemini: bool,
}

/// litellm-rust 0.3.0's built-in providers (`src/router.rs`).
pub const PROVIDERS: [Provider; 6] = [
    Provider {
        name: "openai",
        key_env: "OPENAI_API_KEY",
        base_url: "https://api.openai.com/v1",
        anthropic: false,
        gemini: false,
    },
    Provider {
        name: "anthropic",
        key_env: "ANTHROPIC_API_KEY",
        base_url: "https://api.anthropic.com",
        anthropic: true,
        gemini: false,
    },
    Provider {
        name: "gemini",
        key_env: "GEMINI_API_KEY",
        base_url: "https://generativelanguage.googleapis.com/v1beta",
        anthropic: false,
        gemini: true,
    },
    Provider {
        name: "openrouter",
        key_env: "OPENROUTER_API_KEY",
        base_url: "https://openrouter.ai/api/v1",
        anthropic: false,
        gemini: false,
    },
    Provider {
        name: "xai",
        key_env: "XAI_API_KEY",
        base_url: "https://api.x.ai/v1",
        anthropic: false,
        gemini: false,
    },
    Provider {
        name: "zai",
        key_env: "ZAI_API_KEY",
        base_url: "https://api.z.ai/api/paas/v4",
        anthropic: false,
        gemini: false,
    },
];

/// The provider of a `provider/model` string (no prefix means `openai`, as in litellm).
pub fn provider_of(model: &str) -> Option<Provider> {
    let name = model.split_once('/').map(|(p, _)| p).unwrap_or("openai");
    PROVIDERS.iter().copied().find(|p| p.name == name)
}

/// The litellm-rust client with our own reqwest client (request timeout) and an explicit key.
pub struct LiteLlmClient {
    client: LiteLLM,
    model: String,
}

impl std::fmt::Debug for LiteLlmClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never print the client: its provider config holds the key.
        f.debug_struct("LiteLlmClient")
            .field("model", &self.model)
            .finish_non_exhaustive()
    }
}

impl LiteLlmClient {
    /// A client for `model`, using `api_key` (FR-SEC-04) and a reqwest client with
    /// `request_timeout`.
    pub fn new(model: &str, api_key: &str, request_timeout: Duration) -> Result<Self, LlmError> {
        let provider = provider_of(model)
            .ok_or_else(|| LlmError::Config(format!("Unknown provider in model `{model}`")))?;
        let http = reqwest::Client::builder()
            .timeout(request_timeout)
            .build()
            .map_err(|e| LlmError::Config(format!("Can't create the HTTP client: {e}")))?;
        let kind = if provider.anthropic {
            ProviderKind::Anthropic
        } else if provider.gemini {
            ProviderKind::Gemini
        } else {
            ProviderKind::OpenAICompatible
        };
        let config = ProviderConfig::default()
            .with_base_url(provider.base_url)
            .with_kind(kind)
            .with_api_key(api_key);
        let client = LiteLLM::new()
            .map_err(|e| LlmError::Config(e.to_string()))?
            .with_client(http)
            .with_provider(provider.name, config);
        Ok(Self {
            client,
            model: model.to_string(),
        })
    }
}

fn to_litellm(m: &ChatMessage) -> LChatMessage {
    let base = |role: &str, content: String| LChatMessage {
        role: role.to_string(),
        content: ChatMessageContent::Text(content),
        name: None,
        tool_call_id: None,
        tool_calls: None,
        function_call: None,
        provider_specific_fields: None,
    };
    match m {
        ChatMessage::System(s) => base("system", s.clone()),
        ChatMessage::User(s) => base("user", s.clone()),
        ChatMessage::Assistant {
            text, tool_calls, ..
        } => {
            let mut msg = base("assistant", text.clone());
            if !tool_calls.is_empty() {
                msg.tool_calls = Some(Value::Array(
                    tool_calls
                        .iter()
                        .map(|c| json!({ "id": c.id, "type": "function", "function": { "name": c.name, "arguments": c.arguments } }))
                        .collect(),
                ));
            }
            msg
        }
        ChatMessage::Tool { call_id, content } => {
            let mut msg = base("tool", content.clone());
            msg.tool_call_id = Some(call_id.clone());
            msg
        }
    }
}

/// Maps a litellm-rust error. The crate reports HTTP failures as `"http <status>: <body>"`
/// and drops the response headers, so `Retry-After` can only be recovered from a
/// "try again in Ns" hint in the body (ADR 0033).
pub fn map_error(e: LiteLLMError) -> LlmError {
    match e {
        LiteLLMError::Http { message, source } => {
            let is_timeout = message.contains("timed out")
                || source
                    .as_ref()
                    .and_then(|s| s.downcast_ref::<reqwest::Error>())
                    .is_some_and(reqwest::Error::is_timeout);
            if is_timeout {
                return LlmError::Timeout;
            }
            if let Some(rest) = message.strip_prefix("http ")
                && let Some((code, body)) = rest.split_once(':')
                && let Ok(status) = code.trim().parse::<u16>()
            {
                return LlmError::Status {
                    status,
                    retry_after: retry_hint(body),
                    message: body.trim().chars().take(300).collect(),
                };
            }
            LlmError::Transport(message.chars().take(300).collect())
        }
        LiteLLMError::Parse(m) => LlmError::Parse(m),
        LiteLLMError::MissingApiKey(env) => LlmError::Config(format!("No API key ({env})")),
        LiteLLMError::Truncated { .. } => {
            LlmError::Parse("the response was truncated (max_completion_tokens)".into())
        }
        LiteLLMError::Refusal { text, .. } => LlmError::Parse(format!(
            "the model refused: {}",
            text.chars().take(200).collect::<String>()
        )),
        other => LlmError::Config(other.to_string()),
    }
}

/// Parses "try again in 6.5s" / "retry after 20 seconds" / "in 300ms" hints from an error body.
pub fn retry_hint(body: &str) -> Option<Duration> {
    let lower = body.to_lowercase();
    let idx = lower
        .find("try again in ")
        .map(|i| i + "try again in ".len())
        .or_else(|| lower.find("retry after ").map(|i| i + "retry after ".len()))?;
    let rest = &lower[idx..];
    let num: String = rest
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    let value: f64 = num.parse().ok()?;
    let unit = rest[num.len()..].trim_start();
    let secs = if unit.starts_with("ms") {
        value / 1000.0
    } else if unit.starts_with("min") {
        value * 60.0
    } else {
        value
    };
    (secs.is_finite() && (0.0..=600.0).contains(&secs)).then(|| Duration::from_secs_f64(secs))
}

impl LlmClient for LiteLlmClient {
    fn complete(&self, request: TurnRequest) -> BoxFuture<'_, Result<TurnResponse, LlmError>> {
        Box::pin(async move {
            let mut req = ChatRequest::new(self.model.clone());
            req.messages = request.messages.iter().map(to_litellm).collect();
            req.tools = Some(request.tools);
            req.tool_choice = Some(json!("auto"));
            req.parallel_tool_calls = Some(true);
            req.max_completion_tokens = Some(request.max_completion_tokens);
            req.reasoning_effort = request.reasoning_effort.map(Value::from);
            let resp = self.client.completion(req).await.map_err(map_error)?;
            let tool_calls = match &resp.tool_calls {
                Some(v) => parse_tool_calls(v)?,
                None => Vec::new(),
            };
            Ok(TurnResponse {
                text: resp.content,
                tool_calls,
                usage: Usage {
                    input_tokens: resp.usage.prompt_tokens.unwrap_or(0),
                    output_tokens: resp.usage.completion_tokens.unwrap_or(0),
                },
                replay: Vec::new(),
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn providers_by_prefix() {
        assert_eq!(
            provider_of("openai/gpt-6.1-sol").unwrap().key_env,
            "OPENAI_API_KEY"
        );
        assert_eq!(
            provider_of("anthropic/claude").unwrap().key_env,
            "ANTHROPIC_API_KEY"
        );
        assert_eq!(provider_of("gpt-x").unwrap().name, "openai");
        assert!(provider_of("nope/model").is_none());
    }

    #[test]
    fn http_errors_map_to_status_with_hints() {
        let e = map_error(LiteLLMError::http(
            "http 429: Rate limit reached. Please try again in 6.5s.",
        ));
        assert_eq!(
            e,
            LlmError::Status {
                status: 429,
                retry_after: Some(Duration::from_millis(6500)),
                message: "Rate limit reached. Please try again in 6.5s.".into()
            }
        );
        assert!(e.is_retryable());
        let e = map_error(LiteLLMError::http("http 503: unavailable"));
        assert!(matches!(
            e,
            LlmError::Status {
                status: 503,
                retry_after: None,
                ..
            }
        ));
        assert_eq!(
            map_error(LiteLLMError::http("request timed out")),
            LlmError::Timeout
        );
        assert!(matches!(
            map_error(LiteLLMError::http("connection refused")),
            LlmError::Transport(_)
        ));
        assert!(!map_error(LiteLLMError::http("http 401: bad key")).is_retryable());
    }

    #[test]
    fn retry_hints() {
        assert_eq!(
            retry_hint("try again in 300ms"),
            Some(Duration::from_millis(300))
        );
        assert_eq!(
            retry_hint("Retry after 20 seconds"),
            Some(Duration::from_secs(20))
        );
        assert_eq!(
            retry_hint("try again in 2 min"),
            Some(Duration::from_secs(120))
        );
        assert_eq!(retry_hint("slow down"), None);
        assert_eq!(retry_hint("try again in 99999s"), None);
    }
}
