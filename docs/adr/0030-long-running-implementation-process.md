# 0030. Long-running implementation process

- Status: Accepted
- Date: 2026-10-10
- Related: [0001](0001-record-architecture-decisions.md), [0002](0002-macos-only-without-ci.md), [0024](0024-data-egress-consent-and-network-policy.md), [0028](0028-agent-run-budgets.md), [0029](0029-testing-strategy-and-live-smoke-runs.md); spec: NFR-QUAL

## Context

Implementation is a long-running, mostly autonomous task ([0001](0001-record-architecture-decisions.md)). It must be resumable after interruption, auditable by the owner, and protected against silent scope changes. There is no CI ([0002](0002-macos-only-without-ci.md)).

## Decision

### Milestones

| Milestone | Scope |
|---|---|
| M0 | Scaffold, `.gitignore`, docs and ADRs |
| M1 | Model and validation |
| M2 | Layout and routing |
| M3 | App shell with panes, tree, source viewer and opening existing JSON |
| M4 | Diagram canvas and navigation |
| M5 | Agent |
| M6 | History, settings, theme, saved state and the watcher |
| M7 | Docs completion and the final live smoke |

### End of each milestone

- Local gates green ([0029](0029-testing-strategy-and-live-smoke-runs.md)).
- Docs updated: `architecture.md`, `libraries.md`, ADRs, `progress.md`. `spec.md` changes only with owner approval.
- One local git commit referencing the milestone and requirement IDs. Never push.

### Progress

`docs/progress.md` tracks the requirement checklists, plus one log line per milestone: date, what was verified, commands run.

### Dependency rule

A `libraries.md` entry comes before adding any crate.

### Deviation policy

- The implementer may supersede implementation-level decisions (internal design, a library, an algorithm) with a new ADR carrying the evidence, and update `libraries.md`/`architecture.md`.
- The implementer must stop and ask the owner before changing:
  - goals or non-goals;
  - user-visible behavior in `spec.md`;
  - anything that sends data off the machine ([0024](0024-data-egress-consent-and-network-policy.md));
  - LLM spending limits ([0028](0028-agent-run-budgets.md)).

### Definition of done

- Every acceptance criterion passes.
- Local gates and performance tests are green.
- The final live smoke passes.
- The owner's hands-on check is complete.
- The docs are accurate.

## Alternatives considered

### One big-bang delivery

Rejected. Not resumable and hard to audit.

### A CI-gated PR workflow

Rejected. There is no CI.

### Unconstrained autonomy

Rejected. Risks silent scope changes.

## Consequences

- Positive: progress is resumable from `progress.md` and the milestone commits.
- Positive: the history is auditable.
- Negative: nothing is pushed, so the only copy of the history is local [inference].
- Follow-up: superseded decisions follow the process in [0001](0001-record-architecture-decisions.md).

## References

- [../spec.md](../spec.md) (NFR-QUAL)
- [../architecture.md](../architecture.md)
- [../libraries.md](../libraries.md)
- [../progress.md](../progress.md)
