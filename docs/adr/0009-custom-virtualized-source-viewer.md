# 0009. Custom virtualized source viewer

- Status: Accepted
- Date: 2026-10-10
- Related: [0005](0005-iced-native-gui-toolkit.md), [0010](0010-fonts-text-measurement-and-palette.md), [0011](0011-v1-viewer-feature-scope.md); spec: FR-SRC, NFR-PERF

## Context

The source panel shows a repository file read-only and keeps the range cited by a diagram node visible. iced 0.14 has no virtualized list widget ([0005](0005-iced-native-gui-toolkit.md)). Its `text_editor` highlighter `Format` has only `color` and `font`, and the selection is drawn only while the editor is focused. A cited range therefore can't stay highlighted with `text_editor`.

## Decision

Build a custom read-only viewer in `archalign-desktop`:

- Use `scrollable` + `on_scroll(Viewport)` with a fixed line height and JetBrains Mono ([0010](0010-fonts-text-measurement-and-palette.md)). Render only the visible lines plus overscan, between a top and a bottom spacer.
- Render each line as a `rich_text` of syntect-coloured `span`s, with a line-number gutter.
- Draw a background band over the cited range (`line..=end_line`). The band stays visible regardless of focus.
- Jump to a line with `iced::widget::operation::scroll_to`.
- Do not wrap. Long lines scroll horizontally.
- Mouse text selection is not available. Provide "Copy path:line" and "Copy cited lines" buttons instead.
- Highlight with syntect 5.3 using the pure-Rust regex backend (fancy-regex), the default syntaxes and themes, and without the `html` feature. Fall back to plain text when no syntax applies.
- Limit files to 2 MB. Larger files and binaries (a NUL byte in the first 8 KB) show a placeholder.

## Alternatives considered

### iced `text_editor` with edit actions filtered out

Selection, copying and visible-line shaping come for free. Rejected because the highlighter `Format` has no background, the selection is drawn only while the editor is focused, and without `on_action` the editor is fully disabled. The cited range can't stay highlighted.

### tree-sitter highlighting

Rejected: it needs a grammar crate per language, which is heavier.

### iced's `highlighter` feature

It uses syntect underneath but is tied to `text_editor`, so it inherits the problems above.

## Consequences

- We own the virtualization code. It is shared with the file tree.
- No mouse text selection; copying goes through the buttons.
- Syntax sets load once at startup [inference: off the UI thread].
- Files over 2 MB and binary files are not displayed.

## References

- iced 0.14.0: https://github.com/iced-rs/iced/tree/0.14.0, https://docs.rs/iced/0.14.0
- [../spec.md](../spec.md) (FR-SRC), [../libraries.md](../libraries.md) (syntect), [../architecture.md](../architecture.md)
