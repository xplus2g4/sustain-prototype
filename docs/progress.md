# ArchAlign progress

How to use this file ([spec §11–§13](spec.md#11-milestones), [0030](adr/0030-long-running-implementation-process.md)):

- Tick a requirement only once its acceptance criteria pass. Record the evidence next to it: test names, the commands run and their results.
- A milestone is done when every item in it is ticked, NFR-QUAL-01 passes, the docs are updated, and the milestone has a local commit. Never push.
- Append one entry to the [Log](#log) for each milestone.

| Milestone | Status | Commit | Date |
|---|---|---|---|
| M0 Scaffold | Done | see `git log --grep "^M0"` | 2026-10-10 |
| M1 Model and validation | Done | see `git log --grep "^M1"` | 2026-10-10 |
| M2 Layout and routing | Done | see `git log --grep "^M2"` | 2026-10-10 |
| M3 App shell | Done | see `git log --grep "^M3"` | 2026-10-10 |
| M4 Diagram and navigation | Done | see `git log --grep "^M4"` | 2026-10-10 |
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

- [x] FR-LAY-01 Mode selection. Evidence: `archalign-layout/tests/layout.rs`: `all_positioned_is_authored`, `all_on_grid_is_authored`, `mix_of_grid_and_pos_is_authored_and_pos_wins`, `partially_positioned_is_auto_with_warning`, `none_positioned_is_auto_without_warning`.
- [x] FR-LAY-02 Grid conversion. Evidence: `archify_repo_grid_fixture_resolves_to_expected_positions` (all 9 cells hand-computed), `all_on_grid_is_authored` (defaults).
- [x] FR-LAY-03 Sizes. Evidence: `auto_sizes_are_exact_with_fixed_advance` (219×69, 120×60 minimum, 360×60), `authored_sizes_use_size_or_default`.
- [x] FR-LAY-04 Auto layout. Evidence: `tests/snapshots.rs::auto_layout_geometry_snapshots`: 24 insta JSON snapshots in `crates/archalign-layout/tests/snapshots/`, stable across reruns.
- [x] FR-LAY-05 Routing. Evidence: `endpoints_sit_on_node_sides_and_arrow_is_at_target`, `authored_sides_are_honored`, `authored_via_points_are_visited_in_order`, `routes_avoid_nodes_that_are_not_endpoints`, `parallel_connections_are_offset_by_at_least_8px` (3 placements), `self_loop_is_a_rectangle_on_the_right_side`, `label_is_centered_on_the_longest_segment`. Router replaced per ADR 0031.
- [x] FR-LAY-06 Boundaries. Evidence: `boundary_uses_pad_on_every_side`, `boundary_default_padding_is_30_sides_50_bottom`, `nested_boundary_sits_inside_its_parent`.
- [x] FR-LAY-07 Invariants for auto layouts. Evidence: `tests/invariants.rs::auto_layout_invariants_hold` (proptest, ≤60 nodes, ≤120 edges incl. self-loops and multi-edges, 2-level boundaries; 64 cases by default, plus 2,400 cases run with `PROPTEST_CASES=600/800` in release, all passing); `every_fixture_satisfies_invariants_when_auto_laid_out`.
- [x] FR-LAY-08 Write-back. Evidence: `write_back_sets_integer_pos_and_size_and_nothing_else` (integer pos/size, other fields unchanged, `layout_document` leaves the document untouched, reopens as authored with identical boxes). The app-level "opening never writes" check is repeated in M4 when the open path exists.
- [x] NFR-PERF-03 Layout and routing of 60 nodes < 100 ms. Evidence: `cargo test --workspace --release -- --ignored perf_` → `perf_layout_and_routing_of_60_nodes_under_100ms`: median of 7 = 13.4 ms (max 21.7 ms), 120 edges. Since ADR 0035 (M7), both rank directions are placed: 31.6 ms.

## M3 App shell

- [x] FR-PANE-01 Default layout. Evidence: `desktop/tests/shell.rs`::`default_layout_is_tree_diagram_source_at_20_50_30` (title bars left→right, widths 0.20/0.50/0.30 ±1%); `desktop/tests/panes.rs`::`default_layout_shape`.
- [x] FR-PANE-02 Pane chrome. Evidence: `desktop/tests/shell.rs`::`title_bars_show_name_and_context_and_buttons_emit_messages` (diagram title and source path as context labels; Maximize/Restore/Close emit `PaneMaximize`/`PaneRestore`/`PaneClose`).
- [x] FR-PANE-03 Move and split. Evidence: `desktop/tests/panes.rs`: `dropping_on_an_edge_region_splits_that_pane_on_that_side`, `dropping_on_a_pane_center_swaps_the_two`, `dropping_on_the_outer_edge_moves_the_pane_to_that_edge` (each compared with `pane_grid::State::drop` on a clone).
- [x] FR-PANE-04 Resize. Evidence: `desktop/tests/panes.rs`: `resize_updates_the_ratio`, `no_pane_goes_below_160px_in_either_dimension` (ratios 0…1, incl. a nested split).
- [x] FR-PANE-05 Maximize. Evidence: `desktop/tests/shell.rs`::`maximize_fills_the_grid_and_restore_returns`; `desktop/tests/panes.rs`::`dragging_is_ignored_while_maximized`.
- [x] FR-PANE-06 Close and reopen. Evidence: `desktop/tests/shell.rs`: `close_and_reopen_through_the_panels_menu` (☐/☑ visibility, Tree reinserted left, Source right), `the_last_visible_pane_cannot_be_closed` (disabled button, at most one of each).
- [x] FR-PANE-07 Reset layout. Evidence: `desktop/tests/shell.rs`::`reset_restores_the_default_arrangement_and_clears_maximize` (menu and ⌘⇧0).
- [x] FR-PANE-09 Toolbar. Evidence: `desktop/tests/shell.rs`: `archify_is_disabled_without_a_repo`, `open_repo_is_disabled_during_a_run`, `each_shortcut_triggers_the_same_action_as_its_button` (theme, history, settings, archify, panels, reset; ⌘2/⌘3/⌘R/⌘⇧L), `global_shortcuts_do_not_fire_inside_a_focused_text_field`. Archify/Cancel/Settings/Refresh/Re-layout handlers are completed in M4–M6.
- [x] FR-PANE-10 Status bar. Evidence: `desktop/tests/shell.rs`::`status_bar_shows_repo_git_run_and_watcher` (root path, `git <short> · 1 dirty`, Idle → `Turn 3/40 · 12345 input tokens · 1:15`, "Manual refresh only" until the M6 watcher).
- [x] FR-PANE-11 Errors (missing API key part: M5). Evidence: `desktop/tests/shell.rs`: `errors_show_as_dismissible_banners_and_in_the_status_bar` (malformed JSON), `missing_git_unreadable_files_and_bad_utf8_give_banners_not_panics`.
- [x] FR-IMP-01 Welcome screen. Evidence: `desktop/tests/shell.rs`: `welcome_lists_recents_and_marks_missing_ones` ("Missing", disabled, Remove), `welcome_keeps_at_most_ten_recents_most_recent_first`.
- [x] FR-IMP-02 Open folder (`[unit]`; the rfd picker itself is `[manual]`, owner check). Evidence: `desktop/tests/shell.rs`::`opening_a_repo_sets_the_root_and_resets_state`.
- [x] FR-IMP-03 Initial state. Evidence: `desktop/tests/shell.rs`: `the_newer_valid_diagram_opens_on_import` (newer valid of two; newest invalid skipped), `without_diagrams_the_empty_state_shows_focus_and_archify`; `repo/tests/storage.rs`.
- [x] FR-IMP-04 Git metadata. Evidence: `repo/tests/git.rs`: clean, dirty (modified/untracked/renamed), no origin, origin with credentials, subfolder of a work tree, git missing; plus status-bar rendering in `desktop/tests/shell.rs`.
- [x] FR-IMP-05 Reopen last repo. Evidence: `desktop/tests/shell.rs`::`last_repo_reopens_at_launch_if_it_exists` (and welcome when it's gone). `state.json` loading comes in M6.
- [x] FR-IMP-06 One repo per window. Evidence: `desktop/tests/shell.rs`::`another_repo_cannot_be_opened_while_a_run_is_active`, `open_repo_is_disabled_during_a_run`.
- [x] FR-TREE-01 Lazy listing. Evidence: `desktop/tests/browse.rs`: `folders_come_first_then_files_case_insensitively`, `a_folder_is_listed_on_first_expansion_off_the_ui_thread`; `repo/tests/tree.rs`.
- [x] FR-TREE-02 Filtering. Evidence: `repo/tests/tree.rs` (nested .gitignore, info/exclude, `.git`, dotfiles); `desktop/tests/browse.rs`::`ignored_files_git_and_dotfiles_are_hidden_unless_enabled`.
- [x] FR-TREE-03 Virtualization. Evidence: `virtual_list::tests::window_is_visible_plus_overscan_whatever_the_size`; `desktop/tests/browse.rs`::`only_visible_rows_plus_overscan_are_built` (5,000-file folder: ≤ visible + 40 rows built, before and after scrolling).
- [x] FR-TREE-04 Activation. Evidence: `desktop/tests/browse.rs`::`clicking_a_file_opens_it_and_selects_it_and_folders_toggle`.
- [x] FR-TREE-05 Reveal. Evidence: `desktop/tests/browse.rs`::`reveal_expands_ancestors_selects_and_scrolls_into_view` (3 nested folders listed on demand, row inside the scroll viewport). Node-click wiring in M4.
- [x] FR-TREE-08 Symlinks. Evidence: `repo/tests/tree.rs`::`symlinks_are_marked_with_outside_flag`; `desktop/tests/browse.rs`::`symlinks_show_a_marker_and_outside_links_cannot_be_opened` (↗ marker, outside links disabled, tooltip in `target/smoke/m3/outside-link-tooltip.png`).
- [x] FR-SRC-01 Read-only viewer. Evidence: `desktop/tests/browse.rs`::`the_viewer_is_read_only`.
- [x] FR-SRC-02 Rendering. Evidence: `desktop/tests/browse.rs`: `syntax_is_chosen_by_extension_then_first_line`, `long_lines_scroll_horizontally_without_wrapping`.
- [x] FR-SRC-03 Virtualization. Evidence: `desktop/tests/browse.rs`::`only_visible_lines_plus_overscan_are_built` (50,000 lines, jump to 30,000).
- [x] FR-SRC-04 Cited range. Evidence: `desktop/tests/browse.rs`::`opening_a_citation_bands_the_range_and_puts_it_in_the_top_third` (band 50–60, line 50 in the top third, band kept while scrolling, `line` only, no line → top, no band); screenshot `target/smoke/m3/cited-range.png`.
- [x] FR-SRC-05 Out-of-range citation. Evidence: `desktop/tests/browse.rs`::`an_out_of_range_citation_opens_at_line_1_with_a_banner` ("Cited lines 100–120 are outside this file (40 lines)").
- [x] FR-SRC-06 Limits. Evidence: `repo/tests/text.rs` (2 MB boundary, NUL sniff, lossy); `desktop/tests/browse.rs`::`large_and_binary_files_show_a_placeholder_and_invalid_utf8_a_banner`.
- [x] FR-SRC-07 Copy. Evidence: `desktop/tests/browse.rs`::`copy_path_line_and_cited_lines` (clipboard `src/api.rs`, `src/api.rs:10`, lines 10–12; disabled without a band).
- [x] FR-SRC-08 Confinement. Evidence: `repo/tests/text.rs`::`reads_go_through_the_sandbox`; `desktop/tests/browse.rs`::`reads_are_confined_to_the_root` (symlink to an outside file → error, contents never shown).
- [x] FR-VAL-06 Citation checks. Evidence: `model/tests/citations.rs` (positive + negative per rule), `repo/tests/citations.rs`::`repo_probe_end_to_end`; `desktop/tests/shell.rs`::`opened_diagrams_carry_citation_and_dirty_warnings`.
- [x] FR-VAL-07 Dirty flags. Evidence: `model/tests/citations.rs::dirty_cited_path_is_a_warning_under_both_profiles`; `desktop/tests/shell.rs`::`opened_diagrams_carry_citation_and_dirty_warnings`.
- [x] FR-SEC-01 Root confinement. Evidence: `repo/tests/sandbox.rs`: `dot_dot_escape_is_rejected`, `absolute_path_is_rejected`, `symlink_to_file_outside_is_rejected`, `symlink_to_folder_outside_is_rejected`, `sibling_with_shared_prefix_is_outside`.
- [x] FR-SEC-07 Origin sanitization. Evidence: `repo/tests/git.rs`: `sanitize_strips_https_userinfo_and_keeps_the_rest`, `sanitize_leaves_ssh_and_scp_style_untouched`, `origin_with_credentials_is_sanitized`.
- [x] FR-SEC-08 Git invocation. Evidence: `repo/tests/git.rs`::`read_commands_are_fixed_argv_with_no_optional_locks_first` (`std::process::Command` with fixed argv, no shell).
- [x] NFR-PERF-01 Top-level listing of 50,000 files < 300 ms. Evidence: `perf_top_level_listing_50k_files`: median 0.46 ms.
- [x] NFR-PERF-02 Opening a 2 MB file < 300 ms. Evidence: `desktop/tests/perf.rs::perf_open_2mb_file_read_index_and_highlight_first_window`: median 6.2 ms (read + index + highlight 80 lines); `repo` read + index alone: 2.6 ms.

## M4 Diagram and navigation

- [x] FR-DIA-01 Rendering. Evidence: `diagram::logic::tests::legend_modes_and_overrides` (auto/all/hidden, label override, `visible: false`); `[manual]` part reviewed from `desktop/tests/diagram.rs`::`fixture_screenshots_for_visual_review` → `target/smoke/m4/{web-app,production-deployment,archify-repo-grid,archify-runtime}.png` (boundaries, variants, labels, legend, cards strip); the owner's manual check is pending.
- [x] FR-DIA-02 Camera. Evidence: `diagram::camera::tests` (`zoom_about_keeps_the_anchor_fixed`, `zoom_is_clamped_to_10_and_400_percent`, `fit_centers_with_40px_padding`, `round_trip`); `desktop/tests/diagram.rs`: `opening_fits_the_diagram_with_padding`, `zoom_and_pan_inputs_work_end_to_end` (keys need focus; +/−/0/F; ⌘+wheel about the cursor; wheel and drag pan).
- [x] FR-DIA-03 Node click. Evidence: `desktop/tests/diagram.rs`: `clicking_a_node_opens_its_first_citation_and_reveals_the_file` (click by `Point`, band 3–3 and 10–20, ancestors expanded, row selected), `a_node_without_sources_is_only_selected` ("No source"), `strip_sources_open_their_citation`.
- [x] FR-DIA-04 Hover. Evidence: `desktop/tests/diagram.rs`::`hovering_a_node_shows_a_tooltip_after_400ms` (≥ 400 ms, label/sublabel/tag/sources, hides on leave).
- [x] FR-DIA-05 Reach. Evidence: `diagram::logic::tests::{downstream_reach_follows_edges_and_handles_cycles, upstream_reach_follows_edges_backwards}`; `desktop/tests/diagram.rs`::`reach_keys_and_buttons_toggle_and_replace`.
- [x] FR-DIA-06 Edge click. Evidence: `desktop/tests/diagram.rs`::`clicking_a_connection_selects_it` ("ui → api · calls · default", endpoints outlined).
- [x] FR-DIA-07 Finder. Evidence: `logic::tests::finder_matches_labels_and_ids_case_insensitively_up_to_ten`; `desktop/tests/diagram.rs`::`finder_searches_moves_and_picks` (⌘F and `/`, ↑/↓, Enter centers and clicks).
- [x] FR-DIA-08 Esc. Evidence: `desktop/tests/diagram.rs`: `finder_searches_moves_and_picks` (first Esc closes the finder only), `escape_clears_reach_selection_and_reverse_highlight`.
- [x] FR-DIA-09 Problems. Evidence: `logic::tests::badges_come_from_citation_warnings`; `desktop/tests/diagram.rs`::`problems_list_and_badges` (broken and modified badges, Problems (n) list).
- [x] FR-DIA-11 Re-layout. Evidence: `desktop/tests/diagram.rs`::`opening_never_writes_and_relayout_writes_integer_geometry_only` (title-bar button and ⌘⇧L; integer pos/size; non-geometry fields equal; also the app-level FR-LAY-08 check: opening leaves the bytes unchanged).
- [x] FR-TREE-06 Citation markers. Evidence: `repo/tests/citations.rs::citation_index_maps_files_folders_and_broken_markers`; `desktop/tests/diagram.rs`::`tree_markers_show_cited_files_folder_counts_and_broken_files` (● dot, folder count, ⚠).
- [x] FR-TREE-07 Reverse highlight. Evidence: `desktop/tests/diagram.rs`: `selecting_a_file_highlights_the_nodes_citing_it` (two citing nodes; another file clears), `escape_clears_reach_selection_and_reverse_highlight`.
- [x] NFR-PERF-04 Canvas geometry for 100 nodes < 8 ms. Evidence: `desktop/tests/perf.rs::perf_canvas_geometry_for_100_nodes_under_8ms`: median 0.40 ms (both layers, 150 edges, reach + selection + reverse + hover active).

## M5 Agent

- [x] FR-GEN-01 Preconditions. Evidence: `desktop/tests/agent.rs::{a_missing_key_shows_a_banner_and_makes_no_call, missing_consent_shows_the_modal_and_makes_no_call, an_uncreatable_diagram_folder_shows_a_banner_and_makes_no_call}`. The client is built before the folder is created; an unknown provider prefix is also a banner (`App::archify`).
- [x] FR-GEN-02 Focus. Evidence: `desktop/tests/agent.rs::a_successful_run_saves_logs_and_opens_the_diagram` (the typed Focus is in the seed and in `run_started`, and drives the folder slug).
- [x] FR-GEN-03 Seed message. Evidence: `agent/tests/tools.rs::{seed_contains_focus_tree_readme_and_manifests_but_no_denied_files, seed_is_capped_at_48kb}`; through the UI in `a_successful_run_saves_logs_and_opens_the_diagram`.
- [x] FR-GEN-04 Tools. Evidence: `agent/tests/tools.rs` (11 tests: `list_dir`, `read_file` 400-line/24 KB caps, `grep` 100-match cap and bad patterns, `repo_meta` 200 dirty paths, unknown tools, the 24 KB cap on every result); `repo/tests/agent_fs.rs` (16 tests).
- [x] FR-GEN-05 Loop. Evidence: `agent/tests/run.rs::{success_saves_the_candidate_and_logs_every_event_in_order, a_text_only_reply_gets_a_reminder_and_counts_as_a_turn, invalid_tool_arguments_cost_a_turn_but_not_a_repair}`.
- [x] FR-GEN-06 Gate and repair. Evidence: `agent/tests/run.rs::{rejections_return_errors_to_the_model_and_repairs_can_succeed, the_fourth_rejection_fails_the_run}`; `desktop/tests/agent.rs::a_failed_run_shows_the_reason_and_saves_only_the_log`.
- [x] FR-GEN-07 Budgets. Evidence: `agent/tests/run.rs::{the_turn_cap_fails_the_run, the_input_token_cap_fails_the_run, the_wall_clock_cap_fails_the_run, the_per_request_timeout_is_retried_then_fails_the_run}`.
- [x] FR-GEN-08 Retries. Evidence: `agent/src/retry.rs` tests (`retries_429_5xx_and_timeouts_with_1_2_4_backoff`, `honors_retry_after`, `gives_up_after_three_retries`, `does_not_retry_client_errors`, `an_elapsed_attempt_is_a_retryable_timeout`); `agent/tests/run.rs::http_429_with_retry_after_waits_that_long_and_continues`; `Retry-After` header for OpenAI in `agent/src/openai.rs::complete_posts_to_responses_and_reads_retry_after`; body hints for litellm providers (ADR 0033).
- [x] FR-GEN-09 Progress. Evidence: `desktop/tests/agent.rs::a_successful_run_saves_logs_and_opens_the_diagram` (drawer lines in event order); status-bar progress in `cancel_stops_the_run_and_the_app_stays_usable_meanwhile`. Screenshots reviewed: `target/smoke/m5/{run-in-progress,run-success}.png`.
- [x] FR-GEN-10 Cancel. Evidence: `desktop/tests/agent.rs::cancel_stops_the_run_and_the_app_stays_usable_meanwhile` (Cancel button and ⌘.; idle in < 1 s; the stream is dropped; `run.jsonl` ends `cancelled`; no candidate); `agent/tests/run.rs::cancelling_stops_within_a_second_and_saves_nothing`.
- [x] FR-GEN-11 Concurrency. Evidence: `cancel_stops_the_run_and_the_app_stays_usable_meanwhile` (tree and source work during the run; Open repo disabled; Archify becomes Cancel, so a second run can't start).
- [x] FR-GEN-12 Success. Opens automatically and is first in the rescanned History list. Evidence: `a_successful_run_saves_logs_and_opens_the_diagram`. The History picker UI itself is M6.
- [x] FR-SEC-02 Agent deny-list. Evidence: `repo/tests/deny.rs` (8 tests: every pattern, case-insensitivity, any depth, links to denied targets); `agent/tests/tools.rs::deny_listed_paths_are_hidden_from_every_tool` (every pattern × every tool; direct reads return `denied` and are logged).
- [x] FR-SEC-03 Consent. Evidence: `desktop/tests/agent.rs::{missing_consent_shows_the_modal_and_makes_no_call, consent_accept_decline_and_revoke}`. Screenshot reviewed: `target/smoke/m5/consent.png`. Persisting consents to `state.json` and the Settings Revoke button are M6 (FR-STO-05/06); `Message::RevokeConsent` is in place.
- [x] FR-SEC-04 API key. Evidence: `desktop/src/keys.rs` unit tests (`lookup_order_is_environment_then_launch_dir_then_config_dir`, `not_found_when_no_source_has_the_key`, `the_key_is_redacted_in_debug_output`; `.env` files are read with `dotenvy::from_path_iter`, which never sets variables); `desktop/tests/agent.rs::the_key_never_appears_in_the_run_log_or_tracing_output` (TRACE-level capture); `agent/src/openai.rs::debug_never_shows_the_key`.
- [x] FR-SEC-05 Network. Implementer's review (the owner's `[manual]` review is still due): `cargo tree --workspace -e normal` has one HTTP stack, `reqwest` 0.12.28 → `hyper` 1.12 / `hyper-rustls`, used only by `archalign-agent` (the Responses adapter, ADR 0034) and `litellm-rust`. No native-tls/openssl, no telemetry, updater or crash-reporting crates. Tests can't reach the network: the harness injects an empty key environment and a refusing LLM factory.
- [x] FR-SEC-06 Logging. Evidence: `agent/tests/run.rs::the_key_and_file_contents_never_reach_the_run_log`; `the_key_never_appears_in_the_run_log_or_tracing_output`; live `run.jsonl` files contain no file contents, encrypted reasoning or key (checked on the HyperTrack run: 7.4 KB, events only).
- [x] FR-STO-01 Diagram folder. Evidence: `repo/tests/run_storage.rs` (slug, timestamp, collisions, unusable `.archify`); the folder name pattern in `a_successful_run_saves_logs_and_opens_the_diagram`.
- [x] FR-STO-02 Folder contents. Evidence: `a_successful_run_saves_logs_and_opens_the_diagram` (`candidate.json` + `run.jsonl`; `meta.output` points at the never-written `<slug>.html`); `a_failed_run_shows_the_reason_and_saves_only_the_log`.
- [x] FR-STO-03 Open-file compatibility, `[unit]` part. Evidence: `a_successful_run_saves_logs_and_opens_the_diagram` (the saved file passes the vendored schema; every component has `pos`/`size`); also checked on both live candidates. The `[live]` Archify CLI check is in M7.
- [x] Live development runs: Luna on `.smoke_repo/HyperTrack` and `.smoke_repo/cs2103-ip`. Count used: 3 / 20.
  1. HyperTrack: failed on turn 1 with HTTP 400 (gpt-6 rejects function tools on Chat Completions). This led to ADR 0034, chosen by the owner.
  2. HyperTrack: success in 43 s, 6 turns, 73.8k input / 3.7k output tokens, 11 components. 3 node clicks opened the right file and band.
  3. cs2103-ip: success in 36 s, 7 turns, 38.0k input / 2.8k output tokens, 7 components. 3 node clicks verified.

  Screenshots reviewed: `target/smoke/{HyperTrack,cs2103-ip}/{diagram,node-1,node-2,node-3}.png`. Command: `ARCHALIGN_LIVE=1 ARCHALIGN_LIVE_REPO=… cargo test -p archalign-desktop --release --test live -- --ignored --nocapture` (one run per invocation).

## M6 History, settings, state, watcher

- [x] FR-STO-04 History. Evidence: `repo/tests/storage.rs::{history_is_newest_first_and_hides_folders_without_candidate, history_ignores_gitignore_and_skips_escaping_links}` `[unit]`; `desktop/tests/state.rs::history_lists_newest_first_with_title_time_and_status_and_opens_entries` `[sim]`. Screenshot reviewed: `target/smoke/m6/history.png`.
- [x] FR-STO-05 App state and settings. Evidence: `desktop/src/persist.rs` tests (`state_round_trips_with_a_customized_layout`, `the_saved_json_has_the_documented_shape`, `missing_unreadable_and_invalid_files_fall_back_with_a_problem`, `settings_round_trip_and_missing_fields_take_defaults`, `recents_are_capped_and_tiny_windows_are_ignored`); `desktop/tests/state.rs::{state_survives_a_restart_layout_theme_window_recents_and_consents, an_invalid_state_file_falls_back_with_a_banner}`. A missing file (first launch) shows an Info banner and `settings.json` is created; an unreadable or invalid one shows a Warning and isn't overwritten until the next save.
- [x] FR-STO-06 Settings modal. Evidence: `desktop/tests/state.rs::{settings_edits_persist_never_show_the_key_and_apply_to_the_next_run, show_hidden_files_from_settings_relists_the_tree}` (key source shown as "OPENAI_API_KEY — environment", the key text is never rendered, consents listed with Revoke). Screenshot reviewed: `target/smoke/m6/settings.png`.
- [x] FR-PANE-08 Layout persistence. Evidence: `persist.rs::{state_round_trips_with_a_customized_layout, an_invalid_layout_falls_back_to_the_default_layout_only}` (the invalid case logs `tracing::warn!`); `state.rs::{state_survives_a_restart_layout_theme_window_recents_and_consents, state_saves_are_rate_limited_to_once_per_second}`.
- [x] FR-DIA-10 Theme. Evidence: `state.rs::state_survives_a_restart_layout_theme_window_recents_and_consents` (toggle → restart → light).
- [x] FR-GEN-13 Model settings. Evidence: `settings.rs::draft_tests::a_draft_round_trips_and_validates`; `state.rs::settings_edits_persist_never_show_the_key_and_apply_to_the_next_run` (the next run uses the new model, `reasoning_effort` "high" and 7 max turns).
- [x] FR-WAT-01 Lifecycle. Evidence: `repo/tests/watch.rs::real_watcher_reports_changes_and_stops_on_drop`; `desktop/tests/watching.rs::the_subscription_stream_starts_a_watcher_and_delivers_batches`. The subscription is keyed by repo generation, so iced drops the stream, and with it the watcher, when the repo closes or changes.
- [x] FR-WAT-02 Debounce and filtering. Evidence: `repo/tests/watch.rs::{outside_paths_are_dropped, git_dir_keeps_only_head_and_index_as_git_flag, archify_is_kept_even_when_gitignored_and_hidden, gitignored_paths_are_dropped, hidden_paths_follow_show_hidden, deleted_paths_are_kept_unless_ignored, non_canonical_aliases_of_the_root_are_accepted}`; `DEBOUNCE` = 300 ms.
- [x] FR-WAT-03 Tree. Evidence: `watching.rs::tree_relists_only_affected_expanded_folders_and_keeps_state`.
- [x] FR-WAT-04 Open file. Evidence: `watching.rs::the_open_file_reloads_keeping_scroll_and_band_and_reports_deletion`. Screenshot reviewed: `target/smoke/m6/reloaded.png`.
- [x] FR-WAT-05 Citations. Evidence: `watching.rs::citations_of_changed_files_are_rechecked` (badge and tree marker update; camera and selection kept).
- [x] FR-WAT-06 Diagram files. Evidence: `watching.rs::diagram_files_refresh_history_and_reload_the_open_diagram`.
- [x] FR-WAT-07 Bursts. Evidence: `repo/tests/watch.rs::burst_above_threshold_is_full_refresh` (500 → changes, 501 → full); `watching.rs::a_burst_triggers_one_full_refresh`.
- [x] FR-WAT-08 Self-writes. Evidence: `repo/tests/watch.rs::self_writes_are_dropped_but_later_external_changes_are_kept`; `repo/tests/run_storage.rs::{atomic_write_tracked_writes_and_records, self_writes_reject_later_external_changes}`; `watching.rs::the_apps_own_relayout_write_is_not_reloaded`. Run results and Re-layout both write through `atomic_write_tracked`.
- [x] FR-WAT-09 Git refresh. Evidence: `repo/tests/watch.rs::git_refresh_limiter_spaces_and_coalesces`; `watching.rs::git_refreshes_are_rate_limited_to_one_per_two_seconds` (real git; the first change refreshes at once and updates the "modified" badge; later ones coalesce into one refresh after 2 s).
- [x] FR-WAT-10 Fallback. Evidence: `watching.rs::cmd_r_refreshes_everything_without_a_watcher` (status bar "Manual refresh only" until the watcher starts; ⌘R re-lists the tree, reloads the open file, re-checks citations, rescans History and refreshes git).

## M7 Finish

- [x] NFR-QUAL-04: audit that user data never causes a panic (repositories, diagram files, state files). A read-only reviewer subagent traced user input to panic sites in all `src/` code. Its 7 findings are fixed, each with a regression test:
  1. `pos`/`size`/`pad`/`via` beyond `f32` range made NaN canvas paths, which panic in iced_wgpu's tessellator. Coordinates outside ±1,000,000 (`archalign_layout::geom::MAX_COORD`) or non-finite now count as missing. Test: `layout/tests/layout.rs::out_of_range_geometry_is_ignored_not_fatal`.
  2. Infinite obstacle edges indexed past the router grid. Non-finite obstacles are now skipped and the indices clamped. Same test.
  3. A folder or FIFO named `.env` (a Python virtualenv) made the key lookup loop or block on the UI thread. Only regular files are read now, and lookup stops at the first I/O error. Test: `desktop/src/keys.rs::a_folder_named_env_is_skipped`.
  4. Revealing a citation under a folder that can't be listed re-listed it forever. The reveal is now dropped. Test: `desktop/tests/browse.rs::revealing_under_a_missing_folder_stops_after_one_error`.
  5. A huge saved window height overflowed `virtual_list` arithmetic in debug builds. The arithmetic now saturates, and saved window sizes are capped at 16,384. Test: `virtual_list.rs::huge_viewports_saturate_instead_of_overflowing`.
  6. `max_repairs` = `u32::MAX` overflowed. Now saturating. Test: `agent/src/gate.rs::rejection_text_saturates_at_the_largest_repair_budget`.
  7. Huge LLM usage counts overflowed the token totals. They now saturate (`run.rs`, `runlog.rs`).

  Sites checked and judged safe are listed in the audit output (schema `expect` on embedded constants, ASCII-only slicing, guarded indices, `catch_unwind` around dugong, serde's recursion limit).
- [x] All NFR-PERF tests pass: `cargo test --workspace --release -- --ignored perf_`. NFR-PERF-01 listing 0.48 ms; NFR-PERF-02 open 2 MB 0.88 ms (repo read + index) and 4.6 ms (with the first highlighted window); NFR-PERF-03 layout + routing of 60 nodes 31.6 ms (two placements since ADR 0035; 14 ms before); NFR-PERF-04 canvas geometry 0.43 ms.
- [x] Final live smoke with Sol. Every repo passed on the first attempt; no attempt failed.

  | Repo | Attempts | Components / connections | Turns | Time | Input / output tokens |
  |---|---|---|---|---|---|
  | HyperTrack | 1 / 3 | 15 / 24 | 7 | 61 s | 121,157 / 3,353 |
  | cs2103-ip | 1 / 3 | 8 / 14 | 7 | 42 s | 40,318 / 1,899 |
  | `target/smoke/archify-src` copy | 1 / 3 | 11 / 18 | 8 | 71 s | 199,673 / 3,475 |

  Each candidate passed the gate and was saved with `pos`/`size` and `run.jsonl`. A fresh Simulator reopened it as the newest valid diagram and clicked 3 nodes; path and band matched each time (`live.rs::live_archify_one_repo`). After ADR 0035, the HyperTrack and cs2103-ip diagrams were re-laid out without an LLM call and the clicks were re-verified (`live.rs::smoke_relayout_newest`).
- [x] FR-STO-03 `[live]`: Archify CLI check on one generated `candidate.json`. Results:
  - `node archify-src/archify/bin/archify.mjs validate architecture <HyperTrack candidate> --json`: `ok: false` at stage `render`, "This diagram declares source evidence. Pass --repo-root <repository>…".
  - With `--repo-root .smoke_repo/HyperTrack`: schema, render and every artifact check pass (single_svg, finite_svg, orthogonal_arrows, label_route_clearance, relationship_crossings, relationship_corridors, container_border_runs, route_rhythm, legend_clearance). It still ends `ok: false` on `composition/desktop-readability`, because the viewBox is 1,897 px wide and 8 px edge labels project to 3.9 px (minimum 6).
  - The same check gives cs2103-ip 1,390 px / 5.4 px and archify-src 2,298 px / 4.7 px.
  - Raw output: `target/smoke/cli-validate-*.json`.
- [x] Simulator screenshots saved under `target/smoke/<repo>/` and reviewed: `target/smoke/HyperTrack/{diagram,node-1,node-2,node-3}.png`, `target/smoke/cs2103-ip/{diagram,node-1,node-2,node-3}.png`, `target/smoke/archify-src/.archalign-smoke/{diagram,node-1,node-2,node-3}.png`. The copy is itself the repo, so its screenshots sit in a hidden subfolder.
- [x] `libraries.md`, the ADRs and `architecture.md` match the code. Every `Cargo.toml` dependency is listed with its real users (`tracing` and `tempfile` rows corrected); ADRs 0033–0035 record the deviations.
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
- **2026-10-10. M2 Layout and routing.**
  - `archalign-layout`: `TextMeasure` + `FixedAdvance`, mode selection and Archify grid conversion, boundary nesting and padding, `DugongEngine` (compound, LR) with a grid fallback, one sparse-grid A* orthogonal router for authored and auto layouts, `Scene`, `write_back`.
  - ADR 0031 supersedes 0018. Reason: saved diagrams always reopen as authored (ADR 0019), so routing dagre bend points would only affect the first view. A bend-aware A* heuristic brought routing for 60 nodes from 122 ms to about 6 ms.
  - Property testing found that many self-loops on one node could reach the next rank. Loop nesting is now capped at 3.
  - Visual check of all 24 fixtures (authored and auto), rendered to throwaway SVGs under `target/smoke/layout-svg/`.
  - Gates: fmt, clippy `-D warnings`, `cargo test --workspace` all green; perf: 13.4 ms median.
- **2026-10-10. M3 App shell.**
  - `archalign-repo` (built by a subagent against a fixed contract, then integrated): `Sandbox`/`RelPath`, ignore-aware `tree`, `text` (2 MB cap, binary sniff, lossy, `LineIndex`), `git` (fixed argv, sanitized origin), `RepoProbe`, `storage::scan_history`/`atomic_write`. `archalign-model::citations` (FR-VAL-06/07).
  - `archalign-desktop`: iced app with toolbar, Panels menu, welcome screen, pane grid, status bar, banners, virtualized tree and source viewer (syntect with checkpoints), and auto-opening of the newest valid diagram (canvas in M4).
  - ADR 0032: a UI harness built from `iced_test` parts. A bare `Simulator` loses scroll state and doesn't run tasks; evidence in the ADR. Added `png` 0.18 as a dev-dependency to save screenshots (`libraries.md`).
  - Screenshots reviewed: `target/smoke/m3/{open,default-layout,cited-range,outside-link-tooltip,status-bar,empty-state,after-recent}.png`.
  - Gates: fmt, clippy `-D warnings`, `cargo test --workspace` all green; perf: NFR-PERF-01 0.46 ms, NFR-PERF-02 6.2 ms, NFR-PERF-03 14.1 ms.
- **2026-10-10. M4 Diagram and navigation.**
  - Canvas (`diagram::canvas`) with a cached static layer and an overlay layer, camera math, hit testing, hover tooltip (pinned widget, 400 ms task), reach, finder, Esc, problems/badges, cards strip, Re-layout with atomic save, tree markers and reverse highlight (`CitationIndex` in `archalign-repo`).
  - Keyboard path changed to `event::listen_with` (keys carry their capture status) so Esc can close the finder even when the field captured it; the harness forwards the same status.
  - Screenshots reviewed: `target/smoke/m4/{web-app,production-deployment,archify-repo-grid,archify-runtime,node-click,tooltip,reach,edge-click,problems,tree-markers,reverse-highlight}.png`.
  - Gates: fmt, clippy `-D warnings`, `cargo test --workspace` green; perf: NFR-PERF-04 0.40 ms (others unchanged).
- **2026-10-10. M5 Agent.**
  - `archalign-agent`: the `LlmClient` seam (boxed future), retry wrapper with an injectable sleeper, tools over `agent_fs`, prompt and seed, submission gate (model checks + citations through `RepoProbe`), the loop with all budgets, `RunLog` (`cancelled` on drop). `archalign-repo`: `DenyList`, `agent_fs`, run-folder storage, `SelfWrites`.
  - `archalign-desktop`: `settings`, `keys`, `agent_run`; the Archify flow with preconditions, consent modal, run-log drawer, 1 s tick, Cancel/⌘., and success opening the new diagram.
  - Live run 1 failed: gpt-6 models reject function tools on `/v1/chat/completions` unless `reasoning_effort` is `none`. The owner chose a Responses API adapter for `openai/` (ADR 0034). litellm-rust drops response headers, so `Retry-After` comes from error bodies for the other providers (ADR 0033).
  - Harness: pending task streams are parked and pumped (`pump`, `wait_until`), so long runs and Cancel can be simulated. It never reads the real environment or `.env`, and never reaches the network.
  - Finding for M7: both live candidates fail Archify's `composition/desktop-readability` check (viewBox 2903 px wide for HyperTrack; projected 8 px edge text at 3.7 px). The schema and render stages pass. In the app, the fitted HyperTrack diagram is too small for labels in the default pane width.
  - Gates: fmt, clippy `-D warnings`, `cargo test --workspace` green.
- **2026-10-10. M6 History, settings, state, watcher.**
  - `archalign-repo::watch` (written by a subagent against a fixed contract, then integrated): `classify`, `RepoWatcher` (notify-debouncer-mini 300 ms, FSEvents), `GitRefreshLimiter`.
  - `archalign-desktop`: `persist` (`state.json`/`settings.json` via `directories`, atomic writes, serde mirror of the pane layout, a 1 s save rate limit driven by state diffs after every update), History strip, Settings modal, the watcher subscription (`watch`), reload and recheck paths, and ⌘R.
  - Fixed a latent panic: a `tokio::time::sleep` created in `update()` (outside any runtime) panics, so delayed tasks now build their timer inside the async block.
  - The harness keeps a restored window size and offers `watch(paths)`, which runs real file changes through the real filter.
  - Screenshots reviewed: `target/smoke/m6/{history,settings,reloaded}.png`.
  - Gates: fmt, clippy `-D warnings`, `cargo test --workspace` green.
- **2026-10-10. M7 Finish.**
  - Final Sol smoke: HyperTrack, cs2103-ip and the `archify-src` copy each succeeded on the first of 3 attempts; 3 node clicks verified per repo after reopening in a fresh Simulator.
  - The first Sol diagram (15 components ranked left to right, 4,000 px wide) showed no labels when fitted, and Archify's readability check failed. The owner chose automatic rank direction (ADR 0035; FR-LAY-04 amended). 5 fixture snapshots changed and were reviewed (`target/smoke/m7/auto-*.png`); both `.smoke_repo` diagrams were re-laid out and re-verified without new LLM calls.
  - NFR-QUAL-04 audit (reviewer subagent): 7 findings fixed with regression tests. The reveal loop was reproduced before the fix: the test hung until killed.
  - Archify CLI: the plain command asks for `--repo-root`. With it, schema, render and all artifact checks pass and `composition/desktop-readability` fails (recorded under M7).
  - Docs: `libraries.md` used-by rows corrected; `architecture.md` and the ADR index updated.
  - Gates: fmt, clippy `-D warnings`, `cargo test --workspace`, and release `perf_` all green.
