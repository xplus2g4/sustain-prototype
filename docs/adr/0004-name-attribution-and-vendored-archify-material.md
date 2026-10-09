# 0004. Name, attribution and vendored Archify material

- Status: Accepted
- Date: 2026-10-10
- Related: [0003](0003-cargo-workspace-and-crate-boundaries.md), [0010](0010-fonts-text-measurement-and-palette.md), [0012](0012-archify-json-v1-canonical-format.md), [0014](0014-schema-validation-with-vendored-archify-schemas.md), [0015](0015-diagram-storage-app-state-and-run-log.md), [0026](0026-system-prompt-derived-from-archify.md); spec: FR-STO, NFR-QUAL

## Context

ArchAlign is an unofficial port of Archify (https://github.com/tt-a1i/archify, MIT, v3.0.1). It reuses Archify schemas, fixtures, colors and prompt material. The local reference checkout lives at `archify-src/` (commit `27f78a2bd13a1ee0c71d5288650670513bf64e7c`, 2026-10-09) and is gitignored. The project needs a name that does not imply official status, and the reused material needs attribution.

## Decision

Naming:

- product name "ArchAlign";
- crate prefix `archalign-`;
- binary `archalign`;
- app name for the OS config dir: `ArchAlign`;
- live-test switch `ARCHALIGN_LIVE=1`.

README and About text: "ArchAlign: native viewer and generator for Archify architecture diagrams. Unofficial; not affiliated with Archify. Includes material derived from Archify (MIT)."

Diagrams stay in `<repo>/.archify/`, so ArchAlign interoperates with the Archify CLI and skill.

Vendoring, with a root `THIRD_PARTY_NOTICES.md` that carries the Archify MIT license text (with upstream commit `27f78a2...`) and the JetBrains Mono OFL-1.1 license:

- `archify/schemas/architecture.schema.json` and `common.schema.json` go to `crates/archalign-model/schemas/`.
- Architecture JSON fixtures go to `fixtures/archify/`, copied from:
  - `archify-src/archify/examples/*.architecture.json`
  - `archify-src/examples/*.architecture.json`
  - `archify-src/docs/cases/*.architecture.json`
  - `archify-src/archify/test/fixtures/**/*.architecture.json`
  - the Archify-skill candidate in `.archify/architecture-archify-runtime-20261009-221013/candidate.json`
- Colors from `archify/assets/template.html` go into a Rust theme table.
- The condensed prompt goes to `crates/archalign-agent/prompts/`.

`archify-src/` is gitignored (`.gitignore` line 2) and is never a build input.

## Alternatives considered

### The name "Archify Desktop"

Implies official status. The MIT license covers the code, not the name.

### `include_str!` pointing into `archify-src/`

Breaks fresh clones, because the folder is gitignored.

### A git submodule of Archify

Heavier, and not needed for two schemas and some fixtures.

## Consequences

- Attribution obligations are met.
- Vendored copies can drift from upstream. Updating means re-copying the files and bumping the notice.
- Builds work from a fresh clone without `archify-src/`.

## References

- https://github.com/tt-a1i/archify
- `THIRD_PARTY_NOTICES.md`
- `crates/archalign-model/schemas/`, `fixtures/archify/`, `crates/archalign-agent/prompts/`
- [`../libraries.md`](../libraries.md)
