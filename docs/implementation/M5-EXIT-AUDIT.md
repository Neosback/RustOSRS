# M5 Exit Audit: Reference Fixture Infrastructure

Status: **COMPLETE - PR READY**  
Milestone: `M5 - Reference fixture infrastructure`  
Branch: `impl/m5-reference-fixture-infrastructure`  
Baseline: M4 squash merge `4d3d21dbe449cd345cf46ccbab651bec6366d585`

## Exit decision

M5 is implementation-complete for its reference-fixture infrastructure scope.

The milestone establishes deterministic, source-pinned, offline-consumable fixture infrastructure before M6-M8 scene, normals, lighting, contour, and related semantics depend on differential evidence. It also establishes a narrow `evidence_only` path for contracts whose production executor does not yet exist without allowing existing production semantics to bypass executable fixture checks.

This audit does not claim that later-owned semantics are implemented merely because M5 now preserves exact expected evidence for them.

In particular:

- `NORMALS-001` remains `PARTIAL`;
- `NORMALS-002` remains `REQUIRED`;
- `PLANES-003` remains `REQUIRED`;
- `FACE-002` remains `REQUIRED`;
- placement, contour, lighting, morph, animation, render extraction, GPU rendering, and editor behavior retain their existing later-milestone status.

## Canonical M5 roadmap gates

The canonical M5 roadmap requires:

1. ordinary Rust CI to run offline against checked-in expected outputs;
2. every new fixture to carry exact provenance;
3. regeneration to be unable to silently rewrite accepted expected outputs without reviewable source/manifest changes;
4. relevant `PARITY-MATRIX.md` rows to link concrete fixture/test identifiers;
5. the first required fixture families to exist before later dangerous semantics are ported.

All five requirements are closed by the M5 branch.

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

Corrected implementation head `2ecd37d1bfdcfce95ede5306b3f2c8d4aaa872d7` passed workflow `37908853111`:

- Tier A: PASS;
- Tier B: PASS;
- Tier C: PASS, including the M5 runner over all ten canonical YAML fixtures.

## 2. Canonical normalized fixture inventory: PASS

The M5 inventory contains ten canonical YAML fixtures.

### Production-executed semantic fixtures

1. `model.selection.typed_exact.orientation_4`
   - owned specs: `MODEL-BUILD-001`, `MODEL-BUILD-002`;
   - production executor: `select_object_model`.
2. `model.mirror.geometry_winding`
   - owned spec: `MODEL-BUILD-002`;
   - production executor: `mirror_source_model`.
3. `model.transform.type4_order`
   - owned specs: `MODEL-BUILD-003`, `COORD-002`;
   - production executor: `apply_object_model_instance_transforms`.

### Evidence-only fixtures for later production owners

4. `normals.base.flat_triangle`
   - owned spec: `NORMALS-001`;
   - source symbol: `ModelData.calculateVertexNormals`.
5. `normals.base.smooth_triangle`
   - owned spec: `NORMALS-001`;
   - source symbol: `ModelData.calculateVertexNormals`.
6. `normals.merge.coincident_triangle.hide_false`
   - owned spec: `NORMALS-002`;
   - source symbol: `ModelData.method5262`.
7. `normals.merge.coincident_triangle.hide_true`
   - owned spec: `NORMALS-002`;
   - source symbol: `ModelData.method5262`.
8. `normals.merge.translated_negative`
   - owned spec: `NORMALS-002`;
   - source symbol: `ModelData.method5262`.
9. `planes.link_below.four_plane_column`
   - owned spec: `PLANES-003`;
   - source symbol: `Scene.setLinkBelow`.
10. `priority.all_0_11.threshold_crossing`
   - owned spec: `FACE-002`;
   - source symbol: `Model.method5946`.

Every manifest records:

- stable fixture ID;
- schema version;
- owned spec IDs;
- parity level;
- execution classification;
- exact public oracle repository/commit/file/blob/symbol pins;
- exact migration/harness revision;
- normalized input and expected-output paths;
- expected-output SHA-256;
- explicit normalization rules.

## 3. Required fixture-family roadmap audit: PASS

The M5 roadmap names these first required fixture families:

