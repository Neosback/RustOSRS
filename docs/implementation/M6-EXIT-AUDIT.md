# M6 Exit Audit: Terrain, Loc Placement, and Plane Scene Construction

Status: **COMPLETE - FINAL BRANCH/PR VALIDATION REQUIRED**  
Milestone: `M6 - Terrain, loc placement, and plane scene construction`  
Branch: `impl/m6-scene-construction`  
Baseline: M5 squash merge `e907bbfeadb9a9b8da6b2fbdf746f9b7f7f26c7c`

## Exit decision

M6 is implementation-complete for its owned semantic scene-construction scope.

The milestone establishes the first cache-independent semantic scene with exact shaped terrain topology, explicit flat terrain ownership, location dispatch and storage, footprint/height sampling, typed plane semantics, structural link-below behavior, definition-driven side-effect planning, and semantic scene queries.

M6 does not claim later normal/lighting, morph/animation/contour, render extraction, GPU, or editor behavior.

## Canonical M6 roadmap gates

The M6 roadmap requires:

1. semantic tile storage independent of GPU buffers;
2. exact flat/shaped terrain ownership and `SceneTileModel` shapes `0..12` with rotations;
3. exact loc type dispatch, orientation/displacement tables, footprint/center sampling, and floor-decoration no-lift behavior;
4. explicit source/storage/collision/render plane concepts, collision-plane bridge adjustment, and structural link-below processing;
5. definition-driven collision/clipping/occlusion/wall side-effect scaffolding only where source contracts are promoted;
6. semantic scene query APIs;
7. fully owned M6 parity rows at `EXISTING`, except narrower `LOC-PLACEMENT-005` subcontracts explicitly allowed to remain incremental;
8. four required golden semantic scenes;
9. Tier C semantic verification passing before merge.

The implementation closes items 1 through 8. Item 9 is enforced by `.github/workflows/ci.yml`, which now runs `cargo test --locked -p osrs-scene` in Tier C. The exact final branch and PR heads must pass Tier A/B/C before merge.

## 1. Semantic tile storage and scene queries: PASS

`SceneGrid` owns bounded semantic tile storage with explicit plane count and nullable tile slots required by structural relinking.

The scene preserves distinct slots for:

- floor decorations;
- boundary objects;
- wall decorations;
- game objects.

Game-object insertion preserves:

- exact storage footprint;
- shared semantic instance identity across occupied tiles;
- reference per-tile edge masks;
- five-game-object per-tile capacity;
- atomic footprint rejection;
- anchor identity used by scene queries and link-below plane updates.

Tests cover all loc dispatch paths `0..22` plus representative `>=12`, fixed-layer queries, dual boundary arms, rotated non-square footprints, capacity, and multi-tile identity.

## 2. Exact terrain shape semantics: PASS

`crates/osrs-scene/src/terrain.rs` implements the pinned `SceneTileModel` integer construction path for all shapes `0..12` and rotations `0..3`.

The production representation preserves:

- exact template vertex sets;
- local positions at corners/midpoints/quarter points;
- Java-style midpoint interpolation;
- exact face indices;
- underlay/overlay face ownership;
- optional overlay texture assignment;
- exact flatness state;
- flat terrain four-corner heights/colors and reference two-triangle diagonal.

`crates/osrs-scene/tests/m6_terrain_shape_gallery.rs` checks the complete `13 x 4` gallery against indexed historical evidence `terrain.shape_gallery.all_13x4`.

`TERRAIN-001` is `EXISTING`.

## 3. Flat/shaped terrain and sentinel contract: PASS

Checkpoint 6 closes the remaining `TERRAIN-002` production gap.

The semantic scene retains a mutually exclusive `TerrainSurface::Flat` / `TerrainSurface::Shaped` representation rather than reconstructing a generic renderer quad.

`crates/osrs-scene/src/terrain_contract.rs` pins the imported reference sentinel behavior:

- flat paint is skipped when its northeast color is `12345678`;
- a shaped terrain face is skipped when its first authored face color is `12345678`.

The helper only preserves semantic skip meaning. It does not add batching, UV, buffer, shader, or raster policy to `osrs-scene`.

`TERRAIN-002` is `EXISTING`.

## 4. Exact loc dispatch and orientation semantics: PASS

The production placement planner owns the audited type mapping:

- `22` floor decoration;
- `10/11` full-footprint game object with type-11 flag `256`;
- `0..3` boundary variants;
- `4..8` wall-decoration variants;
- `9` 1x1 scene-storage game-object path;
- `>=12` 1x1 scene-storage game-object path.

The exact wall/decor tables and integer displacement behavior are preserved, including full displacement for type `5`, half displacement for `6/8`, opposite orientation for `7`, and dual semantic arms for type `2`.

Historical orientation evidence is checked by `m6_loc_placement.rs`, while composed scene storage is checked by `m6_placement_matrix.rs`, `m6_scene_insertion.rs`, and the M6 exit golden scenes.

`LOC-PLACEMENT-001` and `LOC-PLACEMENT-002` are `EXISTING`.

## 5. Footprint center and terrain-height sampling: PASS

Checkpoint 6 adds `crates/osrs-scene/src/placement_height.rs`, source-pinned to the initial `FriendSystem.addObjects(...)` path in:

`melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`

The exact placement-height contract is:

- rotate the definition footprint first;
- for a footprint that fits the scene extent, sample the two integer midpoint indices on each axis;
- when the footprint would extend beyond the scene extent, use the reference anchor and anchor+1 fallback;
- sample the four resulting terrain-height grid points;
- sum with Java `int` wrapping behavior and arithmetic-shift right by two.

The composed exit fixture feeds that sampled height into the production placement planner and proves the resulting semantic model/storage centers retain it exactly.

