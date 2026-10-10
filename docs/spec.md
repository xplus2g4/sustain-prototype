# ArchAlign specification

- Status: Accepted, 2026-10-10
- Decisions: [docs/adr/](adr/README.md) · Structure: [architecture.md](architecture.md) · Dependencies: [libraries.md](libraries.md) · Progress: [progress.md](progress.md)

This document is the contract for the implementation. Requirements have stable IDs, and each one carries acceptance criteria (AC) plus how it is verified. Changes follow [§13 Implementer rules](#13-implementer-rules).

Verification tags:

| Tag | How it's verified |
|---|---|
| `[unit]` | Rust unit or integration test |
| `[snap]` | `insta` snapshot |
| `[prop]` | `proptest` property |
| `[sim]` | `iced_test::Simulator` test |
| `[perf]` | Release-mode timing test |
| `[live]` | Opt-in live LLM test |
| `[manual]` | Checked by the owner by hand |

## 1. Purpose

ArchAlign is a native macOS desktop application written in Rust. It runs a built-in LLM agent over a local repository to produce an Archify-format architecture diagram. The diagram is then shown next to the repository's file tree and source code. Clicking a node opens the code it cites.

ArchAlign is an unofficial port of [Archify](https://github.com/tt-a1i/archify) (MIT) and is not affiliated with it. Diagrams are stored in Archify's JSON format, so the Archify CLI can render ArchAlign's output, and ArchAlign can open Archify's output.

## 2. Goals

| ID | Goal |
|---|---|
| G1 | The user imports a local repository and archifies it, i.e. generates an architecture diagram from its code. |
| G2 | Everything is presented in a native app. No HTML, webview or browser rendering anywhere. |
| G3 | Three panels (project tree, diagram, source code) that can be moved and split horizontally or vertically. |
| G4 | Clicking a diagram node opens the cited file at the cited lines. |
| G5 | Written in Rust. |
| G6 | Every decision is justified in `docs/adr/`. |
| G7 | The architecture is documented in `docs/architecture.md`. |
| G8 | Every added crate and bundled asset is justified in `docs/libraries.md`. |

## 3. Non-goals (v1)

- **Other diagram types.** Workflow, sequence, dataflow and lifecycle diagrams are listed as unsupported ([0013](adr/0013-architecture-diagrams-only-in-v1.md)).
- **Editing.** No editing of source files, and no manual editing of diagrams beyond Re-layout.
- **Chat-style refinement** ("add Redis", "move auth left").
- **Other Archify viewer features** ([0011](adr/0011-v1-viewer-feature-scope.md)):
  - exports (PNG/JPEG/WebP/SVG/WebM) and share cards;
  - presentation mode and trace animation;
  - radar/minimap, semantic lens, route probe;
  - guided views (`meta.views`) and reading-depth levels.
- **Display extras.** Node icons and brand marks: parsed and preserved, not drawn. Translations and i18n: English only.
- **Remote repos.** No cloning of remote repositories.
- **Other platforms and CI.** No Linux or Windows support, and no CI ([0002](adr/0002-macos-only-without-ci.md)).
- **Window chrome.** No native menu bar, no tab stacking, no duplicate panels, no multiple windows or repos at once.
- **Network.** No telemetry, no update checks, no network traffic other than the LLM API.
- **Secrets.** No OS keychain storage for API keys.
- **Exact round-trips.** No byte-identical JSON round-trips; round-trips are equal as JSON values.
- **Archify parity.** No parity with Archify's HTML viewer, edge router, or quality gates (browser check, visual review).

## 4. Glossary

| Term | Meaning |
|---|---|
| Repo root | The folder the user imported, canonicalized. Every path in the app is relative to it. |
| Candidate | An Archify architecture JSON document (`schema_version: 1`, `diagram_type: "architecture"`). |
| Component / node | An entry in `components[]`, drawn as a box. |
| Connection / edge | An entry in `connections[]`: a directed relationship `from` → `to`. |
| Boundary | An entry in `boundaries[]` that wraps components. `kind` is `region` or `security-group`. |
| Source reference / citation | An entry in `components[].sources[]` (`path`, optional `line`, `end_line`, `label`). A component has 1–3. |
| Broken citation | A citation whose file is missing, unreadable, binary, or whose lines are out of range. |
| Diagram folder | `<repo>/.archify/architecture-<slug>-<YYYYMMDD-HHMMSS>/` |
| Run | One execution of the agent, ending in `success`, `failed` or `cancelled`. |
| Authored layout | Every component has a position: `pos`, or a grid cell when `layout.mode` is `"grid"`. |
| Auto layout | Positions computed by ArchAlign ([0018](adr/0018-app-computed-layout-dugong-and-orthogonal-routing.md)). |
| Submission / Import profile | The two validation strictness levels ([0027](adr/0027-submission-gate-and-repair-loop.md)). |

## 5. Platform and constraints

- **Platform.** macOS on Apple silicon, verified on macOS 26 (Darwin 25.6, arm64). Nothing else is claimed.
- **Toolchain.** Rust stable, pinned by `rust-toolchain.toml`. Edition 2024. MSRV 1.95, the highest `rust-version` among the dependencies (`dugong` 0.8.0). Developed with rustc 1.97.1.
- **GUI.** iced 0.14 ([0005](adr/0005-iced-native-gui-toolkit.md)).
- **Workspace.** Five crates ([0003](adr/0003-cargo-workspace-and-crate-boundaries.md)): `archalign-model`, `archalign-layout`, `archalign-repo`, `archalign-agent`, and `archalign-desktop` (binary `archalign`).
- **Dependencies.** Only those listed in [libraries.md](libraries.md).
- **Distribution.** The app is run with `cargo run -p archalign-desktop --release` or the built binary. There is no `.app` bundle or code signing in v1.

## 6. Primary flows

1. **Open.** Welcome screen → Open folder → the tree and source panels are usable immediately → the newest valid diagram in `.archify/` opens, if there is one.
2. **Archify.** Type an optional Focus → press Archify.
   - The first time for a repo, a consent modal appears.
   - During the run, the status bar shows progress and the run log streams.
   - On success, the diagram opens.
3. **Navigate.** Click a node → the source panel opens its first citation with the range highlighted, and the tree reveals the file.
4. **Reverse.** Select a file in the tree → the nodes that cite it are highlighted.
5. **Arrange.** Drag panels to split them, resize, maximize, close, reopen, or reset the layout.
6. **History.** Switch between diagrams stored in `.archify/`.
7. **Live.** Edit files outside the app → the tree, open file, citations and History update.

## 7. Functional requirements

### 7.1 FR-PANE: panels, toolbar, shell

ADRs: [0006](adr/0006-three-pane-grid-layout.md), [0007](adr/0007-toolbar-instead-of-menus.md), [0005](adr/0005-iced-native-gui-toolkit.md).

**FR-PANE-01 Default layout.** The main area is an iced `pane_grid` with exactly three panes: Tree, Diagram and Source.
- AC: With no saved state, the panes appear left to right as Tree | Diagram | Source, with width ratios 0.20 / 0.50 / 0.30. `[sim]`

**FR-PANE-02 Pane chrome.** Each pane has a title bar with the pane name, a context label, and Maximize/Restore and Close buttons. The context label is the diagram title for Diagram and the repo-relative path for Source.
- AC: The title bars render with the correct labels. The buttons emit the expected messages. `[sim]`

**FR-PANE-03 Move and split.** Panes are moved by dragging their title bar.
- AC: Dropping a pane:
  - on another pane's edge region splits that pane on that side;
  - on another pane's center swaps the two;
  - on the grid's outer edge moves the pane to that edge.

  The layout after each case matches `pane_grid::State::drop` semantics. `[unit]`

**FR-PANE-04 Resize.** Dividers can be dragged.
- AC: Ratios update while dragging. No pane goes below 160 px in either dimension. `[unit]`

**FR-PANE-05 Maximize.**
- AC: A maximized pane fills the grid, and Restore returns the previous arrangement. Dragging is ignored while a pane is maximized. `[sim]`

**FR-PANE-06 Close and reopen.**
- AC: Closing removes the pane. The Panels menu lists Tree, Diagram and Source with a visibility check.
- AC: Choosing a hidden pane reopens it by splitting the largest visible pane, with Tree inserted on the left and Diagram or Source inserted on the right.
- AC: The last visible pane can't be closed; its Close button is disabled. Each pane exists at most once. `[sim]`

**FR-PANE-07 Reset layout.**
- AC: Reset restores the FR-PANE-01 arrangement and ratios and clears any maximized state. `[sim]`

**FR-PANE-08 Layout persistence.** The arrangement, ratios, visibility and window size are saved in the app state ([FR-STO-05](#77-fr-sto-storage-history-state-settings)), at most once per second, and restored at launch.
- AC: A save→restore round-trip gives an identical layout.
- AC: An unreadable or invalid saved layout falls back to the default and logs a warning. `[unit]`

**FR-PANE-09 Toolbar.** The toolbar holds Open repo · Focus field · Archify (Cancel during a run) · History ▾ · Panels ▾ · Theme · Settings. Every action has the shortcut listed in [§9](#9-keyboard-shortcuts).
- AC: Archify is disabled when no repo is open. Open repo is disabled during a run. Each shortcut triggers the same message as its button. `[sim]`

**FR-PANE-10 Status bar.** The status bar shows:
- the repo root path;
- the git short HEAD and dirty-file count (when the repo uses git);
- run progress (turn n/max, input tokens used, elapsed time), or "Idle";
- the watcher state: "Watching" or "Manual refresh only".
- AC: Each item updates within one UI frame of its state changing. `[sim]`

**FR-PANE-11 Errors.** Errors appear as dismissible banners in the affected pane, plus a status-bar message. The only modal windows are the consent prompt, Settings and the native folder picker.
- AC: Malformed JSON, unreadable files, invalid UTF-8, a missing `git` and a missing API key all produce banners and never panic. `[unit]` `[sim]`

### 7.2 FR-IMP: import

ADR: [0016](adr/0016-local-folder-import-and-git-metadata-via-cli.md).

**FR-IMP-01 Welcome screen.** Shown when no repo is open. It offers Open folder and up to 10 recent repos, most recent first.
- AC: A recent repo whose folder is missing is shown disabled, marked "Missing", with a remove control. `[sim]`

**FR-IMP-02 Open folder.** Opening uses the native folder picker (`rfd`). The chosen folder is canonicalized and becomes the repo root, replacing the current repo, and is added to the recent repos.
- AC: The open-repo message, given a path, sets the root, resets the tree, source and diagram state, and updates the recents. `[unit]`
- AC: The picker itself is checked by hand. `[manual]`

**FR-IMP-03 Initial state.** When a repo opens:
- the tree lists the top level ([NFR-PERF-01](#81-nfr-perf));
- the Source panel shows an empty state;
- the Diagram panel opens the newest valid architecture candidate in `.archify/`, or shows an empty state with the Focus field and the Archify button.
- AC: With a fixture repo containing two diagram folders, the newer valid one opens. With none, the empty state shows. `[sim]`

**FR-IMP-04 Git metadata.** When `git` is on PATH and the root is the top level of a work tree, collect:
- HEAD (40 hex characters);
- the origin URL with HTTP(S) userinfo removed (transport, port, path and `.git` suffix kept);
- dirty paths, from `git --no-optional-locks status --porcelain=v1 -z`.

A root that is inside a work tree but not at its top level is treated as non-git, with an info banner.
- AC: Temporary repos cover: clean, dirty, no origin, an origin with credentials, a subfolder of a work tree, and git missing from PATH. `[unit]`

**FR-IMP-05 Reopen last repo.**
- AC: At launch, the last repo reopens if it still exists. Otherwise the welcome screen shows. `[unit]`

**FR-IMP-06 One repo per window.**
- AC: Opening another repo while a run is active isn't possible; the control is disabled. `[sim]`

### 7.3 FR-TREE: project tree

ADRs: [0009](adr/0009-custom-virtualized-source-viewer.md) (shared virtualization), [0025](adr/0025-agent-tool-sandbox-and-deny-list.md) (confinement).

**FR-TREE-01 Lazy listing.** A directory's children are read the first time it is expanded.
- AC: Folders are listed before files, each group sorted case-insensitively. Listing happens off the UI thread. `[unit]`

**FR-TREE-02 Filtering.**
- AC: `.gitignore` rules are applied, including nested `.gitignore` files, `.git/info/exclude` and the global gitignore.
- AC: `.git/` is never shown.
- AC: Dotfiles are hidden unless "Show hidden files" is enabled in Settings. `[unit]`

**FR-TREE-03 Virtualization.**
- AC: The number of row widgets built is at most the number of visible rows plus 2 × overscan (20), whatever the folder size. `[unit]`

**FR-TREE-04 Activation.**
- AC: Clicking a file opens it in Source (FR-SRC-01) and selects its row. Clicking a folder expands or collapses it. `[sim]`

**FR-TREE-05 Reveal.**
- AC: When a node opens a file, the file's ancestor folders expand, and its row is selected and scrolled into view. `[sim]`

**FR-TREE-06 Citation markers.**
- AC: Files cited by the open diagram show a dot. Folders show how many distinct cited files they contain. Files with a broken line range show a warning marker. `[unit]` (index) `[sim]` (render)

**FR-TREE-07 Reverse highlight.**
- AC: Selecting a file highlights every node that cites it with an accent outline. Selecting another file, or pressing Esc in the diagram, clears it. `[sim]`

**FR-TREE-08 Symlinks.**
- AC: Symlinks show a link marker. A symlink that resolves outside the root can't be expanded or opened, and a tooltip explains why. `[unit]`

### 7.4 FR-SRC: source viewer

ADR: [0009](adr/0009-custom-virtualized-source-viewer.md).

**FR-SRC-01 Read-only viewer.** It opens one repo-relative file at a time. The header shows the path and line count, plus "Copy path:line" and "Copy cited lines" buttons.
- AC: No editing is possible. `[sim]`

**FR-SRC-02 Rendering.** JetBrains Mono at a fixed line height, with a line-number gutter.
- AC: syntect highlighting picks the syntax by file extension, then by the first line. Text without a matching syntax is plain. No wrapping; long lines scroll horizontally. `[unit]` (syntax selection) `[sim]`

**FR-SRC-03 Virtualization.**
- AC: Only the visible lines plus 20 lines of overscan are built. `[unit]`

**FR-SRC-04 Cited range.**
- AC: Opening a citation scrolls so `line` lands in the top third of the viewport, and paints a background band over `line..=end_line` (just `line` when `end_line` is absent).
- AC: The band stays until another file or citation is opened.
- AC: A citation without `line` opens at line 1 with no band. `[sim]`

**FR-SRC-05 Out-of-range citation.**
- AC: If the cited lines go beyond the file's length, the file opens at line 1 with no band, and a banner reads "Cited lines L–E are outside this file (N lines)". `[sim]`

**FR-SRC-06 Limits.**
- AC: Files larger than 2 MB, and binary files (a NUL byte in the first 8 KB), show a placeholder with the file size and "Copy path".
- AC: Invalid UTF-8 is decoded lossily, with a banner. `[unit]`

**FR-SRC-07 Copy.**
- AC: "Copy path:line" puts `path:line` (or just `path` when there's no line) on the clipboard.
- AC: "Copy cited lines" copies the text of the banded lines, and is disabled when there's no band. `[sim]`

**FR-SRC-08 Confinement.**
- AC: All reads go through the repo `Sandbox` (FR-SEC-01). `[unit]`

### 7.5 FR-DIA: diagram view

ADRs: [0011](adr/0011-v1-viewer-feature-scope.md), [0010](adr/0010-fonts-text-measurement-and-palette.md), [0019](adr/0019-geometry-write-back-and-authored-layouts.md).

**FR-DIA-01 Rendering.** The diagram is drawn on an iced `canvas`.
- **Components:** rounded rectangles filled with their type's color (7 types), showing `label`, `sublabel` and `tag`.
- **Boundaries:** labeled rectangles, with distinct styles for `region` and `security-group`.
- **Connections:** orthogonal polylines with an arrowhead at the target, styled by `variant` (`default`, `emphasis`, `security`, `dashed`), with their `label` drawn.
- **Legend** (driven by `meta.legend`):
  - `auto` shows the types present;
  - `all` shows all 7;
  - `hidden` shows none;
  - `entries.<type>.label` overrides a type's text, and `visible: false` hides it.
- **Cards:** in a collapsible strip.
- AC: Legend selection logic. `[unit]`
- AC: Rendering is checked from Simulator screenshots of the fixtures. `[manual]`

**FR-DIA-02 Camera.**
- Opening a diagram fits it to the pane, with 40 px padding.
- Zoom:
  - ⌘/Ctrl+scroll zooms around the cursor;
  - `+`/`-` zoom by 1.25× around the center;
  - `0` sets 100%; `F` fits;
  - zoom is clamped to 10%–400%.
- Pan: scroll or two-finger swipe, or drag on empty canvas.
- Key shortcuts apply only when the canvas has focus and no text field does.
- AC: The camera math (zoom about a point, clamping, fit) is correct. `[unit]` Input handling works end to end. `[sim]`

**FR-DIA-03 Node click.** Clicking a node:
- selects it;
- opens `sources[0]` (FR-SRC-04);
- reveals the file (FR-TREE-05);
- shows the detail strip: label, sublabel, tag, and every source as a clickable `path:line–end_line` entry.
- AC: A node without sources is only selected, and the strip says "No source". `[sim]`

**FR-DIA-04 Hover.**
- AC: After 400 ms over a node, a tooltip shows its label, sublabel, tag and sources. `[sim]`

**FR-DIA-05 Reach.** With a node selected, `U` (Upstream) or `D` (Downstream), or the matching strip buttons, highlight every node and connection transitively reachable along authored directed connections. Everything else is dimmed.
- AC: Pressing the same key again clears the highlight, and the other key replaces it.
- AC: Traversal handles cycles. `[unit]` (traversal) `[sim]`

**FR-DIA-06 Edge click.**
- AC: Clicking a connection selects it, highlights both endpoints, and shows `from → to`, the label and the variant in the strip. `[sim]`

**FR-DIA-07 Finder.** ⌘F or `/` opens a search field that matches labels and IDs (case-insensitive substring) and lists up to 10 results.
- AC: ↑/↓ move through the results. Enter centers the chosen node and acts like a click (FR-DIA-03). `[sim]`

**FR-DIA-08 Esc.**
- AC: Esc closes the finder if it's open. Otherwise it clears the reach highlight, the selection and the reverse highlight. `[sim]`

**FR-DIA-09 Problems.** The detail strip lists the diagram's warnings:
- partial positions ignored;
- missing sources;
- partial boundary overlap;
- broken citations;
- cited files with uncommitted changes.
- AC: A node with a broken citation shows a warning badge. A node citing an uncommitted file shows a "modified" badge. `[unit]` `[sim]`

**FR-DIA-10 Theme.** Dark by default. Toggling switches both the app chrome and the diagram palette between Archify's dark and light colors, and the choice is saved.
- AC: The toggle works and survives a restart. `[sim]`

**FR-DIA-11 Re-layout.** Available from the Diagram title bar and with ⌘⇧L. It strips the geometry (per [0019](adr/0019-geometry-write-back-and-authored-layouts.md)), recomputes the layout (FR-LAY), saves `candidate.json` atomically, and redraws.
- AC: After Re-layout, the file contains integer `pos`/`size` for every component, and all non-geometry fields are unchanged. `[unit]`

**FR-DIA-12 Unsupported diagrams.**
- AC: Opening a non-architecture Archify file shows "Unsupported diagram type: `<type>`" and nothing crashes. `[unit]`

### 7.6 FR-LAY: layout and routing

ADRs: [0018](adr/0018-app-computed-layout-dugong-and-orthogonal-routing.md), [0019](adr/0019-geometry-write-back-and-authored-layouts.md), [0010](adr/0010-fonts-text-measurement-and-palette.md).

**FR-LAY-01 Mode selection.** A diagram is authored if every component has `pos`, or if `layout.mode: "grid"` and every component without `pos` has `row`/`col`. Otherwise every component is auto-laid-out, and if only some had positions, a warning is added.
- AC: Cases covered: all positioned, all on the grid, a mix of grid and `pos`, partially positioned, none positioned. `[unit]`

**FR-LAY-02 Grid conversion.** Use Archify's formula: `pos = origin + (col·(cellW+gapX), row·(cellH+gapY))`, with defaults origin [40, 80], cols 4, gapX 30, gapY 40, cellW 130, cellH 64. `pos` takes precedence over `row`/`col`.
- AC: `archify-repo-grid.architecture.json` resolves to the expected positions. `[unit]`

**FR-LAY-03 Sizes.**
- **Authored layouts:** use `size`, or 120×60 if it's missing.
- **Auto layout:** measure the text through `TextMeasure` (label 14 px, sublabel 12 px, tag 11 px, JetBrains Mono), add 12 px horizontal and 10 px vertical padding, with a minimum of 120×60.
- AC: With the fixed-advance measurer, the sizes are exact. `[unit]`

**FR-LAY-04 Auto layout.** Use `dugong` as a compound graph, ranked left to right or top to bottom, whichever fits a 16:10 viewport at the larger scale (ties: left to right; [0035](adr/0035-auto-rank-direction.md)). Boundaries become compound parents, and nesting is allowed.
- AC: Every vendored fixture produces a geometry snapshot. `[snap]`

**FR-LAY-05 Routing.** Every connection is drawn as axis-aligned segments:
- endpoints sit on node sides, and authored `fromSide`/`toSide` are honored;
- in authored layouts, `via` points are honored;
- segments avoid nodes other than the endpoints;
- parallel connections between the same pair are offset by at least 8 px;
- a self-loop is a small rectangular loop on the node's right side;
- the arrowhead is at the target;
- the label is centered on the longest segment.
- AC: Each rule above is covered. `[unit]`

**FR-LAY-06 Boundaries.** A boundary's rectangle encloses its members with padding: `pad` on every side when given, otherwise Archify's rule of 30 px top/left/right and 50 px bottom. The label sits at the top-left. A nested boundary sits inside its parent's rectangle.
- AC: Both padding cases and nesting are covered. `[unit]`

**FR-LAY-07 Invariants for auto layouts.**
- No two node rectangles overlap.
- Every boundary contains its members and nested boundaries.
- Every segment is axis-aligned.
- No segment crosses a node that isn't one of its endpoints.
- The same input always gives the same output.
- AC: All of the above hold over random graphs of up to 60 nodes, 120 edges and 2-level boundaries. `[prop]`

**FR-LAY-08 Write-back.** After auto layout in a run, or after Re-layout, every component gets integer `pos`/`size`. Opening a diagram never writes to disk.
- AC: Opening a diagram without positions leaves the file's bytes unchanged. `[unit]`

### 7.7 FR-STO: storage, history, state, settings

ADR: [0015](adr/0015-diagram-storage-app-state-and-run-log.md).

**FR-STO-01 Diagram folder.** Each run creates `<repo>/.archify/architecture-<slug>-<YYYYMMDD-HHMMSS>/`.
- The timestamp is local time at run start (`jiff`).
- The slug is built from the first words of the Focus text, or from the repo folder name when Focus is empty:
  - converted to lowercase ASCII kebab-case (`[a-z0-9-]`) and capped at 40 characters;
  - if the result is empty, it falls back to the repo folder name, and then to `repo`.
- If the folder name is already taken, `-2`, `-3`, … is appended.
- AC: Slug generation and collision handling are correct. `[unit]`

**FR-STO-02 Folder contents.**
- `candidate.json` is written only on success. `meta.output` is `.archify/<folder>/<slug>.html`; that HTML file is never written.
- `run.jsonl` is always written, in the format of [Appendix A](#appendix-a-runjsonl-events).
- All writes are atomic: write a temporary file in the same folder, then rename it.
- AC: Interrupted writes never leave a partial `candidate.json` behind. `[unit]`

**FR-STO-03 Open-file compatibility.**
- AC: Every `candidate.json` ArchAlign writes passes the vendored Archify schema, and every component has `pos`/`size`. `[unit]`
- AC: The final smoke test feeds one generated file to Archify's CLI (`node archify-src/archify/bin/archify.mjs validate architecture <file>`) and records the result. `[live]`

**FR-STO-04 History.** History scans `.archify/*/`, whatever `.gitignore` says. It lists every folder that contains a `candidate.json`, newest first, showing the title, the timestamp and a status (valid, invalid, or unsupported type). Selecting an entry opens it. Folders without a `candidate.json` are hidden.
- AC: Ordering, status detection and hiding are correct. `[unit]` `[sim]`

**FR-STO-05 App state and settings.** Both live in the OS config dir (`directories`, app name `ArchAlign`):
- `state.json`: window size, pane layout, theme, last repo, up to 10 recent repos, and consents;
- `settings.json`: model, `reasoning_effort`, budgets, and "Show hidden files".

Writes are atomic. A missing, unreadable or invalid file falls back to defaults and shows a banner.
- AC: Round-trips and the fallback behavior work. `[unit]`

**FR-STO-06 Settings modal.** Lets the user edit `settings.json` values.
- It shows where the API key came from ("environment", "`.env` at `<path>`", or "not found") and never the key itself.
- It lists consents, each with a Revoke button.
- AC: Edits persist, and the key is never displayed. `[sim]`

### 7.8 FR-VAL: model and validation

ADRs: [0012](adr/0012-archify-json-v1-canonical-format.md), [0014](adr/0014-schema-validation-with-vendored-archify-schemas.md), [0027](adr/0027-submission-gate-and-repair-loop.md).

**FR-VAL-01 Load pipeline.** Bytes → JSON → JSON Schema validation against the vendored `architecture.schema.json` and `common.schema.json` (offline) → strict serde types.
- AC: Each error carries a JSON pointer and a message, and the UI shows the first 20. `[unit]`

**FR-VAL-02 Model coverage.** The serde types represent every property in the architecture and common schemas. Unknown fields are rejected.
- AC: Every vendored architecture fixture parses. A fixture with an extra field fails, and the error names that field. `[unit]`

**FR-VAL-03 Lossless save.**
- AC: For every fixture, load→save produces a `serde_json::Value` equal to the input.
- AC: Output is pretty-printed with 2-space indentation, and whole numbers are written as integers. `[unit]`

**FR-VAL-04 Import profile** (opening files). Schema, parse and reference errors (unknown `from`/`to`/`wraps` IDs, duplicate IDs) are errors. Missing sources, partially overlapping boundaries and broken citations are warnings.
- AC: One fixture per rule. `[unit]`

**FR-VAL-05 Submission profile** (agent output), applied in this order:
1. Set the app-owned fields and drop the model-supplied geometry and presentation fields, exactly as listed in [0027](adr/0027-submission-gate-and-repair-loop.md).
2. Schema validation.
3. Serde parse.
4. Semantic checks:
   - IDs are unique;
   - every reference resolves;
   - every component has 1–3 sources;
   - boundaries are separate or strictly nested.
5. Citation checks (FR-VAL-06).
- AC: One rejection case per rule. Dropped fields are reported. `[unit]`

**FR-VAL-06 Citation checks.** A citation is valid when:
- `path` is relative, uses `/`, has no `..` segment, and resolves inside the root (FR-SEC-01);
- the file exists and is text, at most 2 MB;
- `line ≥ 1`, `end_line ≥ line`, and `end_line ≤` the file's line count.

Under the Submission profile, a citation to a denied path is an error.
- AC: Every rule has a positive and a negative case. `[unit]`

**FR-VAL-07 Dirty flags.**
- AC: A cited path that appears among the git dirty paths is flagged as modified. This is a warning, never an error. `[unit]`

### 7.9 FR-GEN: generation (agent)

ADRs: [0020](adr/0020-built-in-llm-agent-authors-diagrams.md), [0021](adr/0021-litellm-rust-behind-llm-client-trait.md), [0022](adr/0022-default-model-gpt-6-1-sol.md), [0025](adr/0025-agent-tool-sandbox-and-deny-list.md), [0026](adr/0026-system-prompt-derived-from-archify.md), [0027](adr/0027-submission-gate-and-repair-loop.md), [0028](adr/0028-agent-run-budgets.md).

**FR-GEN-01 Preconditions.** A run starts only when all of these hold:
- a repo is open and no run is active;
- an API key can be resolved (FR-SEC-04);
- consent exists for (repo, provider) (FR-SEC-03);
- the diagram folder can be created.
- AC: Each failing precondition shows a banner, and no model call is made. `[unit]`

**FR-GEN-02 Focus.** Optional, at most 500 characters. It goes into the first user message, is used for the slug, and is recorded in `run.jsonl`.
- AC: Focus text appears in the seed message. `[unit]`

**FR-GEN-03 Seed message.** The system prompt is embedded from `crates/archalign-agent/prompts/`. The first user message contains:
- the Focus text;
- the repo folder name and a git summary (short HEAD, dirty count);
- a depth-2 tree of at most 400 entries, with ignore and deny rules applied;
- the start of the README: the first 150 lines of the first match among `README.md`, `README.rst`, `README.txt` and `README`;
- the top-level manifests that exist, at most 120 lines each:
  - Rust: `Cargo.toml`
  - Node: `package.json`
  - Python: `pyproject.toml`, `setup.py`, `requirements.txt`
  - Go: `go.mod`
  - JVM: `pom.xml`, `build.gradle`, `build.gradle.kts`, `settings.gradle`, `settings.gradle.kts`
  - Ruby: `Gemfile`
  - PHP: `composer.json`
  - Containers: `Dockerfile`, `docker-compose.yml`, `docker-compose.yaml`
  - `Makefile`

The whole seed is at most 48 KB.
- AC: Fixture repos produce the expected seed. Denied files never appear in it. `[unit]`

**FR-GEN-04 Tools.** Exposed as OpenAI function tools. All are read-only and confined (FR-SEC-01/02).

| Tool | Parameters | Result |
|---|---|---|
| `list_dir` | `path` (repo-relative; `""` = root) | Entries as `name/` (folder) or `name (N bytes)` (file). At most 500, with a truncation notice. |
| `read_file` | `path`, `start_line?` (≥1, default 1), `end_line?` | `N: text` lines. At most 400 lines and 24 KB per call. When truncated, a notice gives the next `start_line`. Binary files and files over 2 MB are errors. |
| `grep` | `pattern` (Rust `regex`), `path_glob?` | `path:line: text` (text cut to 200 characters). At most 100 matches. Skips binaries and files over 1 MB. Gives up after 5 s and reports the skipped count. |
| `repo_meta` | none | Whether git is available, HEAD, the sanitized origin, and up to 200 dirty paths. |
| `submit_diagram` | `diagram` (object) | `accepted`, or a list of `{pointer, message}` errors. |

- AC: One success case and one error case per tool. Every result is capped at 24 KB. `[unit]`

**FR-GEN-05 Loop.** Each turn makes one `completion` call with the tools attached. Tool calls run in the order they were received, and their results are appended to the conversation.
- AC: A text-only reply gets a reminder to use the tools and `submit_diagram`, and counts as a turn.
- AC: Invalid tool arguments return an error result, which counts toward the turn budget but not the repair budget. `[unit]` (fake `LlmClient`)

**FR-GEN-06 Gate and repair.** An accepted submission is laid out (FR-LAY), saved (FR-STO-02), opened, and the run is logged as `success`.
- AC: Each rejection returns the errors to the model. The 4th rejection ends the run as `failed`. `[unit]`

**FR-GEN-07 Budgets.** Defaults are in [Appendix C](#appendix-c-constants). Exceeding any cap fails the run with a message naming that cap. Only `run.jsonl` is written.
- AC: Each cap (turns, input tokens, wall clock, per-request timeout) has its own test. `[unit]`

**FR-GEN-08 Retries.** HTTP 429, HTTP 5xx and timeouts are retried up to 3 times, after waits of 1 s, 2 s and 4 s, or after `Retry-After` when the response includes it. Retries don't count as turns and are logged as `retry` events.
- AC: The retry wrapper is tested with an injected failing call. `[unit]`

**FR-GEN-09 Progress.** The status bar shows the turn number out of the maximum, input tokens used and elapsed time. A collapsible run-log drawer at the bottom of the Diagram pane streams:
- turn starts;
- tool calls (name and argument summary);
- tool results (byte count or error);
- rejections (error count and the first errors);
- usage and the outcome.
- AC: Events appear in order. `[sim]`

**FR-GEN-10 Cancel.** The Cancel button or ⌘. ends the run.
- AC: The run stops within 1 s, `run.jsonl` ends with outcome `cancelled`, and no candidate is saved. `[unit]` `[sim]`

**FR-GEN-11 Concurrency.**
- AC: During a run, the Tree, Source and History stay usable, and Open repo is disabled. Only one run happens at a time. `[sim]`

**FR-GEN-12 Success.**
- AC: The new diagram opens automatically and appears at the top of History. `[sim]`

**FR-GEN-13 Model settings.**
- The model string defaults to `openai/gpt-6.1-sol` and uses litellm's `provider/model` form.
- `reasoning_effort` defaults to unset. When set, the string is passed through unchanged.
- The budgets can be edited in Settings.
- AC: Changed settings apply to the next run. `[unit]`

### 7.10 FR-WAT: file watching

ADR: [0017](adr/0017-file-watching.md).

**FR-WAT-01 Lifecycle.**
- AC: Opening a repo starts a recursive watch on its root. Closing or switching repos stops it. `[unit]`

**FR-WAT-02 Debounce and filtering.**
- AC: Events are debounced for 300 ms and filtered with the tree's ignore rules.
- AC: `.archify/` is always included.
- AC: Under `.git/`, only `HEAD` and `index` changes are kept. `[unit]`

**FR-WAT-03 Tree.**
- AC: Only the affected folders that are expanded get re-listed. Expansion, selection and scroll position are kept. `[unit]` `[sim]`

**FR-WAT-04 Open file.**
- AC: When the open file changes, it reloads and keeps the scroll position and band.
- AC: If it's deleted, a "File deleted" banner shows. `[sim]`

**FR-WAT-05 Citations.**
- AC: Citations of changed paths are re-checked, and the node badges and tree markers update. `[unit]`

**FR-WAT-06 Diagram files.**
- AC: Changes under `.archify/` refresh History.
- AC: An external change to the open `candidate.json` reloads it. If the new content is invalid, the last good version stays and a banner shows. `[unit]` `[sim]`

**FR-WAT-07 Bursts.**
- AC: A debounced batch of more than 500 paths triggers one full refresh instead of incremental updates. `[unit]`

**FR-WAT-08 Self-writes.**
- AC: Events caused by the app's own writes (matched on path, length and mtime) are ignored. `[unit]`

**FR-WAT-09 Git refresh.**
- AC: Git metadata is refreshed at most once every 2 s after relevant changes, including `.git/HEAD` and `.git/index`. `[unit]`

**FR-WAT-10 Fallback.**
- AC: If the watcher can't start, the status bar shows "Manual refresh only".
- AC: ⌘R always refreshes everything: tree, open file, citations, History and git metadata. `[unit]` `[sim]`

### 7.11 FR-SEC: security and privacy

ADRs: [0023](adr/0023-api-key-resolution.md), [0024](adr/0024-data-egress-consent-and-network-policy.md), [0025](adr/0025-agent-tool-sandbox-and-deny-list.md), [0016](adr/0016-local-folder-import-and-git-metadata-via-cli.md).

**FR-SEC-01 Root confinement.** Every file access goes through `Sandbox`: agent tools, UI reads and citation checks. It rejects absolute paths, `..` escapes, and symlinks that resolve outside the root (canonicalize, then check the prefix).
- AC: There are escape tests for `../`, absolute paths, and a symlink to a file and to a folder outside the root. `[unit]`

**FR-SEC-02 Agent deny-list.** The agent tools never return these paths:
- `.git/` and `.archify/`;
- `.env` and `.env.*`, except `.env.example` and `.env.sample`;
- `*.pem`, `*.key`, `*.p12`, `*.pfx`;
- `id_rsa*`, `id_ed25519*`;
- `.npmrc`, `.pypirc`, `.netrc`.

They're left out of listings and grep results, and a direct read returns `denied`, which is logged. The UI tree and Source panel are not affected.
- AC: Every pattern is checked against every tool. `[unit]`

**FR-SEC-03 Consent.** Before the first run for each (repo root, provider) pair, a modal shows:
- the provider and model;
- what is sent: contents of files the agent reads, paths, grep matches, git metadata (HEAD, credential-free origin, dirty paths) and the Focus text;
- what is never sent: the deny-list.

Accept stores the consent. Decline cancels the run, and viewing stays available. Consents can be revoked in Settings.
- AC: Every branch (accept, decline, revoke) is tested. `[sim]`

**FR-SEC-04 API key.** The key comes from the first of:
1. the provider's environment variable;
2. `.env` in the launch working directory;
3. `.env` in the config dir.

`.env` files are parsed with `dotenvy::from_path_iter`, never with `set_var`. The key is passed through `ProviderConfig::with_api_key`. The imported repo's `.env` is never read. The key is never logged, displayed or written by the app.
- AC: The lookup order and the "not found" case are tested. A test asserts the key never appears in `run.jsonl` or in tracing output captured during a fake run. `[unit]`

**FR-SEC-05 Network.** The only outbound traffic is the provider endpoint, called through litellm-rust or, for `openai/` models, the Responses adapter ([0034](adr/0034-openai-via-responses-api.md)). No telemetry, update checks or crash reporting.
- AC: No other HTTP client or network call is present: checked by reviewing the dependencies in [libraries.md](libraries.md). `[manual]`

**FR-SEC-06 Logging.** `tracing` writes to stderr only, with the level set by `RUST_LOG`. Logs and `run.jsonl` never contain file contents, tool output bodies or the key.
- AC: Run-log entries are checked for forbidden content. `[unit]`

**FR-SEC-07 Origin sanitization.**
- AC: HTTP(S) userinfo is stripped before the origin is stored or sent. The transport, port, path and `.git` suffix are kept. `[unit]`

**FR-SEC-08 Git invocation.**
- AC: Git runs as a fixed argument vector with no shell, and with `--no-optional-locks` for read commands. `[unit]`

## 8. Non-functional requirements

### 8.1 NFR-PERF

Measured on Apple silicon with a release build. The tests are `#[ignore]` timing tests whose names start with `perf_`, run with `cargo test --workspace --release -- --ignored perf_`.

| ID | Target |
|---|---|
| NFR-PERF-01 | Listing the top level of a 50,000-file repository: < 300 ms. |
| NFR-PERF-02 | Opening a 2 MB text file (read, build the line index, highlight the first visible window): < 300 ms. |
| NFR-PERF-03 | Layout and routing of a 60-node diagram: < 100 ms. |
| NFR-PERF-04 | Building the canvas geometry for a 100-node diagram: < 8 ms. |

### 8.2 NFR-QUAL

- **NFR-QUAL-01** At the end of every milestone, these all pass:
  - `cargo fmt --check`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace`
- **NFR-QUAL-02** Every dependency and bundled asset has an entry in [libraries.md](libraries.md) before it's added.
- **NFR-QUAL-03** The crate dependency direction follows [0003](adr/0003-cargo-workspace-and-crate-boundaries.md). `archalign-model` and `archalign-layout` do no file or network I/O.
- **NFR-QUAL-04** User data (repositories, diagram files, state files) can never cause a panic (FR-PANE-11).

## 9. Keyboard shortcuts

Global shortcuts work everywhere except inside a focused text field. Diagram shortcuts require focus on the diagram canvas.

| Action | Shortcut | Scope |
|---|---|---|
| Open folder | ⌘O | Global |
| Archify | ⌘G | Global: repo open, no run |
| Cancel run | ⌘. | Global: run active |
| History | ⌘Y | Global |
| Show/hide Tree / Diagram / Source | ⌘1 / ⌘2 / ⌘3 | Global |
| Reset layout | ⌘⇧0 | Global |
| Toggle theme | ⌘⇧D | Global |
| Settings | ⌘, | Global |
| Refresh | ⌘R | Global |
| Re-layout | ⌘⇧L | Global: diagram open |
| Find node | ⌘F or `/` | Diagram |
| Zoom in / out | `+` / `-`, or ⌘/Ctrl+scroll | Diagram |
| 100% / fit | `0` / `F` | Diagram |
| Upstream / downstream reach | `U` / `D` | Diagram, node selected |
| Clear | Esc | Diagram, finder |

## 10. Verification plan

Details are in [0029](adr/0029-testing-strategy-and-live-smoke-runs.md).

- **Fixtures.** Archify architecture JSON is vendored into `fixtures/archify/` ([0004](adr/0004-name-attribution-and-vendored-archify-material.md)). Small synthetic repositories are created with `tempfile` inside tests.
- **Layers.** The tags in §7 assign each requirement to a layer:
  - `[unit]`, `[snap]` and `[prop]` run under `cargo test --workspace`;
  - `[perf]` runs as in §8.1;
  - `[sim]` uses `iced_test`. Canvas nodes are clicked by `Point` computed from the layout. Widgets are found by ID, because text selectors can't see `rich_text` or canvas content.
- **Live tests.** These require `ARCHALIGN_LIVE=1` and are never part of the default suite. A missing repo folder skips the test with a message.
  - During development: `openai/gpt-6-luna` on `.smoke_repo/HyperTrack` and `.smoke_repo/cs2103-ip`, at most 20 runs in total.
  - **Final smoke:** `openai/gpt-6.1-sol` on HyperTrack, cs2103-ip and a copy of `archify-src/` at `target/smoke/archify-src/` (including `.git`), at most 3 attempts each. `archify-src/` itself is never modified. A run passes when:
    - it finishes within the budgets;
    - the candidate passes every gate and is saved with geometry and `run.jsonl`;
    - a `Simulator` script opens the result, clicks 3 nodes, and sees the correct path and band range for each;
    - the screenshots are saved under `target/smoke/<repo>/` for visual review.
  - One of the generated files is also checked with Archify's CLI (FR-STO-03).
- **Manual check.** The owner opens each of the three smoke diagrams in the app and clicks through the nodes.

## 11. Milestones

Each milestone ends with: NFR-QUAL-01 passing, docs updated, `progress.md` ticked, and one local commit referencing the milestone and requirement IDs (never pushed). Checklists are in [progress.md](progress.md).

| Milestone | Scope | Requirements |
|---|---|---|
| M0 Scaffold | Workspace with the 5 crates, `rust-toolchain.toml`, `.gitignore` (`/target`, `.env`, `.archify/`, `archify-src`, `.smoke_repo/`), `THIRD_PARTY_NOTICES.md`, vendored schemas, fixtures and JetBrains Mono, docs | NFR-QUAL-01..03 |
| M1 Model and validation | serde model, schema validation, import and submission profiles (minus citations), lossless save | FR-VAL-01..05, FR-DIA-12 |
| M2 Layout and routing | `TextMeasure`, `LayoutEngine` + dugong, router, boundaries, write-back | FR-LAY-01..08, NFR-PERF-03 |
| M3 App shell | panes, toolbar, status bar, welcome/import, tree, source viewer, `Sandbox`, git metadata, citation checks, auto-opening the newest diagram | FR-PANE-01..07, 09..11, FR-IMP-01..06, FR-TREE-01..05, 08, FR-SRC-01..08, FR-VAL-06..07, FR-SEC-01, 07, 08, NFR-PERF-01..02 |
| M4 Diagram and navigation | canvas rendering, camera, node→file, file→node, reach, finder, problems, Re-layout | FR-DIA-01..09, 11, FR-TREE-06..07, NFR-PERF-04 |
| M5 Agent | `LlmClient` + litellm adapter, retries, tools, deny-list, prompt, loop, gate, budgets, consent, key resolution, run log, cancel, storage | FR-GEN-01..12, FR-SEC-02..06, FR-STO-01..03; live development runs |
| M6 History, settings, state, watcher | History picker, `state.json`/`settings.json`, Settings modal, layout persistence, theme toggle, watcher | FR-STO-04..06, FR-PANE-08, FR-DIA-10, FR-GEN-13, FR-WAT-01..10 |
| M7 Finish | Docs completion, final live smoke (including the Archify CLI check in FR-STO-03), audit of panics on user data, definition of done | NFR-QUAL-04, §12 |

## 12. Definition of done

- Every requirement in §7 and §8 has passed its acceptance criteria. Evidence is recorded in [progress.md](progress.md).
- NFR-QUAL-01 and the NFR-PERF tests pass on the owner's machine.
- The final live smoke in §10 passes, including the Archify CLI check in FR-STO-03.
- The owner has completed the manual check.
- `libraries.md` lists every dependency and asset actually in use.
- Every decision taken during implementation has an ADR, and superseded ADRs are marked.
- `architecture.md` matches the crate graph and data flow.

## 13. Implementer rules

These follow [0030](adr/0030-long-running-implementation-process.md).

1. Work through the milestones in order. Never leave a milestone with failing gates.
2. Before adding a crate or asset, add its entry to [libraries.md](libraries.md).
3. Implementation-level choices (internal design, a library, an algorithm, the tunable constants in Appendix C) may be changed when there's evidence the agreed one fails. Write a superseding ADR that includes the evidence, then update `libraries.md` and `architecture.md`.
4. **Stop and ask the owner** before changing any of the following:
   - goals, non-goals or user-visible behavior in this spec;
   - anything that sends data off the machine;
   - the contractual constants in Appendix C;
   - LLM spending (§10 live-run limits, the default model, budgets).
5. Never read or print the contents of `.env`. Never push. Never modify `archify-src/`. In `.smoke_repo/*`, write only under `.archify/`.
6. Record evidence in `progress.md`: the commands run and their results, for every ticked requirement.

## 14. Risks

| Risk | Mitigation |
|---|---|
| litellm-rust is pre-1.0 with a single maintainer | Exact version pin; one adapter behind `LlmClient`; `async-openai` is the documented fallback ([0021](adr/0021-litellm-rust-behind-llm-client-trait.md)). |
| dugong's compound layout or our routing gives poor results | `LayoutEngine` trait; property tests; ELK trial through a superseding ADR ([0018](adr/0018-app-computed-layout-dugong-and-orthogonal-routing.md)). |
| LLM output quality and cost vary | Submission gate, repair limit, budgets, the Focus field; live smoke on three repos. |
| iced 0.14 gaps (no pinch, no text selection, no native menus) | Accepted: ⌘/Ctrl+scroll zoom, copy buttons, toolbar ([0005](adr/0005-iced-native-gui-toolkit.md)). |
| Prompt injection from repository content | Read-only, confined tools; no shell, writes or network tools ([0025](adr/0025-agent-tool-sandbox-and-deny-list.md)). |
| Vendored Archify material drifts | The upstream commit is recorded in `THIRD_PARTY_NOTICES.md`; re-vendoring is manual ([0004](adr/0004-name-attribution-and-vendored-archify-material.md)). |

## 15. Traceability

| Goal | Requirements | ADRs |
|---|---|---|
| G1 Import and archify | FR-IMP, FR-GEN, FR-VAL, FR-LAY, FR-STO | 0016, 0020–0028, 0012, 0014, 0018, 0019, 0015 |
| G2 Native only | FR-PANE, FR-DIA, FR-SRC, FR-TREE | 0005, 0007, 0009, 0010 |
| G3 Movable three panels | FR-PANE-01..08 | 0006 |
| G4 Node → file | FR-DIA-03, FR-SRC-04, FR-TREE-05, FR-VAL-06 | 0011, 0009 |
| G5 Rust | §5 | 0003 |
| G6 ADRs | §13 | 0001, 0030 |
| G7 Architecture doc | §12 | 0003 |
| G8 Libraries doc | NFR-QUAL-02 | 0001, 0030 |

## Appendix A: `run.jsonl` events

One JSON object per line. Every event carries `ts` (RFC 3339, local offset) and `event`. File contents, tool output bodies and the API key are never written.

| `event` | Fields |
|---|---|
| `run_started` | `model`, `reasoning_effort`, `focus`, `budgets`, `repo` (`name`, `head`, `dirty_count`) |
| `turn` | `n`, `usage` (`input_tokens`, `output_tokens`), `tool_calls` (count) |
| `tool_call` | `turn`, `name`, `args` (paths, line numbers, pattern, glob; never file text), `result_bytes`, `error` |
| `retry` | `turn`, `attempt`, `status` or `timeout`, `wait_ms` |
| `fields_dropped` | `pointers` (JSON pointers of the dropped fields) |
| `submission_rejected` | `attempt`, `errors` (`pointer`, `message`) |
| `run_finished` | `outcome` (`success`, `failed`, `cancelled`), `reason`, `turns`, `input_tokens`, `output_tokens`, `elapsed_ms`, `candidate` (relative path, on success) |

## Appendix B: app state and settings

`state.json`:

```json
{
  "version": 1,
  "window": { "width": 1440, "height": 900 },
  "panes": { "layout": "<serde mirror of pane_grid::Configuration>", "hidden": [], "maximized": null },
  "theme": "dark",
  "last_repo": "/abs/path",
  "recent_repos": ["/abs/path"],
  "consents": [{ "repo": "/abs/path", "provider": "openai", "model": "gpt-6.1-sol", "granted_at": "2026-10-10T10:00:00+08:00" }]
}
```

`settings.json`:

```json
{
  "version": 1,
  "model": "openai/gpt-6.1-sol",
  "reasoning_effort": null,
  "budgets": { "max_turns": 40, "max_input_tokens": 3000000, "max_completion_tokens": 32000, "wall_clock_s": 900, "request_timeout_s": 300, "max_repairs": 3 },
  "show_hidden_files": false
}
```

## Appendix C: constants

"Contract" means changing the value needs the owner's approval (§13). "Tunable" values may be changed with an ADR.

| Constant | Value | Kind |
|---|---|---|
| Max model turns / input tokens / completion tokens per turn | 40 / 3,000,000 / 32,000 | Contract |
| Wall clock / per-request timeout | 15 min / 300 s | Contract |
| Repairs | 3 (the 4th rejection fails the run) | Contract |
| Retries | 3, waiting 1 s / 2 s / 4 s, or `Retry-After` | Contract |
| `read_file` lines / any tool result | 400 / 24 KB | Contract |
| `grep` matches / file size / time | 100 / 1 MB / 5 s | Contract |
| Viewer file size limit | 2 MB | Contract |
| Watcher debounce / burst threshold | 300 ms / 500 paths | Contract |
| Zoom range | 10%–400% | Contract |
| Live-run limits | Luna ≤ 20 during development; Sol ≤ 3 attempts per smoke repo | Contract |
| Default model | `openai/gpt-6.1-sol` | Contract |
| Seed limits | tree 400 entries, README 150 lines, 120 lines per manifest, 48 KB total | Tunable |
| `list_dir` entries / `grep` line text | 500 / 200 characters | Tunable |
| Focus length | 500 characters | Tunable |
| Default pane ratios / minimum pane size | 0.20 / 0.50 / 0.30, 160 px | Tunable |
| Default window size | 1440×900 | Tunable |
| Overscan (tree, source) | 20 rows | Tunable |
| Zoom step / fit padding / tooltip delay | 1.25× / 40 px / 400 ms | Tunable |
| Node fonts and padding | 14 / 12 / 11 px; 12 px horizontal, 10 px vertical; minimum 120×60 | Tunable |
| Boundary padding without `pad` | 30 px top/left/right, 50 px bottom (Archify) | Tunable |
| Parallel-edge offset | ≥ 8 px | Tunable |
| Git refresh rate limit | 2 s | Tunable |
| State save rate limit | 1 s | Tunable |
