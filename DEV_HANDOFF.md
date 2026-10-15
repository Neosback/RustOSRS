# RustOSRS Development Handoff

Last updated: 2026-10-15

`ARCHITECTURE.md` is the authoritative architecture contract. This file records the current integrated baseline, active subsystem, exact validation state, hazards, and next allowed boundary.

## Project invariant

`MapDocument` / `EditorSession` is authoritative edited map state. World-worker edits, rendered geometry, GPU resources, picker data, previews, animation instances, dependency/proof snapshots, and UI state are projections only.

If every projection vanished, the complete edited map must still exist correctly in the semantic session.

## Mandatory development protocol

1. Keep exactly one active development branch/system at a time.
2. Do not start a second feature/refactor branch while the current branch is active.
3. Break each subsystem into reviewable checkpoints.
4. Update this handoff at completed checkpoints with exact SHAs and validation.
5. Keep one subsystem on the same branch until it is complete.
6. Do not mix unrelated architecture, renderer/117HD, cache, or UI work into the active branch.
7. Before merge, validate the exact final branch head with required Tier A/B/C CI.
8. **Merge the current subsystem into `main` before opening any new development branch.**
9. After merge, validate exact integrated `main`, update this handoff, and certify the final handoff-bearing `main` head if docs advance it.
10. Exact SHA matters. A green run certifies only the SHA it tested.
11. One logical user gesture produces at most one logical semantic transaction/history entry.
12. Failed projection preparation must not knowingly leave semantic authority and visible projection divergent.
13. Do not introduce architecture-guard debt exceptions for convenience.

## Certified integrated baseline before current branch

Latest merged subsystem before Object work:
PR #51 - `refactor(editor): zone-scope certified Height invalidation`

Height merge commit on `main`:
`fb2fd2874b18d11bef689fa1954751bcdc64273b`

Post-merge integrated-main CI:
`38696859752` / #1258 - SUCCESS.

Final handoff-bearing `main` head before Object work:
`a7b9fb47835ab00b25db7c02e1aa3ca3157a054b`

Final baseline CI:
`38697404691` / #1260 - SUCCESS across Tier A/B/C.

## Current active subsystem

Branch: `refactor/object-zone-invalidation`

Base `main`: `a7b9fb47835ab00b25db7c02e1aa3ca3157a054b`

Plan: `docs/plans/OBJECT-ZONE-INVALIDATION.md`

Goal: exact Object 8x8 static installation only when every required producer is proven from accepted output, with conservative fail-closed behavior otherwise.

Object checkpoints O1-O5 are complete and code-certified. O6 final documentation-bearing branch certification, PR/merge, integrated-main validation, and final handoff certification is next.

No second development branch may be created during O6.

## O1 - conservative Object fallback hardening

Status: COMPLETE and CERTIFIED.

Head: `643f760a7f318d5519fc9897a588d732afe0c293`
CI: `38699017109` / #1264 - SUCCESS across Tier A/B/C.

O1 established that Object invalidation is not safely reducible to an anchor radius or a current-visible-placement map because of:

- five-per-tile game-object capacity and order dependence
- fixed-slot replacement/reveal
- wall-decoration host/displacement effects
- loc shadow effects on terrain lighting
- ModelData normal reconciliation
- dynamic/morphed/omitted/rejected placement behavior
- 104x104 producer context around owned 64x64 output

O1 corrected the old Object-only under-invalidation by making any unproven Object footprint rebuild the complete 3x3 producer-region ring around touched Object regions.

That 3x3 policy remains the mandatory fail-closed fallback.

## O2 - canonical Object mutation footprint

Status: COMPLETE and CERTIFIED.

Code head: `fed985ea59db7737067b6d19d53844d2d4e9429a`
Code CI: `38700886057` / #1275 - SUCCESS.

Final O2 documentation-bearing head: `d61d9169d271c1dde9009707ff59d73cb8001716`
Final O2 CI: `38701473420` / #1277 - SUCCESS across Tier A/B/C.

`EditFootprint` retains exact removed and added `ObjectPlacement` identities with object id, source plane, anchor tile, loc kind, and orientation. Ordering/deduplication is deterministic.

Semantic history behavior is exact:

- Undo swaps removed/added mutation polarity
- Redo preserves original polarity

The derived anchor-tile summary remains only for compatibility/routing.

## O3 - producer-output proof contract

Status: COMPLETE and CERTIFIED.

Head: `a555e07796a938676ddb3c4f7d0320dd480041aa`
CI: `38703366564` / #1285 - SUCCESS across Tier A/B/C.

O3 chose producer-output comparison instead of a guessed dependency radius.

`ObjectOutputSnapshot` records producer region, explicit 8x8 coverage, exact per-zone `SceneGeometry`, and complete/incomplete state. Full snapshots explicitly cover all 64 owned zones, including empty zones.

Editor-side proof state is stamped with producer region, projection epoch, and presentation generation and exposes stale/incomplete/current lookup semantics.

## O4 - accepted-build Object proof runtime

Status: COMPLETE and CERTIFIED.

Code head: `52eadc6284e71d9bc00de878d89cc67a0f0c7628`
Code CI: `38705182639` / #1294 - SUCCESS across Tier A/B/C.

