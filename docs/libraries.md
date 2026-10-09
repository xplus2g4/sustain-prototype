# Libraries and bundled assets

This document justifies every direct dependency, dev-dependency and bundled asset of the ArchAlign workspace. Decisions are recorded in the ADRs under [`adr/`](adr/README.md). Requirements are in [`spec.md`](spec.md) and the crate structure in [`architecture.md`](architecture.md).

## 1. Purpose and policy

- **An entry comes first.** A crate or asset gets an entry here before it is added to any `Cargo.toml` or to the repository ([0030](adr/0030-long-running-implementation-process.md)).
- **Versions.** Shared version requirements live in `[workspace.dependencies]` in the root `Cargo.toml` ([0003](adr/0003-cargo-workspace-and-crate-boundaries.md)). When a requirement or feature set changes, the entry here is updated in the same commit.
- **Rejected alternatives are recorded.** Each entry lists the alternatives considered. Crates that were evaluated and not adopted are listed in [section 4](#4-considered-but-not-used).
- **Replacing a library.** The implementer may replace a library with a new ADR carrying the evidence, and must update this file and [`architecture.md`](architecture.md) ([0030](adr/0030-long-running-implementation-process.md)).
- **Verification.** Licenses, MSRVs, latest versions and feature names below were checked with `cargo info <crate>@<version>` on 2026-10-10. "Resolves to" is the newest version matching the requirement on that date.
- **Workspace MSRV** is 1.95 ([0003](adr/0003-cargo-workspace-and-crate-boundaries.md)). That's the highest `rust-version` any direct dependency declares (`dugong` 0.8.0). `rust-toolchain.toml` pins the stable channel. Any new dependency that declares a higher `rust-version` needs an update to ADR 0003.

## 2. Summary table

### Dependencies

| Crate | Version requirement | Features | Used by | Purpose | License | ADR |
|---|---|---|---|---|---|---|
| `iced` | `0.14.0` | `canvas`, `tokio`, `advanced` | archalign-desktop | GUI toolkit, diagram canvas, text measurement | MIT | [0005](adr/0005-iced-native-gui-toolkit.md), [0010](adr/0010-fonts-text-measurement-and-palette.md) |
| `litellm-rust` | `=0.3.0` | none (crate has no features) | archalign-agent | LLM Chat Completions client | MIT | [0021](adr/0021-litellm-rust-behind-llm-client-trait.md) |
| `reqwest` | `0.12` | `default-features = false`; `rustls-tls`, `json` | archalign-agent | HTTP client handed to litellm (300 s timeout) | MIT OR Apache-2.0 | [0021](adr/0021-litellm-rust-behind-llm-client-trait.md) |
| `tokio` | `1.53` | `rt`, `time` | archalign-agent, archalign-desktop | Async runtime, timers, `spawn_blocking` | MIT | [0008](adr/0008-async-model-tokio-and-iced-tasks.md) |
| `serde` | `1.0` | `derive` | all crates | (De)serialization of every persisted format | MIT OR Apache-2.0 | [0012](adr/0012-archify-json-v1-canonical-format.md) |
| `serde_json` | `1.0` | defaults | all crates | JSON for diagrams, settings, state, run log, snapshots | MIT OR Apache-2.0 | [0012](adr/0012-archify-json-v1-canonical-format.md), [0015](adr/0015-diagram-storage-app-state-and-run-log.md) |
| `jsonschema` | `0.58` | `default-features = false` | archalign-model | JSON Schema validation, parity with Archify's AJV gate | MIT | [0014](adr/0014-schema-validation-with-vendored-archify-schemas.md) |
| `thiserror` | `2.0` | defaults | library crates | Typed error enums | MIT OR Apache-2.0 | [0003](adr/0003-cargo-workspace-and-crate-boundaries.md) |
| `dugong` | `0.8.0` | none (crate has no features) | archalign-layout | dagre-compatible compound graph layout | MIT OR Apache-2.0 | [0018](adr/0018-app-computed-layout-dugong-and-orthogonal-routing.md) |
| `ignore` | `0.4` | defaults | archalign-repo | `.gitignore`-aware directory walking | Unlicense OR MIT | [0025](adr/0025-agent-tool-sandbox-and-deny-list.md) |
| `regex` | `1.13` | defaults | archalign-repo | `grep` tool over model-supplied patterns | MIT OR Apache-2.0 | [0025](adr/0025-agent-tool-sandbox-and-deny-list.md) |
| `notify-debouncer-mini` | `0.7` | defaults (`macos_fsevent`) | archalign-repo | Debounced recursive file watching | MIT OR Apache-2.0 (`notify`: CC0-1.0) | [0017](adr/0017-file-watching.md) |
| `jiff` | `0.2` | defaults | archalign-repo | Local-time timestamps for `.archify/` folder names | Unlicense OR MIT | [0015](adr/0015-diagram-storage-app-state-and-run-log.md) |
| `syntect` | `5.3` | `default-features = false`; `default-syntaxes`, `default-themes`, `dump-load`, `regex-fancy` | archalign-desktop | Syntax highlighting in the source viewer | MIT | [0009](adr/0009-custom-virtualized-source-viewer.md) |
| `rfd` | `0.17` | defaults | archalign-desktop | Native folder picker (`NSOpenPanel`) | MIT | [0016](adr/0016-local-folder-import-and-git-metadata-via-cli.md) |
| `directories` | `6.0` | none (crate has no features) | archalign-desktop | OS config directory for app state and settings | MIT OR Apache-2.0 | [0015](adr/0015-diagram-storage-app-state-and-run-log.md) |
| `dotenvy` | `0.15` | defaults | archalign-desktop | Parse `.env` without mutating the environment | MIT | [0023](adr/0023-api-key-resolution.md) |
| `tracing` | `0.1` | defaults | all crates | Structured log events and performance spans | MIT | [0024](adr/0024-data-egress-consent-and-network-policy.md) |
| `tracing-subscriber` | `0.3` | `env-filter` | archalign-desktop | stderr subscriber filtered by `RUST_LOG` | MIT | [0024](adr/0024-data-egress-consent-and-network-policy.md) |

### Dev-dependencies

| Crate | Version requirement | Features | Used by | Purpose | License | ADR |
|---|---|---|---|---|---|---|
| `insta` | `1.49` | `json` | archalign-layout | Layout geometry snapshots | Apache-2.0 | [0029](adr/0029-testing-strategy-and-live-smoke-runs.md) |
| `proptest` | `1.11` | defaults | archalign-layout | Layout invariant property tests | MIT OR Apache-2.0 | [0029](adr/0029-testing-strategy-and-live-smoke-runs.md) |
| `iced_test` | `0.14.0` | none (crate has no features) | archalign-desktop | Headless UI `Simulator` | MIT | [0029](adr/0029-testing-strategy-and-live-smoke-runs.md) |
| `tempfile` | `3.27` | defaults | archalign-repo | Temporary repositories in tests | MIT OR Apache-2.0 | [0029](adr/0029-testing-strategy-and-live-smoke-runs.md) |

### Bundled assets

| Asset | Used by | Purpose | License | ADR |
|---|---|---|---|---|
| JetBrains Mono (TTF) | archalign-desktop | Diagram labels and source viewer | OFL-1.1 | [0010](adr/0010-fonts-text-measurement-and-palette.md), [0004](adr/0004-name-attribution-and-vendored-archify-material.md) |

## 3. Dependencies

### `iced`

- **Version and features.** `0.14.0`, features `canvas`, `tokio`, `advanced`, on top of the crate defaults. MSRV 1.88. Released 2025-12-07.
- **Used by.** archalign-desktop.
- **Purpose.** The native GUI toolkit (Elm architecture: state, `update`, `view`). It provides the three-pane `pane_grid` (FR-PANE), the widgets for the tree and source viewer (FR-TREE, FR-SRC) and the task/subscription model for async work.
  - `canvas` draws the diagram (FR-DIA).
  - `tokio` makes tokio the executor, which reqwest and litellm-rust require ([0008](adr/0008-async-model-tokio-and-iced-tasks.md)).
  - `advanced` exposes `iced::advanced::graphics::text::Paragraph`. Node sizes are measured with `Paragraph::with_text(…).min_bounds()`, which uses the global font system and needs no renderer ([0010](adr/0010-fonts-text-measurement-and-palette.md)).
- **Why this one.** Owner goal: native presentation only, no HTML, webview or browser rendering. The owner preferred iced among the native toolkits. It has a headless test harness (`iced_test`) and pane splitting built in ([0005](adr/0005-iced-native-gui-toolkit.md)).
- **Alternatives considered.** egui/eframe + egui_dock, gpui, Slint, and webview stacks (Tauri, wry, Dioxus desktop). See [section 4](#4-considered-but-not-used).
- **License.** MIT.
- **Risks and notes.** Gaps verified in the 0.14.0 source, each with its own workaround ([0005](adr/0005-iced-native-gui-toolkit.md)):
  - `pane_grid` has no tab stacking ([0006](adr/0006-three-pane-grid-layout.md));
  - no native menu bar API ([0007](adr/0007-toolbar-instead-of-menus.md));
  - no pinch or magnify gesture events, so zoom uses ⌘/Ctrl+scroll;
  - the `text_editor` highlighter `Format` has only `color` and `font`, hence the custom source viewer ([0009](adr/0009-custom-virtualized-source-viewer.md));
  - no virtualized list widget, so virtualization is custom.
  - The optional `highlighter` feature is not enabled; it is tied to `text_editor`.
  - iced is pre-1.0. A minor-version upgrade must be treated as a breaking change [inference].

### `litellm-rust`

- **Version and features.** `=0.3.0` (exact pin). The crate declares no features. MSRV 1.88. Released 2026-07-11.
- **Used by.** archalign-agent, only inside the adapter module behind the `LlmClient` trait.
- **Purpose.** Sends each agent turn as a non-streaming `LiteLLM::completion(ChatRequest)` with `tools`, `tool_choice`, `parallel_tool_calls`, `max_completion_tokens` and optional `reasoning_effort` (FR-GEN). The model prefix selects the provider (`openai/…`, `anthropic/`, `gemini/`, `openrouter/`, `xai/`, `zai/`). The API key is passed with `ProviderConfig::with_api_key` ([0023](adr/0023-api-key-resolution.md)).
- **Why this one.** One client covers several providers through the litellm `provider/model` string. `ChatRequest` exposes `reasoning_effort` and `extra_body`. It accepts a caller-supplied `reqwest::Client` through `with_client` ([0021](adr/0021-litellm-rust-behind-llm-client-trait.md)).
- **Alternatives considered.** `async-openai` (the designated fallback), a hand-rolled reqwest client, and official SDKs (none mature; `anthropic` is at 0.0.8).
- **License.** MIT.
- **Risks and notes.**
  - Small project: 4,096 total downloads, 6 GitHub stars, a single maintainer (avivsinai). Pre-1.0. Mitigated by the exact pin and by confining it to one adapter module, so a swap touches only that module.
  - Chat Completions only (`{base}/chat/completions`). No Responses-API features.
  - No built-in retry: `send_json_with_retry` executes once. ArchAlign retries 429, 5xx and timeouts itself, up to 3 times with 1 s, 2 s, 4 s backoff, honoring `Retry-After`.
  - The default client times out after 60 s, so ArchAlign supplies its own client (see `reqwest`).
  - `stream_completion` does not surface tool-call deltas, so token streaming is not used.
  - `tool_calls` arrives as a raw `serde_json::Value`; the adapter parses it into typed structs.
  - Depends on `reqwest` 0.12, `tokio` 1.36, `thiserror` 2.0 and `anyhow` 1.0 (from `cargo info -v`).

### `reqwest`

- **Version and features.** `0.12` (resolves to 0.12.28; the latest release is 0.13.5). `default-features = false`, features `rustls-tls` and `json`. MSRV 1.64.0.
- **Used by.** archalign-agent.
- **Purpose.** ArchAlign builds its own `reqwest::Client` with a 300 s timeout and passes it to litellm-rust through `with_client`. The 300 s matches the per-request budget ([0028](adr/0028-agent-run-budgets.md)).
- **Why this one.** It is the HTTP client litellm-rust is built on. The version must be 0.12, because litellm-rust 0.3.0 depends on `reqwest@0.12` and a 0.13 `Client` is a different type that `with_client` does not accept.
  - `default-features = false` drops `default-tls` (native-tls), `charset`, `http2` and `system-proxy`.
  - `rustls-tls` is an alias for `rustls-tls-webpki-roots`: rustls with the `ring` provider and the bundled `webpki-roots` certificate set.
  - `json` enables JSON request and response bodies.
- **Alternatives considered.** reqwest 0.13.5 (incompatible `Client` type). Keeping litellm's internal client (60 s timeout is too short for long reasoning turns [inference]).
- **License.** MIT OR Apache-2.0.
- **Risks and notes.**
  - Cargo unifies features across the dependency graph. litellm-rust 0.3.0 declares `reqwest` 0.12 with `default-features = false` and the features `json`, `rustls-tls`, `multipart` and `stream`, so native-tls is not pulled in; the unified build only adds `multipart` and `stream`. Confirm with `cargo tree -e features -i reqwest` when the crate is added.
  - With `webpki-roots`, trust comes from the bundled root set, not the macOS keychain [inference: matters only behind TLS-intercepting proxies].
  - Moving to reqwest 0.13 requires a litellm-rust release built on 0.13.

### `tokio`

- **Version and features.** `1.53` (resolves to 1.53.2). Features `rt` (`spawn_blocking`) and `time` (sleep and timeouts). MSRV 1.71.
- **Used by.** archalign-agent, archalign-desktop.
- **Purpose.** The async runtime under reqwest and litellm-rust. The iced `tokio` feature makes it the executor for iced tasks ([0008](adr/0008-async-model-tokio-and-iced-tasks.md)).
  - `time`: retry backoff, the 15-minute run cap and the 5 s `grep` cap ([0028](adr/0028-agent-run-budgets.md)).
  - `rt`: `tokio::task::spawn_blocking` for file-system work (listing, file loads, citation checks, git commands) and the agent's file tools.
- **Why this one.** reqwest and litellm-rust require a tokio runtime; iced's default thread-pool executor is not enough.
- **Alternatives considered.** Hand-managed std threads and channels (more plumbing, manual cancellation). iced's default thread-pool executor. `smol` via iced's `smol` feature [inference: incompatible with reqwest].
- **License.** MIT.
- **Risks and notes.** Blocking work already inside `spawn_blocking` runs to its own caps after a cancel. If tests use `#[tokio::test]`, the `macros` feature is needed in that crate's dev-dependencies [inference].

### `serde`

- **Version and features.** `1.0` (resolves to 1.0.229), feature `derive`. MSRV 1.56.
- **Used by.** All crates.
- **Purpose.** Derived (de)serialization for the Archify JSON v1 model, settings, app state, the pane layout tree, `run.jsonl` events and test snapshots.
  - Model types use `deny_unknown_fields` to mirror the schema's `additionalProperties: false` ([0012](adr/0012-archify-json-v1-canonical-format.md)).
  - The pane layout is our own serde tree, because `pane_grid::Configuration` derives only `Debug, Clone` ([0006](adr/0006-three-pane-grid-layout.md)).
- **Why this one.** The de facto standard for Rust serialization; every other crate here that reads or writes data integrates with it [inference].
- **Alternatives considered.** Untyped `serde_json::Value` as the model: no compile-time guarantees ([0012](adr/0012-archify-json-v1-canonical-format.md)).
- **License.** MIT OR Apache-2.0.
- **Risks and notes.** None.

### `serde_json`

- **Version and features.** `1.0` (resolves to 1.0.151), default features. MSRV 1.71.
- **Used by.** All crates.
- **Purpose.** Reads and writes `candidate.json` (pretty-printed, 2-space indentation, whole numbers as integers), app state and settings, `run.jsonl` lines, and the `Value` input to `jsonschema` (FR-STO, FR-VAL). Tool schemas and `tool_calls` are exchanged with litellm-rust as `serde_json::Value`.
- **Why this one.** The serde JSON implementation; `jsonschema`, litellm-rust and `insta` all use it.
- **Alternatives considered.** None evaluated.
- **License.** MIT OR Apache-2.0.
- **Risks and notes.** `preserve_order` is not enabled. Lossless saving is defined as equal JSON values, not identical bytes ([0012](adr/0012-archify-json-v1-canonical-format.md)).

### `jsonschema`

- **Version and features.** `0.58` (resolves to 0.58.6), `default-features = false`. MSRV 1.85.0.
- **Used by.** archalign-model.
- **Purpose.** Validates documents against the vendored `architecture.schema.json` and `common.schema.json` (draft 2020-12) before strict serde parsing (FR-VAL). The common schema is registered as a resource, so `common.schema.json#/$defs/…` references resolve offline. Each error yields a JSON pointer and a message, which are shown to the user and sent back to the model for repair ([0027](adr/0027-submission-gate-and-repair-loop.md)).
- **Why this one.** Exact parity with Archify's AJV schema gate: ID patterns, enums, source counts, length limits, `repository.revision` and `portableOutputPath` rules come from the same schema files ([0014](adr/0014-schema-validation-with-vendored-archify-schemas.md)).
  - Turning off the defaults removes `resolve-http` (reqwest 0.13 plus rustls), `resolve-file`, `tls-aws-lc-rs` and `idna`. With them off, the validator never fetches a `$ref` over HTTP or from disk, which keeps archalign-model free of I/O ([0003](adr/0003-cargo-workspace-and-crate-boundaries.md)) and keeps the LLM endpoint the only network destination ([0024](adr/0024-data-egress-consent-and-network-policy.md)).
- **Alternatives considered.** serde plus hand-written constraint checks (fewer dependencies, but drifts from the AJV gate). Other JSON Schema crates were not evaluated.
- **License.** MIT.
- **Risks and notes.** `idna` is a default feature and is dropped [inference: it affects only internationalized `format` checks]. The Archify schemas in `archify-src/archify/schemas/` contain no `format` keyword (checked on 2026-10-10). Fast-moving 0.x versions; the requirement is pinned to the 0.58 minor.
  - `common.schema.json`'s `portableOutputPath` contains `not: {pattern: "[\uD800-\uDFFF]"}`. Rust's regex engines cannot compile surrogate code points, so building the validator fails on the vendored file as-is (observed 2026-10-10: `ValidationError … kind: Format { format: "regex" } … /$defs/portableOutputPath/allOf/2/not/pattern`). A Rust `String` can never hold a surrogate, so that clause always passes; `archalign-model::schema` swaps it in memory for the never-matching `[^\s\S]`. The vendored file stays byte-identical, and the remaining path rules are covered by `output_path_rules_are_enforced`.
  - `cargo tree -p archalign-model -i reqwest` finds no match: no HTTP client is linked through `jsonschema`.

### `thiserror`

- **Version and features.** `2.0` (resolves to 2.0.21), default features. MSRV 1.77.
- **Used by.** The library crates: archalign-model, archalign-layout, archalign-repo, archalign-agent.
- **Purpose.** Derives `std::error::Error` for typed error enums at each crate boundary (validation, sandbox, storage, agent run failures), so callers can match on the cause and the UI can show a specific message (NFR-QUAL).
- **Why this one.** A derive macro with no runtime cost [inference]. iced and litellm-rust already depend on `thiserror` 2.
- **Alternatives considered.** `anyhow` (see [section 4](#4-considered-but-not-used)). Hand-written `Error` impls [inference: boilerplate].
- **License.** MIT OR Apache-2.0.
- **Risks and notes.** None.

### `dugong`

- **Version and features.** `0.8.0`. The crate declares no features. MSRV **1.95**.
- **Used by.** archalign-layout, behind the `LayoutEngine` trait.
- **Purpose.** Computes node positions when a diagram is not fully authored (FR-LAY). Components become nodes sized by `TextMeasure`; boundaries become compound parents, with nesting allowed; rank direction is left→right. The in-house orthogonal router turns dagre's polyline bend points into axis-aligned segments ([0018](adr/0018-app-computed-layout-dugong-and-orthogonal-routing.md)).
- **Why this one.** A Rust port of dagre that supports compound (nested cluster) layout, under a permissive license (MIT OR Apache-2.0).
- **Alternatives considered.** `merman-layout-elk`, `mmdflux`, `rust-sugiyama`, `layout-rs`, `gen-sugiyama`, `abstracttui-graph`, porting Archify's `routing.mjs`, and a fully in-house Sugiyama. See [section 4](#4-considered-but-not-used).
- **License.** MIT OR Apache-2.0.
- **Risks and notes.**
  - **Sets the workspace MSRV.** `cargo info` reports `rust-version = 1.95`, the highest of any direct dependency. ADR [0003](adr/0003-cargo-workspace-and-crate-boundaries.md) therefore sets the workspace MSRV to 1.95.
  - Depends on `dugong-graphlib` 0.8.0, `rustc-hash`, `serde` and `serde_json`. Repository: `Latias94/merman`.
  - dagre's spacing is tuned for polylines, and edge routing won't match Archify's quality. Guarded by the `proptest` invariants.

### `ignore`

- **Version and features.** `0.4` (resolves to 0.4.33), default features. MSRV 1.88.
- **Used by.** archalign-repo.
- **Purpose.** `.gitignore`-aware walking for the file tree (FR-TREE), the agent's `list_dir` and `grep` tools, and watcher event filtering ([0017](adr/0017-file-watching.md), [0025](adr/0025-agent-tool-sandbox-and-deny-list.md)). The fixed deny-list is applied on top (FR-SEC).
- **Why this one.** The ignore-file engine used by ripgrep, so ignore semantics match common tooling.
- **Alternatives considered.** Hand-parsing `.gitignore` files [inference: error-prone]. Asking `git` for ignored files [inference: only works inside a work tree, and git is optional ([0016](adr/0016-local-folder-import-and-git-metadata-via-cli.md))].
- **License.** Unlicense OR MIT.
- **Risks and notes.** None.

### `regex`

- **Version and features.** `1.13` (resolves to 1.13.1), default features. MSRV 1.65.
- **Used by.** archalign-repo.
- **Purpose.** Compiles the model-supplied pattern of the agent's `grep(pattern, path_glob)` tool (FR-SEC).
- **Why this one.** It guarantees linear-time matching on all inputs, so a hostile or careless pattern cannot cause catastrophic backtracking. Combined with the ≤100 matches, 1 MB file and 5 s caps, the tool stays bounded ([0025](adr/0025-agent-tool-sandbox-and-deny-list.md)).
- **Alternatives considered.** `fancy-regex` [inference: supports backtracking features, so no linear-time guarantee].
- **License.** MIT OR Apache-2.0.
- **Risks and notes.** Rust `regex` syntax has no look-around or backreferences; the prompt tells the model which syntax applies [inference].

### `notify-debouncer-mini`

- **Version and features.** `0.7` (resolves to 0.7.0), default features (`macos_fsevent`). MSRV 1.77. It depends on `notify` 8.2.0, re-exported as `notify_debouncer_mini::notify`; `notify` is not a separate direct dependency.
- **Used by.** archalign-repo.
- **Purpose.** Watches the repo root recursively, using FSEvents on macOS, with a 300 ms debounce (FR-WAT). Events are filtered by the tree's ignore rules and reach the UI through an iced `Subscription` ([0008](adr/0008-async-model-tokio-and-iced-tasks.md), [0017](adr/0017-file-watching.md)).
- **Why this one.** Debouncing prevents event storms. The mini debouncer has no rename tracking or file-ID cache, which ArchAlign does not need. It sits on the stable `notify` 8.2.
- **Alternatives considered.** Raw `notify` (event storms), `notify-debouncer-full` (unneeded rename tracking and file-ID cache), polling (wasteful), manual refresh only (the owner asked for a watcher).
- **License.** MIT OR Apache-2.0. The underlying `notify` 8.2.0 is CC0-1.0.
- **Risks and notes.** `notify` 9.0.0-rc.5 is a prerelease. An upgrade to notify 9 is possible later. ⌘R manual refresh always works if the watcher fails to start.

### `jiff`

- **Version and features.** `0.2` (resolves to 0.2.38), default features (they include the system time-zone lookup). MSRV 1.70.
- **Used by.** archalign-repo.
- **Purpose.** Formats the local time at run start for `.archify/architecture-<slug>-<YYYYMMDD-HHMMSS>/` folder names (FR-STO, [0015](adr/0015-diagram-storage-app-state-and-run-log.md)).
- **Why this one.** Local time needs the system time zone, which `std` doesn't provide [inference]. jiff resolves it with its default features.
- **Alternatives considered.** `chrono`, `time`. See [section 4](#4-considered-but-not-used).
- **License.** Unlicense OR MIT.
- **Risks and notes.** Pre-1.0 [inference: API changes possible at 0.3].

### `syntect`

- **Version and features.** `5.3` (resolves to 5.3.0), `default-features = false`, features:
  - `default-syntaxes`: the bundled syntax definitions (implies `parsing` and `dump-load`);
  - `default-themes`: the bundled themes (implies `dump-load`);
  - `dump-load`: loads the compressed binary packs the defaults ship as (listed explicitly);
  - `regex-fancy`: the pure-Rust `fancy-regex` backend.

  Not enabled: `html`, `regex-onig` (Oniguruma, a C library), `plist-load`, `yaml-load`. MSRV not declared.
- **Used by.** archalign-desktop.
- **Purpose.** Colors each visible line of the source viewer as `rich_text` spans (FR-SRC). Falls back to plain text when no syntax matches ([0009](adr/0009-custom-virtualized-source-viewer.md)).
- **Why this one.** Sublime Text grammars cover many languages from one crate with bundled defaults. The pure-Rust regex backend avoids a C build. iced's own `highlighter` feature uses syntect too, but it is tied to `text_editor`.
- **Alternatives considered.** tree-sitter highlighting (a grammar crate per language, heavier). iced's `highlighter` feature.
- **License.** MIT.
- **Risks and notes.** The syntax sets load once at startup [inference: off the UI thread]. Files over 2 MB and binaries are not highlighted (placeholder instead).

### `rfd`

- **Version and features.** `0.17` (resolves to 0.17.2), default features. MSRV not declared.
- **Used by.** archalign-desktop.
- **Purpose.** The native folder picker (`NSOpenPanel` on macOS) for importing a local repository (FR-IMP). Called through its async API inside an iced `Task`, so the UI does not block ([0016](adr/0016-local-folder-import-and-git-metadata-via-cli.md)).
- **Why this one.** A native dialog rather than a custom in-app browser. Cross-platform, which keeps a later port plausible ([0002](adr/0002-macos-only-without-ci.md)).
- **Alternatives considered.** An in-app folder browser built from iced widgets [inference: more code, non-native].
- **License.** MIT.
- **Risks and notes.** The default features `xdg-portal` and `wayland` are Linux backends [inference: unused on macOS].

### `directories`

- **Version and features.** `6.0` (resolves to 6.0.0). The crate declares no features. MSRV not declared.
- **Used by.** archalign-desktop.
- **Purpose.** Locates the OS config directory for app name `ArchAlign`. App state (window size, pane layout, theme, recent repos, consents) and settings are stored there as JSON, and it is the third `.env` lookup location (FR-STO, [0015](adr/0015-diagram-storage-app-state-and-run-log.md), [0023](adr/0023-api-key-resolution.md)).
- **Why this one.** Follows the platform conventions for config locations; small, with a single dependency (`dirs-sys`).
- **Alternatives considered.** Hard-coding a macOS path [inference: breaks a later port].
- **License.** MIT OR Apache-2.0.
- **Risks and notes.** None.

### `dotenvy`

- **Version and features.** `0.15` (resolves to 0.15.7), default features. MSRV 1.56.1.
- **Used by.** archalign-desktop.
- **Purpose.** Parses `.env` files with `dotenvy::from_path_iter` to find the provider API key: first in the launch working directory, then in the app config directory (FR-SEC, [0023](adr/0023-api-key-resolution.md)). The key is passed to litellm-rust explicitly.
- **Why this one.** `from_path_iter` returns the key/value pairs without calling `std::env::set_var`, which is `unsafe` in edition 2024 because other threads may read the environment.
- **Alternatives considered.** `dotenvy::dotenv()` (mutates the process environment). `keyring` (see [section 4](#4-considered-but-not-used)).
- **License.** MIT.
- **Risks and notes.** The imported repository's `.env` is never read. The key is never logged or displayed.

### `tracing`

- **Version and features.** `0.1` (resolves to 0.1.44), default features. MSRV 1.65.0.
- **Used by.** All crates.
- **Purpose.** Structured log events, and spans around measured operations for performance instrumentation (NFR-PERF).
- **Why this one.** The standard structured-logging facade in the tokio ecosystem [inference]; spans give timings without a separate profiler.
- **Alternatives considered.** `log` [inference: no spans].
- **License.** MIT.
- **Risks and notes.** Logs go to stderr only and must never contain the API key or file contents ([0024](adr/0024-data-egress-consent-and-network-policy.md)).

### `tracing-subscriber`

- **Version and features.** `0.3` (resolves to 0.3.23), feature `env-filter` on top of the defaults (`fmt`, `ansi`, `tracing-log`, `smallvec`, `std`). MSRV 1.65.0.
- **Used by.** archalign-desktop, which installs the subscriber once at startup [inference: the library crates only emit events].
- **Purpose.** Formats `tracing` output to stderr, filtered by `RUST_LOG` through `EnvFilter`. No log files ([0024](adr/0024-data-egress-consent-and-network-policy.md)).
- **Why this one.** The companion subscriber crate for `tracing`.
- **Alternatives considered.** None evaluated.
- **License.** MIT.
- **Risks and notes.** The optional `chrono`, `time` and `local-time` features stay off.

## 3a. Dev-dependencies

All four are dev-dependencies only and do not ship in the binary. Test strategy: [0029](adr/0029-testing-strategy-and-live-smoke-runs.md) (NFR-QUAL).

### `insta`

- **Version and features.** `1.49` (resolves to 1.49.0), feature `json` (enables JSON-serialized snapshots). MSRV 1.66.0.
- **Used by.** archalign-layout tests.
- **Purpose.** Geometry snapshots for every vendored fixture, using a fixed-advance `TextMeasure` so snapshots are deterministic.
- **Why this one.** Reviewable snapshot files with an update workflow [inference].
- **Alternatives considered.** Hand-written expected geometry [inference: brittle for many fixtures].
- **License.** Apache-2.0 (not dual-licensed).
- **Risks and notes.** Snapshots change whenever the layout engine changes; updates must be reviewed.

### `proptest`

- **Version and features.** `1.11` (resolves to 1.11.0), default features. MSRV 1.85.
- **Used by.** archalign-layout tests.
- **Purpose.** Layout invariants: no node overlap; boundaries contain their members and nested boundaries; edge segments are axis-aligned and avoid nodes they don't connect; same input gives the same output.
- **Why this one.** Generated inputs with shrinking to a minimal failing case.
- **Alternatives considered.** `quickcheck` [inference: weaker shrinking].
- **License.** MIT OR Apache-2.0.
- **Risks and notes.** None.

### `iced_test`

- **Version and features.** `0.14.0`, must match `iced`. The crate declares no features. MSRV 1.88.
- **Used by.** archalign-desktop tests.
- **Purpose.** Headless `Simulator` smoke tests: open a fixture repo, click a node by `Point`, check the source panel's path and highlight band; click a tree file and check the reverse highlight; close, reopen and reset panes. Also drives the live-smoke node clicks and saves screenshots for visual review.
- **Why this one.** The official iced headless test harness.
- **Alternatives considered.** Screenshot-diff gates (brittle across font and rendering changes [inference]).
- **License.** MIT.
- **Risks and notes.** Text selectors can't find `rich_text` or canvas content, so tests use widget IDs and coordinates.

### `tempfile`

- **Version and features.** `3.27` (resolves to 3.27.0), default features. MSRV 1.63.
- **Used by.** archalign-repo tests.
- **Purpose.** Builds temporary repositories for sandbox escapes (`..`, absolute paths, symlinks out), the deny-list, binary detection, caps, `.gitignore`, watcher filtering and self-write suppression.
- **Why this one.** Directories are removed on drop. Already in the graph through `notify-debouncer-mini`.
- **Alternatives considered.** Fixture folders checked into the repo [inference: can't express symlinks out of the root portably].
- **License.** MIT OR Apache-2.0.
- **Risks and notes.** None.

## 4. Considered but not used

GUI:

- `egui` / `eframe` + `egui_dock`: immediate mode, with tab stacking/docking built in; the owner preferred iced ([0005](adr/0005-iced-native-gui-toolkit.md)).
- `gpui`: fast, but its API is unstable and its docs thin ([0005](adr/0005-iced-native-gui-toolkit.md)).
- `slint`: declarative and polished, but no docking ([0005](adr/0005-iced-native-gui-toolkit.md)).
- `tauri` / `wry` / `dioxus-desktop`: webview rendering, which violates the no-HTML goal ([0005](adr/0005-iced-native-gui-toolkit.md)).
- `iced_aw`: 0.14.1 offers an in-window `MenuBar` behind its `menu` feature, but the toolbar is built from plain widgets without the extra dependency ([0007](adr/0007-toolbar-instead-of-menus.md)).
- `muda`: needs `menu.init_for_nsapp()` and an `EventLoopProxy`, which iced doesn't expose; no documented iced integration ([0007](adr/0007-toolbar-instead-of-menus.md)).
- iced `highlighter` feature: syntect underneath, but tied to `text_editor` ([0009](adr/0009-custom-virtualized-source-viewer.md)).

Secrets and git:

- `keyring`: OS keychain storage was dropped by the owner, whose key lives in `.env` ([0023](adr/0023-api-key-resolution.md)).
- `git2`: libgit2 is a C dependency with a more complex build; the git CLI is used instead ([0016](adr/0016-local-folder-import-and-git-metadata-via-cli.md)).
- `gix`: a large dependency tree [inference] ([0016](adr/0016-local-folder-import-and-git-metadata-via-cli.md)).

Layout ([0018](adr/0018-app-computed-layout-dugong-and-orthogonal-routing.md)):

- `merman-layout-elk` 0.8.0: links `merman-elk-layered`, which is EPL-2.0 (EPL text and source-availability information would have to ship); stable only since 2026-10-05, ~15% doc coverage, self-described as an internal adapter. Can be trialled later behind `LayoutEngine`.
- `mmdflux` 2.6.1: input must be Mermaid or MMDS text, and subgraph bounds are width/height only.
- `rust-sugiyama` 0.4.0: no cluster support.
- `layout-rs` 0.1.3: its docs say it "lacks … nested graphs".
- `gen-sugiyama`, `abstracttui-graph`: no cluster support.

LLM client ([0021](adr/0021-litellm-rust-behind-llm-client-trait.md)):

- `async-openai`: a mature OpenAI client; the designated fallback if litellm-rust gets in the way.
- `anthropic` 0.0.8: no official Rust SDK is mature.

File watching and time:

- `notify-debouncer-full`: adds rename tracking and a file-ID cache that ArchAlign doesn't need ([0017](adr/0017-file-watching.md)).
- `chrono`: jiff already covers local-time formatting; one date-time crate is enough [inference].
- `time`: same reason as `chrono` [inference].

Highlighting and errors:

- tree-sitter highlighting: needs a grammar crate per language, which is heavier ([0009](adr/0009-custom-virtualized-source-viewer.md)).
- `anyhow`: the library crates expose typed `thiserror` enums so callers can match on causes [inference]. It is still present transitively through litellm-rust.

## 5. Bundled assets

### JetBrains Mono

- **What.** JetBrains Mono 2.304, the `JetBrainsMono-Regular.ttf` and `JetBrainsMono-Bold.ttf` files from the official release archive `https://github.com/JetBrains/JetBrainsMono/releases/download/v2.304/JetBrainsMono-2.304.zip` (SHA-256 `6f6376c6ed2960ea8a963cd7387ec9d76e3f629125bc33d1fdcd7eb7012f7bbf`), stored in `crates/archalign-desktop/assets/fonts/` and loaded at startup with `iced::application(…).font(bytes)` ([0010](adr/0010-fonts-text-measurement-and-palette.md)). Regular is used for body text and the source viewer, Bold for node labels; both share the same advance widths, so measurement is unaffected.
- **Used for.** Diagram labels and the source viewer (FR-DIA, FR-SRC). The source viewer relies on its fixed line height for virtualization ([0009](adr/0009-custom-virtualized-source-viewer.md)).
- **Why.** A single bundled monospace font makes text measurement, and therefore layout, the same on every machine. System fonts would measure differently, and a proportional font such as Inter would complicate tests.
- **License.** SIL Open Font License 1.1 (OFL-1.1). The license text is carried in the root `THIRD_PARTY_NOTICES.md` ([0004](adr/0004-name-attribution-and-vendored-archify-material.md)).
- **Risks and notes.** Increases binary size [inference: a few hundred KB].

### Vendored Archify material

Not a library, but shipped in the repository and recorded for completeness. Details in [0004](adr/0004-name-attribution-and-vendored-archify-material.md). All of it is MIT, from Archify commit `27f78a2bd13a1ee0c71d5288650670513bf64e7c`, credited in `THIRD_PARTY_NOTICES.md`.

- `architecture.schema.json` and `common.schema.json` → `crates/archalign-model/schemas/` (used by `jsonschema`).
- Architecture JSON fixtures → `fixtures/archify/` (tests).
- Colors from `archify/assets/template.html` → a Rust theme table ([0010](adr/0010-fonts-text-measurement-and-palette.md)).
- The condensed system prompt → `crates/archalign-agent/prompts/`, embedded with `include_str!` ([0026](adr/0026-system-prompt-derived-from-archify.md)).

`archify-src/` is never a build input.