`LOC-PLACEMENT-003` is `EXISTING`.

## 6. Floor-decoration height behavior: PASS

Floor decoration storage preserves the supplied sampled semantic height on flat and synthetic sloped inputs.

There is no generic global vertical lift in scene placement.

`LOC-PLACEMENT-006` is `EXISTING`.

## 7. Plane domains, collision adjustment, and link-below: PASS

The semantic foundation keeps source, storage, collision, and render-level concepts distinct.

M6 implements:

- exact collision-plane bridge adjustment;
- structural four-plane `setLinkBelow` relinking;
- tile storage-plane relocation;
- linked-below preservation of the original plane-zero tile;
- qualifying anchored game-object plane decrements propagated across multi-tile instance occupancy.

The normalized fixture `planes.link_below.four_plane_column` executes against production `osrs-scene`, and the M6 exit suite includes a composed four-plane bridge column.

`PLANES-001`, `PLANES-002`, and `PLANES-003` are `EXISTING`.

## 8. Definition-driven side-effect planning: PASS within promoted scope

M6 preserves canonical object-definition inputs needed to regenerate non-renderable scene meaning:

- interaction type;
- projectile blocking;
- clipping;
- model clipping;
- ground obstruction;
- solidity;
- wall-decoration displacement.

The side-effect planner source-pins exact operation-level behavior for:

- floor-decoration blocking only for `interactType == 1`;
- boundary collision retaining loc type/orientation/projectile behavior;
- game-object collision using the rotated definition footprint, including visually 1x1 type `9` storage;
- no invented wall-decoration collision;
- audited wall-displacement metadata updates.

`LOC-PLACEMENT-005` intentionally remains `PARTIAL`. The M6 roadmap explicitly permits narrower side-effect subcontracts not yet promoted to remain incomplete. M6 does not copy or guess the reference client's private collision, shadow/clipping, or occlusion bit-grid formulas.

## 9. Semantic coordinate boundary: PASS

Checkpoint 6 adds the required region-border composed fixture.

The fixture crosses a 64-tile region boundary and proves:

- in-region coordinate transition `63 -> 0`;
- stable world/map identity;
- scene-relative coordinates can span the border without changing world identity;
- scene-to-map round trip is exact;
- semantic local centers remain exact 128-unit coordinates with the 64-unit half-tile center;
- both border-adjacent objects remain independently queryable after scene insertion.

The semantic side of `COORD-003` is `EXISTING`. Camera/view/clip conversion remains renderer-owned and is not implemented by M6.

## 10. Required golden semantic scenes: PASS

The four M6 exit scenes are now checked in:

1. terrain shape gallery
   - `crates/osrs-scene/tests/m6_terrain_shape_gallery.rs`
   - indexed historical ID `terrain.shape_gallery.all_13x4`;
2. wall/decor orientation scene
   - `m6_exit_golden_scenes::golden_wall_and_decor_orientation_scene_survives_semantic_storage`;
3. four-plane bridge column
   - `m6_exit_golden_scenes::golden_four_plane_bridge_column_relinks_structurally`
   - normalized fixture `planes.link_below.four_plane_column`;
4. region-border coordinate fixture
   - `m6_exit_golden_scenes::golden_region_border_fixture_preserves_world_scene_and_local_identity`.

## 11. Terrain revision-sensitive boundary: PRESERVED

`TERRAIN-004` remains `BLOCKED` / revision-sensitive.

M6 does not infer or implement the unverified complete terrain color/light builder, including the historical 11x11 neighborhood, jitter, slope-light, overlay special-color composition, or related derived flags.

Decoded floor-definition inputs and independently verified helpers remain preserved for a later evidence-backed closure.

## 12. Later semantic ownership remains intact

M6 does not implement:

- `LOC-PLACEMENT-004` pending/live replacement semantics;
- production base-normal generation;
- cross-model normal merging;
- scene normal reconciliation;
- final reference lighting;
- morph resolution;
- contour execution;
- animation execution;
- renderer face-priority execution;
- render extraction;
- wgpu rendering;
- editor behavior.

Those remain assigned to M7 and later milestones.

## 13. Architecture and scope boundary: PASS

M6 production changes remain within semantic scene construction and its verification boundary.

Expected milestone files are limited to:

- `crates/osrs-scene/**`;
- M6-relevant semantic verification/implementation documentation;
- the CI semantic-parity gate needed to execute `osrs-scene` in Tier C.

Existing lower-layer `osrs-core` coordinate types are consumed rather than duplicated. Cache transport remains outside `osrs-scene`. Production crates do not depend on `osrs-reference`. No renderer or editor crate is introduced.

## Final validation and merge rule

Before the M6 PR may merge:

1. compare the exact branch head against `main` baseline `e907bbfeadb9a9b8da6b2fbdf746f9b7f7f26c7c`;
2. verify the complete changed-file list contains only expected M6 scope;
3. run final branch-head Tier A, Tier B, and Tier C, including the M6 semantic scene suite;
4. open one M6 PR;
5. review the exact PR changed-file list and diff;
6. verify `main` has not moved;
7. require PR-triggered Tier A/B/C success on the exact PR head;
8. squash merge only if the PR is mergeable and all checks remain green;
9. verify `main` points to the resulting M6 squash merge commit;
10. stop before M7.

## Milestone conclusion

All implementation work required by M6's fully owned semantic contracts is present. The narrower `LOC-PLACEMENT-005` exception remains intentionally partial and `TERRAIN-004` remains explicitly blocked rather than guessed.

The branch is ready for final branch-head validation and the M6 PR/merge sequence. M7 must not begin until M6 is merged and the user explicitly continues.
