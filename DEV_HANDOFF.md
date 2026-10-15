# RustOSRS Development Handoff

Last updated: 2026-10-15

`ARCHITECTURE.md` is the authoritative architecture contract. This file records the current integrated baseline, active subsystem, exact checkpoint state, validation state, hazards, and next allowed boundary.

## Project invariant

`MapDocument` / `EditorSession` is authoritative edited map state. World-worker edits, rendered geometry, GPU resources, picker data, previews, and UI state are projections only.

If every projection vanished, the complete edited map must still exist correctly in the semantic session.

## Mandatory development protocol

These rules apply to all continuing development unless the user explicitly changes them.

1. Keep exactly one active development branch/system at a time.
2. Do not start a second feature/refactor branch while the current branch is active.
3. Break each subsystem into small, reviewable checkpoints.
4. Stop after checkpoints unless the user explicitly directs continuation or completion/merge.
5. Update `DEV_HANDOFF.md` as checkpoints close so repository state, exact SHAs, validation, hazards, and the next boundary stay current.
6. Keep a subsystem on the same branch across its checkpoints unless there is a concrete reason to close it.
7. Do not mix unrelated architecture, renderer/117HD, cache, or UI work into the active subsystem branch.
8. Before merging a completed subsystem, validate the exact final branch head with Tier A/B/C CI.
9. Merge the current subsystem before opening any new development branch.
10. After merge, validate exact integrated `main`, record the result in this handoff, and certify the final handoff-only `main` head if the validation record itself changes `main`.
11. Do not create the next branch until the prior subsystem is merged, final `main` is green, and the user explicitly directs continuation.
12. One logical user gesture produces at most one logical semantic transaction/history entry.
13. Failed projection preparation must not leave semantic authority and visible projection knowingly diverged.
14. Exact SHA matters. A green run certifies only the SHA it tested.
15. New or fully refactored production Rust files remain under architecture guard limits. Do not introduce debt exceptions for convenience.

The user explicitly reinforced rule 9 on 2026-10-15 after noticing multiple refs while `main` had not advanced. Treat merge-before-new-branch as a hard operating rule.

## Current integrated baseline

`main`:
`5938df7d723c55b66ad4522f79c08cf89bf0acbf`

Latest integrated subsystem:
PR #50 - `fix(editor): reject stale region build results`

Integrated projection/edit architecture already includes:

- central semantic `EditRuntime` execute/replay gateway
- Paint, Height, Paste, Path, Objects, semantic Undo, and semantic Redo through that gateway
- receipt-driven projection invalidation
- pure terrain 8x8 zone invalidation
- full contextual scene assembly with zone-scoped final geometry emission
- stale asynchronous region build rejection by request identity, projection epoch, and presentation generation

## Repository branch state

Current development branch:
`refactor/height-zone-invalidation`

Base `main`:
`5938df7d723c55b66ad4522f79c08cf89bf0acbf`

At the latest comparison before the final documentation refresh, the Height branch was ahead of `main` and 0 behind. No rebase conflict pressure existed.

There are no open PRs at this handoff refresh.

A ref named `tmp-noop-should-not-create` exists at the old H2 snapshot `0fbd4d907004e9b6b0353842f9bec9f9ff963c2b`. It is not a second subsystem and must not receive development. After the Height merge, ensure it carries no unmerged work; delete it if the available GitHub capability permits, otherwise move/leave it only as an already-integrated historical ref and report that clearly.

## Active subsystem: Height zone invalidation

Goal:
Replace Height's conservative region-ring projection invalidation with exact or conservatively bounded 8x8 zone invalidation only when projection dependency metadata proves the complete affected output set.

This branch is Height-only. Object fine-grained invalidation is a separate future subsystem and must not start before this branch is merged and final `main` is certified.

### H1 - dependency audit and operating protocol

Status: COMPLETE AND CERTIFIED.

Certified content head:
`73694a9d97fd2845c8ed818400430709d6c704c5`

CI:
`38688385096` / #1221 - SUCCESS.

Key conclusion:
A single fixed Height radius is not proven safe across placement, contouring, dynamic models, and downstream scene relationships. Exact narrowing therefore requires explicit dependency metadata with conservative fallback.

### H2 - explicit Height dependency metadata

Status: COMPLETE AND CERTIFIED.

Certified code/test head:
`c59eea2cadffbc5861f6ecf8c138a9a0b6b11f99`

CI:
`38689828881` / #1227 - SUCCESS.

Delivered:

- canonical `HeightVertexCoord` preserving source plane + world X/Y
- deterministic ordered source-to-zone mapping
- producer-region validation
- projection epoch + presentation generation freshness stamps
- explicit complete/incomplete state
- stale/incomplete/current lookup outcomes
- fail-closed merge and deterministic deduplication
- boundary, freshness, producer/stamp mismatch, known-empty, and incomplete-merge tests

