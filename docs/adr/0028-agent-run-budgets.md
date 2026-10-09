# 0028. Agent run budgets

- Status: Accepted
- Date: 2026-10-10
- Related: [0015](0015-diagram-storage-app-state-and-run-log.md), [0022](0022-default-model-gpt-6-1-sol.md), [0025](0025-agent-tool-sandbox-and-deny-list.md), [0027](0027-submission-gate-and-repair-loop.md), [0030](0030-long-running-implementation-process.md); spec: FR-GEN, FR-VAL

## Context

An agent run calls a paid LLM API in a loop. Without limits, a confused model, a large repository or prompt injection ([0025](0025-agent-tool-sandbox-and-deny-list.md)) can drive cost and time up without bound. The run needs hard caps that fail clearly.

## Decision

Defaults, editable in Settings:

| Budget | Default |
|---|---|
| Model turns | at most 40 |
| Input tokens, total | at most 3,000,000 |
| `max_completion_tokens` per turn | 32,000 |
| Overall run time | 15 minutes |
| Per-request time | 300 s |
| `read_file` | 400 lines |
| `grep` | 100 matches |
| Any tool result | 24 KB |
| Repairs | 3 ([0027](0027-submission-gate-and-repair-loop.md)) |

- Hitting a cap fails the run with a message naming the cap.
- No partial candidate is saved.
- `run.jsonl` records the cap ([0015](0015-diagram-storage-app-state-and-run-log.md)).
- A text-only reply (no tool call) gets a reminder and counts as a turn.

## Alternatives considered

### No caps

Rejected. Runaway cost.

### A dollar cap

Rejected. It needs a price table that has to be maintained.

## Consequences

- Positive: cost and time per run are bounded.
- Negative: [inference] the worst case for uncached input on Sol ([0022](0022-default-model-gpt-6-1-sol.md)) is about $6 per run.
- Negative: large repositories may hit the caps. The Focus field narrows the scope.
- Follow-up: under [0030](0030-long-running-implementation-process.md), changing LLM spending limits requires the owner's approval.

## References

- [../spec.md](../spec.md) (FR-GEN)
- [../architecture.md](../architecture.md)
