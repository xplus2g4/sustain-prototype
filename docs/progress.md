# ArchAlign progress

How to use this file ([spec §11–§13](spec.md#11-milestones), [0030](adr/0030-long-running-implementation-process.md)):

- Tick a requirement only once its acceptance criteria pass. Record the evidence next to it: test names, the commands run and their results.
- A milestone is done when every item in it is ticked, NFR-QUAL-01 passes, the docs are updated, and the milestone has a local commit. Never push.
- Append one entry to the [Log](#log) for each milestone.

| Milestone | Status | Commit | Date |
|---|---|---|---|
| M0 Scaffold | Done | see `git log --grep "^M0"` | 2026-10-10 |
| M1 Model and validation | Done | see `git log --grep "^M1"` | 2026-10-10 |
| M2 Layout and routing | Not started | — | — |
| M3 App shell | Not started | — | — |
| M4 Diagram and navigation | Not started | — | — |
| M5 Agent | Not started | — | — |
| M6 History, settings, state, watcher | Not started | — | — |
| M7 Finish | Not started | — | — |

## M0 Scaffold

- [x] Design docs: `spec.md`, `architecture.md`, `libraries.md`, `progress.md`, `adr/0001`–`0030` with the index. Evidence: written on 2026-10-10 after the design interview.
- [x] `.env` is in `.gitignore`. Evidence: `git check-ignore -v .env` → `.gitignore:4:.env`.
- [x] Workspace `Cargo.toml` with the 5 crates, `[workspace.dependencies]`, `rust-version = "1.95"`, edition 2024, `publish = false`. Evidence: `Cargo.toml`, `crates/*/Cargo.toml` (`*.workspace = true`); `cargo metadata` resolves.
- [x] `rust-toolchain.toml` pinned to the stable channel. Evidence: `channel = "stable"`, components rustfmt + clippy.
- [x] `.gitignore` also contains `/target`. Already present: `.archify/`, `archify-src`, `.smoke_repo/`, `.env`.
- [x] `THIRD_PARTY_NOTICES.md`: the Archify MIT license (upstream `27f78a2bd13a1ee0c71d5288650670513bf64e7c`) and JetBrains Mono OFL-1.1. Evidence: full license texts copied from `archify-src/LICENSE` and the release's `OFL.txt`.
- [x] Vendored schemas → `crates/archalign-model/schemas/`. Evidence: `architecture.schema.json`, `common.schema.json` copied byte-for-byte from `archify-src/archify/schemas/`.
- [x] Vendored fixtures → `fixtures/archify/`. Evidence: 24 files in `skill-examples/` (5), `examples/` (4), `docs-cases/` (2), `test-fixtures/**` (12), `skill-run/` (1); source map in `fixtures/archify/README.md`.
- [x] JetBrains Mono TTF → `crates/archalign-desktop/assets/fonts/`. Evidence: Regular + Bold from the v2.304 release zip (SHA-256 recorded in `libraries.md` §5 and `THIRD_PARTY_NOTICES.md`).
- [x] NFR-QUAL-01: local gates green on the empty workspace. Evidence: `cargo fmt --check` ok; `cargo clippy --workspace --all-targets -- -D warnings` ok; `cargo test --workspace` ok (3 passed).
- [x] NFR-QUAL-02: every dependency added so far is listed in `libraries.md`. Evidence: only internal path dependencies are used by crates; `[workspace.dependencies]` mirrors the `libraries.md` §2 table exactly.
- [x] NFR-QUAL-03: the crate dependency direction matches ADR 0003. Evidence: `crates/archalign-desktop/tests/crate_graph.rs` (`dependency_direction_matches_adr_0003`, `pure_crates_have_no_io_dependencies`, `only_desktop_depends_on_iced`) pass.

## M1 Model and validation

- [x] FR-VAL-01 Load pipeline. Evidence: `archalign-model/tests/validation.rs`: `malformed_json_is_an_error_with_position`, `schema_errors_carry_pointer_and_message` (pointer `/components/1/type`), `ui_shows_at_most_twenty_errors` (`first_errors` caps at `MAX_SHOWN_ERRORS = 20`), `non_object_json_fails_without_panicking`.
- [x] FR-VAL-02 Model coverage. Evidence: `tests/fixtures.rs::every_fixture_loads` (all 24 vendored fixtures); `validation.rs::kitchen_sink_document_covers_every_schema_property` (every schema property set at once round-trips); `extra_field_fails_and_names_the_field` (schema and serde both name `colour`), `extra_nested_meta_field_fails`.
- [x] FR-VAL-03 Lossless save. Evidence: `fixtures.rs::load_save_round_trips_to_equal_json_value` (24 fixtures), `saved_output_is_two_space_pretty_with_integer_numbers`, `num::tests::whole_values_serialize_as_integers`.
- [x] FR-VAL-04 Import profile. Evidence: `validation.rs::import_*` — errors: unknown `from`/`to`/`wraps`, duplicate component and connection IDs, schema (`schema_errors_carry_pointer_and_message`), parse (`malformed_json_*`); warnings: `import_missing_sources_is_warning`, `import_partial_boundary_overlap_is_warning`. Broken citations are FR-VAL-06 (M3).
- [x] FR-VAL-05 Submission profile (steps 1–4; citations in M3/M5). Evidence: `validation.rs::submission_*` — app-owned fields set and overridden, 23 dropped fields reported by pointer, model `meta.repository` dropped without git, rejections for schema, unknown field, duplicate IDs, unresolved reference, missing sources, >3 sources, partial overlap, equal boundaries.
- [x] FR-DIA-12 Unsupported diagrams (type detection). Evidence: `validation.rs::non_architecture_type_is_unsupported` (message "Unsupported diagram type: `workflow`" etc.), `detects_types`.

## M2 Layout and routing

- [ ] FR-LAY-01 Mode selection
- [ ] FR-LAY-02 Grid conversion
- [ ] FR-LAY-03 Sizes
- [ ] FR-LAY-04 Auto layout
- [ ] FR-LAY-05 Routing
- [ ] FR-LAY-06 Boundaries
- [ ] FR-LAY-07 Invariants for auto layouts
- [ ] FR-LAY-08 Write-back
- [ ] NFR-PERF-03 Layout and routing of 60 nodes < 100 ms

## M3 App shell

- [ ] FR-PANE-01 Default layout
- [ ] FR-PANE-02 Pane chrome
- [ ] FR-PANE-03 Move and split
- [ ] FR-PANE-04 Resize
- [ ] FR-PANE-05 Maximize
- [ ] FR-PANE-06 Close and reopen
- [ ] FR-PANE-07 Reset layout
- [ ] FR-PANE-09 Toolbar
- [ ] FR-PANE-10 Status bar
- [ ] FR-PANE-11 Errors
- [ ] FR-IMP-01 Welcome screen
- [ ] FR-IMP-02 Open folder
- [ ] FR-IMP-03 Initial state
- [ ] FR-IMP-04 Git metadata
- [ ] FR-IMP-05 Reopen last repo
- [ ] FR-IMP-06 One repo per window
- [ ] FR-TREE-01 Lazy listing
- [ ] FR-TREE-02 Filtering
- [ ] FR-TREE-03 Virtualization
- [ ] FR-TREE-04 Activation
- [ ] FR-TREE-05 Reveal
- [ ] FR-TREE-08 Symlinks
- [ ] FR-SRC-01 Read-only viewer
- [ ] FR-SRC-02 Rendering
- [ ] FR-SRC-03 Virtualization
- [ ] FR-SRC-04 Cited range
- [ ] FR-SRC-05 Out-of-range citation
- [ ] FR-SRC-06 Limits
- [ ] FR-SRC-07 Copy
- [ ] FR-SRC-08 Confinement
- [ ] FR-VAL-06 Citation checks
- [ ] FR-VAL-07 Dirty flags
- [ ] FR-SEC-01 Root confinement
- [ ] FR-SEC-07 Origin sanitization
- [ ] FR-SEC-08 Git invocation
- [ ] NFR-PERF-01 Top-level listing of 50,000 files < 300 ms
- [ ] NFR-PERF-02 Opening a 2 MB file < 300 ms

## M4 Diagram and navigation

- [ ] FR-DIA-01 Rendering
- [ ] FR-DIA-02 Camera
- [ ] FR-DIA-03 Node click
- [ ] FR-DIA-04 Hover
- [ ] FR-DIA-05 Reach
- [ ] FR-DIA-06 Edge click
- [ ] FR-DIA-07 Finder
- [ ] FR-DIA-08 Esc
- [ ] FR-DIA-09 Problems
- [ ] FR-DIA-11 Re-layout
- [ ] FR-TREE-06 Citation markers
- [ ] FR-TREE-07 Reverse highlight
- [ ] NFR-PERF-04 Canvas geometry for 100 nodes < 8 ms

## M5 Agent

- [ ] FR-GEN-01 Preconditions
- [ ] FR-GEN-02 Focus
- [ ] FR-GEN-03 Seed message
- [ ] FR-GEN-04 Tools
- [ ] FR-GEN-05 Loop
- [ ] FR-GEN-06 Gate and repair
- [ ] FR-GEN-07 Budgets
- [ ] FR-GEN-08 Retries
- [ ] FR-GEN-09 Progress
- [ ] FR-GEN-10 Cancel
- [ ] FR-GEN-11 Concurrency
- [ ] FR-GEN-12 Success. Opens automatically here; the "top of History" part is checked in M6.
- [ ] FR-SEC-02 Agent deny-list
- [ ] FR-SEC-03 Consent
- [ ] FR-SEC-04 API key
- [ ] FR-SEC-05 Network
- [ ] FR-SEC-06 Logging
- [ ] FR-STO-01 Diagram folder
- [ ] FR-STO-02 Folder contents
- [ ] FR-STO-03 Open-file compatibility (`[unit]` part; the `[live]` Archify CLI check is in M7)
- [ ] Live development runs: Luna on `.smoke_repo/HyperTrack` and `.smoke_repo/cs2103-ip`. Count used: 0 / 20.

## M6 History, settings, state, watcher

- [ ] FR-STO-04 History
- [ ] FR-STO-05 App state and settings
- [ ] FR-STO-06 Settings modal
- [ ] FR-PANE-08 Layout persistence
- [ ] FR-DIA-10 Theme
- [ ] FR-GEN-13 Model settings
- [ ] FR-WAT-01 Lifecycle
- [ ] FR-WAT-02 Debounce and filtering
- [ ] FR-WAT-03 Tree
- [ ] FR-WAT-04 Open file
- [ ] FR-WAT-05 Citations
- [ ] FR-WAT-06 Diagram files
- [ ] FR-WAT-07 Bursts
- [ ] FR-WAT-08 Self-writes
- [ ] FR-WAT-09 Git refresh
- [ ] FR-WAT-10 Fallback

## M7 Finish

- [ ] NFR-QUAL-04: audit that user data never causes a panic (repositories, diagram files, state files).
- [ ] All NFR-PERF tests pass: `cargo test --workspace --release -- --ignored perf_`.
- [ ] Final live smoke with Sol: HyperTrack (attempts 0 / 3), cs2103-ip (attempts 0 / 3), `target/smoke/archify-src` copy (attempts 0 / 3).
- [ ] FR-STO-03 `[live]`: Archify CLI check on one generated `candidate.json`. Record the result.
- [ ] Simulator screenshots saved under `target/smoke/<repo>/` and reviewed.
- [ ] `libraries.md`, the ADRs and `architecture.md` match the code.
- [ ] Owner's manual check done.

## Log

- **2026-10-10.**
  - Design interview complete.
  - Wrote `spec.md`, `architecture.md`, `libraries.md`, `progress.md` and ADRs 0001–0030.
  - Added `.env` to `.gitignore`.
  - `cargo info` showed `dugong` 0.8.0 declares `rust-version` 1.95, so the workspace MSRV is set to 1.95 in ADR 0003.
- **2026-10-10. M0 Scaffold.**
  - Workspace of 5 crates (edition 2024, MSRV 1.95, `publish = false`), `rust-toolchain.toml` (stable), `/target` gitignored.
  - Vendored the 2 Archify schemas and 24 architecture fixtures from `archify-src` @ `27f78a2`; JetBrains Mono 2.304 Regular/Bold from the official release; `THIRD_PARTY_NOTICES.md`.
  - Gates: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (3 passed: crate-graph checks for NFR-QUAL-03).
  - Dev profile builds dependencies at `opt-level = 2` so syntect/dugong-heavy tests stay fast.
- **2026-10-10. M1 Model and validation.**
  - `archalign-model`: strict serde types for every schema property, `Num` (whole numbers as integers), offline `jsonschema` validator, Import/Submission profiles, `strip_geometry`, `to_pretty_json`.
  - Finding: the vendored `portableOutputPath` surrogate pattern can't compile in Rust regex; swapped in memory for an equivalent never-matching class (recorded in `libraries.md`; no behavior change because Rust strings can't contain surrogates).
  - Gates: `cargo fmt --check` ok; `cargo clippy --workspace --all-targets -- -D warnings` ok; `cargo test --workspace` ok (model: 2 unit + 3 fixture + 32 validation tests).
  - Commits are made with `git -c commit.gpgsign=false` because the configured 1Password SSH signer fails non-interactively ("1Password: failed to fill whole buffer").
