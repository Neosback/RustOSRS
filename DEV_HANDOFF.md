# RustOSRS Development Handoff

Last updated: 2026-10-15

`ARCHITECTURE.md` is the authoritative architecture contract. This file records the current integrated baseline, active subsystem, validation state, known hazards, and next allowed boundary.

## Project invariant

`MapDocument` / `EditorSession` is authoritative edited map state. World-worker edits, rendered geometry, GPU resources, picker data, previews, animation instances, and UI state are projections only.

If every projection vanished, the complete edited map must still exist correctly in the semantic session.

## Mandatory development protocol

1. Keep exactly one active development branch/system at a time.
2. Do not start a second feature/refactor branch while the current branch is active.
3. Break each subsystem into small reviewable checkpoints.
4. Stop after every checkpoint and report progress unless the user explicitly directs continuation.
5. Update `DEV_HANDOFF.md` at every completed checkpoint with exact repository state, SHAs, validation, hazards, and next boundary.
6. Keep a subsystem on the same branch across checkpoints unless there is a concrete reason to close it.
7. Do not mix unrelated architecture, renderer/117HD, cache, or UI work into the active subsystem branch.
8. Before merging a completed subsystem, validate the exact final branch head with required Tier A/B/C CI.
9. **Merge the current subsystem into `main` before opening any new development branch.**
10. After merge, validate exact integrated `main`, update this handoff, and certify the final handoff-bearing `main` head if the documentation update advances `main`.
11. Do not create the next branch until the prior subsystem is merged, final `main` is green, and the user explicitly directs the next subsystem.
12. One logical user gesture produces at most one logical semantic transaction/history entry.
13. Failed projection preparation must not knowingly leave semantic authority and visible projection divergent.
14. Exact SHA matters. A green run certifies only the SHA it tested.
15. Do not introduce architecture-guard debt exceptions for convenience.

The merge-before-new-branch rule is a hard operating rule.

## Certified integrated baseline before current branch

Latest merged subsystem:
PR #51 - `refactor(editor): zone-scope certified Height invalidation`

Height merge commit on `main`:
`fb2fd2874b18d11bef689fa1954751bcdc64273b`

Post-merge integrated-main CI:
`38696859752` / #1258 - SUCCESS.

Final handoff-bearing `main` head:
`a7b9fb47835ab00b25db7c02e1aa3ca3157a054b`

Final handoff-bearing `main` CI:
`38697404691` / #1260 - SUCCESS.

That exact `main` head passed:

- Tier A architecture boundaries
- architecture guard
- reference regeneration safety
- historical deob golden index
- rustfmt
- workspace compile
- strict Clippy
- WebAssembly portability
- Tier B full workspace tests
- Tier C M3-M9 semantic/reference parity

No development branch was created until that exact final `main` head was green.

## Current active subsystem

Branch:
`refactor/object-zone-invalidation`

Base `main`:
`a7b9fb47835ab00b25db7c02e1aa3ca3157a054b`

Goal:
Narrow finalized Object edit projection work to exact 8x8 zones only when every affected producer output is proven, while retaining conservative region fallback for every unproven case.

Plan:
`docs/plans/OBJECT-ZONE-INVALIDATION.md`

No PR is open for this branch yet.

## Object checkpoint O1 - dependency audit and conservative fallback hardening

Status: COMPLETE; exact-head CI certification pending at the time of this handoff commit.

O1 established that Object invalidation is not safely reducible to an anchor radius or a simple map from currently visible placement to output zones.

### O1 audit findings

1. `MapOp::AddObject` / `RemoveObject` already carry full canonical `ObjectPlacement` identity, but `EditFootprint` currently collapses Object mutations to plane-less anchor tiles and loses add/remove polarity.
2. Static Reference and HD loc geometry is emitted into the loc anchor zone, but the scene that produces it is fully contextual.
3. Game-object insertion is footprint-wide and limited to five game objects per tile. Placement order matters, so removing one object can cause a later overlapping object to become accepted; adding one can cause a later object to be rejected.
4. Fixed scene layers are replacement slots. Removing the final occupant can reveal an earlier placement; adding can replace an earlier occupant.
5. Boundary/roof objects can rescale a wall decoration already placed on the same tile. The optional diagonal-decoration pass also derives decoration offsets from a type-9 host model.
6. Clipped locs write the terrain shadow grid. Terrain slope light reads neighboring shadow cells, so Object edits can change terrain output, not only loc geometry.
7. Smooth-terrain presentation can extend changed terrain colors to adjacent tiles sharing averaged vertices.
8. Scene `ModelData` normal reconciliation changes neighboring models and can hide matched faces. Its traversal depends on storage footprint and can include the next storage plane.
9. Dynamic, morphed, omitted, rejected, or overwritten placements cannot be treated as dependency-free merely because they currently emit no static geometry.
10. Each producer region builds a 104x104 scene around its owned 64x64 output, with 16 tiles of context before and 24 after. Therefore an Object anchored in one map region can affect a neighboring producer's owned output.

