# 0014. Schema validation with vendored Archify schemas

- Status: Accepted
- Date: 2026-10-10
- Related: [0004](0004-name-attribution-and-vendored-archify-material.md), [0012](0012-archify-json-v1-canonical-format.md), [0027](0027-submission-gate-and-repair-loop.md); spec: FR-VAL, FR-GEN

## Context

Archify's Node CLI validates JSON with AJV before rendering. ArchAlign loads Archify JSON as its canonical format ([0012](0012-archify-json-v1-canonical-format.md)) and gates agent submissions ([0027](0027-submission-gate-and-repair-loop.md)), so it needs the same checks. The schemas `architecture.schema.json` and `common.schema.json` are vendored into `crates/archalign-model/schemas/` ([0004](0004-name-attribution-and-vendored-archify-material.md)).

## Decision

- Validate documents with the `jsonschema` crate (0.58, draft 2020-12) against the vendored `architecture.schema.json` and `common.schema.json`.
- Register the common schema as a resource so `common.schema.json#/$defs/…` references resolve offline.
- Turn default features off: no HTTP or file reference fetching.
- Order: schema validation, then strict serde parsing, then semantic checks.
- Each error carries a JSON pointer and a message. The same text is sent back to the model for repair.

Rules this reproduces exactly:

- ID pattern `^[a-zA-Z][a-zA-Z0-9_-]*$`;
- the enums: componentType, variant, side, boundary kind, card dot;
- 1–3 source references per component;
- source `path` at most 240 characters, source `label` at most 48;
- `repository.revision` matching `^[a-fA-F0-9]{40}$`;
- the `portableOutputPath` rules.

## Alternatives considered

### serde plus hand-written constraint checks

Fewer dependencies, but rejected because it drifts from Archify's AJV gate.

### Other JSON Schema crates

Not evaluated.

## Consequences

- Exact parity with Archify's schema gate.
- Validation works offline.
- Vendored schemas must be re-copied when upstream changes.

## References

- `crates/archalign-model/schemas/architecture.schema.json`, `crates/archalign-model/schemas/common.schema.json`
- `bin/archify.mjs finalize` (AJV), in the gitignored `archify-src/` checkout
- [../libraries.md](../libraries.md) (`jsonschema`), [../spec.md](../spec.md)
