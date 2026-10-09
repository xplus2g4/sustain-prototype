# 0006. Three-pane `pane_grid` layout

- Status: Accepted
- Date: 2026-10-10
- Related: [0005](0005-iced-native-gui-toolkit.md), [0007](0007-toolbar-instead-of-menus.md), [0015](0015-diagram-storage-app-state-and-run-log.md); spec: FR-PANE

## Context

The owner requires three panels (tree, diagram, source) that can be moved and stacked horizontally or vertically. iced 0.14 `pane_grid` provides splits, drag-to-edge, resize, maximize and close, but no tab stacking ([0005](0005-iced-native-gui-toolkit.md)).

## Decision

- Exactly three panes, each present at most once: Tree, Diagram, Source.
- The default layout is Tree | Diagram | Source, left to right.
- **Moving.** Drag a pane by its title bar:
  - onto another pane's edge region (its outer thirds) to split on that side (`State::drop(pane, target)` -> `split_with` / `move_to_edge`);
  - onto the center of another pane to swap the two.
- **Resizing.** Dividers resize panes (`on_resize`, `min_size`).
- **Maximize and close.**
  - Maximize/restore uses `State::maximize` / `restore`.
  - Close uses `State::close`. The toolbar's Panels menu reopens a closed pane ([0007](0007-toolbar-instead-of-menus.md)).
  - At least one pane always stays open.
  - "Reset layout" restores the default.
- **Persistence.** `pane_grid::Configuration` and `Node` derive only `Debug, Clone`, so write our own serde tree that mirrors `Configuration`.
  - Save by walking `State::layout()`.
  - Restore by rebuilding with `State::with_configuration`.
  - Store the tree with the window size in the app state ([0015](0015-diagram-storage-app-state-and-run-log.md)).

## Alternatives considered

### Tab stacking in the style of egui_dock

Needs custom docking code in iced. The owner accepted `pane_grid` behavior instead.

### A fixed three-column layout

Violates the goal that panels can be moved and stacked.

## Consequences

- No tabs: panes can be split, swapped, resized, maximized and closed, but not stacked as tabs.
- Dragging is ignored while a pane is maximized (pane_grid.rs L248-256).
- Layout persistence is our own code and must be kept in sync with `pane_grid::Configuration`.

## References

- https://docs.rs/iced/0.14.0
- [`../spec.md`](../spec.md) (FR-PANE)
