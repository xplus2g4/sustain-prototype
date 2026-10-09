# 0031. dugong placement plus one grid A* orthogonal router

- Status: Accepted
- Date: 2026-10-10
- Supersedes: [0018](0018-app-computed-layout-dugong-and-orthogonal-routing.md)
- Related: [0010](0010-fonts-text-measurement-and-palette.md), [0019](0019-geometry-write-back-and-authored-layouts.md), [0029](0029-testing-strategy-and-live-smoke-runs.md); spec: FR-LAY, NFR-PERF-03

## Context

[0018](0018-app-computed-layout-dugong-and-orthogonal-routing.md) chose `dugong` for placement and an in-house router that orthogonalizes dagre's polyline bend points through the gaps between ranks, while authored layouts would be routed separately.

While implementing M2 it became clear that this split doesn't hold up:

- [0019](0019-geometry-write-back-and-authored-layouts.md) writes `pos`/`size` back after every run, so every saved diagram reopens as an **authored** layout. A dagre-only router would be used only for the few seconds between a run finishing and the next reopen. The diagram would then change its routes on reopen, and the router the owner actually sees (the authored one) would need all the same rules anyway: sides, node avoidance, parallel offsets, `via`.
- dagre's bend points don't carry `fromSide`/`toSide`/`via`, and they don't exist for authored layouts.

Measurements (release build, Apple silicon, 2026-10-10):

- One sparse-grid A* router over all node rectangles routes the 60-node / 120-edge benchmark in about 6 ms; the whole of layout plus routing takes a median of 13 ms (`perf_layout_and_routing_of_60_nodes_under_100ms`; the target is 100 ms). A first version with a plain Manhattan heuristic took 122 ms for routing alone; a bend-aware lower bound and tie-breaking toward deeper states fixed that.
- The FR-LAY-07 property test passed 2,400+ random graphs (up to 60 nodes, 120 edges, 2-level boundaries) once self-loop nesting was capped.

## Decision

- **Placement (unchanged from 0018).** `dugong` 0.8.0 as a compound graph ranked left to right. Components are nodes sized by `TextMeasure`; boundaries are compound parents (a component's parent is its innermost boundary); self-loops are left out of the dagre graph. Settings: `nodesep` 60, `ranksep` 110, `edgesep` 20. Coordinates are rounded to whole pixels and the drawing, boundaries included, is moved to start at (40, 40).
- **Fallback engine.** If dugong returns an error, panics (caught with `catch_unwind`), returns non-finite coordinates, or produces overlapping boxes, a square-grid placement is used instead. It can't overlap and never fails.
- **One router for every layout.** Authored and auto layouts use the same router (`archalign-layout::route`):
  1. Sides: authored `fromSide`/`toSide`, otherwise from the relative position of the two boxes.
  2. Ports: spread evenly along each side, ordered by the far end's position.
  3. A* over a sparse grid made of every node's edges inflated by a 10 px margin, the port and `via` lines, and corridor midlines (plus quarter lines in corridors wider than 64 px). Every node is an obstacle, so a route can't enter any node except at its own ports. Cost: length, plus 40 px per bend, plus 3× the length of any grid segment that another route already uses. The heuristic adds a lower bound on the bends still needed.
  4. Authored `via` points are visited in order.
  5. A self-loop is a rectangle on the node's right side, at most 48 px wide; further loops reuse the outermost size.
  6. The label anchor is the midpoint of the longest segment, and the arrowhead is at the target.
- **`LayoutEngine` stays.** An ELK engine can still be trialled behind it.

## Alternatives considered

### Orthogonalize dagre's bend points (0018)

Rejected: it only applies until the first reopen (see Context), and a second router is still needed for authored layouts.

### Write the computed routes back as `via`/`fromSide`/`toSide`

That would keep reopened routes identical even with a dagre-only router. Rejected: FR-LAY-08 and [0019](0019-geometry-write-back-and-authored-layouts.md) write back only `pos`/`size`, and Re-layout strips the routing hints.

### A full orthogonal visibility graph (libavoid style)

Better routes in dense diagrams, but much more code. The sparse grid already meets the invariants and the time budget.

## Consequences

- Positive: a diagram looks the same right after a run and after it is reopened.
- Positive: the FR-LAY-07 invariants hold by construction for every route the grid search finds. Clearance comes from the inflated obstacles.
- Negative: routes can share corridor lines in dense regions. The usage penalty discourages this but doesn't forbid it.
- Negative: edge labels aren't reserved in placement, so a label can overlap a box or another line.
- Follow-up: if the router ever can't find a path (only possible when boxes are less than 20 px apart, i.e. in authored layouts), it falls back to an L-shaped leg.

## References

- `crates/archalign-layout/src/{engine,route,scene,boundaries}.rs`
- `crates/archalign-layout/tests/{layout,invariants,snapshots,perf}.rs`
- [../spec.md](../spec.md) (FR-LAY, NFR-PERF-03)
- [../libraries.md](../libraries.md) (`dugong`, `insta`, `proptest`)
