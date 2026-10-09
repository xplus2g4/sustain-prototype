# 0020. A built-in LLM agent authors diagrams

- Status: Accepted
- Date: 2026-10-10
- Related: [0008](0008-async-model-tokio-and-iced-tasks.md), [0021](0021-litellm-rust-behind-llm-client-trait.md), [0022](0022-default-model-gpt-6-1-sol.md), [0023](0023-api-key-resolution.md), [0024](0024-data-egress-consent-and-network-policy.md), [0025](0025-agent-tool-sandbox-and-deny-list.md), [0026](0026-system-prompt-derived-from-archify.md), [0027](0027-submission-gate-and-repair-loop.md), [0028](0028-agent-run-budgets.md); spec: FR-GEN

## Context

Owner goal 1 is to import a repository and archify it. Archify is an agent skill: an LLM agent reads the repository and writes typed JSON, and a Node CLI (`bin/archify.mjs finalize`) validates, routes and emits HTML. ArchAlign is a native Rust app with no HTML or browser rendering, so it needs its own way to produce the Archify candidate.

## Decision

- ArchAlign runs its own agent loop in Rust (`archalign-agent`). The model plus read-only tools over the repo produce the Archify candidate. No external agent CLI, no Node.
- **Steering.** An optional Focus text. Regenerate creates a new timestamped diagram. No chat refinement in v1.
- **During a run:**
  - show live progress in the status bar plus a run log;
  - Cancel works;
  - the app stays usable;
  - only one run at a time;
  - a successful diagram opens automatically.
- Existing architecture JSON (for example, Archify-produced diagrams) opens without running the agent.

## Alternatives considered

### Spawn an installed agent CLI (`claude -p` / `codex exec`) with the Archify skill

Least effort. Rejected: it needs a user-installed, authenticated agent plus Node, and gives less control over tools and the sandbox.

### Static analysis only (e.g. a tree-sitter import graph)

Rejected. It yields a dependency graph, not an architecture, and loses semantic components, boundaries and labels.

### Import only

Rejected. Fails goal 1.

## Consequences

- Positive: one self-contained binary; no Node or external agent required.
- Negative: we own the prompt, tool sandbox, budgets and validation loop ([0021](0021-litellm-rust-behind-llm-client-trait.md)–[0028](0028-agent-run-budgets.md)).
- Negative: needs an API key ([0023](0023-api-key-resolution.md)) and data-egress consent ([0024](0024-data-egress-consent-and-network-policy.md)).
- Negative: LLM output is nondeterministic, so the validation gate ([0027](0027-submission-gate-and-repair-loop.md)) is essential.

## References

- [../spec.md](../spec.md) (FR-GEN)
- [../architecture.md](../architecture.md)
- https://github.com/tt-a1i/archify (`bin/archify.mjs`)
