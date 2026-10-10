# 0034. OpenAI models through the Responses API

- Status: Accepted (owner decision, 2026-10-10)
- Date: 2026-10-10
- Supersedes: [0021](0021-litellm-rust-behind-llm-client-trait.md) for the `openai/` provider only
- Related: [0022](0022-default-model-gpt-6-1-sol.md), [0024](0024-data-egress-consent-and-network-policy.md), [0033](0033-retry-after-from-error-bodies-for-litellm-providers.md); spec: FR-GEN-05, FR-GEN-08, FR-GEN-13, FR-SEC-05

## Context

On 2026-10-10, the first live development run (`openai/gpt-6-luna` on HyperTrack, live run 1 of 20) failed on turn 1 with:

> HTTP 400: Function tools with reasoning_effort are not supported for gpt-6-luna in /v1/chat/completions. To use function tools, use /v1/responses or set reasoning_effort to 'none'.

The request had no `reasoning_effort` (FR-GEN-13: unset by default), so the conflict comes from the model's own default reasoning. litellm-rust 0.3.0 only supports Chat Completions ([0021](0021-litellm-rust-behind-llm-client-trait.md): "no Responses-API features"). The same restriction is expected for `gpt-6.1-sol`, the default model ([0022](0022-default-model-gpt-6-1-sol.md)).

Three options went to the owner:
- an OpenAI Responses adapter;
- defaulting `reasoning_effort` to `none`;
- a gpt-5.x default model.

The owner chose the Responses adapter.

## Decision

- `archalign_agent::client_for(model, key, timeout)` picks the client:
  - `openai/…` → `openai::OpenAiResponsesClient`;
  - every other provider → `litellm::LiteLlmClient` (0021, unchanged).
- **Transport.** `OpenAiResponsesClient` sends `POST {base}/responses` with our reqwest 0.12 client (300 s timeout, rustls) and `Authorization: Bearer <key>`. `base` is `https://api.openai.com/v1`, the same host as before. No new dependency.
- **Request.**
  - `input`: the system prompt as a `developer` message, user messages, then for each assistant turn: its replayed reasoning items, any text, and one `function_call` item per tool call. Tool results become `function_call_output` items.
  - `tools`: the same schemas, flattened to `{type, name, description, parameters}`.
  - `tool_choice: "auto"`, `parallel_tool_calls: true`, `max_output_tokens` (the `max_completion_tokens` budget).
  - `reasoning.effort` only when the setting is set (FR-GEN-13 unchanged).
  - `store: false`, so OpenAI doesn't keep the response.
  - `include: ["reasoning.encrypted_content"]`.
- **Reasoning across turns.** Encrypted `reasoning` output items are returned in `TurnResponse::replay`. They are kept on `ChatMessage::Assistant { replay }` and sent back verbatim with later turns. They are opaque, never written to `run.jsonl`, and never shown.
- **Response.**
  - `output_text` parts become the text, `function_call` items become tool calls (the `call_id` is the ID), `usage.input_tokens`/`output_tokens` become usage.
  - `status: "incomplete"` with `max_output_tokens` is reported as a truncation; a `refusal` part is reported as a refusal.
- **Errors.** The HTTP status maps to `LlmError::Status`, with `error.message` from the body. The real `Retry-After` header (seconds) is honored, and the body hint from [0033](0033-retry-after-from-error-bodies-for-litellm-providers.md) is the fallback. reqwest timeouts map to `LlmError::Timeout`, which is retried.

## Alternatives considered

### Default `reasoning_effort` to `none` on Chat Completions

The smallest change, but it would change the FR-GEN-13 default and probably weaken diagrams. Rejected by the owner.

### A gpt-5.x default model

That would change [0022](0022-default-model-gpt-6-1-sol.md) and the final smoke plan. Rejected by the owner.

### Fork or patch litellm-rust

Adding a Responses provider to the pinned crate would mean maintaining a fork. Not chosen.

## Consequences

- Positive: gpt-6.x models use tools with their default reasoning. Live runs 2 and 3 (Luna on HyperTrack and cs2103-ip) succeeded.
- Positive: `Retry-After` works for OpenAI.
- Positive: `store: false` reduces what the provider retains. The data sent is the same as before: the consent text (FR-SEC-03) and the network policy ([0024](0024-data-egress-consent-and-network-policy.md)) are unchanged.
- Negative: about 300 lines of our own wire code, tested offline with fixtures and a local socket (`crates/archalign-agent/src/openai.rs` tests). OpenAI API changes need updates here.
- FR-SEC-05's "the provider endpoint" now means either litellm-rust or this adapter. Both call only the configured provider host.

## References

- `crates/archalign-agent/src/openai.rs`, `crates/archalign-agent/src/lib.rs` (`client_for`)
- `crates/archalign-desktop/src/agent_run.rs` (`provider_factory`)
- [../progress.md](../progress.md) (live runs)
