//! OpenAI Responses API client for `openai/…` models (ADR 0034).
//!
//! gpt-6.x models reject function tools on Chat Completions unless `reasoning_effort` is
//! `none`, so OpenAI turns go to `POST /v1/responses` with our reqwest client. Requests use
//! `store: false`; encrypted reasoning items are returned in [`TurnResponse::replay`] and sent
//! back with the next turn, so the model keeps its reasoning across tool calls.

use std::time::Duration;

use serde_json::{Value, json};

use crate::litellm::retry_hint;
use crate::llm::{
    BoxFuture, ChatMessage, LlmClient, LlmError, ToolCall, TurnRequest, TurnResponse, Usage,
};

/// A Responses API client with an explicit key (FR-SEC-04).
pub struct OpenAiResponsesClient {
    http: reqwest::Client,
    base: String,
    model: String,
    key: String,
}

impl std::fmt::Debug for OpenAiResponsesClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never print the key.
        f.debug_struct("OpenAiResponsesClient")
            .field("base", &self.base)
            .field("model", &self.model)
            .finish_non_exhaustive()
    }
}

impl OpenAiResponsesClient {
    /// A client for `model` (with or without the `openai/` prefix).
    pub fn new(
        model: &str,
        api_key: &str,
        request_timeout: Duration,
        base: &str,
    ) -> Result<Self, LlmError> {
        let http = reqwest::Client::builder()
            .timeout(request_timeout)
            .build()
            .map_err(|e| LlmError::Config(format!("Can't create the HTTP client: {e}")))?;
        Ok(Self {
            http,
            base: base.trim_end_matches('/').to_string(),
            model: model.strip_prefix("openai/").unwrap_or(model).to_string(),
            key: api_key.to_string(),
        })
    }
}

/// Converts Chat Completions tool schemas (`{type, function: {name, description, parameters}}`)
/// to the Responses shape (`{type, name, description, parameters}`).
pub fn responses_tools(tools: &Value) -> Value {
    let items = tools.as_array().cloned().unwrap_or_default();
    Value::Array(
        items
            .into_iter()
            .map(|t| match t.get("function") {
                Some(f) => {
                    let mut out = json!({ "type": "function" });
                    for k in ["name", "description", "parameters", "strict"] {
                        if let Some(v) = f.get(k) {
                            out[k] = v.clone();
                        }
                    }
                    out
                }
                None => t,
            })
            .collect(),
    )
}

/// Converts the conversation to Responses `input` items.
pub fn responses_input(messages: &[ChatMessage]) -> Vec<Value> {
    let mut out = Vec::new();
    for m in messages {
        match m {
            ChatMessage::System(s) => out.push(json!({ "role": "developer", "content": s })),
            ChatMessage::User(s) => out.push(json!({ "role": "user", "content": s })),
            ChatMessage::Assistant {
                text,
                tool_calls,
                replay,
            } => {
                out.extend(replay.iter().cloned());
                if !text.is_empty() {
                    out.push(json!({ "role": "assistant", "content": text }));
                }
                for c in tool_calls {
                    out.push(json!({
                        "type": "function_call",
                        "call_id": c.id,
                        "name": c.name,
                        "arguments": c.arguments,
                    }));
                }
            }
            ChatMessage::Tool { call_id, content } => out.push(json!({
                "type": "function_call_output",
                "call_id": call_id,
                "output": content,
            })),
        }
    }
    out
}

/// The request body for one turn.
pub fn request_body(model: &str, request: &TurnRequest) -> Value {
    let mut body = json!({
        "model": model,
        "input": responses_input(&request.messages),
        "tools": responses_tools(&request.tools),
        "tool_choice": "auto",
        "parallel_tool_calls": true,
        "max_output_tokens": request.max_completion_tokens,
        "store": false,
        "include": ["reasoning.encrypted_content"],
    });
    if let Some(effort) = &request.reasoning_effort {
        body["reasoning"] = json!({ "effort": effort });
    }
    body
}

