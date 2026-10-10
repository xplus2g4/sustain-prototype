# Architecture Decision Records

This folder records the architecture decisions for ArchAlign. See [0001](0001-record-architecture-decisions.md) for the process.

## Adding an ADR

- Use the next free number and a kebab-case slug: `NNNN-kebab-title.md`.
- Follow the MADR-lite template below.
- Never edit an accepted decision. Add a new ADR and mark the old one `Superseded by NNNN`.
- Add the new ADR to the index table.

```
# NNNN. Title

- Status: Accepted
- Date: 2026-10-10
- Related: [NNNN](NNNN-slug.md), … ; spec: FR-XXX

## Context
## Decision
## Alternatives considered
### <name>
Why it was rejected.
## Consequences
Positive, negative, follow-ups.
## References
Paths and URLs from this brief only.
```

## Index

| # | Title | Status |
|---|---|---|
| 0001 | [Record architecture decisions](0001-record-architecture-decisions.md) | Accepted |
| 0002 | [macOS only, without CI](0002-macos-only-without-ci.md) | Accepted |
| 0003 | [Cargo workspace and crate boundaries](0003-cargo-workspace-and-crate-boundaries.md) | Accepted |
| 0004 | [Name, attribution and vendored Archify material](0004-name-attribution-and-vendored-archify-material.md) | Accepted |
| 0005 | [iced as the native GUI toolkit](0005-iced-native-gui-toolkit.md) | Accepted |
| 0006 | [Three-pane `pane_grid` layout](0006-three-pane-grid-layout.md) | Accepted |
| 0007 | [Toolbar instead of menus](0007-toolbar-instead-of-menus.md) | Accepted |
| 0008 | [Async model: tokio and iced tasks](0008-async-model-tokio-and-iced-tasks.md) | Accepted |
| 0009 | [Custom virtualized source viewer](0009-custom-virtualized-source-viewer.md) | Accepted |
| 0010 | [Fonts, text measurement and palette](0010-fonts-text-measurement-and-palette.md) | Accepted |
| 0011 | [v1 viewer feature scope](0011-v1-viewer-feature-scope.md) | Accepted |
| 0012 | [Archify JSON v1 as the canonical format](0012-archify-json-v1-canonical-format.md) | Accepted |
| 0013 | [Architecture diagrams only in v1](0013-architecture-diagrams-only-in-v1.md) | Accepted |
| 0014 | [Schema validation with vendored Archify schemas](0014-schema-validation-with-vendored-archify-schemas.md) | Accepted |
| 0015 | [Diagram storage, app state and run log](0015-diagram-storage-app-state-and-run-log.md) | Accepted |
| 0016 | [Local-folder import and git metadata via the git CLI](0016-local-folder-import-and-git-metadata-via-cli.md) | Accepted |
| 0017 | [File watching](0017-file-watching.md) | Accepted |
| 0018 | [App-computed layout: dugong plus orthogonal routing](0018-app-computed-layout-dugong-and-orthogonal-routing.md) | Superseded by 0031 |
| 0019 | [Geometry write-back and authored layouts](0019-geometry-write-back-and-authored-layouts.md) | Accepted |
| 0020 | [A built-in LLM agent authors diagrams](0020-built-in-llm-agent-authors-diagrams.md) | Accepted |
| 0021 | [litellm-rust behind an `LlmClient` trait](0021-litellm-rust-behind-llm-client-trait.md) | Accepted; superseded for `openai/` by 0034 |
| 0022 | [Default model: gpt-6.1-sol](0022-default-model-gpt-6-1-sol.md) | Accepted |
| 0023 | [API key resolution](0023-api-key-resolution.md) | Accepted |
| 0024 | [Data egress consent and network policy](0024-data-egress-consent-and-network-policy.md) | Accepted |
| 0025 | [Agent tool sandbox and deny-list](0025-agent-tool-sandbox-and-deny-list.md) | Accepted |
| 0026 | [System prompt derived from Archify](0026-system-prompt-derived-from-archify.md) | Accepted |
| 0027 | [Submission gate and repair loop](0027-submission-gate-and-repair-loop.md) | Accepted |
| 0028 | [Agent run budgets](0028-agent-run-budgets.md) | Accepted |
| 0029 | [Testing strategy and live smoke runs](0029-testing-strategy-and-live-smoke-runs.md) | Accepted |
| 0030 | [Long-running implementation process](0030-long-running-implementation-process.md) | Accepted |
| 0031 | [dugong placement plus one grid A* orthogonal router](0031-dugong-placement-and-grid-astar-router.md) | Accepted |
| 0032 | [UI test harness built on `iced_test` parts](0032-ui-test-harness-on-iced-test-parts.md) | Accepted (refines the desktop part of 0029) |
| 0033 | [`Retry-After` from error bodies for litellm-rust providers](0033-retry-after-from-error-bodies-for-litellm-providers.md) | Accepted (refines 0021) |
| 0034 | [OpenAI models through the Responses API](0034-openai-via-responses-api.md) | Accepted (supersedes 0021 for `openai/`) |
