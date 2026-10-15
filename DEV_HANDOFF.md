# RustOSRS Development Handoff

Last updated: 2026-10-15

`ARCHITECTURE.md` is the authoritative architecture contract. This file records the current integrated baseline, active subsystem, validation state, known hazards, and next allowed boundary.

## Project invariant

`MapDocument` / `EditorSession` is authoritative edited map state. World-worker edits, rendered geometry, GPU resources, picker data, previews, animation instances, dependency/proof snapshots, and UI state are projections only.

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

Object checkpoints O1-O4 are complete. O5 adversarial coverage is next and has not started.

## Object checkpoint O1 - dependency audit and conservative fallback hardening

Status: COMPLETE and CERTIFIED.

Certified O1 head:
`643f760a7f318d5519fc9897a588d732afe0c293`

O1 CI:
`38699017109` / #1264 - SUCCESS across Tier A/B/C.

O1 established that Object invalidation is not safely reducible to an anchor radius or a simple map from currently visible placement to output zones.

### O1 findings

1. `MapOp::AddObject` / `RemoveObject` already carry full canonical `ObjectPlacement` identity, but the O1 `EditFootprint` collapsed Object mutations to plane-less anchor tiles and lost add/remove polarity.
2. Static Reference and HD loc geometry is emitted into the loc anchor zone, but the scene that produces it is fully contextual.
3. Game-object insertion is footprint-wide and limited to five game objects per tile. Placement order matters, so removal/addition can change whether later overlapping objects are accepted.
4. Fixed scene layers are replacement slots; deletion can reveal an earlier placement and addition can replace one.
5. Boundary/roof objects can change wall-decoration displacement and the diagonal-decoration presentation pass can derive offsets from a type-9 host.
6. Clipped locs write the terrain shadow grid, which feeds terrain slope lighting; Object edits can therefore change terrain output.
7. Smooth-terrain presentation can extend changed terrain colors to adjacent shared vertices.
8. Scene `ModelData` normal reconciliation can change neighboring models and hide matched faces.
9. Dynamic, morphed, omitted, rejected, or overwritten placements cannot be treated as dependency-free merely because they currently emit no static geometry.
10. Each producer builds a 104x104 scene around its owned 64x64 output, so an Object anchored in one region can affect neighboring producers.

### O1 hardening

Before O1, Object-only edits could rebuild only their anchor region. O1 corrected this under-invalidation.

Any unproven Object footprint now uses the complete **3x3 producer-region ring** around every touched Object region.

This O1 3x3 producer-ring policy remains the mandatory fail-closed fallback after O4 runtime narrowing.

## Object checkpoint O2 - canonical Object mutation footprint

Status: COMPLETE and CERTIFIED.

O2 code head:
`fed985ea59db7737067b6d19d53844d2d4e9429a`

O2 code-head CI:
`38700886057` / #1275 - SUCCESS across Tier A/B/C.

Final documentation-bearing O2 head:
`d61d9169d271c1dde9009707ff59d73cb8001716`

Final O2 CI:
`38701473420` / #1277 - SUCCESS across Tier A/B/C.

### O2 delivered

`EditFootprint` retains exact Object mutation sets:

- `object_removed: Vec<ObjectPlacement>`
- `object_added: Vec<ObjectPlacement>`

Each placement preserves object id, source plane, world anchor tile, loc kind, and orientation.

The placement vectors are normalized deterministically by `(source plane, x, y, object id, loc kind, orientation)` and exact duplicates are collapsed independently inside each polarity set.

`EditFootprint::from_patch` preserves operation polarity directly. Semantic history replay reconstructs the same exact identities:

- Redo preserves original removed/added polarity
- Undo swaps original added/removed polarity

`object_tiles` remains only as a derived compatibility summary for region routing/fallback.

The worker replay path remains unchanged and still validates authoritative post-replay semantics before projection.

## Object checkpoint O3 - producer-output proof contract

Status: COMPLETE and CERTIFIED.

Certified O3 head:
`a555e07796a938676ddb3c4f7d0320dd480041aa`

O3 CI:
`38703366564` / #1285 - SUCCESS across Tier A/B/C.

### O3 proof choice

O3 chose **producer output comparison** rather than a guessed causal radius or a `placement -> visible zones` dependency table.

This proof compares the exact static output of a fully assembled producer scene, so the comparison naturally includes consequences of:

- game-object capacity/order
- fixed-slot replacement/reveal
- wall-decoration host effects
- terrain shadow/lighting changes
- ModelData normal reconciliation
- additions/removals and previously hidden/rejected placements

### O3 contract

`ObjectOutputSnapshot` records:

- producer region
- explicit 8x8 zone coverage
- exact per-zone `SceneGeometry`
- explicit complete/incomplete state

A full producer snapshot covers all 64 owned 8x8 zones, including covered empty zones. Scoped snapshots carry explicit coverage; absent coverage never means unchanged.

Exact comparison returns added, removed, or changed zones by structural geometry equality.

