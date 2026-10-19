# osrs-hd guidance

## Owns

- Reusable 117HD-style authored appearance data, settings, material/environment semantics, and deterministic resolution logic.
- Renderer inputs that are meaningful without `wgpu` or editor UI state.

## Does not own

- GPU pipelines/resources, editor widgets, map authority, world scheduling, or platform services.
- Semantic map edits disguised as appearance state.

## Invariants

- Appearance must remain separate from `MapDocument` revision/cache export unless a feature explicitly authors map semantics.
- Resolution rules should be deterministic and explainable.
- Preserve source/reference behavior where parity is claimed; do not replace specific 117HD rules with vague generic material logic.
- Keep data/settings reusable by renderers and tests without depending back on `osrs-render`, `osrs-world`, or `osrs-editor`.

## Before adding code

1. Search existing theme/material/environment rules and HD source audits.
2. Read the relevant parity/spec documentation and pinned source evidence.
3. Confirm whether the behavior belongs here or in GPU projection code.
4. Prefer extending existing rule data/resolvers over adding a second settings path.

## Verification

Use the narrow HD resolver/source test first. Escalate to `python3 scripts/verify.py -p osrs-hd` when the public HD contract changes.