/// Parses a Responses API reply.
pub fn parse_response(v: &Value) -> Result<TurnResponse, LlmError> {
    if v.get("status").and_then(Value::as_str) == Some("incomplete") {
        let reason = v
            .pointer("/incomplete_details/reason")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        if reason == "max_output_tokens" {
            return Err(LlmError::Parse(
                "the response was truncated (max_completion_tokens)".into(),
            ));
        }
        return Err(LlmError::Parse(format!(
            "the response is incomplete ({reason})"
        )));
    }
    let output = v
        .get("output")
        .and_then(Value::as_array)
        .ok_or_else(|| LlmError::Parse("the response has no output".into()))?;
    let mut text = String::new();
    let mut tool_calls = Vec::new();
    let mut replay = Vec::new();
    for item in output {
        match item.get("type").and_then(Value::as_str) {
            Some("reasoning") => replay.push(item.clone()),
            Some("message") => {
                for part in item
                    .get("content")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    match part.get("type").and_then(Value::as_str) {
                        Some("output_text") => {
                            text.push_str(part.get("text").and_then(Value::as_str).unwrap_or(""))
                        }
                        Some("refusal") => {
                            let r = part.get("refusal").and_then(Value::as_str).unwrap_or("");
                            return Err(LlmError::Parse(format!(
                                "the model refused: {}",
                                r.chars().take(200).collect::<String>()
                            )));
                        }
                        _ => {}
                    }
                }
            }
            Some("function_call") => {
                let name = item
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| LlmError::Parse("function_call has no name".into()))?;
                let id = item
                    .get("call_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| LlmError::Parse("function_call has no call_id".into()))?;
                tool_calls.push(ToolCall {
                    id: id.to_string(),
                    name: name.to_string(),
                    arguments: item
                        .get("arguments")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                });
            }
            _ => {}
        }
    }
    let usage = Usage {
        input_tokens: v
            .pointer("/usage/input_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        output_tokens: v
            .pointer("/usage/output_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
    };
    Ok(TurnResponse {
        text,
        tool_calls,
        usage,
        replay,
    })
}

/// `Retry-After` in seconds (the form OpenAI sends).
fn retry_after_header(h: &reqwest::header::HeaderMap) -> Option<Duration> {
    let secs = h
        .get(reqwest::header::RETRY_AFTER)?
        .to_str()
        .ok()?
        .trim()
        .parse::<f64>()
        .ok()?;
    (secs.is_finite() && (0.0..=600.0).contains(&secs)).then(|| Duration::from_secs_f64(secs))
}

/// Maps an HTTP failure; the body's `error.message` is preferred over the raw body.
pub fn status_error(status: u16, retry_after: Option<Duration>, body: &str) -> LlmError {
    let message = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| {
            v.pointer("/error/message")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| body.trim().to_string());
    LlmError::Status {
        status,
        retry_after: retry_after.or_else(|| retry_hint(&message)),
        message: message.chars().take(300).collect(),
    }
}

fn transport_error(e: reqwest::Error) -> LlmError {
    if e.is_timeout() {
        LlmError::Timeout
    } else {
        // reqwest errors carry the URL, never headers, so the key can't leak here.
        LlmError::Transport(e.to_string().chars().take(300).collect())
    }
}

impl LlmClient for OpenAiResponsesClient {
    fn complete(&self, request: TurnRequest) -> BoxFuture<'_, Result<TurnResponse, LlmError>> {
        Box::pin(async move {
            let body = request_body(&self.model, &request);
            let resp = self
                .http
                .post(format!("{}/responses", self.base))
                .bearer_auth(&self.key)
                .json(&body)
                .send()
                .await
                .map_err(transport_error)?;
            let status = resp.status();
            let retry_after = retry_after_header(resp.headers());
            let text = resp.text().await.map_err(transport_error)?;
            if !status.is_success() {
                return Err(status_error(status.as_u16(), retry_after, &text));
            }
            let v: Value = serde_json::from_str(&text)
                .map_err(|e| LlmError::Parse(format!("the response is not JSON: {e}")))?;
            parse_response(&v)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tools_are_flattened() {
        let t = json!([{ "type": "function", "function": { "name": "read_file", "description": "d", "parameters": { "type": "object" } } }]);
        assert_eq!(
            responses_tools(&t),
            json!([{ "type": "function", "name": "read_file", "description": "d", "parameters": { "type": "object" } }])
        );
    }

    #[test]
    fn the_conversation_maps_to_input_items_with_reasoning_replayed_first() {
        let r =
            json!({ "type": "reasoning", "id": "rs_1", "encrypted_content": "xx", "summary": [] });
        let msgs = vec![
            ChatMessage::System("sys".into()),
            ChatMessage::User("hi".into()),
            ChatMessage::Assistant {
                text: String::new(),
                tool_calls: vec![ToolCall {
                    id: "call_1".into(),
                    name: "list_dir".into(),
                    arguments: "{\"path\":\"\"}".into(),
                }],
                replay: vec![r.clone()],
            },
            ChatMessage::Tool {
                call_id: "call_1".into(),
                content: "src/".into(),
            },
        ];
        assert_eq!(
            Value::Array(responses_input(&msgs)),
            json!([
                { "role": "developer", "content": "sys" },
                { "role": "user", "content": "hi" },
                r,
                { "type": "function_call", "call_id": "call_1", "name": "list_dir", "arguments": "{\"path\":\"\"}" },
                { "type": "function_call_output", "call_id": "call_1", "output": "src/" }
            ])
        );
    }

    #[test]
    fn the_body_sets_store_false_and_effort_only_when_set() {
        let mut req = TurnRequest {
            messages: vec![],
            tools: json!([]),
            max_completion_tokens: 32000,
            reasoning_effort: None,
        };
        let b = request_body("gpt-6-luna", &req);
        assert_eq!(b["store"], false);
        assert_eq!(b["max_output_tokens"], 32000);
        assert_eq!(b["include"], json!(["reasoning.encrypted_content"]));
        assert!(b.get("reasoning").is_none());
        req.reasoning_effort = Some("high".into());
        assert_eq!(
            request_body("gpt-6-luna", &req)["reasoning"],
            json!({ "effort": "high" })
        );
    }

    #[test]
    fn replies_parse_text_calls_usage_and_reasoning() {
        let v = json!({
            "status": "completed",
            "output": [
                { "type": "reasoning", "id": "rs_1", "encrypted_content": "xx", "summary": [] },
                { "type": "message", "role": "assistant", "content": [{ "type": "output_text", "text": "Looking." }] },
                { "type": "function_call", "id": "fc_1", "call_id": "call_9", "name": "grep", "arguments": "{\"pattern\":\"x\"}" }
            ],
            "usage": { "input_tokens": 1500, "output_tokens": 70 }
        });
        let r = parse_response(&v).unwrap();
        assert_eq!(r.text, "Looking.");
        assert_eq!(
            r.tool_calls,
            vec![ToolCall {
                id: "call_9".into(),
                name: "grep".into(),
                arguments: "{\"pattern\":\"x\"}".into()
            }]
        );
        assert_eq!(
            r.usage,
            Usage {
                input_tokens: 1500,
                output_tokens: 70
            }
        );
        assert_eq!(r.replay.len(), 1);
    }

    #[test]
    fn truncation_and_errors_map_to_llm_errors() {
        let v = json!({ "status": "incomplete", "incomplete_details": { "reason": "max_output_tokens" }, "output": [] });
        assert!(matches!(parse_response(&v), Err(LlmError::Parse(m)) if m.contains("truncated")));
        let e = status_error(
            429,
            None,
            r#"{"error":{"message":"Rate limit reached. Please try again in 1.5s."}}"#,
        );
        assert_eq!(
            e,
            LlmError::Status {
                status: 429,
                retry_after: Some(Duration::from_millis(1500)),
                message: "Rate limit reached. Please try again in 1.5s.".into()
            }
        );
        assert!(e.is_retryable());
        let e = status_error(400, Some(Duration::from_secs(3)), "plain");
        assert!(matches!(
            e,
            LlmError::Status {
                status: 400,
                retry_after: Some(_),
                ..
            }
        ));
    }

    #[test]
    fn debug_never_shows_the_key() {
        let c = OpenAiResponsesClient::new(
            "openai/gpt-6-luna",
            "sk-secret-123",
            Duration::from_secs(5),
            "https://api.openai.com/v1",
        )
        .unwrap();
        assert!(!format!("{c:?}").contains("sk-secret-123"));
        assert_eq!(c.model, "gpt-6-luna");
    }

    /// One HTTP exchange against a local socket: path, auth header, body and `Retry-After`.
    #[test]
    fn complete_posts_to_responses_and_reads_retry_after() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}/v1", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            let mut buf = Vec::new();
            let mut chunk = [0u8; 4096];
            loop {
                let n = s.read(&mut chunk).unwrap();
                buf.extend_from_slice(&chunk[..n]);
                let text = String::from_utf8_lossy(&buf).to_string();
                if let Some(h) = text.find("\r\n\r\n") {
                    let len: usize = text
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(|v| v.trim().parse().unwrap())
                        })
                        .unwrap_or(0);
                    if buf.len() >= h + 4 + len {
                        break;
                    }
                }
            }
            let body = r#"{"error":{"message":"Rate limit reached"}}"#;
            write!(
                s,
                "HTTP/1.1 429 Too Many Requests\r\nRetry-After: 2\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
            String::from_utf8(buf).unwrap()
        });
        let c = OpenAiResponsesClient::new(
            "openai/gpt-6-luna",
            "sk-local",
            Duration::from_secs(5),
            &base,
        )
        .unwrap();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let err = rt
            .block_on(c.complete(TurnRequest {
                messages: vec![ChatMessage::User("hi".into())],
                tools: json!([]),
                max_completion_tokens: 100,
                reasoning_effort: None,
            }))
            .unwrap_err();
        let req = server.join().unwrap();
        assert!(req.starts_with("POST /v1/responses HTTP/1.1"), "{req}");
        assert!(
            req.to_ascii_lowercase()
                .contains("authorization: bearer sk-local")
        );
        assert!(req.contains("\"store\":false") && req.contains("\"model\":\"gpt-6-luna\""));
        assert_eq!(
            err,
            LlmError::Status {
                status: 429,
                retry_after: Some(Duration::from_secs(2)),
                message: "Rate limit reached".into()
            }
        );
    }
}
