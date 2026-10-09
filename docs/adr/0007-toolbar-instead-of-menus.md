# 0007. Toolbar instead of menus

- Status: Accepted
- Date: 2026-10-10
- Related: [0005](0005-iced-native-gui-toolkit.md), [0006](0006-three-pane-grid-layout.md); spec: FR-PANE

## Context

The app needs entry points for opening a repo, starting a run, history, panel visibility, theme and settings. iced 0.14 has no native or app-menu API; the only menu-related action is `window::Action::ShowSystemMenu`.

## Decision

- Build an in-window toolbar from plain iced widgets with these items:
  - Open repo
  - Archify, with a Focus text field; it becomes Cancel during a run
  - History ▾
  - Panels ▾
  - Theme
  - Settings
- Every action has a keyboard shortcut. The shortcut table is in the spec under FR-PANE.
- No native menu bar, no `iced_aw`, no `muda`.

## Alternatives considered

### `iced_aw` `MenuBar`

`iced_aw` 0.14.1 (2026-04-27) offers an in-window `MenuBar` behind its `menu` feature. Rejected because it costs an extra dependency.

### `muda` native menus

`muda` needs `menu.init_for_nsapp()` and an `EventLoopProxy`, which iced does not expose. There is no documented iced integration.

## Consequences

- The dropdowns (History, Panels) are built from basic widgets and overlays.
- The macOS app menu is limited to whatever winit provides by default [inference].
- Keyboard shortcuts carry the discoverability that a menu bar would otherwise provide.

## References

- https://docs.rs/iced/0.14.0
- [`../spec.md`](../spec.md) (FR-PANE)
- [`../libraries.md`](../libraries.md)
