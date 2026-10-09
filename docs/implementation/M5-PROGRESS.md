# M5 Progress: Reference Fixture Infrastructure

Status: **IN PROGRESS**  
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
5. **Historical `deob_golden.txt` indexing plus FACE-002 priority evidence** - IMPLEMENTATION COMPLETE; FINAL DOCUMENTATION-HEAD CI PENDING
6. **M5 verification closure, parity links, exit audit, and milestone PR** - NOT STARTED

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

Checkpoint 3 added deterministic recursive discovery of every YAML manifest under `reference-fixtures/manifest/`, with:

- deterministic path ordering;
- symlink rejection;
- path/provenance/hash verification through `FixtureRepository`;
- duplicate fixture-ID rejection;
- empty-inventory rejection.

The exact semantic runner maps normalized model fixtures to real production functions:

- `model_selection` -> `select_object_model`;
- `model_mirror` -> `mirror_source_model`;
- `model_transform` -> `apply_object_model_instance_transforms`.

Tier C permanently runs:

`cargo test --locked -p osrs-reference --test m5_fixture_runner`

### Checkpoint 3 fixture correction

The first repository-wide execution correctly exposed that the Checkpoint 2 manual `model.transform.type4_order` artifact did not reproduce the executable M4 test it cited. Production transform code remained green and unchanged.

The corrected canonical input now matches M4 test `combined_pipeline_matches_reference_order_exactly` and yields:

- vertices `(-262, 27, -115)`, `(-80, -5, -25)`, `(-80, 59, -25)`;
- face color `300`;
- face texture `9`;
- expected SHA-256 `e40dcd4a67048e4d05d7baee3340d8aa4d96f1ae13bf4f3af4bae71d0f2b78fc`.

`M5-MODEL-MIGRATION-v2.md` records the correction. The original v1 record remains historical evidence rather than being rewritten.

---

## Checkpoint 4 - explicit candidate regeneration and isolated deob adapter

Status: **COMPLETE**

Implementation validation head: `a6364411c473f1c4e847fe43dc6ee8bb9ae9a604`  
Implementation CI: `37903583746`  
Documentation-complete head: `817a74304512c564b90e6b6fbd58b2cd9eed32e3`  
Documentation-head CI: `37903891175`

Checkpoint 4 separated oracle execution from ordinary tests and made regeneration candidate-only.

### Public development command

`tools/reference-fixtures/regenerate.py` exposes adapter `melxin-deob-golden` and pins:

`melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`

It requires explicit local paths for the pinned checkout, caller-supplied compile-only bcprov jar, candidate output, and optional isolated work directory.

It does not clone source, download dependencies, edit manifests, update expected hashes, or accept generated output.

### Candidate-only safety boundary

Both the Python entry point and `tools/deob-harness/run.sh` refuse output paths inside `reference-fixtures/` and refuse silently overwriting an existing candidate. There is no `--accept` mode.

The deob harness now:

- has no developer-machine absolute RuneLite path;
- requires exact expected Git commit verification;
- requires explicit source roots and caller-supplied bcprov jar;
- uses isolated build state;
- does not download Maven artifacts itself;
- writes only the requested external candidate;
- reports candidate SHA-256 and byte length.

Tier A runs `scripts/test_reference_regeneration.py` to verify the safety contract entirely offline. Ordinary CI never executes Java/deob oracle code.

---

## Checkpoint 5 - historical evidence index and priority contract

Status: **IMPLEMENTATION COMPLETE; FINAL DOCUMENTATION-HEAD CI PENDING**

Implementation validation head: `feac12faef49bf8d7aefc90e5231987d97c7a80e`  
Implementation CI: `37905055308`

### Historical `deob_golden.txt` index

The monolithic historical fixture remains unchanged. Checkpoint 5 adds:

`reference-fixtures/historical/deob_golden.index.json`

Historical source identity:

- `reference-fixtures/deob_golden.txt` Git blob `49887733ad463572cf61bc059733b7c5f5fd26f4`;
- `tools/deob-harness/src/Dumper.java` Git blob `ceefbd6e97c8e0b09c2ef196b3f9fd2e0f763236`.

The index deliberately classifies this evidence as historical local-harness output corroborated by pinned public source. It does **not** claim the original developer-machine source tree was byte-identical to the public upstream commit.

Indexed semantic families:

1. `terrain.shape_gallery.all_13x4`
   - owns `TERRAIN-001`;
   - exactly 52 `tri shape=` lines;
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
   - indexes roof-like type 14, type 9, 2x1 shared footprint/edge masks, and five-object capacity evidence.

Placement families are corroborated by pinned public `Scene.java` blob `f15260a63103952fe8f5ffbdb62f5c7c39d94565`.

