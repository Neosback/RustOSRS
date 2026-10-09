# M5 Progress: Reference Fixture Infrastructure

Status: **COMPLETE - PR READY**  
Milestone: `M5 - Reference fixture infrastructure`  
Branch: `impl/m5-reference-fixture-infrastructure`  
Baseline: M4 squash merge `4d3d21dbe449cd345cf46ccbab651bec6366d585`

## Milestone purpose

M5 turns `osrs-reference` into a development/test-only reference fixture system before scene, normals, lighting, placement, priority, and other high-risk semantic work depends on differential evidence.

Production crates must not depend on `osrs-reference`. Ordinary verification remains offline and deterministic. Oracle execution and fixture regeneration are separate development workflows and must never silently rewrite checked-in expected outputs during CI.

## Checkpoint plan

1. **Fixture loader/comparator/provenance foundation** - COMPLETE
2. **Canonical normalized schemas and first source-pinned model fixtures** - COMPLETE
3. **Repository-wide fixture inventory/runner and dedicated M5 CI gate** - COMPLETE
4. **Explicit regeneration command and isolated oracle/deob adapters** - COMPLETE
5. **Historical `deob_golden.txt` indexing plus FACE-002 priority evidence** - COMPLETE
6. **M5 verification closure, parity links, exit audit, and milestone PR preparation** - COMPLETE

Completed checkpoints must not silently pull later responsibilities forward.

---

## Checkpoint 1 - fixture loader/comparator/provenance foundation

Status: **COMPLETE**

Implementation validation head: `2c4455a99c4f6bbc76e964d243f2f0b1244596fe`  
Documentation-complete head: `237100710b61ce4d92c13099b0fc39596b12c6af`  
Implementation CI: `37890081413`  
Documentation-head CI: `37890229954`

Checkpoint 1 established:

- strict fixture-manifest validation;
- exact oracle and harness provenance requirements;
- root-safe fixture path resolution;
- expected-output SHA-256 verification before admission;
- exact byte comparison with first-difference diagnostics;
- offline ordinary verification.

No production semantic crate changed.

---

## Checkpoint 2 - normalized schemas and first model fixtures

Status: **COMPLETE**

Implementation validation head: `f2c35f5b2be07d83b5e4af6f916d2e3837948406`  
Documentation-complete head: `38117bbbb511bafed527fe5acb6bc335bcc9e330`  
Implementation CI: `37899385456`  
Documentation-head CI: `37899649831`

Normalized schema version `1` was introduced for model selection, model mirroring, and exact object-model instance transforms.

Initial source-pinned fixtures:

1. `model.selection.typed_exact.orientation_4`
   - owns `MODEL-BUILD-001`, `MODEL-BUILD-002`;
   - selected model `200`, mirror `true`;
   - expected SHA-256 `e21314f355237d336ac5c5eb1497e477238384824d653e5ca20fff2442c86024`.
2. `model.mirror.geometry_winding`
   - owns `MODEL-BUILD-002`;
   - exact model-local Z negation and A/C winding swap;
   - expected SHA-256 `5880185db6949fb6f6a10feb2b84c0600032cd80073a0bdf24d1e868e8e6bde1`.
3. `model.transform.type4_order`
   - owns `MODEL-BUILD-003`, `COORD-002`;
   - exact type-4/rotation/recolor/retexture/resize/translation order.

Pinned public oracle source:

- repository `melxin/runelite`;
- commit `1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`;
- `ObjectComposition.java` blob `079451cd9a6dcfd2666efd15b0524250eaafe4c4`;
- `ModelData.java` blob `2cc9406b2504fbd4fae0c0c952aa2d133809e928`;
- `Rasterizer3D.java` blob `f32216b5e564c6a03a173438e3b19004c27c1c9e`.

`M5-MODEL-MIGRATION-v1.md` records the original manual source-pinned normalization. It does not claim executable Java generation.

---

## Checkpoint 3 - repository-wide inventory and semantic runner

Status: **COMPLETE**

