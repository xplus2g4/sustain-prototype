# 0005. iced as the native GUI toolkit

- Status: Accepted
- Date: 2026-10-10
- Related: [0006](0006-three-pane-grid-layout.md), [0007](0007-toolbar-instead-of-menus.md), [0008](0008-async-model-tokio-and-iced-tasks.md), [0009](0009-custom-virtualized-source-viewer.md), [0010](0010-fonts-text-measurement-and-palette.md), [0029](0029-testing-strategy-and-live-smoke-runs.md); spec: FR-PANE, FR-DIA, FR-SRC

## Context

The owner requires native presentation only, with no HTML, webview or browser rendering. The app is written in Rust and needs movable panels, a canvas-drawn diagram and a source viewer.

## Decision

- Use iced 0.14.0 (latest stable; released 2025-12-07; MSRV 1.88).
- Enable the `canvas`, `tokio` and `advanced` features.
- Structure the app with the Elm architecture: state, `update`, `view`.

## Alternatives considered

### Tauri, wry, Dioxus desktop

They render through a webview, which violates the native-only goal.

### egui/eframe with egui_dock

Immediate mode, and egui_dock has tab stacking and docking built in. The owner preferred iced.

### gpui (Zed)

Fast, but its API is unstable and its docs are thin.

### Slint

Declarative and polished, but has no docking.

## Consequences

Facts verified in the iced 0.14.0 source:

- `pane_grid` provides splits, drag-to-edge, resize, maximize and close, but no tab stacking. See [0006](0006-three-pane-grid-layout.md).
- There is no native menu bar API. See [0007](0007-toolbar-instead-of-menus.md).
- There are no pinch or magnify gesture events. `core::Event` has only Keyboard/Mouse/Window/Touch/InputMethod, and `winit/src/conversion.rs` has no PinchGesture mapping. Zoom therefore uses ⌘/Ctrl+scroll, with `ScrollDelta::{Lines,Pixels}` and modifiers tracked from `keyboard::Event::ModifiersChanged`.
- The `text_editor` highlighter's `Format` has only `color` and `font`, and the selection is drawn only while the editor is focused. Hence the custom source viewer in [0009](0009-custom-virtualized-source-viewer.md).
- There is no virtualized list widget, so virtualization is custom: `scrollable` + `on_scroll(Viewport)` + spacers.
- Headless UI testing uses the `iced_test` 0.14 `Simulator`. See [0029](0029-testing-strategy-and-live-smoke-runs.md).
- Text measurement uses `iced::advanced::graphics::text::Paragraph`. See [0010](0010-fonts-text-measurement-and-palette.md).

## References

- https://github.com/iced-rs/iced/tree/0.14.0
- https://docs.rs/iced/0.14.0
- [`../libraries.md`](../libraries.md)
