# 0019. Geometry write-back and authored layouts

- Status: Accepted
- Date: 2026-10-10
- Related: [0012](0012-archify-json-v1-canonical-format.md), [0015](0015-diagram-storage-app-state-and-run-log.md), [0018](0018-app-computed-layout-dugong-and-orthogonal-routing.md); spec: FR-LAY, FR-STO

## Context

The agent writes topology only, and ArchAlign computes the layout ([0018](0018-app-computed-layout-dugong-and-orthogonal-routing.md)). Archify's renderer requires `pos` in free placement, or row/col in grid mode. Diagrams also arrive from Archify itself with authored geometry. ArchAlign must decide when to compute a layout, when to draw what the file says, and whether computed geometry is persisted.

## Decision

- **Write-back.** After an auto-layout for an agent run, or after an explicit Re-layout, write the computed `pos`/`size` (rounded to whole pixels) into `candidate.json`. Keep all other fields. Opening a diagram never writes.
- **On load**, a diagram is authored when every component has `pos`, or when `layout.mode: "grid"` gives every component a cell. Draw it as-is:
  - Convert grid cells with Archify's formula: `pos = origin + (col·(cellW+gapX), row·(cellH+gapY))`.
  - Grid defaults: origin [40,80], cols 4, gapX 30, gapY 40, cellW 130, cellH 64 (`grid.mjs` L3–11, L27–30).
  - `pos` wins over row/col.
  - A missing `size` defaults to Archify's 120×60 (`render-architecture.mjs` L61–62).
- If only some components have positions, auto-lay-out all of them and show a warning.
- **Re-layout** strips component `pos`, `size`, `row`, `col`, the `layout` block, and the connection routing hints (`via`, `fromSide`, `toSide`, `route`, `labelAt`, `labelDx`, `labelDy`, `labelSegment`). It then recomputes and saves.

Rationale: writing positions back keeps files renderable by the Archify CLI and makes reopening stable.

## Alternatives considered

### A sidecar layout file

Rejected. Archify can't render a candidate that has only topology.

### Recompute on every open

Rejected. The layout shifts whenever the engine changes, and the file stays Archify-incompatible.

### Write back on open

Rejected. It silently mutates the user's files.

## Consequences

- Positive: saved diagrams render in the Archify CLI and reopen identically.
- Positive: Archify-authored layouts, free or grid, display without change.
- Negative: layout-engine improvements reach existing diagrams only through Re-layout.
- Note: an auto-laid-out diagram becomes "authored" once saved.

## References

- [../spec.md](../spec.md) (FR-LAY, FR-STO)
- [../architecture.md](../architecture.md)
- Archify `grid.mjs` L3–11, L27–30; `render-architecture.mjs` L61–62 (in `archify-src/`)
- https://github.com/tt-a1i/archify