Implementation validation head: `eb4d6ee8909a8170374a5e90e419d0639399c5c3`  
Implementation CI: `37901677283`  
Documentation validation head: `b9c97271e51fbca63b1a0c1910f3389d80d12db8`  
Documentation-head CI: `37901932522`  
Closeout head: `cda2dd13214fd342b1f0c1b6e17f2e65c32e6b5e`  
Closeout CI: `37902148092`

Checkpoint 3 added deterministic recursive discovery of every YAML manifest under `reference-fixtures/manifest/`, with deterministic ordering, symlink rejection, path/provenance/hash verification, duplicate-ID rejection, and empty-inventory rejection.

The exact semantic runner maps normalized model fixtures to real production functions:

- `model_selection` -> `select_object_model`;
- `model_mirror` -> `mirror_source_model`;
- `model_transform` -> `apply_object_model_instance_transforms`.

Tier C permanently runs:

`cargo test --locked -p osrs-reference --test m5_fixture_runner`

The first repository-wide execution also exposed and corrected the manually normalized `model.transform.type4_order` artifact without changing production model-transform code. `M5-MODEL-MIGRATION-v2.md` preserves that correction trail.

---

## Checkpoint 4 - explicit candidate regeneration and isolated deob adapter

Status: **COMPLETE**

Implementation validation head: `a6364411c473f1c4e847fe43dc6ee8bb9ae9a604`  
Implementation CI: `37903583746`  
Documentation-complete head: `817a74304512c564b90e6b6fbd58b2cd9eed32e3`  
Documentation-head CI: `37903891175`

Checkpoint 4 separated oracle execution from ordinary tests and made regeneration candidate-only.

`tools/reference-fixtures/regenerate.py` exposes adapter `melxin-deob-golden`, pins `melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`, and requires explicit local source/dependency/output paths.

Both the Python entry point and `tools/deob-harness/run.sh`:

- refuse output paths inside `reference-fixtures/`;
- refuse silently overwriting existing candidates;
- provide no `--accept` mode;
- do not clone or download dependencies during ordinary operation;
- do not edit manifests or expected hashes;
- do not run in ordinary CI.

Tier A runs `scripts/test_reference_regeneration.py` to verify the regeneration safety contract entirely offline.

---

## Checkpoint 5 - historical evidence index and priority contract

Status: **COMPLETE**

Implementation validation head: `0e8f38ab41c3fcf4ce9b0283e243ac87f39cb338`  
Implementation CI: `37905951808`  
Documentation validation head: `28ca80c9d5f8213cb5c53eda97ca01381c46e97f`  
Documentation-head CI: `37906255568`  
Final documentation-complete head: `a813c1986a0bd86ece519e08a6cd0b03a5e7bfcc`  
Final CI: `37906492886`

### Historical `deob_golden.txt` index

The monolithic historical fixture remains unchanged. Checkpoint 5 adds:

`reference-fixtures/historical/deob_golden.index.json`

Historical source identity:

- `reference-fixtures/deob_golden.txt` Git blob `49887733ad463572cf61bc059733b7c5f5fd26f4`;
- `tools/deob-harness/src/Dumper.java` Git blob `ceefbd6e97c8e0b09c2ef196b3f9fd2e0f763236`.

The index deliberately classifies this evidence as `historical_local_harness_corroborated_by_public_source`. It does **not** claim the original developer-machine source tree was byte-identical to the pinned public upstream commit.

Indexed semantic families:

1. `terrain.shape_gallery.all_13x4`
   - owns `TERRAIN-001`;
   - exactly 52 `tri shape=` rows;
   - public `SceneTileModel.java` blob `ce6a179cfa93e02271af87164e102ee538223718`.
2. `contour.synthetic.flat_slope`
   - owns `CONTOUR-001`;
   - exact historical outputs `contour flat y=0`, `contour slope y=-28`;
   - public `Model.java` blob `c2aa55c0e8fea89fae0da33d782119f8c109cacf`.
3. `lighting.synthetic_triangle.loc_rig`
   - owns `LIGHTING-001`;
   - exact historical output `tolit c1=4638 c2=4638 c3=4638`;
   - public `ModelData.java` blob `2cc9406b2504fbd4fae0c0c952aa2d133809e928`.
4. `placement.wall_types.orientation_matrix`
   - owns `LOC-PLACEMENT-001`;
   - exactly 16 wall storage rows.
