# 0029. Testing strategy and live smoke runs

- Status: Accepted
- Date: 2026-10-10
- Related: [0002](0002-macos-only-without-ci.md), [0003](0003-cargo-workspace-and-crate-boundaries.md), [0005](0005-iced-native-gui-toolkit.md), [0021](0021-litellm-rust-behind-llm-client-trait.md), [0022](0022-default-model-gpt-6-1-sol.md), [0025](0025-agent-tool-sandbox-and-deny-list.md), [0026](0026-system-prompt-derived-from-archify.md), [0027](0027-submission-gate-and-repair-loop.md), [0028](0028-agent-run-budgets.md), [0030](0030-long-running-implementation-process.md); spec: NFR-QUAL, NFR-PERF

## Context

There is no CI ([0002](0002-macos-only-without-ci.md)), so every quality gate runs locally. Most of the system is deterministic and can be tested headless thanks to the crate split ([0003](0003-cargo-workspace-and-crate-boundaries.md)). The agent depends on a paid, nondeterministic LLM, so live tests must be opt-in and bounded.

## Decision

### Per crate

- **Model.** Every vendored architecture fixture validates and round-trips to an equal JSON value. One invalid fixture per validation rule.
- **Layout.** `insta` geometry snapshots for each fixture, using a fixed-advance `TextMeasure`. `proptest` invariants:
  - no node overlap;
  - boundary rectangles contain their members and nested boundaries;
  - every edge segment is axis-aligned and doesn't cross a node it doesn't connect;
  - same input gives the same output.
- **Repo** (on `tempfile` repos):
  - sandbox escapes: `..`, absolute paths, symlinks pointing out;
  - deny-list, binary detection, caps, `.gitignore`;
  - watcher filtering and self-write suppression.
- **Agent.** A fake `LlmClient` ([0021](0021-litellm-rust-behind-llm-client-trait.md)) replays scripted conversations: success, and repair-then-success; repairs exhausted; turn, token and time caps hit; cancel; a text-only reply; invalid tool arguments.
- **Desktop.** `iced_test` 0.14 `Simulator` smoke tests ([0005](0005-iced-native-gui-toolkit.md)):
  - open a fixture repo, click a node by `Point`, and check the source panel's path and highlight band;
  - click a tree file and check the reverse highlight;
  - close, reopen and reset panes.

  Text selectors can't find `rich_text` or canvas content, so tests use widget IDs and coordinates.
- **Performance.** Release-mode, `#[ignore]`d timing tests for the NFR-PERF targets.

### Live tests

Opt-in with `ARCHALIGN_LIVE=1`; never in the default suite.

| Phase | Model | Repositories | Limit |
|---|---|---|---|
| Development | `openai/gpt-6-luna` | `.smoke_repo/HyperTrack` (Python, 76 tracked files, HEAD `790efed`); `.smoke_repo/cs2103-ip` (Java/Gradle, 47 files including 6 PNGs, HEAD `6b31473`) | At most 20 Luna runs during the whole of development |
| Final smoke | `openai/gpt-6.1-sol` | HyperTrack, cs2103-ip, and a copy of `archify-src/` at `target/smoke/archify-src/` (so `archify-src/` itself is never modified) | At most 3 attempts each |

A run passes when:

- it finishes within the budgets ([0028](0028-agent-run-budgets.md));
- the candidate passes every gate ([0027](0027-submission-gate-and-repair-loop.md));
- it's saved with geometry and `run.jsonl`;
- the Simulator clicks 3 nodes and sees the correct path and highlighted range for each.

Also:

- Simulator screenshots are saved for visual review.
- If a repo folder is missing, the live test skips with a message.
- The smoke repos receive only `.archify/` output.

### Local gates every milestone

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`

No CI ([0002](0002-macos-only-without-ci.md)).

## Alternatives considered

### A CI matrix

Rejected. The owner dropped CI.

### Live tests in the default suite

Rejected. Cost and nondeterminism.

### Screenshot-diff gates

Rejected. Brittle across font and rendering changes [inference]. Screenshots are kept for human review instead.

## Consequences

- Positive: LLM spending on testing stays bounded.
- Positive: deterministic layers are covered by fast headless tests.
- Negative: live behavior is verified only at milestones ([0030](0030-long-running-implementation-process.md)).
- Negative: prompt quality ([0026](0026-system-prompt-derived-from-archify.md)) is judged only through these few live runs.

## References

- `.smoke_repo/HyperTrack`, `.smoke_repo/cs2103-ip`
- `target/smoke/archify-src/`
- `fixtures/archify/`
- [../spec.md](../spec.md) (NFR-QUAL, NFR-PERF)
- [../libraries.md](../libraries.md) (`insta`, `proptest`, `tempfile`, `iced_test`)
- [../progress.md](../progress.md)
