# 0008. Async model: tokio and iced tasks

- Status: Accepted
- Date: 2026-10-10
- Related: [0005](0005-iced-native-gui-toolkit.md), [0017](0017-file-watching.md), [0020](0020-built-in-llm-agent-authors-diagrams.md), [0021](0021-litellm-rust-behind-llm-client-trait.md), [0028](0028-agent-run-budgets.md); spec: FR-GEN, FR-WAT, NFR-PERF

## Context

Agent runs make long HTTP calls to an LLM. File listing, file loads, citation checks and git commands block. Watcher events arrive from a separate thread. None of this may block the UI, and runs must be cancellable.

## Decision

- Use the iced `tokio` feature as the executor.
- An agent run is `Task::run(stream, ...)`, where the stream is built with `iced::stream::channel` and emits `AgentEvent`s.
- Cancel uses `Task::abortable` -> `Handle::abort()`, which drops the future and the in-flight HTTP request.
- File-system work (listing, file loads, citation checks, git commands) goes through `Task::perform`, with `tokio::task::spawn_blocking` inside.
- Watcher events reach the UI through a `Subscription::run_with(repo_root, ...)` stream, fed by a channel from the debouncer thread ([0017](0017-file-watching.md)).
- One active run per window.

## Alternatives considered

### Hand-managed std threads and channels

More plumbing and manual cancellation.

### iced's default thread-pool executor

reqwest and litellm-rust need a tokio runtime.

### `Subscription::run` for agent runs

It takes a non-capturing `fn` pointer, which is awkward for per-run parameters.

## Consequences

- The UI never blocks.
- Cancelling a run drops the in-flight request immediately.
- Blocking tool work that is already inside `spawn_blocking` finishes within its own caps (grep <=5 s) after a cancel.

## References

- https://docs.rs/iced/0.14.0
- [`../architecture.md`](../architecture.md)
- [`../libraries.md`](../libraries.md)
