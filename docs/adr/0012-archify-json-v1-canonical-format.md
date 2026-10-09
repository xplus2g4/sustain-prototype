# 0012. Archify JSON v1 as the canonical format

- Status: Accepted
- Date: 2026-10-10
- Related: [0003](0003-cargo-workspace-and-crate-boundaries.md), [0013](0013-architecture-diagrams-only-in-v1.md), [0014](0014-schema-validation-with-vendored-archify-schemas.md), [0015](0015-diagram-storage-app-state-and-run-log.md), [0019](0019-geometry-write-back-and-authored-layouts.md); spec: FR-VAL, FR-STO

## Context

Archify's agent writes typed JSON that follows `archify/schemas/architecture.schema.json` (`schema_version: 1`). Components carry authored `pos`/`size` and `sources: [{path, line, end_line, label}]` (1–3 per component). The Node CLI (`bin/archify.mjs finalize`) validates and renders it. ArchAlign needs a model on disk and in memory, and should interoperate with Archify outputs.

## Decision

- Archify JSON `schema_version: 1` (architecture) is the canonical model on disk and in memory, owned by `archalign-model` ([0003](0003-cargo-workspace-and-crate-boundaries.md)).
- Strict serde types cover every field of the architecture and common schemas. `deny_unknown_fields` mirrors `additionalProperties: false`.
- Saving is lossless: load then save produces equal JSON values, not identical bytes. Output is pretty-printed with 2-space indentation, and whole numbers are written as integers.
- Files ArchAlign writes must remain valid input for Archify's Node CLI. That CLI requires `pos` on every component in free placement (`render-architecture.mjs` L441–443) or `row`/`col` in grid mode (`grid.mjs` L44–45). This is why [0019](0019-geometry-write-back-and-authored-layouts.md) writes positions back.
- Schema validation is described in [0014](0014-schema-validation-with-vendored-archify-schemas.md).

## Alternatives considered

### A new Rust-native model

Rejected: incompatible with existing `.archify` outputs and community examples, and would need converters.

### Untyped `serde_json::Value`

Rejected: no compile-time guarantees.

## Consequences

- Two-way compatibility with Archify.
- `schema_version` is the constant 1. A future Archify schema change requires updating ArchAlign.
- Unknown fields are rejected with clear errors.

## References

- `archify/schemas/architecture.schema.json`, `archify/schemas/common.schema.json`
- `render-architecture.mjs` L441–443, `grid.mjs` L44–45 (in the gitignored `archify-src/` checkout)
- https://github.com/tt-a1i/archify
- [../spec.md](../spec.md), [../architecture.md](../architecture.md)