5. `placement.decor_types.orientation_matrix`
   - owns `LOC-PLACEMENT-001`, `LOC-PLACEMENT-002`;
   - exactly 20 wall-decoration storage rows.
6. `placement.floor_type22.storage`
   - owns `LOC-PLACEMENT-001`, `LOC-PLACEMENT-006`;
   - exact floor-decoration storage row.
7. `placement.game_object.footprint_and_capacity`
   - owns `LOC-PLACEMENT-003`;
   - indexes type-14/type-9 centers, 2x1 shared footprint/edge masks, and five-object capacity evidence.

Placement families are corroborated by pinned public `Scene.java` blob `f15260a63103952fe8f5ffbdb62f5c7c39d94565`.

`scripts/test_deob_golden_index.py` gates this index offline. It verifies:

- exact Git blob identities recomputed from the checked-in `deob_golden.txt` and `Dumper.java` bytes;
- the historical evidence classification;
- unique stable fixture IDs;
- public source-pin shape;
- exact-line uniqueness;
- expected prefix counts.

The historical index prepares evidence for M6-M8. It does not promote scene, contour, or lighting semantics to `EXISTING`.

### `FACE-002` priority-order fixture

The historical dump contains no face-priority emission-order oracle, so Checkpoint 5 does not fabricate one from unrelated historical rows.

The canonical priority fixture is source-pinned directly to:

- `melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`;
- `runescape-client/src/main/java/Model.java`;
- blob `c2aa55c0e8fea89fae0da33d782119f8c109cacf`;
- symbol `method5946`.

Fixture ID:

`priority.all_0_11.threshold_crossing`

Artifacts:

- `reference-fixtures/priority/all_0_11_threshold_crossing.input.json`;
- `reference-fixtures/priority/all_0_11_threshold_crossing.expected.json`;
- `reference-fixtures/manifest/priority-all-0-11-threshold-crossing.yaml`;
- `reference-fixtures/manifest/M5-PRIORITY-MIGRATION-v1.md`.

The crafted case covers priorities `0..11`, distinct threshold bands, priority-10 and priority-11 queues, and representative signed alpha metadata.

Exact thresholds:

- `avg12 = 80`;
- `avg34 = 50`;
- `avg68 = 20`.

Exact source-derived face order:

`10, 0, 1, 2, 11, 3, 4, 12, 5, 6, 7, 8, 9, 13, 14, 15, 16, 17`

Expected-output SHA-256:

`773a8f503cd998944f79b1195f034a5c64bb8504d9e1fee471e9ecf2808ba787`

The ordering proves that the reference routine drains priority `10` before switching to priority `11`; it does not globally merge both special queues by depth.

### Evidence-only execution boundary

Checkpoint 5 adds explicit manifest execution classification:

- `semantic` is the default and requires an implemented production executor;
- `evidence_only` validates schema, provenance, expected SHA-256, deterministic inventory inclusion, and input/expected kind equality without pretending an unimplemented production path exists.

The priority fixture is `execution: evidence_only` because ordered renderer face emission belongs to a later milestone. The runner explicitly rejects `priority_order` as semantic until that owner exists.

The runner also refuses to use `evidence_only` to bypass any fixture kind that already has an M5 production semantic executor. A regression test proves that an implemented model fixture cannot be downgraded to evidence-only.

`FACE-002` therefore remains **REQUIRED**, not `EXISTING`.

### Normalized inventory

The M5 repository-wide YAML inventory now contains four canonical fixtures:

1. `model.mirror.geometry_winding`
2. `model.selection.typed_exact.orientation_4`
3. `model.transform.type4_order`
4. `priority.all_0_11.threshold_crossing`

The first three execute against production semantics. The fourth remains integrity/provenance/schema/hash gated as evidence-only.

### Validation

Final Checkpoint 5 head `a813c1986a0bd86ece519e08a6cd0b03a5e7bfcc` passed workflow `37906492886`:

- Tier A: PASS
  - architecture boundaries;
  - architecture guard;
  - reference regeneration safety;
  - historical deob golden index and exact historical Git blob verification;
  - rustfmt;
  - locked workspace check;
  - strict clippy.
