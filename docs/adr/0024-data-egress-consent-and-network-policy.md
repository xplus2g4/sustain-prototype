# 0024. Data egress consent and network policy

- Status: Accepted
- Date: 2026-10-10
- Related: [0020](0020-built-in-llm-agent-authors-diagrams.md), [0021](0021-litellm-rust-behind-llm-client-trait.md), [0025](0025-agent-tool-sandbox-and-deny-list.md), [0030](0030-long-running-implementation-process.md); spec: FR-SEC, FR-GEN

## Context

The built-in agent ([0020](0020-built-in-llm-agent-authors-diagrams.md)) sends repository content to a third-party LLM provider. That is data leaving the machine. The user has to know what is sent, and to whom, before it happens. The app must not open any other network channel.

## Decision

### Consent

- Before the first run for each (repo root, provider) pair, show a modal that names the provider and the model.
- The modal lists what is sent:
  - the contents of files the agent reads;
  - file paths and grep matches;
  - git metadata (HEAD, the credential-stripped origin URL, dirty paths);
  - the Focus text.
- The modal also lists what is never sent: the deny-list from [0025](0025-agent-tool-sandbox-and-deny-list.md).
- Store consent in app state. It can be revoked in Settings.
- Declining keeps the app usable for viewing existing diagrams.

### Network policy

- The LLM API endpoint is the only network destination.
- No telemetry.
- No update checks. Archify has an optional update check; ArchAlign does not port it.
- No crash reporting.

### Logging

- `tracing` to stderr only. No log files.
- Logs never contain the API key or file contents.

## Alternatives considered

### No consent

Rejected. Data would leave the machine silently.

### Consent per run

Rejected. Too much friction for repeated runs on the same repository.

### One global consent

Rejected. It doesn't name the repository being sent.

## Consequences

- Positive: an explicit opt-in per repository and provider.
- Positive: the only outbound traffic is to the configured LLM endpoint.
- Negative: changing provider asks for consent again.
- Negative: with stderr-only logging, there is no persisted log to inspect after the fact; the per-run `run.jsonl` ([0015](0015-diagram-storage-app-state-and-run-log.md)) is the persisted record [inference].
- Follow-up: under the deviation policy in [0030](0030-long-running-implementation-process.md), any change that sends data off the machine requires the owner's approval.

## References

- [../spec.md](../spec.md) (FR-SEC)
- [../architecture.md](../architecture.md)
- Archify: https://github.com/tt-a1i/archify
