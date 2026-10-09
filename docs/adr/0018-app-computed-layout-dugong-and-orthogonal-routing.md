# 0018. App-computed layout: dugong plus orthogonal routing

- Status: Superseded by [0031](0031-dugong-placement-and-grid-astar-router.md)
- Date: 2026-10-10
- Related: [0003](0003-cargo-workspace-and-crate-boundaries.md), [0010](0010-fonts-text-measurement-and-palette.md), [0012](0012-archify-json-v1-canonical-format.md), [0019](0019-geometry-write-back-and-authored-layouts.md), [0020](0020-built-in-llm-agent-authors-diagrams.md); spec: FR-LAY

## Context

Archify depends on LLM-authored coordinates (`pos`/`size`), plus a real-browser check (`bin/visual-check.mjs`) and LLM repair rounds. Its edge routing is large: `renderers/architecture/routing.mjs` (1,157 lines), `shared/geometry.mjs` (1,941) and `shared/route-quality.mjs` (614). ArchAlign has no browser gate, and LLM-placed coordinates would overlap [inference].

## Decision

- The model writes topology only. ArchAlign computes the layout whenever not every component has a position ([0019](0019-geometry-write-back-and-authored-layouts.md)).
- **Engine: `dugong` 0.8.0**, a Rust port of dagre (MIT/Apache-2.0), used as a compound graph:
  - components become nodes sized by `TextMeasure`;
  - boundaries become compound parents (nesting allowed; partial overlap is rejected earlier);
  - rank direction is left to right, because Archify JSON has no direction field.
- **Edge routing (in-house):**
  - turn dagre's polyline bend points into axis-aligned segments through the gaps between ranks;
  - place endpoints on node sides;
  - honor authored `fromSide`/`toSide`/`via`;
  - route edges around nodes that aren't their endpoints;
  - put labels at the midpoint of the longest segment;
  - draw arrowheads at the target.
- Put both behind a `LayoutEngine` trait, so ELK can be trialled later.
- Keep `archalign-layout` pure and deterministic.

## Alternatives considered

### The LLM authors `pos`/`size` (Archify's way)

Rejected. A native pipeline has no visual check to catch bad placement.

### The LLM places nodes in an Archify grid (row/col)

Rejected. Coarse, and still LLM placement.

### `merman-layout-elk` 0.8.0

An ELK layered port with nested groups and orthogonal routing built in. Rejected:

- it links `merman-elk-layered`, which is EPL-2.0, so we'd have to ship the EPL text plus source-availability information;
- stable only since 2026-10-05, about 15% doc coverage, and self-described as an internal adapter.

### `mmdflux` 2.6.1

MIT, with compound layout plus orthogonal routing. Rejected: input must be Mermaid or MMDS text, and subgraph bounds are width/height only.

### `rust-sugiyama` 0.4.0, `layout-rs` 0.1.3, `gen-sugiyama`, `abstracttui-graph`

Rejected. No cluster support (the `layout-rs` docs say it "lacks … nested graphs").

### Port Archify's `routing.mjs`

Rejected. A large port: 1,157 lines plus geometry (1,941) and route quality (614).

### Fully in-house Sugiyama

Rejected. The most code.

## Consequences

- Positive: layout is computed, testable headless and deterministic; the engine can be swapped behind `LayoutEngine`.
- Negative: edge-route quality won't match Archify's.
- Negative: dagre's spacing is tuned for polylines, not orthogonal routes.
- Follow-up: guard quality with property tests: no overlap, containment, edges avoid nodes, determinism.

## References

- [../spec.md](../spec.md) (FR-LAY)
- [../libraries.md](../libraries.md) (`dugong`)
- [../architecture.md](../architecture.md)
- `archify-src/renderers/architecture/routing.mjs`, `shared/geometry.mjs`, `shared/route-quality.mjs`, `bin/visual-check.mjs`
- https://github.com/tt-a1i/archify
