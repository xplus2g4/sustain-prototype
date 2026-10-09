# 0021. litellm-rust behind an `LlmClient` trait

- Status: Accepted
- Date: 2026-10-10
- Related: [0003](0003-cargo-workspace-and-crate-boundaries.md), [0008](0008-async-model-tokio-and-iced-tasks.md), [0020](0020-built-in-llm-agent-authors-diagrams.md), [0022](0022-default-model-gpt-6-1-sol.md), [0023](0023-api-key-resolution.md); spec: FR-GEN

## Context

The built-in agent ([0020](0020-built-in-llm-agent-authors-diagrams.md)) needs a Rust LLM client with tool calling, a long request timeout and several providers. Facts about litellm-rust 0.3.0:

- MIT, MSRV 1.88, released 2026-07-11.
- 4,096 total downloads; 6 GitHub stars; a single maintainer (avivsinai); last push 2026-10-09.
- Chat Completions only (`{base}/chat/completions`, `src/providers/openai_compat.rs` L168).
- `ChatRequest` has `reasoning_effort` and `extra_body`.

## Decision

- Depend on `litellm-rust = "=0.3.0"`, wrapped by an `LlmClient` trait in a single adapter module in `archalign-agent`.
- **Each turn** calls the non-streaming `LiteLLM::completion(ChatRequest)` with:
  - `tools` (OpenAI function schemas as JSON) and `tool_choice`;
  - `parallel_tool_calls`;
  - `max_completion_tokens`;
  - optional `reasoning_effort`.
- **HTTP client.** Build our own `reqwest` 0.12 `Client` with a 300 s timeout and pass it through `with_client`.
  - litellm's default client times out after 60 s (`src/client.rs` L26).
  - It must be reqwest 0.12, because litellm depends on 0.12 (the newest reqwest is 0.13.5).
- **Retries are ours.** litellm doesn't retry: `send_json_with_retry` ignores its config and executes once (`src/http.rs` L76–84). Retry 429, 5xx and timeouts up to 3 times, backing off 1 s, 2 s, 4 s and honoring `Retry-After`.
- **Tool calls.** Parse `tool_calls` into typed structs. It arrives as a raw `serde_json::Value` in the OpenAI shape `{id, type: "function", function: {name, arguments: "<json string>"}}`.
- **No token streaming.** `stream_completion` doesn't surface tool-call deltas, so the run log updates once per tool call.
- **Key.** Pass it explicitly with `ProviderConfig::with_api_key` ([0023](0023-api-key-resolution.md)).
- **Provider** comes from the model prefix: `openai/…` → `https://api.openai.com/v1`. `anthropic/`, `gemini/`, `openrouter/`, `xai/` and `zai/` are also built in (`src/router.rs` L51–92).

## Alternatives considered

### `async-openai`

A mature OpenAI client. Not chosen as primary; it is the fallback if litellm gets in the way.

### A hand-rolled reqwest client

Not chosen. litellm-rust already provides request types and provider routing [inference].

### An official Rust SDK

Rejected. None is mature (the `anthropic` crate is at 0.0.8).

## Consequences

- Negative: pre-1.0 risk with a single maintainer, mitigated by pinning an exact version.
- Positive: swapping the client only touches the adapter module.
- Negative: no Responses-API features.
- Negative: the run log updates per tool call, not per token.

## References

- [../libraries.md](../libraries.md) (`litellm-rust`, `reqwest`)
- [../spec.md](../spec.md) (FR-GEN)
- litellm-rust 0.3.0 source: `src/client.rs` L26, `src/http.rs` L76–84, `src/router.rs` L51–92, `src/providers/openai_compat.rs` L168
