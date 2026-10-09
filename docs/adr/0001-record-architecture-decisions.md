# 0001. Record architecture decisions

- Status: Accepted
- Date: 2026-10-10
- Related: [0030](0030-long-running-implementation-process.md); spec: NFR-QUAL

## Context

Implementation will be a long-running, mostly autonomous task. The project owner requires every decision to be justified in `docs/adr`. Decisions need a durable, auditable record that survives across work sessions.

## Decision

- Record decisions as MADR-lite ADRs at `docs/adr/NNNN-kebab-title.md`.
- Each ADR has the sections Context, Decision, Alternatives considered, Consequences and References.
- Status values are `Proposed`, `Accepted` and `Superseded by NNNN`.
- Never rewrite an accepted decision. To change it, add a new ADR and mark the old one `Superseded by NNNN`.
- Write one ADR per decision that had real alternatives. Details without real alternatives go into [`docs/spec.md`](../spec.md) as requirements, not ADRs.
- [`docs/adr/README.md`](README.md) indexes every ADR with its status.
- Document roles:
  - [`spec.md`](../spec.md) is the requirements contract: numbered requirements with acceptance criteria.
  - [`architecture.md`](../architecture.md) describes the structure.
  - [`libraries.md`](../libraries.md) justifies every dependency and bundled asset.
  - [`progress.md`](../progress.md) tracks milestone completion.

## Alternatives considered

### Nygard's original ADR template

It has no alternatives section, so the justification for each decision is weaker.

### Full MADR template

Decision drivers and pros/cons per option make each record heavier than needed.

### No ADRs

Violates the owner's goal that every decision is justified in `docs/adr`.

## Consequences

- The owner can audit decisions later.
- Superseding instead of editing keeps the history.
- The project starts with about 30 initial ADRs.
- Small details without real alternatives live in the spec, so the ADR set stays focused.

## References

- `docs/adr/README.md`
- `docs/spec.md`, `docs/architecture.md`, `docs/libraries.md`, `docs/progress.md`