- model selection;
- mirror/winding;
- transform order;
- base normals;
- normal merge controls;
- lighting controls;
- loc dispatch/orientation;
- plane/bridge synthetic inputs;
- priority order crafted model.

M5 now closes each family without stealing later production ownership:

| Roadmap family | M5 evidence |
|---|---|
| model selection | `model.selection.typed_exact.orientation_4` |
| mirror/winding | `model.mirror.geometry_winding` |
| transform order | `model.transform.type4_order` |
| base normals | `normals.base.smooth_triangle`, `normals.base.flat_triangle` |
| normal merge controls | coincident hide=false, coincident hide=true, translated no-match fixtures |
| lighting controls | indexed historical evidence `lighting.synthetic_triangle.loc_rig` |
| loc dispatch/orientation | indexed historical wall/decor/floor/game-object placement evidence |
| plane/bridge synthetic input | `planes.link_below.four_plane_column` |
| priority order | `priority.all_0_11.threshold_crossing` |

The initial Checkpoint 6 audit caught that base-normal, normal-merge, and plane/bridge normalized families were still absent. M5 was not declared merge-ready at that point. Those missing families were added and validated before this exit audit was finalized.

## 4. Evidence-only safety boundary: PASS

`evidence_only` is permitted only for normalized kinds whose production owner does not yet exist:

- `base_normals`;
- `normal_merge`;
- `plane_link_below`;
- `priority_order`.

Production-executed M5 model fixture kinds remain ineligible for `evidence_only`:

- `model_selection`;
- `model_mirror`;
- `model_transform`.

A permanent regression test proves an implemented model fixture cannot be downgraded to evidence-only.

Semantic execution of the deferred evidence kinds also fails explicitly today, so the runner cannot accidentally imply those production paths exist.

Therefore the new evidence strengthens future implementation contracts while leaving `NORMALS-001`, `NORMALS-002`, `PLANES-003`, and `FACE-002` at their correct production statuses.

## 5. Normal fixture evidence: PASS

`reference-fixtures/manifest/M5-SEMANTIC-EVIDENCE-MIGRATION-v1.md` records the exact manual source normalization from pinned public source.

Pinned source:

- `melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`;
- `ModelData.java` blob `2cc9406b2504fbd4fae0c0c952aa2d133809e928`.

The base triangle uses exact integer vertices `(0,0,0)`, `(128,0,0)`, `(0,0,128)` and reference winding `(0,1,2)`.

The pinned reference normal is exactly `(0,-256,0)`.

The merge controls preserve:

- separate left/right model identity;
- exact translation;
- nullable merged-normal storage;
- exact merged normals `(0,-512,0,magnitude=2)` for coincident smooth triangles;
- render type `2` when fully matched faces are hidden;
- no merged storage for the translated no-match control.

These are source-pinned evidence for M7, not an M5 implementation of normal generation or normal merging.

## 6. Plane/link-below evidence: PASS

`planes.link_below.four_plane_column` is source-pinned to:

- `Scene.java` blob `f15260a63103952fe8f5ffbdb62f5c7c39d94565`;
- symbol `setLinkBelow`.

The synthetic case records:

- a four-plane tile column;
- storage shift `1 -> 0`, `2 -> 1`, `3 -> 2`;
- exact tile-plane decrements;
- cleared storage plane 3;
- original plane-0 tile retained as the linked-below tile;
- qualifying type-2 anchored game-object plane decrements;
- non-anchor and non-type-2 negative controls that remain unchanged.

This is exact source-pinned structural evidence for M6. It does not implement `osrs-scene` relinking in M5.

## 7. Priority-order evidence: PASS

`priority.all_0_11.threshold_crossing` is source-pinned to:

- `Model.java` blob `c2aa55c0e8fea89fae0da33d782119f8c109cacf`;
- symbol `method5946`.

It preserves:

- priorities `0..11`;
- `avg12 = 80`;
- `avg34 = 50`;
- `avg68 = 20`;
- priority-10 queue behavior;
- priority-11 queue behavior;
- representative signed alpha metadata;
- exact ordered face IDs.

`FACE-002` remains `REQUIRED` until the renderer-owned production priority executor exists and consumes the same contract.

## 8. Historical evidence migration/indexing: PASS

M5 does not relabel `reference-fixtures/deob_golden.txt` as if it were generated from the pinned public source.

