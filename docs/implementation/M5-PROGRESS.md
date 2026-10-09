# M5 Progress: Reference Fixture Infrastructure

Status: **COMPLETE - PR READY**  
Milestone: `M5 - Reference fixture infrastructure`  
Branch: `impl/m5-reference-fixture-infrastructure`  
Baseline: M4 squash merge `4d3d21dbe449cd345cf46ccbab651bec6366d585`

## Milestone purpose

M5 turns `osrs-reference` into development/test-only reference fixture infrastructure before scene, normals, lighting, placement, priority, and other high-risk semantic work depends on differential evidence.

Production crates must not depend on `osrs-reference`. Ordinary verification remains offline and deterministic. Oracle execution and fixture regeneration remain separate development workflows and cannot silently rewrite checked-in expected output.

## Checkpoint status

1. Fixture loader/comparator/provenance foundation - COMPLETE
2. Canonical normalized schemas and first source-pinned model fixtures - COMPLETE
3. Repository-wide fixture inventory/runner and dedicated M5 CI gate - COMPLETE
4. Explicit regeneration command and isolated oracle/deob adapters - COMPLETE
5. Historical `deob_golden.txt` indexing plus FACE-002 priority evidence - COMPLETE
6. Verification closure, missing fixture-family closure, parity/source-pin audit, exit audit, PR preparation - COMPLETE

## Checkpoint 1

Validation heads: `2c4455a99c4f6bbc76e964d243f2f0b1244596fe`, `237100710b61ce4d92c13099b0fc39596b12c6af`  
CI: `37890081413`, `37890229954`

Closed strict manifest validation, exact oracle/harness provenance, root-safe fixture paths, expected-output SHA-256 checks, exact comparison diagnostics, and offline verification.

## Checkpoint 2

Validation heads: `f2c35f5b2be07d83b5e4af6f916d2e3837948406`, `38117bbbb511bafed527fe5acb6bc335bcc9e330`  
CI: `37899385456`, `37899649831`

Added normalized schema version 1 and the first three source-pinned model fixtures:

- `model.selection.typed_exact.orientation_4`;
- `model.mirror.geometry_winding`;
- `model.transform.type4_order`.

Pinned source revision: `melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`.

## Checkpoint 3

Validation heads: `eb4d6ee8909a8170374a5e90e419d0639399c5c3`, `cda2dd13214fd342b1f0c1b6e17f2e65c32e6b5e`  
CI: `37901677283`, `37901932522`, `37902148092`

Added deterministic recursive YAML discovery and exact execution through production semantic owners. Tier C permanently runs `cargo test --locked -p osrs-reference --test m5_fixture_runner`.

The repository-wide execution also caught and corrected the manually normalized type-4 transform artifact. The correction trail remains in `M5-MODEL-MIGRATION-v2.md`.

## Checkpoint 4

Validation heads: `a6364411c473f1c4e847fe43dc6ee8bb9ae9a604`, `817a74304512c564b90e6b6fbd58b2cd9eed32e3`  
CI: `37903583746`, `37903891175`

Separated oracle execution from ordinary tests and made regeneration candidate-only. The workflow requires explicit local inputs, rejects output inside `reference-fixtures/`, rejects silent overwrite, exposes no automatic acceptance mode, does not download dependencies in ordinary operation, and is permanently safety-tested in Tier A.

## Checkpoint 5

Final head: `a813c1986a0bd86ece519e08a6cd0b03a5e7bfcc`  
Final CI: `37906492886`

Added `reference-fixtures/historical/deob_golden.index.json` while preserving the historical golden unchanged.

Historical identities:

- `reference-fixtures/deob_golden.txt` Git blob `49887733ad463572cf61bc059733b7c5f5fd26f4`;
- `tools/deob-harness/src/Dumper.java` Git blob `ceefbd6e97c8e0b09c2ef196b3f9fd2e0f763236`.

Classification: `historical_local_harness_corroborated_by_public_source`.

Indexed evidence IDs:

- `terrain.shape_gallery.all_13x4`;
- `contour.synthetic.flat_slope`;
- `lighting.synthetic_triangle.loc_rig`;
- `placement.wall_types.orientation_matrix`;
- `placement.decor_types.orientation_matrix`;
- `placement.floor_type22.storage`;
- `placement.game_object.footprint_and_capacity`.

