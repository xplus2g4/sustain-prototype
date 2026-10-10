# 0035. Rank direction chosen per diagram: left to right or top to bottom

- Status: Accepted (owner decision, 2026-10-10)
- Date: 2026-10-10
- Supersedes: [0031](0031-dugong-placement-and-grid-astar-router.md) for the rank direction only (0031 ranked every auto layout left to right)
- Related: [0019](0019-geometry-write-back-and-authored-layouts.md), [0010](0010-fonts-text-measurement-and-palette.md); spec: FR-LAY-04, FR-DIA-02, FR-STO-03, NFR-PERF-03

## Context

The first final-smoke run (`openai/gpt-6.1-sol` on HyperTrack) produced 15 components. Ranked left to right, they made a drawing about 4,000 px wide and 400 px tall.

- In the app, fitting that into the default Diagram pane (about 780 px wide) gives a zoom of about 0.19. Node labels fall below the minimum drawn text size, so the fitted view shows boxes with no labels (`target/smoke/HyperTrack/diagram.png`, before this change).
- Archify's CLI (`validate architecture … --repo-root`) passed the schema and render stages. It failed `composition/desktop-readability`: viewBox 4,056 px, projected text 2.7 px, minimum 6 px.
- The Luna development runs (7 and 11 components) had the same problem, less severely.

Long dependency chains are common in generated architecture diagrams. Because nodes are wider than they are tall, a rank costs about 270 px horizontally in a left-to-right layout (node width plus `ranksep`), but only about 170 px vertically in a top-to-bottom one.

The owner picked "auto direction" over a minimum fit zoom and over leaving it as an open issue.

## Decision

- `AutoDirectionEngine`, the default engine of `layout_document`, runs `DugongEngine` twice: once ranked left to right, once top to bottom. Both use the same `nodesep` 60, `ranksep` 110 and `edgesep` 20.
- A placement counts only if every node is placed and no boxes overlap. Among those, it keeps the one whose node bounding box fits a 1600×1000 (16:10) reference viewport at the larger scale (`fit_scale`). On a tie, left to right is kept. If neither direction works, the grid fallback from 0031 applies.
- Everything after placement stays as in 0031: boundaries, the grid A* router, label anchors, `ORIGIN` and write-back. The router chooses sides from relative positions, so it doesn't depend on the direction.
- Saved diagrams keep their written-back geometry ([0019](0019-geometry-write-back-and-authored-layouts.md)). An older diagram picks up the new direction only through Re-layout.

## Consequences

- Positive: the HyperTrack smoke diagram is readable when fitted, after Re-layout (`target/smoke/HyperTrack/diagram.png`). Archify's viewBox width dropped from 4,056 px to 1,897 px.
- Positive: 5 of the 24 fixture snapshots switched to top to bottom (`astron-agent-runtime`, `mco-runtime`, `archify-runtime`, `audit-terra`, `queue-luna`). They were reviewed visually (`target/smoke/m7/auto-*.png`).
- Negative: placement runs twice. NFR-PERF-03 went from about 14 ms to about 32 ms median, against a target of 100 ms.
- Negative: Archify's `desktop-readability` check can still fail for large diagrams, because its 8 px edge labels need a viewBox under about 1,800 px wide that also fits its 900 px viewport height. FR-STO-03 only asks for the result to be recorded.

## Alternatives considered

### Keep left to right; never fit below the label zoom

Labels stay readable, but wide diagrams open partly off-screen and Archify's check still fails. Not chosen by the owner.

### Leave it as specified

Not chosen by the owner.

### Pick by Archify's width-only budget

Archify caps by viewport height too, and the app fits both dimensions, so a 16:10 two-dimensional fit matches both readers better.

## References

- `crates/archalign-layout/src/engine.rs` (`AutoDirectionEngine`, `RankDirection`, `fit_scale`)
- `crates/archalign-layout/tests/layout.rs::auto_direction_picks_the_better_fitting_rank_direction`
- `crates/archalign-desktop/tests/live.rs::smoke_relayout_newest`
