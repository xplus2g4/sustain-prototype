# 0025. Agent tool sandbox and deny-list

- Status: Accepted
- Date: 2026-10-10
- Related: [0003](0003-cargo-workspace-and-crate-boundaries.md), [0020](0020-built-in-llm-agent-authors-diagrams.md), [0024](0024-data-egress-consent-and-network-policy.md), [0027](0027-submission-gate-and-repair-loop.md), [0028](0028-agent-run-budgets.md); spec: FR-SEC, FR-GEN

## Context

The agent explores the repository through tools. Whatever a tool returns goes to the LLM provider ([0024](0024-data-egress-consent-and-network-policy.md)). The tools must therefore be confined to the repository root, keep secrets out of the conversation, and give the model no way to change the machine. The tools run over the `Sandbox` in `archalign-repo` ([0003](0003-cargo-workspace-and-crate-boundaries.md)).

## Decision

### Tools (all read-only)

| Tool | Behavior |
|---|---|
| `list_dir(path)` | Lists a directory. |
| `read_file(path, start_line, end_line)` | Line-numbered output, ≤400 lines per call. |
| `grep(pattern, path_glob)` | Rust `regex` syntax (linear-time, safe for model-supplied patterns); ≤100 matches; skips binaries and files over 1 MB; ≤5 s. |
| `repo_meta()` | Repository metadata. |
| `submit_diagram(diagram)` | Submits a candidate to the gate ([0027](0027-submission-gate-and-repair-loop.md)). |

Never available: shell, writes, or network tools.

### Confinement

- Paths are repo-relative.
- Reject absolute paths, `..` escapes, and symlinks that resolve outside the root. Canonicalize, then check the prefix.
- Respect `.gitignore` via the `ignore` crate.
- Cap any tool result at 24 KB, with a truncation notice.
- UI reads use the same confinement, so a crafted `candidate.json` can't open files outside the root.

### Deny-list

Fixed in v1. Matching entries are omitted from listings and grep results, and a direct read returns `denied`.

- `.git/` and `.archify/` (the latter stops the model copying an old diagram)
- `.env` and `.env.*`, except `.env.example` / `.env.sample`
- `*.pem`, `*.key`, `*.p12`, `*.pfx`
- `id_rsa*`, `id_ed25519*`
- `.npmrc`, `.pypirc`, `.netrc`

### First message

Seed the conversation with a depth-2 tree (capped), the start of the README and the top-level build manifests. This saves turns.

### Prompt-injection posture

Repository content is untrusted data. With read-only, confined tools, the worst case is a wrong diagram or wasted budget ([0028](0028-agent-run-budgets.md)).

## Alternatives considered

### Shell access

Rejected. Unsafe: the model could run arbitrary commands.

### Unrestricted file reads

Rejected. A privacy problem: anything on disk could reach the provider.

### No deny-list

Rejected. Secrets could leak to the provider.

## Consequences

- Positive: the agent can't see secrets or ignored files.
- Positive: prompt injection can't escalate beyond a wrong diagram or wasted budget.
- Negative: architecture that lives only in ignored, generated code is missed. This is accepted.
- Negative: the deny-list is not user-configurable in v1.

## References

- [../spec.md](../spec.md) (FR-SEC)
- [../architecture.md](../architecture.md)
- [../libraries.md](../libraries.md) (`ignore`, `regex`)
