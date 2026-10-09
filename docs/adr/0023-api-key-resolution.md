# 0023. API key resolution

- Status: Accepted
- Date: 2026-10-10
- Related: [0020](0020-built-in-llm-agent-authors-diagrams.md), [0021](0021-litellm-rust-behind-llm-client-trait.md), [0024](0024-data-egress-consent-and-network-policy.md); spec: FR-SEC

## Context

The agent needs a provider API key ([0020](0020-built-in-llm-agent-authors-diagrams.md)). The owner keeps the key in a `.env` file. The workspace uses edition 2024, where `std::env::set_var` is `unsafe` because other threads may read the environment. The imported repository may contain its own `.env`, which is untrusted.

## Decision

- **Lookup order:**
  1. the provider's environment variable in the process environment (`OPENAI_API_KEY` for `openai/…`; litellm's names otherwise: `ANTHROPIC_API_KEY`, `GEMINI_API_KEY`, `OPENROUTER_API_KEY`, `XAI_API_KEY`);
  2. `.env` in the working directory the app was launched from;
  3. `.env` in the app config directory.
- **Parsing.** Read `.env` with `dotenvy::from_path_iter`. Never call `std::env::set_var`. Pass the key to litellm via `ProviderConfig::with_api_key` ([0021](0021-litellm-rust-behind-llm-client-trait.md)).
- **Never** read the imported repo's `.env`, and never log or display the key. Settings shows only which source supplied it.
- **Keychain:** none in v1.
- The project's `.gitignore` already contains `.env` (added 2026-10-10).

## Alternatives considered

### OS keychain via `keyring`

Rejected. Dropped by the owner, whose key lives in `.env`.

### `dotenvy::dotenv()`

Rejected. It mutates the process environment.

### Reading the repo's `.env`

Rejected. Untrusted, and it holds the repo's own secrets.

## Consequences

- Positive: no process-environment mutation; the key never appears in logs or UI.
- Negative: launches from outside the project need the environment variable or the config-dir `.env`.
- Negative: the key sits in a plaintext file (the owner's choice).

## References

- [../spec.md](../spec.md) (FR-SEC)
- [../libraries.md](../libraries.md) (`dotenvy`)