`reference-fixtures/historical/deob_golden.index.json` records:

`historical_local_harness_corroborated_by_public_source`

The historical source and harness identities are byte-gated as Git blobs:

- `reference-fixtures/deob_golden.txt` -> `49887733ad463572cf61bc059733b7c5f5fd26f4`;
- `tools/deob-harness/src/Dumper.java` -> `ceefbd6e97c8e0b09c2ef196b3f9fd2e0f763236`.

Indexed IDs:

1. `terrain.shape_gallery.all_13x4`;
2. `contour.synthetic.flat_slope`;
3. `lighting.synthetic_triangle.loc_rig`;
4. `placement.wall_types.orientation_matrix`;
5. `placement.decor_types.orientation_matrix`;
6. `placement.floor_type22.storage`;
7. `placement.game_object.footprint_and_capacity`.

`scripts/test_deob_golden_index.py` verifies the checked-in historical fixture/harness identity, classification, unique IDs, public source-pin shape, exact-line uniqueness, and expected prefix counts entirely offline.

## 9. Source-pin closure: PASS

`docs/verification/SOURCE-PINS.md` reflects the actual M5 harness state.

The old absolute developer-machine path is retained only as historical provenance. The current harness requires explicit caller-supplied checkout, expected commit, dependency JAR, output path, and optional work directory.

M5 public pins were revalidated against:

`melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`

Relevant file pins include:

- `ObjectComposition.java` -> `079451cd9a6dcfd2666efd15b0524250eaafe4c4`;
- `ModelData.java` -> `2cc9406b2504fbd4fae0c0c952aa2d133809e928`;
- `Model.java` -> `c2aa55c0e8fea89fae0da33d782119f8c109cacf`;
- `Rasterizer3D.java` -> `f32216b5e564c6a03a173438e3b19004c27c1c9e`;
- `Scene.java` -> `f15260a63103952fe8f5ffbdb62f5c7c39d94565`;
- `SceneTileModel.java` -> `ce6a179cfa93e02271af87164e102ee538223718`.

Whole-snapshot equivalence for the historical developer-machine checkout remains explicitly unresolved and is not overstated.

`TERRAIN-004` remains blocked and revision-sensitive. M5 does not revive the disproven `class470` attribution.

## 10. Regeneration safety: PASS

`tools/reference-fixtures/regenerate.py` and `tools/deob-harness/run.sh` implement a candidate-only regeneration boundary.

The workflow:

- rejects candidate paths inside `reference-fixtures/`;
- rejects silent overwrite of existing candidates;
- provides no automatic `--accept` path;
- does not edit manifests or expected hashes;
- does not clone/download dependencies during ordinary operation;
- is not invoked by ordinary CI.

Tier A permanently tests this boundary through `scripts/test_reference_regeneration.py`.

## 11. Parity-matrix linkage: PASS

`docs/verification/PARITY-MATRIX.md` tracks through M5 and links concrete M5 fixture/evidence IDs to the relevant rows.

The matrix distinguishes:

- normalized semantic fixtures executing production code;
- evidence-only normalized fixtures;
- indexed historical evidence.

Later production status is not inflated by source evidence.

## 12. Architecture and scope audit: PASS

M5 remains development/test infrastructure.

Changes are confined to:

- `osrs-reference` fixture infrastructure;
- checked-in fixture inputs/expected outputs/manifests/indexes;
- reference/deob tooling;
- verification scripts;
- CI gates;
- M5/provenance/parity documentation.

M5 does not add production semantic implementation to:

- `osrs-core`;
- `osrs-cache`;
- `osrs-scene`;
- renderer code;
- editor code.

Production crates remain forbidden from depending on `osrs-reference`.

## 13. Deferred production work

M5 does not implement:

- semantic scene tile storage;
- loc placement dispatch;
- bridge/collision/storage/render plane execution;
- base-normal generation in production;
- cross-model normal merging in production;
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

All M5 roadmap gates and first required fixture-family deliverables are closed by checked-in implementation and evidence.

The branch is ready for final documentation-head Tier A/B/C validation, one M5 pull request, exact PR changed-file review, PR-triggered CI, and squash merge.

After M5 is merged, stop before M6. M6 must begin only after explicit user instruction on a fresh branch from the resulting `main`.