### H3 - Height receipt to zone invalidation

Status: COMPLETE. Final branch certification is H5.

Code/test head before H4 boundary additions and final documentation refresh included the full runtime integration. The final branch head will supersede the intermediate H3 SHAs and is the only SHA that matters for merge certification.

Delivered:

- `EditFootprint` Height coordinates retain canonical `HeightVertexCoord`, including `SourcePlane`
- semantic Undo/Redo reconstruct canonical Height coordinates instead of plane-less `MapTile` identities
- world-worker `RegionGeometry` carries a region-complete Height dependency snapshot produced from the full contextual `WorldScene`
- dependency extraction runs before optional zone-scoped static geometry emission
- terrain geometry and slope-light reads are recorded
- static loc placement midpoint reads are recorded from rotated definition footprint behavior
- static lit-model contour reads use actual final model X/Z radius sampling
- `ModelData` normal-reconciliation participation marks the snapshot incomplete because cross-model lighting closure is not yet proven
- posed animated/dynamic contouring marks the snapshot incomplete when active pose bounds can alter the read-set
- worker results install dependency snapshots only after the existing stale-build gate accepts request identity, projection epoch, and presentation generation
- accepted metadata is stamped with current projection epoch and presentation generation
- source replacement, full presentation rebuild, and unload paths clear/remove trusted metadata
- Height planner resolves through all resident non-empty producer snapshots
- complete/current proof produces exact dirty zones
- absent, stale, incomplete, unsupported, or unproven metadata retains the previous region fallback
- mixed terrain + Height footprints union terrain dependency zones with proven Height zones
- Object participation continues to force conservative region invalidation
- semantic authority remains in `EditorSession`
- full contextual scene assembly remains unchanged

### H4 - adversarial boundary and replay coverage

Status: COMPLETE. Final branch certification is H5.

Coverage includes:

- Height proof spanning an 8x8 zone edge
- Height proof spanning a 64x64 region edge
- Euclidean negative-coordinate zone ownership
- rotated/multi-tile placement midpoint helper behavior and edge fallback
- contour sampling expansion and scene clamping
- dependency edge deduplication
- missing metadata region-ring fallback
- stale projection epoch / presentation generation rejection
- incomplete metadata rejection
- semantic Undo/Redo with canonical Height footprints
- stale in-flight build rejection
- cancelled older build results unable to consume newer requests

Dynamic/morphed contoured models are deliberately fail-closed rather than falsely certified for exact-zone invalidation. Their unsupported active-pose contour case marks metadata incomplete and therefore uses the conservative region policy.

### H5 - subsystem certification and merge

Status: IN PROGRESS.

Latest code/test/style head before this documentation refresh:
`6bf2b0b6c133c07a4c9e9617acfb48ef857c95f5`

That head had already demonstrated architecture, formatting, workspace compilation, and strict Clippy success in the immediately preceding CI progression before this documentation refresh superseded it. The final documentation head must receive a fresh full Tier A/B/C run; prior green steps do not certify the new SHA.

Required final branch validation:

- architecture boundaries
- architecture guard
- reference regeneration safety
- historical deob golden index
- rustfmt
- workspace compile
- strict Clippy
- WebAssembly portability
- Tier B full workspace tests
- Tier C M3-M9 semantic/reference parity

Merge sequence now authorized by the user:

1. certify the exact final Height branch head
2. open one PR from `refactor/height-zone-invalidation` to `main`
3. merge it
4. validate the exact integrated `main`
5. update this handoff on `main` with merge SHA and post-merge CI result
6. certify that final handoff-only `main` head if it changed
7. ensure temporary/merged refs contain no unmerged work
8. stop and report
9. do not create another branch

## Current Height runtime behavior

Height edits now use zone-scoped projection invalidation only when every required resident producer supplies a complete/current Height dependency snapshot.

The existing region-ring policy remains the mandatory fallback for missing, stale, incomplete, unsupported, Object-mixed, or otherwise unproven cases.

This is intentionally asymmetric: supported static dependencies gain fine-grained invalidation, while difficult dynamic/normal-reconciliation cases remain conservative rather than weakening correctness.

## Remaining major roadmap after Height

Each item is a separate future subsystem. None may begin until the current Height branch is merged and final `main` is certified.

1. Object dependency footprint and zone invalidation.
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

## Runtime ownership current state

Completed ownership boundaries:

- `EditRuntime`
- `RenderRuntime`
- `WorldRuntime`
- `InteractionRuntime` field ownership

Future behavioral/ownership extraction remains separate work.

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

FINISH AND MERGE THE CURRENT HEIGHT SUBSYSTEM.

No new development branch is allowed before:

- the exact final Height branch head passes Tier A/B/C
- the Height branch is merged into `main`
- exact integrated `main` passes CI
- this handoff records the merged state
- the final handoff-bearing `main` head is green
- the user explicitly directs the next subsystem
