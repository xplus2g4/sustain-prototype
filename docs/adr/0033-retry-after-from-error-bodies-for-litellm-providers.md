# 0033. `Retry-After` from error bodies for litellm-rust providers

- Status: Accepted
- Date: 2026-10-10
- Refines: [0021](0021-litellm-rust-behind-llm-client-trait.md) (retries)
- Related: [0034](0034-openai-via-responses-api.md); spec: FR-GEN-08

## Context

[0021](0021-litellm-rust-behind-llm-client-trait.md) says our retry wrapper honors `Retry-After`. FR-GEN-08 waits for `Retry-After` "when the response includes it".

litellm-rust 0.3.0 drops the response headers. `http.rs` `send_json_once` turns any non-2xx response into `LiteLLMError::http(format!("http {}: {}", status, body))`. The caller gets the status and the body text, but no headers. The header can't be read without forking the crate.

Providers usually repeat the wait in the error body: OpenAI-compatible APIs say "Please try again in 6.5s", and others say "retry after 20 seconds".

## Decision

- The litellm adapter (`archalign-agent/src/litellm.rs`) parses the status from the `"http <status>: <body>"` message. `retry_hint` reads "try again in N(ms|s|min)" or "retry after N …" from the body. Hints are capped at 600 s; anything else is ignored.
- If there is no hint, the 1 s / 2 s / 4 s backoff applies.
- `openai/…` models don't go through litellm-rust ([0034](0034-openai-via-responses-api.md)). That adapter reads the real `Retry-After` header (in seconds) and falls back to the body hint.

## Consequences

- Negative: with litellm-rust providers, a `Retry-After` header that isn't repeated in the body is ignored. The fixed backoff is used instead.
- Positive: no fork of litellm-rust.

## References

- litellm-rust 0.3.0 `src/http.rs` (`send_json_once`)
- `crates/archalign-agent/src/litellm.rs` (`map_error`, `retry_hint`, tests `http_errors_map_to_status_with_hints`, `retry_hints`)
- `crates/archalign-agent/src/retry.rs` (`honors_retry_after`)