Also added `priority.all_0_11.threshold_crossing`, source-pinned to `Model.method5946`. It is `execution: evidence_only` because the renderer priority executor does not exist yet, so `FACE-002` remains `REQUIRED`.

## Checkpoint 6

Status: **COMPLETE - PR READY**

Checkpoint 6 began as the milestone verification/parity/source-pin exit audit. That audit found a real roadmap gap before any PR was opened: the canonical M5 roadmap explicitly required base-normal, normal-merge-control, and plane/bridge synthetic fixture families, but those normalized families were still absent.

M5 was therefore not merged on documentation claims alone.

### Missing-family correction

Added six source-pinned evidence-only fixtures:

- `normals.base.flat_triangle`;
- `normals.base.smooth_triangle`;
- `normals.merge.coincident_triangle.hide_false`;
- `normals.merge.coincident_triangle.hide_true`;
- `normals.merge.translated_negative`;
- `planes.link_below.four_plane_column`.

The normal fixtures pin `ModelData.calculateVertexNormals` / `ModelData.method5262`. The plane fixture pins `Scene.setLinkBelow`.

`reference-fixtures/manifest/M5-SEMANTIC-EVIDENCE-MIGRATION-v1.md` records exact source-derived normalization and the acceptance boundary.

### Evidence-only hardening

The runner now permits `evidence_only` only for deferred normalized kinds:

- `base_normals`;
- `normal_merge`;
- `plane_link_below`;
- `priority_order`.

Production-executed M5 model kinds remain ineligible for downgrade:

- `model_selection`;
- `model_mirror`;
- `model_transform`.

The downgrade-bypass regression remains green, and semantic execution of deferred evidence kinds still fails explicitly until their production owner exists.

### Corrected canonical YAML inventory

The repository-wide M5 inventory now contains ten fixtures:

1. `model.mirror.geometry_winding`
2. `model.selection.typed_exact.orientation_4`
3. `model.transform.type4_order`
4. `normals.base.flat_triangle`
5. `normals.base.smooth_triangle`
6. `normals.merge.coincident_triangle.hide_false`
7. `normals.merge.coincident_triangle.hide_true`
8. `normals.merge.translated_negative`
9. `planes.link_below.four_plane_column`
10. `priority.all_0_11.threshold_crossing`

The first three execute production semantics. The remaining seven are evidence-only because their production owners are later milestones.

### Required-family closure

The M5 roadmap families are now covered:

- model selection -> normalized semantic fixture;
- mirror/winding -> normalized semantic fixture;
- transform order -> normalized semantic fixture;
- base normals -> smooth and flat normalized evidence fixtures;
- normal merge controls -> positive hide=false, positive hide=true, translated negative fixtures;
- lighting controls -> indexed historical lighting evidence;
- loc dispatch/orientation -> indexed historical placement evidence;
- plane/bridge synthetic input -> four-plane normalized evidence fixture;
- priority order -> normalized evidence fixture.

No M6/M7 production semantics were implemented to achieve this closure.

### Corrected implementation validation

Corrected implementation head: `2ecd37d1bfdcfce95ede5306b3f2c8d4aaa872d7`  
Workflow: `37908853111`

Results:

- Tier A: PASS, including architecture, regeneration safety, historical-index verification, rustfmt, workspace check, and strict Clippy;
- Tier B: PASS, full workspace tests;
- Tier C: PASS, M3/M4 gates plus the M5 repository-wide runner across all ten canonical YAML fixtures.

### Documentation closure

Checkpoint 6 reconciles:

- `docs/verification/PARITY-MATRIX.md`;
- `docs/verification/SOURCE-PINS.md`;
- `docs/implementation/M5-EXIT-AUDIT.md`.

The parity matrix links evidence fixtures without promoting later production semantics to `EXISTING`.

### Scope

M5 remains confined to `osrs-reference`, fixture evidence/manifests/indexes, reference tooling, verification scripts, CI gates, and M5/provenance/parity documentation.

No M5-owned production semantic implementation is added to `osrs-core`, `osrs-cache`, `osrs-scene`, renderer, or editor crates.

## Current milestone boundary

M5 implementation and documentation are complete and ready for final exact-head CI plus the single milestone PR/merge process.

M6 has not started. After M5 is merged, stop and wait for explicit user instruction before creating or starting an M6 branch.
