# 0013. Architecture diagrams only in v1

- Status: Accepted
- Date: 2026-10-10
- Related: [0011](0011-v1-viewer-feature-scope.md), [0012](0012-archify-json-v1-canonical-format.md), [0015](0015-diagram-storage-app-state-and-run-log.md), [0018](0018-app-computed-layout-dugong-and-orthogonal-routing.md); spec: FR-DIA, FR-GEN, FR-LAY

## Context

Archify supports five diagram types: architecture, workflow, sequence, dataflow and lifecycle.

- All five Archify schemas allow `sources` (dataflow L149, lifecycle L168, sequence L129, workflow L304), so all of them could support click-to-source navigation.
- Architecture is Archify's mode for components, services and libraries/CLI repos (`archify/SKILL.md` type router).
- Each other type needs its own layout compiler. The workflow compiler alone is 4,913 lines, and the Archify renderers total about 21.9k lines of JS.

## Decision

- v1 generates and renders only `diagram_type: "architecture"`.
- Other Archify types (workflow, sequence, dataflow, lifecycle) appear in History marked unsupported ([0015](0015-diagram-storage-app-state-and-run-log.md)).
- The model and renderer keep a per-type seam, for example a document enum with an `Architecture` variant, so types can be added later.

## Alternatives considered

### All five types in v1

Rejected: a multiple of the layout and rendering work.

## Consequences

- A smaller v1.
- Adding a type later means a new model variant, a layout engine ([0018](0018-app-computed-layout-dugong-and-orthogonal-routing.md)) and a renderer.
- Non-architecture diagrams produced by the Archify skill are visible in History but cannot be opened.

## References

- `archify/SKILL.md` (type router) and `archify/schemas/*.schema.json` (in the gitignored `archify-src/` checkout)
- https://github.com/tt-a1i/archify
- [../spec.md](../spec.md)