`scripts/test_deob_golden_index.py` gates this index offline. It verifies exact source/harness identities, unique stable IDs, public source-pin shape, exact-line uniqueness, and expected prefix counts. Tier A runs this test on every push/PR.

The historical index is evidence preparation for M6-M8. It does not promote unimplemented scene, contour, or lighting semantics to `EXISTING`.

### `FACE-002` priority-order fixture

The historical dump contains no face-priority emission-order oracle. Checkpoint 5 therefore does **not** fabricate one from unrelated historical rows.

Instead, the new canonical fixture is source-pinned directly to:

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

The crafted input includes priorities `0..11`, deliberately distinct threshold bands, priority-10 and priority-11 queues, and representative alpha metadata.

Exact thresholds:

- `avg12 = 80`;
- `avg34 = 50`;
- `avg68 = 20`.

Exact source-derived face order:

`10, 0, 1, 2, 11, 3, 4, 12, 5, 6, 7, 8, 9, 13, 14, 15, 16, 17`

Expected-output SHA-256:

`773a8f503cd998944f79b1195f034a5c64bb8504d9e1fee471e9ecf2808ba787`

The ordering intentionally proves that the reference implementation drains priority `10` before switching to priority `11`; it does not globally merge both special queues by depth.

### Evidence-only execution mode

Checkpoint 5 adds explicit manifest execution classification:

- `semantic` is the default and requires an implemented production executor;
- `evidence_only` validates manifest provenance, schema, expected SHA-256, deterministic inventory inclusion, and input/expected kind equality without falsely executing a production path that does not exist.

The priority fixture is `execution: evidence_only` because the renderer-owned priority-order implementation belongs to a later milestone. If it is accidentally changed to semantic today, the runner fails explicitly because no priority production executor exists.

This means the fixture contract is ready now while `FACE-002` itself remains **REQUIRED**, not `EXISTING`.

### Normalized inventory

The M5 repository-wide YAML inventory now contains four canonical fixtures:

1. `model.mirror.geometry_winding`
2. `model.selection.typed_exact.orientation_4`
3. `model.transform.type4_order`
4. `priority.all_0_11.threshold_crossing`

The first three execute against production semantics. The fourth is integrity/provenance/schema/hash gated as evidence-only.

### Validation

Implementation head `feac12faef49bf8d7aefc90e5231987d97c7a80e` passed workflow `37905055308`:

- Tier A: PASS
  - architecture boundaries;
  - architecture guard;
  - reference regeneration safety;
  - historical deob golden index gate;
  - rustfmt;
  - locked workspace check;
  - strict clippy.
- Tier B: PASS
  - complete workspace tests, including evidence-only manifest/schema/hash validation.
- Tier C: PASS
  - all existing M3/M4 semantic gates;
  - M5 repository-wide normalized fixture runner over all four fixtures.

An earlier implementation head failed only because a new unit test used `expect()`, which strict clippy forbids. The test was rewritten without lint suppression; the corrected head above is fully green.

### Checkpoint 5 implementation diff against Checkpoint 4

Implementation head is 12 commits ahead and 0 behind Checkpoint 4 documentation head `817a74304512c564b90e6b6fbd58b2cd9eed32e3`.

Implementation changes are confined to reference infrastructure/evidence and CI:

- `.github/workflows/ci.yml`;
- `crates/osrs-reference/src/fixture.rs`;
- `crates/osrs-reference/src/runner.rs`;
- `crates/osrs-reference/src/schema.rs`;
- `crates/osrs-reference/tests/m5_fixture_runner.rs`;
- `reference-fixtures/historical/deob_golden.index.json`;
- `reference-fixtures/manifest/M5-PRIORITY-MIGRATION-v1.md`;
- `reference-fixtures/manifest/priority-all-0-11-threshold-crossing.yaml`;
- priority input/expected JSON artifacts;
- `scripts/test_deob_golden_index.py`.

No `osrs-core`, `osrs-cache`, `osrs-scene`, renderer, editor, or production semantic implementation changed.

### Deferred to Checkpoint 6 and later milestones

Checkpoint 5 does not:

- implement scene placement from the historical rows;
- implement contouring or lighting;
- implement renderer face-priority ordering;
- promote `FACE-002`, placement, contour, or lighting rows to `EXISTING`;
- claim the historical local source tree is byte-identical to the public deob commit;
- regenerate or replace `deob_golden.txt`;
- open an M5 pull request.

Checkpoint 6 owns final M5 verification closure, parity-matrix links/status wording, source-pin cleanup, exit audit, and the milestone PR process.

---

## Current milestone boundary

M5 Checkpoint 5 implementation is complete. The documentation-complete branch head must pass the full Tier A/B/C chain before Checkpoint 5 is closed.

No M5 pull request should be opened until Checkpoint 6 milestone exit.
