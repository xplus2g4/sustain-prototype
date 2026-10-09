# 0016. Local-folder import and git metadata via the git CLI

- Status: Accepted
- Date: 2026-10-10
- Related: [0003](0003-cargo-workspace-and-crate-boundaries.md), [0008](0008-async-model-tokio-and-iced-tasks.md), [0012](0012-archify-json-v1-canonical-format.md), [0014](0014-schema-validation-with-vendored-archify-schemas.md), [0017](0017-file-watching.md); spec: FR-IMP, FR-TREE, FR-SEC

## Context

Owner goal 1 is to import a repository and archify it. Archify JSON can carry `meta.repository` (`url`, `revision`), and the schema requires both a `url` and a 40-hex `revision` when present ([0014](0014-schema-validation-with-vendored-archify-schemas.md)). The tree also shows "modified" badges from git status. `archalign-repo` owns the filesystem and git boundary ([0003](0003-cargo-workspace-and-crate-boundaries.md)).

## Decision

**Import.** The user picks a local folder with the native picker (`rfd`). The repo root is that folder, canonicalized. Remote URLs are not cloned.

**Git metadata.** Git is optional. Collect metadata only when `git` is on PATH and the folder is the top level of a work tree:

- HEAD (40 hex characters);
- the origin URL with HTTP(S) userinfo stripped (transport, port, path and `.git` suffix kept);
- dirty paths from `git --no-optional-locks status --porcelain=v1 -z`.

A folder inside a work tree but not at its top, or a folder without git, is treated as non-git and gets no `meta.repository`.

**`meta.repository`.**

- Contains `url` and `revision`.
- `provider` is `github`/`gitee` when the origin is an HTTPS GitHub or Gitee URL; otherwise `link_mode: "local-only"`.

**Execution.** Git commands run as fixed argument vectors, never through a shell. They run off the UI thread ([0008](0008-async-model-tokio-and-iced-tasks.md)).

## Alternatives considered

### `git2` (libgit2)

Rejected: a C dependency and a more complex build.

### `gix`

Rejected: a large dependency tree [inference].

### Clone support

Rejected: extra scope plus credential handling.

## Consequences

- Without git there is no repository metadata and no "modified" badges.
- Behavior depends on the user's git install.
- Credentials in origin URLs do not leak into diagrams.
- Git metadata refreshes on `.git/HEAD` and `.git/index` changes ([0017](0017-file-watching.md)).

## References

- [../libraries.md](../libraries.md) (`rfd`), [../spec.md](../spec.md), [../architecture.md](../architecture.md)
