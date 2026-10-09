# 0027. Submission gate and repair loop

- Status: Accepted
- Date: 2026-10-10
- Related: [0012](0012-archify-json-v1-canonical-format.md), [0014](0014-schema-validation-with-vendored-archify-schemas.md), [0017](0017-file-watching.md), [0018](0018-app-computed-layout-dugong-and-orthogonal-routing.md), [0025](0025-agent-tool-sandbox-and-deny-list.md), [0026](0026-system-prompt-derived-from-archify.md), [0028](0028-agent-run-budgets.md); spec: FR-GEN, FR-VAL

## Context

Clicking a diagram node must open a real file at real lines. A candidate that merely parses can still reference missing components or cite lines that don't exist. Archify catches these with its CLI gates and LLM repair rounds. ArchAlign needs an equivalent inside the agent loop, so the model can fix its own errors before the run ends.

## Decision

### Model-authored fields

- `meta.title`, `meta.subtitle`, `meta.legend`
- components: `id`, `type`, `label`, `sublabel`, `tag`, `sources`
- boundaries: `kind`, `label`, `wraps`
- connections: `id`, `from`, `to`, `label`, `variant`
- cards

### Gate, run by `submit_diagram`

1. **App-owned fields.** Set or override:
   - `schema_version: 1` and `diagram_type: "architecture"`;
   - `meta.output`, `meta.locale: "en"`, `meta.quality_profile: "showcase"`;
   - `meta.repository`, when git metadata is available.

   Drop any other model-supplied fields and log them as `fields_dropped`: `pos`, `size`, `row`, `col`, `layout`, `pad`, `icon`, `brand`, connection `fromSide`/`toSide`/`route`/`via`/`label*` hints/`width`, `meta.viewBox`, `meta.views`, `meta.animation`, `meta.translations`, `meta.visual_preset`, `meta.engineering_profile`.
2. JSON Schema validation ([0014](0014-schema-validation-with-vendored-archify-schemas.md)).
3. Strict serde parsing.
4. **Semantic checks:**
   - unique component IDs, and unique connection IDs where present;
   - connection `from`/`to` and boundary `wraps` reference existing components;
   - every component has 1–3 sources;
   - boundaries are separate or strictly nested, never partially overlapping.
5. **Citations:**
   - the path is repo-relative, inside the root and not denied ([0025](0025-agent-tool-sandbox-and-deny-list.md));
   - the file exists and is text;
   - `line ≥ 1`, `end_line ≥ line`, and `end_line ≤` the file's line count.

### Repair loop

- Return errors as the tool result, each with a JSON pointer and a message.
- The model gets 3 rejected submissions to repair. The 4th rejection fails the run ([0028](0028-agent-run-budgets.md)).

### Warnings, not errors

- A cited file with uncommitted changes gets a warning badge.

### Import profile (opening existing files)

| Severity | Conditions |
|---|---|
| Error | Schema problems, parse failures, broken references. |
| Warning / badge | Missing sources, partial boundary overlap, broken citations. |

## Alternatives considered

### Accept anything that parses

Rejected. Broken references and bad citations break navigation.

### Validate only after the run

Rejected. The model gets no chance to repair.

### Unlimited repairs

Rejected. Unbounded cost.

## Consequences

- Positive: every generated node navigates to lines that exist.
- Positive: geometry fields are always app-computed ([0018](0018-app-computed-layout-dugong-and-orthogonal-routing.md)), never taken from the model.
- Negative: citations can go stale later. The watcher re-checks them ([0017](0017-file-watching.md)).
- Negative: the import profile is more lenient than the generation gate, so imported diagrams can show badges.

## References

- `crates/archalign-model/schemas/architecture.schema.json`
- `crates/archalign-model/schemas/common.schema.json`
- [../spec.md](../spec.md) (FR-GEN, FR-VAL)
- [../architecture.md](../architecture.md)
