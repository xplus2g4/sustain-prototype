# 0032. UI test harness built on `iced_test` parts

- Status: Accepted
- Date: 2026-10-10
- Related: [0005](0005-iced-native-gui-toolkit.md), [0008](0008-async-model-tokio-and-iced-tasks.md), [0009](0009-custom-virtualized-source-viewer.md), [0029](0029-testing-strategy-and-live-smoke-runs.md); spec: §10, every `[sim]` criterion

## Context

[0029](0029-testing-strategy-and-live-smoke-runs.md) says the desktop is tested with `iced_test` 0.14 `Simulator` smoke tests. Writing the M3 tests showed three gaps in a bare `Simulator`:

- A `Simulator` is built from one `view()`. Rebuilding it after `update` starts from an empty widget cache, so scroll offsets and text-input focus are lost. The source viewer and the tree are virtualized (0009) and depend on real scroll offsets: after a jump, a fresh `Simulator` shows the top spacer, and the cited lines are off screen.
- A `Simulator` doesn't run the app's `Task`s. File loads, listings and diagram loads all happen in tasks (0008). `scroll_to`, `is_focused` and clipboard writes are tasks as well.
- `Snapshot` screenshots come from the same cache-less interface, so they show the wrong scroll position.

`iced_test::Emulator` runs tasks, but only through its event-channel API, and it ignores clipboard actions.

Evidence (2026-10-10): a `scroll_to` sent right after a file opened left the scrollable at offset 0 when the interface was rebuilt without the previous cache. A test that opened `src/api.rs` at line 50 found `src-line:50` with `visible_bounds: None`.

## Decision

The desktop's `[sim]` tests use a small harness in `crates/archalign-desktop/tests/common/`, built only from public `iced_test` parts:

- the `iced_test` headless renderer (`Renderer::new` through `Headless`);
- `iced_test::runtime::UserInterface`, with its `Cache` persisted between rebuilds. Before running a widget operation, the harness rebuilds the interface for the latest state, as the runtime does;
- `iced_test::selector`: find by widget ID, by text, `find_all` by ID prefix, and `Point`;
- the `iced_test::simulator::{click, typewrite}` event builders, plus keyboard events with modifiers;
- `iced_test::runtime::task::into_stream`, which runs the real `Task`s on a tokio current-thread runtime:
  - outputs are fed back into `update`;
  - widget operations run against the live interface;
  - clipboard writes are recorded;
- key presses the interface ignores are delivered to `update` the way the `keyboard::listen` subscription delivers them;
- screenshots are drawn from the live interface and written as PNG files.

Tests click canvas content by `Point` and find widgets by ID, as 0029 requires.

## Alternatives considered

### A bare `Simulator` per step

Rejected for the reasons in Context: no scroll state, no tasks.

### `iced_test::Emulator`

It runs tasks and keeps its cache. Rejected because driving it means an event loop over an `mpsc` channel with `Instruction`s, and it doesn't record clipboard writes, which FR-SRC-07 needs.

### `.ice` instruction files

Rejected: they can't assert on application state, and the `[sim]` criteria do.

## Consequences

- Positive: tests run the production `update`, `view` and task code end to end. That includes scroll jumps, focus-gated shortcuts and clipboard writes.
- Positive: screenshots match what the app shows, so they are usable for visual review (§10).
- Negative: about 300 lines of test-support code depend on `iced_test` internals (`UserInterface`, `into_stream`). An iced upgrade may require changes here.
- `png` 0.18 becomes a dev-dependency (`libraries.md`).

## References

- `crates/archalign-desktop/tests/common/mod.rs`
- [../libraries.md](../libraries.md) (`iced_test`, `png`)
- [../spec.md](../spec.md) §10