Editor-side `ObjectOutputState` binds accepted output to:

- producer region
- projection epoch
- presentation generation

Lookup is explicitly stale, incomplete, or current with exact changed zones.

O3 itself did not enable runtime narrowing.

## Object checkpoint O4 - accepted-build runtime integration and planner

Status: COMPLETE and CODE-CERTIFIED. Final documentation-bearing exact-head certification is the current closeout step.

Certified O4 code head:
`52eadc6284e71d9bc00de878d89cc67a0f0c7628`

O4 code-head CI:
`38705182639` / #1294 - SUCCESS across Tier A/B/C.

### O4 delivered

`ObjectOutputStore` now retains disposable accepted Object producer output and pending pre-edit comparison stamps.

Trusted state is installed only after existing worker result freshness gates accept the request identity, projection epoch, and presentation generation.

Trusted Object output state is cleared/removed on:

- source/streamer replacement
- full presentation rebuild
- region unload
- accepted full absent-region result
- invalid proof/store update

### O4 planner behavior

For one finalized Object edit:

1. derive the same O1 conservative 3x3 producer-region universe
2. consider every resident non-empty producer in that universe
3. require a current, complete accepted pre-edit snapshot for every required producer
4. if any required producer is missing, stale, or incomplete, use the O1 full-region fallback
5. otherwise queue one **full producer build** per required producer
6. preserve full 104x104 contextual scene assembly
7. retain the accepted pre-edit baseline stamp while normal invalidation advances the producer projection epoch
8. accept a candidate only after the normal request/epoch and presentation-generation gates pass
9. compare accepted full post-edit output against the retained pre-edit snapshot
10. install only exact changed 8x8 static zones when comparison is current and complete
11. fall back to full producer installation on missing, stale, incomplete, mismatched, or invalid proof state

A later semantic invalidation cancels pending proof state and/or advances the projection epoch, so an old candidate cannot be trusted after the world advances.

An exact empty static diff is valid: static upload is a no-op, while accepted region-complete inspection/picker, animations, Height dependency state, and Object proof freshness still advance.

### O4 deliberately unchanged

O4 does not narrow or replace:

- semantic authority
- worker Object replay
- full contextual scene assembly
- region-complete picker/inspection projection replacement
- animation projection replacement
- Height dependency replacement

No worker protocol change was required. Static install scope is resolved only after the accepted full candidate has been compared with the trusted baseline.

## Integrated projection/edit architecture

Current integrated `main` plus this branch through O4 includes:

- central semantic `EditRuntime` execute/replay gateway
- Paint, Height, Paste, Path, Objects, semantic Undo, and semantic Redo through the gateway
- receipt-driven projection invalidation
- pure terrain 8x8 zone invalidation
- Height exact 8x8 zone invalidation only with complete/current dependency proof
- conservative Height region fallback for unproven cases
- full contextual scene assembly with zone-scoped final static geometry emission
- stale asynchronous region-build rejection by request identity, projection epoch, and presentation generation
- accepted-build Height dependency freshness tied to the same projection identity
- exact canonical Object removed/added mutation footprints retained through commit receipts and semantic Undo/Redo
- Object exact producer-output comparison contract
- accepted Object producer-output freshness tied to projection epoch + presentation generation
- Object full contextual candidate builds across the O1 producer universe
- exact changed-zone static installation only after accepted pre/post comparison
- O1 3x3 producer-region fail-closed fallback for every absent/stale/incomplete/invalid Object proof

## O5 required adversarial coverage

Status: NOT STARTED.

O5 must exercise the implemented runtime proof against at least:

- 8x8 zone boundary
- 64x64 region boundary
- fixed-slot replacement/reveal
- wall-decoration displacement host changes
- five-per-tile game-object capacity where removal reveals a later object
- overlapping multi-tile capacity chains
- clipped wall shadow changing neighboring terrain light
- type-10/11 footprint shadow changing terrain across a zone edge
- ModelData normal merge changing neighboring object output
- dynamic/morphed/omitted cases and fail-closed behavior
- semantic Undo/Redo exact mutation polarity
- stale/in-flight worker result rejection

Prefer end-to-end producer-output assertions rather than reproducing scene dependency rules in the planner tests.

## Remaining major roadmap

Each item is a separate subsystem and therefore a separate branch after the prior subsystem is merged and certified on `main`.

1. Complete Object adversarial coverage, exact final branch certification, PR/merge, and integrated-main certification.
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

**STOP AFTER OBJECT CHECKPOINT O4 CERTIFICATION.**

Do not start O5 until:

1. this documentation-bearing O4 branch head is green under required Tier A/B/C CI
2. this handoff and the Object plan are present on that certified head
3. O4 is reported to the user
4. the user explicitly directs continuation

Do not open a PR or merge yet. The Object subsystem remains on this same branch for O5. O6 performs exact final branch certification, PR/merge, integrated-main validation, handoff update, and final `main` certification before any later subsystem branch may be created.
