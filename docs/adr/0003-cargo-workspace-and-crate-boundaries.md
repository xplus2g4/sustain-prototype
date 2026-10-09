# 0003. Cargo workspace and crate boundaries

- Status: Accepted
- Date: 2026-10-10
- Related: [0004](0004-name-attribution-and-vendored-archify-material.md), [0005](0005-iced-native-gui-toolkit.md), [0018](0018-app-computed-layout-dugong-and-orthogonal-routing.md), [0021](0021-litellm-rust-behind-llm-client-trait.md), [0025](0025-agent-tool-sandbox-and-deny-list.md), [0029](0029-testing-strategy-and-live-smoke-runs.md); spec: NFR-QUAL

## Context

ArchAlign combines a data model, a layout engine, filesystem and git access, an LLM agent and a GUI. Layout and agent logic need to be testable without starting the GUI, and the agent needs headless live tests.

## Decision

Use a Cargo workspace at the project root, with crates under `crates/`.

Dependency direction: `model <- layout`, `model <- repo <- agent`. The desktop crate depends on all four.

- `archalign-model`
  - serde types for the Archify v1 architecture and common definitions;
  - lossless load/save;
  - JSON Schema and semantic validation.
  - Pure: no file or network I/O. It takes strings or bytes.
- `archalign-layout` (pure, depends on model)
  - the `TextMeasure` trait;
  - the `LayoutEngine` trait and its `dugong` adapter;
  - the orthogonal edge router;
  - the output scene geometry.
- `archalign-repo` (filesystem and git boundary, depends on model)
  - root-confined access (`Sandbox`);
  - `.gitignore`-aware walking, binary detection and size caps;
  - git metadata via the git CLI;
  - citation checks and the file-to-node reverse index;
  - `.archify/` storage: folders, atomic writes, history scan;
  - the file watcher.
- `archalign-agent` (depends on model and repo; no GUI dependency)
  - the `LlmClient` trait and its litellm adapter;
  - the tools, operating over the repo `Sandbox`;
  - the embedded prompt;
  - the agent loop and budgets;
  - run-log events.
- `archalign-desktop`: the iced binary `archalign`.

Workspace settings:

- edition 2024;
- `rust-toolchain.toml` pins the stable channel;
- MSRV 1.95, the highest `rust-version` among direct dependencies (`dugong` 0.8.0 declares 1.95; iced 0.14 and litellm-rust 0.3.0 declare 1.88). The development toolchain is stable rustc 1.97.1;
- every crate has `publish = false`;
- shared versions live in `[workspace.dependencies]`.

`archify-src/` is never a build input.

## Alternatives considered

### A single crate

Layout and agent code could not be tested without the GUI.

### Finer-grained crates

Separate validator or watcher crates add boundaries that buy nothing.

### The agent inside the desktop crate

Couples the LLM loop to the GUI and blocks headless live tests.

## Consequences

- Layout and model can be tested headless.
- The agent can be tested with a fake `LlmClient`.
- Layering is enforced by Cargo: a crate cannot reach code it does not depend on.
- Shared dependency versions are managed in one place.

## References

- `crates/archalign-model`, `crates/archalign-layout`, `crates/archalign-repo`, `crates/archalign-agent`, `crates/archalign-desktop`
- [`../architecture.md`](../architecture.md)
- [`../libraries.md`](../libraries.md)
