# M6 Exit Audit: Terrain, Loc Placement, and Plane Scene Construction

Status: **COMPLETE - FINAL BRANCH/PR VALIDATION REQUIRED**  
Milestone: `M6 - Terrain, loc placement, and plane scene construction`  
Branch: `impl/m6-scene-construction`  
Baseline: M5 squash merge `e907bbfeadb9a9b8da6b2fbdf746f9b7f7f26c7c`

## Exit decision

M6 is implementation-complete for its owned semantic scene-construction scope.

The milestone establishes the first cache-independent semantic region scene with exact terrain topology, location placement/storage, footprint-height sampling, typed plane semantics, structural link-below processing, definition-driven side-effect planning, and semantic scene queries.

M6 does not claim later normals/lighting, morph/animation/contour, render extraction, GPU, or editor behavior.

## Canonical exit gates

The M6 roadmap requires:

1. semantic tile storage independent of GPU buffers;
2. exact flat/shaped terrain and `SceneTileModel` shapes `0..12` with rotations;
3. exact loc dispatch, orientation/displacement, footprint/center sampling, and floor-decoration no-lift behavior;
4. explicit source/storage/collision/render plane concepts plus structural link-below;
5. definition-driven collision/clipping/occlusion/wall scaffolding only where exact contracts are promoted;
6. semantic scene query APIs;
7. fully owned M6 parity rows at `EXISTING`, except narrower `LOC-PLACEMENT-005` subcontracts explicitly allowed to remain incremental;
8. four required golden semantic scenes;
9. Tier C semantic verification passing before merge.

Items 1 through 8 are closed. Tier C now permanently runs `cargo test --locked -p osrs-scene`; the exact final branch and PR heads must pass Tier A/B/C before merge.

## Terrain: PASS

`crates/osrs-scene/src/terrain.rs` implements the pinned integer `SceneTileModel` construction contract for all shapes `0..12` and rotations `0..3`, including exact vertices, face indices, underlay/overlay ownership, texture assignment, and flatness.

`crates/osrs-scene/tests/m6_terrain_shape_gallery.rs` checks the complete `13 x 4` gallery against indexed historical evidence `terrain.shape_gallery.all_13x4`.

Checkpoint 6 closes `TERRAIN-002` with explicit flat/shaped semantic ownership and `crates/osrs-scene/src/terrain_contract.rs`, which preserves the pinned reference sentinel behavior:

- flat paint skips when northeast color is `12345678`;
- a shaped face skips when its first authored face color is `12345678`.

`TERRAIN-001` and `TERRAIN-002` are `EXISTING`.

`TERRAIN-004` remains `BLOCKED` / revision-sensitive. M6 does not guess the unverified complete terrain color/light builder.

## Loc placement and storage: PASS

The production placement path covers:

- type `22` floor decoration;
- types `10/11` full-footprint game objects, with type `11` flag `256`;
- types `0..3` boundary variants;
- types `4..8` wall decorations;
- type `9` 1x1 scene-storage game-object path;
- representative `>=12` 1x1 scene-storage game-object paths.

Exact wall/decor orientation tables, integer displacement, dual type-2 boundary arms, floor-decoration no-lift semantics, and bounded scene-layer insertion/query behavior are exercised by M6 tests.

`LOC-PLACEMENT-001`, `LOC-PLACEMENT-002`, and `LOC-PLACEMENT-006` are `EXISTING`.

## Footprint height sampling: PASS

Checkpoint 6 adds `crates/osrs-scene/src/placement_height.rs`, source-pinned to the initial `FriendSystem.addObjects(...)` path at `melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`.

The exact contract is:

- rotate definition footprint first;
- use the two integer midpoint indices per axis when the footprint fits;
- use anchor and anchor+1 at the scene edge;
- sample four height-grid points;
- sum with Java `int` wrapping semantics and arithmetic-shift right by two.

The composed exit test feeds the exact sample into production placement and verifies semantic model/storage centers.

`LOC-PLACEMENT-003` is `EXISTING`.

## Plane semantics and structural link-below: PASS

M6 keeps source, storage, collision, and render-level concepts distinct and implements exact collision-plane bridge adjustment plus four-plane `setLinkBelow` relinking.

The normalized M5 fixture `planes.link_below.four_plane_column` is promoted in M6 to execute production `osrs-scene`. That promotion necessarily changes:

