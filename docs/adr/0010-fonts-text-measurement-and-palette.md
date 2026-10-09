# 0010. Fonts, text measurement and palette

- Status: Accepted
- Date: 2026-10-10
- Related: [0004](0004-name-attribution-and-vendored-archify-material.md), [0005](0005-iced-native-gui-toolkit.md), [0009](0009-custom-virtualized-source-viewer.md), [0011](0011-v1-viewer-feature-scope.md), [0018](0018-app-computed-layout-dugong-and-orthogonal-routing.md); spec: FR-DIA, FR-LAY, FR-SRC, NFR-QUAL

## Context

The app computes diagram layout itself ([0018](0018-app-computed-layout-dugong-and-orthogonal-routing.md)), so node sizes depend on measured label text. Measurement must match what is drawn and must be deterministic in tests. ArchAlign also renders Archify diagrams and should look like them.

## Decision

- **Font.** Bundle the JetBrains Mono TTF (OFL-1.1) in `archalign-desktop`. Load it at startup with `iced::application(…).font(bytes)`. Use it for diagram labels and the source viewer ([0009](0009-custom-virtualized-source-viewer.md)).
- **Measurement.** Measure node sizes with `iced::advanced::graphics::text::Paragraph::with_text(…).min_bounds()`. It uses the global font system and needs no renderer. Put it behind the `TextMeasure` trait in `archalign-layout`. Tests use a fixed-advance monospace measurer so snapshots are deterministic.
- **Palette.** Copy Archify's dark and light colors from `archify/assets/template.html` into a Rust theme table ([0004](0004-name-attribution-and-vendored-archify-material.md)), covering:
  - the 7 component types: frontend, backend, database, cloud, security, messagebus, external;
  - the 4 connection variants: default, emphasis, security, dashed;
  - the 2 boundary kinds: region, security-group;
  - the 7 card dot colors: cyan, emerald, violet, amber, rose, orange, slate.
- **Theme.** Dark by default, with a toggle. The choice is saved in app state.
- **Not drawn in v1.** Node icons (`icon`) and brand marks (`brand`) are parsed and preserved but not drawn ([0011](0011-v1-viewer-feature-scope.md)).

## Alternatives considered

### System fonts

Rejected: measurement differs between machines.

### A proportional UI font (e.g. Inter) for labels

Rejected: measurement differences complicate tests.

### Our own palette

Rejected: loses Archify's visual identity.

## Consequences

- Layout is deterministic.
- `THIRD_PARTY_NOTICES.md` must carry the JetBrains Mono OFL-1.1 notice.
- The font file adds to the binary size [inference: a few hundred KB].
- Palette values can drift from upstream Archify and must be re-copied on update.

## References

- `archify/assets/template.html` (in the gitignored `archify-src/` checkout)
- `iced::advanced::graphics::text::Paragraph`, https://docs.rs/iced/0.14.0
- [../libraries.md](../libraries.md), [../spec.md](../spec.md)
