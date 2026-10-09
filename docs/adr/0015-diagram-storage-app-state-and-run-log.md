# 0015. Diagram storage, app state and run log

- Status: Accepted
- Date: 2026-10-10
- Related: [0004](0004-name-attribution-and-vendored-archify-material.md), [0006](0006-three-pane-grid-layout.md), [0010](0010-fonts-text-measurement-and-palette.md), [0012](0012-archify-json-v1-canonical-format.md), [0013](0013-architecture-diagrams-only-in-v1.md), [0017](0017-file-watching.md); spec: FR-STO, FR-GEN

## Context

Generated diagrams, per-run logs and app settings need a home. Archify stores its outputs in `<repo>/.archify/` (`archify/SKILL.md` L15), and ArchAlign interoperates with the Archify CLI and skill ([0004](0004-name-attribution-and-vendored-archify-material.md)).

## Decision

**Location.** Write each diagram to `<repo>/.archify/architecture-<slug>-<YYYYMMDD-HHMMSS>/`, following Archify's convention.

- The timestamp is local time at run start, from `jiff`.
- The slug is kebab-case ASCII of at most 40 characters, taken from the Focus text, or from the repo folder name if Focus is empty.
- Same-second collisions get a `-2` suffix.

**Contents.**

- `candidate.json` (only on success) and `run.jsonl` (always).
- `meta.output` is `.archify/<folder>/<slug>.html`, relative to the repo root. It is never written, but kept so the Archify CLI can still render the file.
- Writes are atomic: write a temp file, then rename.

**History.**

- History lists folders that contain `candidate.json`. The newest valid one opens on import.
- A failed or cancelled run keeps only `run.jsonl`.
- If `.archify/` can't be created or written, the run fails before any model call.
- Non-architecture diagrams are marked unsupported ([0013](0013-architecture-diagrams-only-in-v1.md)).

**App state.** JSON in the OS config dir via `directories` (app name `ArchAlign`):

- window size, pane layout ([0006](0006-three-pane-grid-layout.md)), theme ([0010](0010-fonts-text-measurement-and-palette.md)), recent repos and consents;
- settings: model, `reasoning_effort`, budgets, show hidden files.

**`run.jsonl` events:**

- `run_started`
- `turn` (with usage)
- `tool_call` (name, arguments, result byte count, error)
- `submission_rejected` (errors)
- `fields_dropped`
- `run_finished` (outcome, reason, totals)

The log never contains tool output bodies, file contents or the API key.

## Alternatives considered

### Diagrams only in the app data dir

Rejected: invisible to the Archify CLI and skill.

### One file per repo, overwritten

Rejected: loses history.

### SQLite

Rejected: not needed.

## Consequences

- The app writes into the user's repo; `.archify/` may show up as untracked in git.
- Interoperates with the Archify skill's outputs.
- Each run, including failures, leaves an auditable log.
- Changes under `.archify/` are picked up by the watcher ([0017](0017-file-watching.md)).

## References

- `archify/SKILL.md` L15 (in the gitignored `archify-src/` checkout)
- [../libraries.md](../libraries.md) (`jiff`, `directories`), [../spec.md](../spec.md), [../architecture.md](../architecture.md)
