# 0026. System prompt derived from Archify

- Status: Accepted
- Date: 2026-10-10
- Related: [0004](0004-name-attribution-and-vendored-archify-material.md), [0018](0018-app-computed-layout-dugong-and-orthogonal-routing.md), [0020](0020-built-in-llm-agent-authors-diagrams.md), [0027](0027-submission-gate-and-repair-loop.md), [0029](0029-testing-strategy-and-live-smoke-runs.md); spec: FR-GEN, FR-VAL

## Context

Archify is an agent skill. Its authoring guidance lives in Markdown files written for an agent that drives the Node CLI. ArchAlign's built-in agent ([0020](0020-built-in-llm-agent-authors-diagrams.md)) needs a system prompt with the same authoring quality, but it has no Node CLI, no browser gate, and it does not place coordinates, because layout is app-computed ([0018](0018-app-computed-layout-dugong-and-orthogonal-routing.md)).

## Decision

- Condense the system prompt from these Archify (MIT) files:
  - `archify/SKILL.md`
  - `archify/references/authoring-defaults.md`
  - `archify/references/repository-authoring.md`
  - the relevant parts of `archify/references/authoring-contract.md`
- Store it in `crates/archalign-agent/prompts/`, embed it with `include_str!`, and credit it in `THIRD_PARTY_NOTICES.md` ([0004](0004-name-attribution-and-vendored-archify-material.md)).

| | Content |
|---|---|
| Dropped | Instructions about the Node CLI, finalize/browser gates, and coordinate placement. |
| Kept | Evidence discipline: trace call sites, cite path plus line ranges, name uncertainty, never invent services. |
| Kept | Composition guidance: primary path; add a boundary only when it answers a reader's question. |
| Kept | Schema field rules. |
| Added | Tool-usage instructions. |
| Added | The submission contract ([0027](0027-submission-gate-and-repair-loop.md)). |

## Alternatives considered

### Write the prompt from scratch

Rejected. It loses Archify's refined authoring guidance.

### Embed the references verbatim

Rejected. Token cost: `delivery-contract.md` is 41.4 KB and `authoring-contract.md` is 26.5 KB, and much of that covers the CLI and gates ArchAlign drops.

## Consequences

- Positive: the prompt is versioned in the repo and ships inside the binary.
- Positive: attribution obligations are met through `THIRD_PARTY_NOTICES.md`.
- Negative: the condensed prompt can drift from upstream Archify guidance; updating means re-condensing [inference].
- Follow-up: prompt quality is judged through live smoke runs ([0029](0029-testing-strategy-and-live-smoke-runs.md)).

## References

- `archify-src/archify/SKILL.md`
- `archify-src/archify/references/authoring-defaults.md`
- `archify-src/archify/references/repository-authoring.md`
- `archify-src/archify/references/authoring-contract.md`
- `crates/archalign-agent/prompts/`
- `THIRD_PARTY_NOTICES.md`
- Archify: https://github.com/tt-a1i/archify
- [../spec.md](../spec.md) (FR-GEN)
