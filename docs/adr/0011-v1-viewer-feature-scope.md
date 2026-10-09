# 0011. v1 viewer feature scope

- Status: Accepted
- Date: 2026-10-10
- Related: [0010](0010-fonts-text-measurement-and-palette.md), [0012](0012-archify-json-v1-canonical-format.md), [0013](0013-architecture-diagrams-only-in-v1.md); spec: FR-DIA, FR-SRC, FR-TREE

## Context

Archify's HTML viewer runtime (`archify/references/viewer-runtime.md`) offers:

- Diagram Guide, Reading Depth, Semantic Lens and Intent Trace;
- Node Finder, Semantic Passport, Semantic Radar, Direct Relationship Pin and Route Probe;
- trace motion and Presentation Stage;
- PNG/JPEG/WebP/SVG/WebM exports;
- Route/Reach Share Cards.

ArchAlign renders natively and needs a defined v1 scope.

## Decision

In v1:

- pan, zoom and fit;
- select and focus;
- upstream/downstream reach over authored directed connections;
- a node finder over labels and IDs;
- boundaries, connection labels and variants, cards and legend;
- dark/light theme ([0010](0010-fonts-text-measurement-and-palette.md));
- clicking a node opens `sources[0]`, lists the other sources, and reveals the file in the tree;
- selecting a file highlights the nodes that cite it;
- clicking an edge highlights its endpoints;
- hover tooltips.

Out of scope:

- trace animation;
- all exports and share cards;
- presentation mode;
- radar/minimap, semantic lens, route probe;
- guided views (`meta.views`);
- reading-depth levels;
- translations and i18n (English only);
- node icons and brand marks.

Fields for out-of-scope features are still parsed and preserved ([0012](0012-archify-json-v1-canonical-format.md)).

## Alternatives considered

### Full parity with the Archify viewer

Rejected: huge, and much of it is HTML-specific (exports, share cards).

### Minimal viewer (no reach, no finder)

Rejected: drops exploration features that are cheap to build.

## Consequences

- Archify-authored diagrams that use views, translations or animation still open. Those features go unused.
- Saving does not lose the unused fields.

## References

- `archify/references/viewer-runtime.md` (in the gitignored `archify-src/` checkout)
- [../spec.md](../spec.md)