### Existing correctness hole found and hardened

Before O1, an Object-only semantic edit carries `LOCATIONS | COLLISION | BOUNDS`. The generic region fallback only expanded to neighboring regions for terrain/normals/lighting impacts, so Object-only edits rebuilt only the touched anchor region.

That was not conservative with respect to the worker's cross-region contextual scene.

O1 changes Object fallback behavior so **any Object footprint invalidates the complete 3x3 producer-region ring** around each touched Object region.

This is intentionally conservative and matches the neighboring producer universe that can include anchors from the touched region under the current region-window architecture.

No Object 8x8 narrowing is enabled yet.

Focused invalidation tests now assert:

- Object-only receipts produce a 3x3 region ring
- Object participation still overrides any Height exact-zone proof
- Height-only and Paint-only invalidation behavior is unchanged

### Object proof requirement established by O1

A future Object dependency proof must represent **mutation effects**, not only current render ownership.

A complete proof must account for removed and added placements, capacity/replacement ordering, terrain shadow/light effects, normal-reconciliation effects, and producer freshness.

A simple `ObjectPlacement -> current output zones` snapshot is explicitly insufficient for removals because removal can expose a previously rejected/replaced later placement.

Missing, stale, incomplete, unsupported, or non-counterfactual proof must retain the O1 3x3 producer-region fallback.

## Next checkpoint O2

Status: NOT STARTED.

Allowed boundary only after O1 exact-head certification and explicit user continuation:

- preserve full canonical `ObjectPlacement` identity in finalized Object footprints
- split Object footprint into exact removed and added placement sets so mutation polarity is retained
- keep deterministic canonical ordering/deduplication
- reconstruct the same canonical mutation footprint through semantic Undo/Redo with polarity reversed correctly
- update touched-region derivation from canonical placements
- retain the O1 3x3 producer-region fallback
- do **not** introduce Object zone narrowing yet

O3 after that will choose/implement the fail-closed dependency proof contract capable of representing addition/removal causal effects.

## Integrated projection/edit architecture

Current integrated `main` plus this branch baseline includes:

- central semantic `EditRuntime` execute/replay gateway
- Paint, Height, Paste, Path, Objects, semantic Undo, and semantic Redo through the gateway
- receipt-driven projection invalidation
- pure terrain 8x8 zone invalidation
- Height exact 8x8 zone invalidation only with complete/current dependency proof
- conservative Height region fallback for unproven cases
- full contextual scene assembly with zone-scoped final static geometry emission
- stale asynchronous region-build rejection by request identity, projection epoch, and presentation generation
- accepted-build Height dependency freshness tied to the same projection identity
- Object edits currently remain region-conservative, now hardened to the contextual 3x3 producer ring

## Remaining major roadmap

Each item is a separate subsystem and therefore a separate branch after the prior subsystem is merged and certified on `main`.

1. Complete Object dependency footprint and zone invalidation.
2. `InteractionRuntime` behavioral encapsulation.
3. `ProjectRuntime` ownership extraction.
4. `UiRuntime` ownership extraction.
5. `DiagnosticsRuntime` ownership extraction.
6. Reference renderer parity gates and external visual oracle work.
7. 117HD dynamic/placed lights.
8. 117HD sky/day-night/reflection systems and remaining effects.
9. Edited-cache fingerprint/profile strategy, new-group support, and broader index repack when justified.
10. Project/browser lifecycle polish and future import/merge product work.

## Cache/export current state

Map stream encoding and existing map-group cache-copy repacking are implemented. Known limitations remain:

- only existing map-square groups are repacked
- new groups absent from the reference table are not created
- other indices are not generally rewritten by the editor export path
- an edited cache copy intentionally no longer matches the bundled pristine-cache fingerprint/profile

## 117HD current high-level state

Already substantially implemented:

- render-profile/HDR foundation
- HD data layer/material tables
- HD terrain/model extraction
- material replacement
- ground blending and normals
- environments/fog/directional lighting
- directional shadows and filtering
- water surfaces, flow/normal maps, foam, Fresnel, underwater tint/caustics
- animated-loc HD material routing
- material flow maps
- parallax/displacement and displaced shadow position

Large remaining systems include dynamic/placed point lights, full sky/day-night/reflection systems, remaining post-processing/settings parity, and remaining model/material edge cases.

## Next allowed boundary

**STOP AFTER OBJECT CHECKPOINT O1 CERTIFICATION.**

Do not start O2 until:

1. the exact final O1 branch head is green under required CI
2. this handoff and the Object plan are present on that certified head
3. the O1 checkpoint is reported to the user
4. the user explicitly directs continuation

Do not open a PR or merge yet. The Object subsystem remains on this same branch through its checkpoints, then must merge into `main` before any later subsystem branch is created.