- `crates/osrs-reference/src/runner.rs`;
- `reference-fixtures/manifest/planes-link-below-four-plane-column.yaml`.

These are M6 verification-boundary changes, not production ownership leakage. `osrs-reference` remains a dev/test consumer of production semantic code, and production crates still do not depend on it.

`PLANES-001`, `PLANES-002`, and `PLANES-003` are `EXISTING`.

## Definition-driven side effects: PASS within promoted scope

M6 preserves and plans exact operation-level semantics for interaction type, projectile blocking, clipping/model-clipping/ground-obstruction inputs, solidity, collision category, and wall displacement.

Implemented exact behavior includes:

- floor-decoration blocking only for `interactType == 1`;
- boundary collision retaining type/orientation/projectile behavior;
- game-object collision using the rotated definition footprint, including visually 1x1 type `9` storage;
- no invented wall-decoration collision;
- audited wall-displacement metadata updates.

`LOC-PLACEMENT-005` intentionally remains `PARTIAL` under the M6 roadmap exception. Private collision/shadow/occlusion bit-grid formulas are not guessed or copied without promoted contracts.

`LOC-PLACEMENT-004` remains later pending/live replacement work and is not an M6 exit gate.

## Semantic coordinate boundary: PASS

Checkpoint 6 adds the required region-border composed fixture. It crosses a 64-tile region boundary and proves:

- in-region `63 -> 0` transition;
- stable world/map identity;
- exact scene/map round trip;
- exact 128-unit local coordinates and 64-unit half-tile centers;
- independent semantic storage/query on both sides of the boundary.

The semantic side of `COORD-003` is `EXISTING`. Camera/view/clip conversion remains renderer-owned.

## Required golden semantic scenes: PASS

All four M6 exit scenes exist:

1. terrain shape gallery: `crates/osrs-scene/tests/m6_terrain_shape_gallery.rs`;
2. wall/decor orientation scene: `m6_exit_golden_scenes::golden_wall_and_decor_orientation_scene_survives_semantic_storage`;
3. four-plane bridge column: `m6_exit_golden_scenes::golden_four_plane_bridge_column_relinks_structurally` plus normalized fixture `planes.link_below.four_plane_column`;
4. region-border coordinate fixture: `m6_exit_golden_scenes::golden_region_border_fixture_preserves_world_scene_and_local_identity`.

## Architecture and scope audit

Against M5 baseline `e907bbfeadb9a9b8da6b2fbdf746f9b7f7f26c7c`, the final pre-PR scope is expected to contain only:

- `.github/workflows/ci.yml` for the M6 Tier C gate;
- `crates/osrs-scene/**` for production scene semantics and M6 tests;
- `crates/osrs-reference/src/runner.rs` solely to execute the promoted plane-link normalized fixture against production `osrs-scene`;
- `reference-fixtures/manifest/planes-link-below-four-plane-column.yaml` solely to promote that fixture from evidence-only to semantic execution;
- M6-related verification and implementation documentation.

No `osrs-core` or `osrs-cache` production semantics are changed by M6. No renderer/editor crate is introduced. Cache transport remains outside `osrs-scene`. Production crates remain forbidden from depending on `osrs-reference`.

## Deferred ownership

M6 does not implement:

- `LOC-PLACEMENT-004` pending/live replacement;
- production base normals or cross-model normal merge;
- scene normal reconciliation or final lighting;
- morph, contour, or animation execution;
- renderer face-priority execution;
- render extraction, wgpu rendering, or editor behavior.

Those remain M7 and later work.

## Final validation and merge rule

Before merge:

1. compare exact branch head against the unchanged M5 baseline;
2. verify every changed file is within the scope above;
3. require final branch Tier A/B/C success, including the M6 semantic suite;
4. open one M6 PR;
5. review exact PR files and diff;
6. verify `main` has not moved;
7. require PR-triggered Tier A/B/C on the exact PR head;
8. squash merge only when mergeable and green;
9. verify `main` points to the M6 squash commit;
10. stop before M7.

## Milestone conclusion

All implementation work required by M6's fully owned semantic contracts is present. `LOC-PLACEMENT-005` remains intentionally partial under its roadmap exception, and `TERRAIN-004` remains blocked rather than guessed.

M7 must not begin until M6 is merged and the user explicitly continues.
