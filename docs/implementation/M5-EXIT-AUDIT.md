# M5 Exit Audit: Reference Fixture Infrastructure

Status: **COMPLETE - PR READY**  
Milestone: `M5 - Reference fixture infrastructure`  
Branch: `impl/m5-reference-fixture-infrastructure`  
Baseline: M4 squash merge `4d3d21dbe449cd345cf46ccbab651bec6366d585`

## Exit decision

M5 is implementation-complete for its reference-fixture infrastructure scope.

The milestone establishes deterministic, source-pinned, offline-consumable fixture infrastructure before M6-M8 scene, normal, lighting, contour, and related semantics depend on differential evidence. It also establishes a narrow evidence-only path for contracts whose production executor does not yet exist without allowing existing production semantics to bypass their executable fixture checks.

This audit does not claim that later-owned semantics are already implemented merely because M5 now preserves or indexes evidence for them.

In particular:

- `FACE-002` remains `REQUIRED`;
- placement rows retain their M5-entry status;
- contour and lighting rows retain their M5-entry status;
- normal merging, bridge/plane scene construction, morph execution, animation execution, render extraction, GPU rendering, and editor behavior remain deferred to their owning milestones.

## Canonical M5 roadmap gates

The M5 roadmap requires:

1. ordinary Rust CI to run offline against checked-in expected outputs;
2. every new fixture to carry exact provenance;
3. regeneration to be unable to silently rewrite accepted expected outputs without reviewable source/manifest changes;
4. relevant `PARITY-MATRIX.md` rows to link concrete fixture/test identifiers.

All four gates are closed by the M5 branch.

## 1. Offline ordinary verification: PASS

Ordinary CI does not require the public deob checkout, Java oracle execution, network downloads, or regeneration.

Permanent gates include:

- Tier A architecture checks;
- `scripts/test_reference_regeneration.py`;
- `scripts/test_deob_golden_index.py`;
- rustfmt;
- locked workspace check;
- strict Clippy;
- Tier B workspace tests;
- Tier C M3/M4 semantic gates;
- Tier C `cargo test --locked -p osrs-reference --test m5_fixture_runner`.

The M5 runner recursively discovers YAML manifests under `reference-fixtures/manifest/`, sorts deterministically, rejects symlinks, verifies provenance and expected hashes, rejects duplicate fixture IDs, and rejects an empty inventory.

## 2. Exact fixture provenance: PASS

### Normalized semantic fixtures

The canonical M5 inventory contains three fixtures that execute through production semantic functions:

1. `model.selection.typed_exact.orientation_4`
   - manifest: `reference-fixtures/manifest/model-selection-typed-orientation-4.yaml`;
   - owned specs: `MODEL-BUILD-001`, `MODEL-BUILD-002`;
   - production executor: `select_object_model`.
2. `model.mirror.geometry_winding`
   - manifest: `reference-fixtures/manifest/model-mirror-geometry-winding.yaml`;
   - owned spec: `MODEL-BUILD-002`;
   - production executor: `mirror_source_model`.
3. `model.transform.type4_order`
   - manifest: `reference-fixtures/manifest/model-transform-type4-order.yaml`;
   - owned specs: `MODEL-BUILD-003`, `COORD-002`;
   - production executor: `apply_object_model_instance_transforms`.

Each manifest records:

- stable fixture ID;
- schema version;
- owned spec IDs;
- parity level;
- exact public oracle repository/commit/file/blob/symbol pins;
- exact migration/harness revision;
- normalized input and expected-output paths;
- expected-output SHA-256;
- explicit normalization rules.

### Evidence-only normalized fixture

The fourth canonical YAML fixture is:

`priority.all_0_11.threshold_crossing`

It is source-pinned to:

- `melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`;
- `runescape-client/src/main/java/Model.java`;
- blob `c2aa55c0e8fea89fae0da33d782119f8c109cacf`;
- symbol `method5946`.

Its expected artifact preserves:

- priorities `0..11`;
- `avg12 = 80`;
- `avg34 = 50`;
- `avg68 = 20`;
- priority-10 queue behavior;
- priority-11 queue behavior;
- representative signed alpha metadata;
- exact ordered face IDs.

It is intentionally `execution: evidence_only` because the renderer-owned production priority executor does not exist yet.

The runner explicitly rejects `priority_order` as a semantic executor today and also rejects using `evidence_only` to bypass any fixture kind that already has an M5 production semantic executor. The downgrade-bypass regression is permanently tested.

Therefore `FACE-002` remains `REQUIRED`.

## 3. Historical evidence migration/indexing: PASS

M5 does not rewrite or relabel the historical `reference-fixtures/deob_golden.txt` as if it were generated from the pinned public source.

Instead:

`reference-fixtures/historical/deob_golden.index.json`

records the classification:

`historical_local_harness_corroborated_by_public_source`

The historical source and harness identities are byte-gated as Git blobs:

- `reference-fixtures/deob_golden.txt` -> `49887733ad463572cf61bc059733b7c5f5fd26f4`;
- `tools/deob-harness/src/Dumper.java` -> `ceefbd6e97c8e0b09c2ef196b3f9fd2e0f763236`.

The index exposes seven stable evidence IDs:

1. `terrain.shape_gallery.all_13x4`;
2. `contour.synthetic.flat_slope`;
3. `lighting.synthetic_triangle.loc_rig`;
4. `placement.wall_types.orientation_matrix`;
5. `placement.decor_types.orientation_matrix`;
6. `placement.floor_type22.storage`;
7. `placement.game_object.footprint_and_capacity`.