Documentation-bearing O4 head: `55f42228003770c3adc5ef5500a32052ebf1ff2a`
Documentation-bearing O4 CI: `38705932458` / #1296 - SUCCESS across Tier A/B/C.

For Object edits the runtime now:

1. derives the O1 3x3 producer universe
2. requires current, complete accepted pre-edit output for every required resident non-empty producer
3. falls back to O1 full-region rebuilding if any producer is untrusted
4. otherwise queues full contextual candidate producer builds
5. accepts candidates only through the existing request identity, projection epoch, and presentation-generation gates
6. compares accepted post-edit output with retained pre-edit output
7. installs only exact changed static 8x8 zones when comparison is current and complete
8. falls back to full producer installation for invalid proof state

Full contextual scene assembly, region-complete inspection/picker state, animations, and Height dependencies remain intact.

Later semantic invalidation cancels pending Object proof state and/or invalidates older candidates through the projection epoch.

## O5 - adversarial coverage

Status: COMPLETE and CERTIFIED.

Certified O5 code head:
`398545757f67b56cfb4bdb9b19864b620b1c822a`

O5 CI:
`38707609239` / #1300 - SUCCESS across Tier A/B/C.

O5 added a test-only producer-output adversarial suite covering:

- 8x8 zone-boundary changes
- 64x64 producer-region boundary changes
- fixed-slot replacement/reveal
- wall-decoration host/displacement output changes
- five-per-tile capacity reveal
- overlapping capacity chains changing multiple output zones
- clipped-wall shadow changing neighboring terrain output
- type-10/11 footprint shadow changing terrain across a zone edge
- ModelData normal-reconciliation changes in neighboring object output
- incomplete dynamic/morphed/omitted candidate state failing closed
- complete dynamic-only static no-op producing an exact empty diff

Existing focused tests in the full Tier B workspace gate also cover:

- semantic Undo/Redo exact mutation polarity
- stale projection epoch rejection
- stale presentation-generation rejection
- old completion not consuming a newer request
- missing/stale/incomplete Object proof fallback

O5 changes no production behavior.

## Object subsystem behavior at O5 closure

The active branch now combines:

- central semantic `EditRuntime` authority
- exact canonical Object mutation footprints
- full contextual producer CPU builds
- accepted-output Object proof snapshots
- exact changed-zone static GPU installation when proven
- O1 3x3 region fallback for every untrusted proof state
- stale async result rejection through request identity, projection epoch, and presentation generation
- unchanged region-complete picker/inspection, animation, and Height dependency refresh behavior

The proof answers the postcondition question "which producer-owned static zones actually changed?" rather than duplicating capacity, replacement, shadow, or normal-reconciliation dependency logic in the planner.

## O6 - next allowed boundary

Status: READY.

The user explicitly authorized continuing and merging to `main` when the Object subsystem reaches a good stopping point. O1-O5 are complete, so O6 is authorized without another confirmation.

O6 procedure:

1. certify the exact final documentation-bearing Object branch head with Tier A/B/C
2. confirm branch comparison to `main` has no unexpected divergence and remains Object-only plus plan/handoff documentation
3. open one PR from `refactor/object-zone-invalidation` to `main`
4. merge the PR before creating any new branch
5. validate exact integrated `main` with Tier A/B/C
6. update this handoff on `main` with final branch SHA, PR number, merge commit, and integrated-main CI
7. certify the final handoff-bearing `main` head if the handoff commit advances `main`
8. stop with no active next-subsystem branch

Do not create another development branch during or immediately after O6.

## Remaining major roadmap after Object merge

Each item is a separate future subsystem/branch after final Object integrated `main` is certified and the user explicitly directs continuation:

1. `InteractionRuntime` behavioral encapsulation
2. `ProjectRuntime` ownership extraction
3. `UiRuntime` ownership extraction
4. `DiagnosticsRuntime` ownership extraction
5. Reference renderer parity gates and external visual oracle
6. 117HD dynamic/placed lights
7. 117HD sky/day-night/reflection systems and remaining effects
8. edited-cache fingerprint/profile strategy, new-group support, and broader index repack when justified
9. project/browser lifecycle polish and future import/merge product work

## Cache/export current state

Implemented:

- locations exact 2936/2936
- terrain 2935/2936 retained semantics
- five-file repack exact
- map stream encoding and cache-copy repack for existing map groups

Known limitations:

- only existing map-square groups are repacked
- new groups absent from the reference table are not created
- other indices are not generally rewritten by the editor export path
- edited cache copies intentionally no longer match the pristine-cache fingerprint/profile

## 117HD high-level state

Already substantially implemented:

- render-profile/HDR foundation
- HD data/material tables
- HD terrain/model extraction
- material replacement
- ground blending/normals
- environments/fog/directional lighting
- directional shadows/filtering
- water surfaces, flow/normal maps, foam, Fresnel, underwater tint/caustics
- animated-loc HD material routing
- material flow maps
- parallax/displacement and displaced shadow position

Large remaining systems include dynamic/placed point lights, sky/day-night/reflection systems, remaining post-processing/settings parity, and remaining model/material edge cases.