- Tier B: PASS
  - complete workspace tests;
  - evidence-only semantic-bypass regression.
- Tier C: PASS
  - all existing M3/M4 semantic gates;
  - M5 repository-wide normalized fixture runner over all four fixtures.

### Checkpoint 5 scope

Permanent Checkpoint 5 changes are confined to reference infrastructure/evidence and CI/documentation. No `osrs-core`, `osrs-cache`, `osrs-scene`, renderer, editor, or production semantic implementation changed.

---

## Checkpoint 6 - verification closure, parity links, source pins, and exit audit

Status: **COMPLETE - PR READY**

Checkpoint 6 performed the M5 milestone closure without adding later semantic behavior.

### Parity-matrix closure

`docs/verification/PARITY-MATRIX.md` now tracks through M5 and links concrete M5 fixture/evidence IDs to the relevant rows.

Key linkage rules:

- `model.selection.typed_exact.orientation_4` links to `MODEL-BUILD-001` and `MODEL-BUILD-002`;
- `model.mirror.geometry_winding` links to `MODEL-BUILD-002`;
- `model.transform.type4_order` links to `MODEL-BUILD-003` and `COORD-002`;
- `priority.all_0_11.threshold_crossing` links to `FACE-002` as evidence-only;
- indexed historical IDs link terrain, contour, lighting, and placement evidence without promoting later production executors.

Status discipline is preserved:

- `FACE-002` remains `REQUIRED`;
- placement rows retain their prior production status;
- `CONTOUR-001` remains `PARTIAL`;
- `LIGHTING-001` remains `PARTIAL`;
- no M6/M7 semantic implementation is claimed.

### Source-pin closure

`docs/verification/SOURCE-PINS.md` was reconciled with the actual M5 harness state.

The old absolute local source path is retained only as historical provenance. The current harness requires an explicit Git checkout and exact expected commit.

The M5 public pins were revalidated against `melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`, including:

- `ObjectComposition.java` -> `079451cd9a6dcfd2666efd15b0524250eaafe4c4`;
- `ModelData.java` -> `2cc9406b2504fbd4fae0c0c952aa2d133809e928`;
- `Model.java` -> `c2aa55c0e8fea89fae0da33d782119f8c109cacf`;
- `Rasterizer3D.java` -> `f32216b5e564c6a03a173438e3b19004c27c1c9e`;
- `Scene.java` -> `f15260a63103952fe8f5ffbdb62f5c7c39d94565`;
- `SceneTileModel.java` -> `ce6a179cfa93e02271af87164e102ee538223718`.

Whole-snapshot equivalence for the historical developer-machine checkout remains explicitly unresolved and is not overstated.

### Exit audit

`docs/implementation/M5-EXIT-AUDIT.md` records closure of all canonical M5 roadmap gates:

1. ordinary CI is offline against checked-in expected outputs;
2. normalized fixtures have exact source/harness/hash provenance;
3. regeneration is candidate-only and cannot silently accept expected-output changes;
4. relevant parity rows link concrete fixture/test IDs;
5. historical evidence retains its correct provenance classification;
6. architecture/scope remains reference-only rather than adding later production semantics.

### Scope audit

Relative to M4 `main`, M5 is confined to:

- `osrs-reference` fixture infrastructure;
- fixture evidence/manifests/indexes;
- reference/deob tooling;
- verification scripts;
- CI gates;
- M5/provenance/parity documentation.

No `osrs-core`, `osrs-cache`, `osrs-scene`, renderer, editor, or production semantic implementation is added by M5.

### Final validation process

Checkpoint 6 closure is followed by:

1. exact branch-vs-`main` review;
2. final push-triggered Tier A/B/C validation on the exact branch head;
3. one M5 pull request;
4. exact PR changed-file/diff review;
5. PR-triggered CI on the exact PR head;
6. squash merge only if every required gate is green and `main` has not unexpectedly moved;
7. verification of the resulting `main` merge commit.

M6 is not part of this checkpoint.

---

## Current milestone boundary

M5 implementation and documentation are complete and ready for final validation and the milestone PR/merge process.

M6 has not started. After M5 is merged, stop and wait for explicit user instruction before creating or starting any M6 branch.
