# 0017. File watching

- Status: Accepted
- Date: 2026-10-10
- Related: [0002](0002-macos-only-without-ci.md), [0008](0008-async-model-tokio-and-iced-tasks.md), [0015](0015-diagram-storage-app-state-and-run-log.md), [0016](0016-local-folder-import-and-git-metadata-via-cli.md); spec: FR-WAT

## Context

The owner asked for the app to follow changes in the imported repository on disk. The file tree, the open source file, citation badges, diagram history, the open `candidate.json` and git status can all go stale when files change outside the app. Watching lives in `archalign-repo`, and events reach the UI as described in [0008](0008-async-model-tokio-and-iced-tasks.md).

## Decision

- Use `notify-debouncer-mini` 0.7 on stable `notify` 8.2 (FSEvents on macOS). Watch the repo root recursively with a 300 ms debounce.
- **Filtering.**
  - Pass events through the same ignore rules as the file tree.
  - Always watch `.archify/`.
  - Under `.git/`, count only `HEAD` and `index` changes. They trigger a git-metadata refresh.
- **Reactions.**
  - Tree: re-list only the affected folders that are expanded. Keep expansion, selection and scroll.
  - Open file: reload it and keep scroll and the cited-range band. If it was deleted, show a banner.
  - Citations: re-check those pointing at changed paths, so badges update.
  - History: refresh it.
  - Open `candidate.json`: if it changes on disk, reload it. If the new version is invalid, keep the last good one and show a banner.
  - Git status: refresh it, at most once every 2 s.
- If one batch holds more than 500 paths, do a single full refresh instead.
- Ignore the app's own writes. Record each as (path, len, mtime) and drop matching events.
- If the watcher fails to start, warn in the status bar. ⌘R manual refresh always works.

## Alternatives considered

### Manual refresh only

Rejected. The owner asked for a watcher. Manual refresh stays as a fallback.

### Raw `notify` without a debouncer

Rejected. It produces event storms.

### `notify-debouncer-full`

Rejected. It adds rename tracking and a file-ID cache that ArchAlign doesn't need.

### Polling

Rejected. Wasteful.

## Consequences

- Positive: views stay current without user action; the app does not react to its own writes.
- Positive: large batches degrade to one full refresh instead of many partial ones.
- Negative: behavior depends on FSEvents; other platforms are out of scope, so Linux inotify limits are irrelevant ([0002](0002-macos-only-without-ci.md)).
- Follow-up: a possible upgrade to notify 9 later (currently 9.0.0-rc.5).

## References

- [../spec.md](../spec.md) (FR-WAT)
- [../libraries.md](../libraries.md) (`notify`, `notify-debouncer-mini`)
- [../architecture.md](../architecture.md)