`scripts/test_deob_golden_index.py` verifies offline:

- exact checked-in historical source/harness Git blob identity;
- historical evidence classification;
- unique fixture IDs;
- public source-pin structure;
- exact-line uniqueness;
- expected prefix counts.

This converts an opaque historical dump into reviewable indexed evidence without falsifying provenance.

## 4. Source-pin closure: PASS

`docs/verification/SOURCE-PINS.md` now reflects the M5 implementation state rather than the pre-M5 harness state.

The current harness requires explicit caller-supplied checkout, expected commit, dependency JAR, output path, and optional work directory. The old absolute developer-machine path remains documented only as historical provenance for the existing golden dump.

M5 source pins were revalidated against exact public revision:

`melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`

The M5-facing public file pins are recorded for:

- `ObjectComposition.java` -> `079451cd9a6dcfd2666efd15b0524250eaafe4c4`;
- `ModelData.java` -> `2cc9406b2504fbd4fae0c0c952aa2d133809e928`;
- `Model.java` -> `c2aa55c0e8fea89fae0da33d782119f8c109cacf`;
- `Rasterizer3D.java` -> `f32216b5e564c6a03a173438e3b19004c27c1c9e`;
- `Scene.java` -> `f15260a63103952fe8f5ffbdb62f5c7c39d94565`;
- `SceneTileModel.java` -> `ce6a179cfa93e02271af87164e102ee538223718`.

The unresolved whole-snapshot equivalence of the old developer-machine checkout remains explicit and is not needed to claim exact provenance for the individually pinned public M5 fixtures.

`TERRAIN-004` remains blocked and revision-sensitive. M5 does not revive the disproven `class470` attribution.

## 5. Regeneration safety: PASS

`tools/reference-fixtures/regenerate.py` and `tools/deob-harness/run.sh` implement a candidate-only regeneration boundary.

The workflow requires explicit source/dependency/output inputs and:

- rejects candidate output paths inside `reference-fixtures/`;
- rejects silently overwriting existing candidate files;
- provides no automatic `--accept` path;
- does not edit fixture manifests or expected hashes;
- does not clone/download dependencies during ordinary operation;
- is not invoked by ordinary CI.

Tier A permanently tests this safety contract offline through `scripts/test_reference_regeneration.py`.

Any future accepted fixture change must therefore be an explicit repository diff that updates the relevant source/manifest/hash evidence rather than an ordinary test side effect.

## 6. Parity-matrix linkage: PASS

`docs/verification/PARITY-MATRIX.md` now tracks through M5 and links concrete M5 fixture IDs to relevant semantic rows.

The matrix distinguishes between:

- normalized semantic fixtures executing production code;
- evidence-only normalized fixtures;
- indexed historical evidence.

The links do not over-promote later production semantics. In particular:

- `FACE-002` stays `REQUIRED` despite the priority evidence fixture;
- placement remains `PARTIAL` or `REQUIRED` as appropriate;
- `CONTOUR-001` remains `PARTIAL`;
- `LIGHTING-001` remains `PARTIAL`;
- M6/M7 scene and lighting executors are not implied to exist.

## 7. Architecture and scope audit: PASS

M5 remains development/test infrastructure.

The branch adds or changes only:

- `osrs-reference` fixture infrastructure;
- checked-in fixture inputs/expected outputs/manifests/indexes;
- reference/deob tooling;
- verification scripts;
- CI gates;
- M5/provenance/parity documentation.

It does not add M5-owned changes to:

- `osrs-core` production semantics;
- `osrs-cache` production semantics;
- `osrs-scene` production semantics;
- renderer implementation;
- editor implementation.

Production crates remain forbidden from depending on `osrs-reference`.

## 8. Required fixture-family roadmap audit

The M5 roadmap lists first required families for model selection, mirror/winding, transform order, base normals, normal merge controls, lighting controls, loc dispatch/orientation, plane/bridge synthetic inputs, and priority order.

M5 closes the infrastructure and canonical source/evidence foundation needed for these families without stealing implementation ownership from later milestones:

- model selection, mirror/winding, and transform order have normalized executable semantic fixtures now;
- priority order has a normalized source-pinned evidence-only fixture now;
- terrain/placement/contour/lighting historical evidence is indexed now;
- the remaining normal-merge, bridge/plane, richer lighting, and scene-placement normalized fixtures are intentionally produced/activated with their owning production milestones so M5 does not fabricate expected output disconnected from an implementation checkpoint.

This interpretation is consistent with the M5 purpose: make the reference fixture system real before dangerous scene semantics are ported, not prematurely implement M6-M8.

## 9. Deferred work

M5 does not implement:

- semantic scene tile storage;
- loc placement dispatch;
- bridge/collision/storage/render plane behavior;
- cross-model normal merging;
- final reference lighting;
- contour execution;
- morph execution;
- animation execution;
- renderer face-priority execution;
- render extraction;
- wgpu rendering;
- editor behavior.

Those remain bound to M6 and later milestones.

## Milestone conclusion

All M5 roadmap exit gates are closed by checked-in implementation and documentation.

The milestone branch is ready for final exact-head Tier A/B/C validation, one M5 pull request, exact changed-file review, PR-triggered CI, and squash merge.

After M5 is merged, stop before M6. M6 must begin only after an explicit user instruction on a fresh implementation branch from the resulting `main`.
