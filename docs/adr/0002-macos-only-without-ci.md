# 0002. macOS only, without CI

- Status: Accepted
- Date: 2026-10-10
- Related: [0003](0003-cargo-workspace-and-crate-boundaries.md), [0017](0017-file-watching.md), [0029](0029-testing-strategy-and-live-smoke-runs.md); spec: NFR-QUAL

## Context

ArchAlign is a native desktop app developed on a single macOS machine. Supporting and verifying more platforms costs build infrastructure and testing effort. The owner dropped CI.

## Decision

- v1 is supported and verified only on macOS on Apple silicon. The development machine runs macOS 26 (Darwin 25.6, arm64).
- There is no CI. Quality gates run locally:
  - `cargo fmt --check`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace`
  - release-mode performance tests
- Platform-specific code goes only behind `cfg`.
- Make no claims about Linux or Windows support.

## Alternatives considered

### GitHub Actions matrix for macOS, Linux and Windows

Rejected because the owner dropped CI.

### Claiming cross-platform support via `cargo check --target ...` from macOS

Unreliable. The TLS stack under reqwest compiles C/asm for the target and needs cross toolchains [inference]. A check that compiles is also not a verified build.

## Consequences

- Linux and Windows builds can break unnoticed.
- The main dependencies (iced/winit/wgpu, rfd, notify) are cross-platform, so a later port is plausible [inference].
- macOS-specific behavior is acceptable: FSEvents file watching, the native open panel through rfd, and ⌘ shortcuts.
- The local gates must be run by hand before work is considered done.

## References

- [`../spec.md`](../spec.md) (NFR-QUAL)
- [`../libraries.md`](../libraries.md)
