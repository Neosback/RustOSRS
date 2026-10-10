# M7 Exit Audit: Normals, Lighting, and Scene Finalization

Status: **COMPLETE - FINAL BRANCH/PR VALIDATION REQUIRED**  
Milestone: `M7 - Normals, lighting, and scene finalization`  
Branch: `impl/m7-normals-lighting`  
Baseline: M6 squash merge `1bf660ff8bd1855d822db54f5557102405e19036`  
Checkpoint 9 implementation head: `f6586eb0a9bef7ec2ddc5db35360cd6c0d50d053`

## Exit decision

M7 is implementation-complete for its owned normal generation, cross-model reconciliation, initial static ModelData lifecycle, scene finalization, exact object-lighting, and constructed-model identity scope.

The milestone closes the highest-risk pre-renderer lighting semantics without moving semantic truth into a renderer. Final models are produced only after required scene-local normal reconciliation, and qualifying `nonFlatShading` initial static entities remain ModelData long enough to participate in that pass.

M7 does not claim pending/live replacement, contouring, morph execution, animation pose, renderer priority/transparency, GPU extraction, or editor behavior.

## Canonical exit gates

The M7 roadmap requires:

1. exact base normal generation with smooth/flat distinction;
2. translated cross-model normal reconciliation without topology welding;
3. merged-normal ownership separate from immutable source data;
4. matched-face render type `2` behavior;
5. scene traversal for boundaries, game objects, floor decorations, dual-arm walls, and plane-above neighbors;
6. initial `nonFlatShading` ModelData lifecycle;
7. final ModelData-to-lit-Model conversion after reconciliation;
8. exact ambient/contrast/light-vector integer lighting;
9. all normal/lighting parity-matrix rows at `EXISTING`;
10. permanent Tier C coverage for the milestone-owned semantic suite before merge.

Items 1 through 9 are closed by checked-in implementation and exact tests. The CI workflow now contains an explicit M7 Tier C step. The exact final branch and PR heads must pass Tier A/B/C before merge.

## Base normals: PASS

`crates/osrs-core/src/normals.rs` implements the pinned integer face-normal and vertex-normal behavior before final lighting.

Exact coverage includes:

- smooth triangle accumulation;
- flat face-normal ownership;
- shared-vertex accumulation across a two-triangle quad;
- winding reversal changing the exact normal direction;
- non-smooth render types not contributing smooth vertex normals;
- source-pinned smooth/flat normalized fixtures through `crates/osrs-reference/tests/m7_base_normals.rs`;
- source immutability while calculating normals.

`NORMALS-001` is `EXISTING`.

## Cross-model normal reconciliation: PASS

`merge_model_normals` implements scene-local normal accumulation using translated integer vertex equality. It does not weld vertices, faces, or scene objects.

Coverage includes:

- zero-translation positive merge;
- translated positive merge using the reference subtraction convention;
- translated no-match control;
- `hideMatchedFaces=false` while normals still merge;
- `hideMatchedFaces=true` with fully matched faces authored as render type `2`;
- repeated reconciliation accumulating from existing merged-normal slots;
- source models remaining unchanged.

The source-pinned normalized controls remain in `reference-fixtures/normals/` and execute production behavior through `crates/osrs-reference/tests/m7_normal_merge.rs`.

`NORMALS-002` is `EXISTING`.

## Scene reconciliation traversal: PASS

`crates/osrs-scene/src/normal_finalization.rs` owns scene-level reconciliation before final lighting.

`crates/osrs-scene/tests/m7_scene_normal_reconciliation.rs` proves the required traversal controls:

- type-2 style dual-arm boundary reconciliation with no arm-to-arm face hiding;
- boundary-to-game-object footprint translation;
- floor-decoration neighbor reconciliation with matched-face hiding;
- plane-above reconciliation using relative average-height delta;
- pending ModelData close-state transition after reconciliation.

Boundary, game-object, and floor-decoration identities remain separate while only eligible normal state is shared.

`NORMALS-003` is `EXISTING`.

## Merged normals affect final lighting: PASS

`crates/osrs-reference/tests/m7_lighting.rs` includes a control where cross-model reconciliation intentionally changes the selected vertex normal and therefore changes the exact final baked face color.

The final lighting path selects a merged vertex normal when one exists and otherwise uses the base vertex normal.

`NORMALS-004` is `EXISTING`.

## Exact reference object lighting: PASS

`crates/osrs-core/src/lighting.rs` owns the integer ModelData-to-lit-model conversion.

The exact test suite covers:

- the checked-in historical loc lighting rig;
- reference HSL/lightness helper clamps;
- smooth and flat faces;
- textured and untextured paths;
- mapped texture triangles/selectors;
- merged-normal precedence;
- ambient and contrast extremes using signed integer semantics;
- alpha sentinel behavior.

Initial object lighting parameters remain definition-driven through the pinned contract:

```text
ambient  = objectDefinition.ambient + 64
contrast = objectDefinition.contrast + 768
lightX   = -50
lightY   = -10
lightZ   = -50
```

`LIGHTING-001` is `EXISTING`.

## Initial static entity lifecycle: PASS

`crates/osrs-core/src/static_entity.rs` preserves the required `nonFlatShading` split.

For `nonFlatShading == false`, transformed ModelData is finalized to a lit semantic model and may be reused from the initial static entity cache.

For `nonFlatShading == true`, the cache retains normalized ModelData plus loc lighting parameters and returns a scene-local working copy with base normals available for reconciliation.

