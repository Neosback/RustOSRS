# osrs-render guidance

## Owns

- `wgpu` projection, GPU resources, render pipelines, shaders, mesh upload, batching, and frame submission.
- Efficient presentation of already-resolved scene/HD inputs.

## Does not own

- Editable map authority, semantic history, cache decoding, OSRS placement rules, or UI feature decisions.
- A second semantic object/terrain/material model invented for rendering convenience.

## Invariants

- Renderer state is disposable projection state. Losing GPU buffers must never lose authored map data.
- Never infer semantic truth from meshes, picker triangles, draw lists, or GPU allocation state.
- Reject stale projection generations instead of presenting old work over newer semantic state.
- Rendering optimizations must preserve validated OSRS/117HD outputs.
- Prefer immutable/revisioned render inputs over broad shared mutable world state.

## Before adding code

1. Confirm the behavior is presentation, not scene semantics or HD rule resolution.
2. Search existing passes, resource owners, upload paths, and shader helpers.
3. Inspect parity fixtures before combining or reordering operations that can alter output.
4. Measure before adding caches, arenas, packing, or new GPU passes.

## Verification

For shader/pipeline changes, run the smallest relevant render/parity test first. Use crate-wide verification only when Rust/API changes require it; reserve full workspace/visual gates for integration checkpoints.
