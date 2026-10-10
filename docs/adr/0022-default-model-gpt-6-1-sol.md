# 0022. Default model: gpt-6.1-sol

- Status: Accepted; the transport for `openai/` models is set by [0034](0034-openai-via-responses-api.md)
- Date: 2026-10-10
- Related: [0020](0020-built-in-llm-agent-authors-diagrams.md), [0021](0021-litellm-rust-behind-llm-client-trait.md), [0028](0028-agent-run-budgets.md), [0029](0029-testing-strategy-and-live-smoke-runs.md); spec: FR-GEN

## Context

The agent ([0020](0020-built-in-llm-agent-authors-diagrams.md)) needs a default model that handles agentic, tool-calling work at a reasonable price.

- On 2026-10-10, `GET /v1/models` with the owner's key listed `gpt-6.1-sol`, `gpt-6-luna`, `gpt-6-astra` and the gpt-5.x models.
- All three 6.x models support Chat Completions, with a 1,050,000-token context and 128,000 max output.
- Prices per 1M tokens:

  | Model | Input | Output | Cached input |
  |---|---|---|---|
  | GPT-6 Astra | $10 | $50 | $1 |
  | GPT-6.1 Sol | $2 | $10 | $0.10 |
  | GPT-6 Luna | $0.10 | $0.50 | $0.01 |

- OpenAI on GPT-6.1 Sol: "nearly matches GPT-6 Astra's intelligence on agentic coding, computer use, and professional work at one-fifth of Astra's standard input and output token prices".

## Decision

- The default model is `openai/gpt-6.1-sol`.
- Settings expose the model string (litellm `provider/model` form) and `reasoning_effort`. `reasoning_effort` is unset by default, so the provider default applies.
- Development live tests use `openai/gpt-6-luna`.

## Alternatives considered

### GPT-6 Astra

Rejected. 5× Sol's price.

### GPT-6 Luna

Rejected as default. Cheap, but weaker. Used for development live tests.

### gpt-5.x

Rejected. Older.

## Consequences

- [inference] A typical Sol run should cost roughly $0.5–2 with prompt caching.
- The run budgets ([0028](0028-agent-run-budgets.md)) bound the worst case.
- Users can switch to any litellm-supported `provider/model` in Settings.

## References

- https://developers.openai.com/api/docs/models/compare.md
- https://openai.com/index/introducing-gpt-6-1-sol/
- [../spec.md](../spec.md) (FR-GEN)