`crates/osrs-reference/tests/m7_static_entity_lifecycle.rs` verifies:

- flat entities are lit once and reused;
- non-flat entities remain ModelData rather than being prematurely lit;
- scene-local mutation does not mutate the cached ModelData or source model;
- missing model data remains absent;
- flat/non-flat branches retain distinct semantic representations.

`MODEL-BUILD-004` is `EXISTING`.

The scene-normal/source-immutability portion of `MODEL-BUILD-005` is closed. The overall spec remains `PARTIAL` because contouring and animation pose are later-owned.

## Initial placement through final scene lighting: PASS

`crates/osrs-scene/src/reference_finalization.rs` and `crates/osrs-reference/tests/m7_initial_scene_finalization.rs` connect initial non-flat entities to the real scene reconciliation and final-lighting lifecycle.

Coverage proves:

- adjacent initial non-flat floor decorations reconcile before final lighting;
- matched-face suppression is reflected in final lit output;
- retained loc ambient/contrast parameters are used after reconciliation;
- source assets remain unchanged;
- unplaced ModelData is rejected rather than prematurely finalized.

This closes the M7-owned initial side of `LOC-PLACEMENT-004`. The full row remains `PARTIAL` because the pending/live replacement half is intentionally deferred.

## Constructed-model semantic identity: PASS

M7 keeps model provenance/identity semantically meaningful through the complete working and lighting lifecycle.

`crates/osrs-reference/tests/m7_identity_threading.rs` proves identity preservation through working ModelData, lit output, and static entity cache paths.

Checkpoint 9 closes the remaining genuine multi-source seam with `WorkingModel::from_assembled(&AssembledModel)` and `crates/osrs-reference/tests/m7_composite_admission.rs`:

- a real two-source M4 assembly is admitted into M7;
- the complete ordered composite `ModelSemanticIdentity` survives final lighting;
- multi-source working/lit models do not expose a fabricated singular model id or format;
- raw source identity/format remains owned by raw `SourceModel`, while reusable semantic payload is identity-neutral.

## Parity matrix hard gate: PASS

`docs/verification/PARITY-MATRIX.md` is updated through this M7 exit audit.

The M7 hard-gate rows are all `EXISTING`:

- `NORMALS-001`;
- `NORMALS-002`;
- `NORMALS-003`;
- `NORMALS-004`;
- `LIGHTING-001`.

The matrix intentionally does not over-promote later-owned work. `MODEL-BUILD-005`, `LOC-PLACEMENT-004`, morph/animation/contour, face priority/transparency, terrain color building, renderer, and editor rows retain their narrower statuses.

## Tier C merge gate: PASS in configuration, exact-head run still required

The CI workflow now explicitly runs an M7 semantic suite in Tier C, including:

- `osrs-core` normal unit tests;
- `osrs-core` lighting unit tests;
- `m7_base_normals`;
- `m7_normal_merge`;
- `m7_lighting`;
- `m7_static_entity_lifecycle`;
- `m7_initial_scene_finalization`;
- `m7_identity_threading`;
- `m7_composite_admission`;
- `m7_scene_normal_reconciliation`.

This makes M7 semantic ownership a permanent merge gate rather than relying only on the broad Tier B workspace test.

## Architecture and scope audit

Against M6 baseline `1bf660ff8bd1855d822db54f5557102405e19036`, M7 is expected to contain only:

- `.github/workflows/ci.yml` for the permanent M7 Tier C gate;
- `crates/osrs-core` normal, lighting, static-entity, working-model identity/lifecycle support;
- `crates/osrs-scene` normal reconciliation and reference-finalization support;
- `crates/osrs-reference/tests/m7_*` exact semantic verification;
- M7 verification/implementation documentation.

M7 does not modify cache transport/decoders, introduce renderer/editor crates, or move semantic ownership into `osrs-reference`.

Production crates remain forbidden from depending on `osrs-reference`. `osrs-core` remains independent of cache transport, wgpu, egui, and eframe. `osrs-scene` remains independent of concrete cache transport.

## Deferred ownership

M7 does not implement or claim:

- pending/live replacement completion for `LOC-PLACEMENT-004`;
- contouring execution;
- morph runtime resolution;
- animation pose/execution;
- the remaining later-owned portions of `MODEL-BUILD-005`;
- complete terrain color building under `TERRAIN-004`;
- renderer face-priority/transparency execution;
- render extraction or wgpu rendering;
- editor behavior.

Those remain M8 and later work according to the roadmap.

## Final validation and merge rule

Before merge:

1. compare the exact final branch head against unchanged M6 baseline `1bf660ff8bd1855d822db54f5557102405e19036`;
2. verify every changed file belongs to the M7 scope above;
3. require final branch Tier A/B/C success, including the explicit M7 Tier C suite;
4. open one M7 PR;
5. review the exact PR file list and diff;
6. verify `main` has not moved from the audited M6 baseline;
7. require PR-triggered Tier A/B/C success on the exact PR head;
8. squash merge only when the PR is mergeable, scope-correct, and green;
9. verify `main` points to the M7 squash commit;
10. stop before M8.

## Milestone conclusion

All implementation work required by M7's owned semantic contracts is present, and every normal/lighting hard-gate row is backed by exact production tests at `EXISTING` status.

The milestone is ready for final exact-head branch validation. A PR must not be opened until that final branch run is green and the changed-file scope has been audited against the unchanged M6 baseline.
