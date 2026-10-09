# M5 Progress: Reference Fixture Infrastructure

Status: **IN PROGRESS**  
Milestone: `M5 - Reference fixture infrastructure`  
Branch: `impl/m5-reference-fixture-infrastructure`  
Baseline: M4 squash merge `4d3d21dbe449cd345cf46ccbab651bec6366d585`

## Milestone purpose

M5 turns `osrs-reference` into a real development/test-only reference fixture system before scene, normals, lighting, and other high-risk semantic work depends on differential evidence.

Production crates must not depend on `osrs-reference`. Ordinary Rust verification must remain offline and deterministic. Source/oracle execution and fixture regeneration are separate development workflows and must never silently rewrite checked-in expected outputs during CI.

## Checkpoint plan

1. **Fixture loader/comparator/provenance foundation** - COMPLETE
2. **Canonical normalized fixture schemas and first migrated source-pinned fixtures** - COMPLETE
3. Exact fixture inventory/runner integration and dedicated M5 CI gates - NOT STARTED
4. Explicit regeneration command and isolated oracle/harness adapters - NOT STARTED
5. Historical `deob_golden.txt` indexing/migration plus additional priority semantic families - NOT STARTED
6. M5 verification closure, parity links, exit audit, and milestone PR - NOT STARTED

Later checkpoint boundaries may be refined if source evidence requires it, but completed checkpoints must not silently pull later responsibilities forward.

---

## Checkpoint 1 - fixture loader/comparator/provenance foundation

Status: **COMPLETE**

Implementation validation head: `2c4455a99c4f6bbc76e964d243f2f0b1244596fe`  
Documentation-complete head: `237100710b61ce4d92c13099b0fc39596b12c6af`  
Implementation CI workflow: `37890081413`  
Documentation-head CI workflow: `37890229954`

### Scope completed

Checkpoint 1 hardened the M0-era manifest parser into an offline fixture integrity boundary.

- `crates/osrs-reference/src/fixture.rs`
  - schema-version validation;
  - non-empty and unique owned specs;
  - exact oracle commit/blob identities;
  - repository/commit pairing;
  - duplicate source-path rejection;
  - mandatory harness identity and expected SHA-256.
- `crates/osrs-reference/src/loader.rs`
  - canonical fixture root;
  - root-relative path validation;
  - parent/root/prefix traversal rejection;
  - canonicalized symlink-escape rejection;
  - offline input/expected loading;
  - expected-output SHA-256 admission check.
- `crates/osrs-reference/src/comparator.rs`
  - exact byte comparison;
  - first-difference and length diagnostics;
  - no hidden sorting, tolerance, coercion, or normalization.
- `sha2 0.10` is reused from the workspace lock for expected-output integrity.

### Checkpoint 1 policy

A fixture is not trusted merely because YAML parses. The manifest must identify its oracle, harness/migration provenance, normalized paths, and exact expected-output hash. Ordinary CI consumes checked-in files offline and never regenerates them.

Checkpoint 1 deliberately did not add semantic fixture schemas, a repository-wide runner, regeneration commands, Java/deob adapters, historical golden migration, or new semantic implementations.

### Validation

Both the implementation and documentation-complete Checkpoint 1 heads passed the normal Tier A/B/C chain. No production semantic crate was modified.

---

## Checkpoint 2 - canonical normalized schemas and first source-pinned model fixtures

Status: **COMPLETE**

Implementation validation head: `f2c35f5b2be07d83b5e4af6f916d2e3837948406`  
Implementation CI workflow: `37899385456`

### Scope completed

Checkpoint 2 establishes the first typed normalized semantic fixture documents and migrates three high-value M4 model cases into source-pinned manifest/input/expected triplets.

#### Typed normalized schema

`crates/osrs-reference/src/schema.rs` defines normalized schema version `1` for:

- model selection inputs and expected selection/absence;
- typed and untyped object-model lists;
- model mirror geometry inputs/outputs;
- exact model instance transform inputs/outputs;
- signed integer model points;
- exact triangle indices;
- recolor/retexture replacement pairs;
- model scale and translation;
- optional texture IDs where `null` remains distinct from an integer ID.

The schema rejects unknown top-level/variant fields and unsupported schema versions. It remains development-only inside `osrs-reference`; no normalized fixture types leak into production crates.

`crates/osrs-reference/src/lib.rs` exposes the schema module.

#### Source-pinned migration provenance

`reference-fixtures/manifest/M5-MODEL-MIGRATION-v1.md` records exactly how the first M5 fixtures were normalized.

The migration record is intentionally classified `SOURCE_PINNED_MANUAL_NORMALIZATION`.

It does **not** claim that a Java/deob harness generated the expected outputs. These cases were normalized from M4 equality cases already audited against the pinned deob source. Executable oracle regeneration remains owned by Checkpoint 4.

The migration record's exact Git blob identity is:

`7909807bdd2c18c8fa75595644685f31923c7969`

Each Checkpoint 2 manifest records that blob as its harness/migration revision.

Pinned public oracle:

- repository: `melxin/runelite`
- commit: `1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`
- `ObjectComposition.java`: `079451cd9a6dcfd2666efd15b0524250eaafe4c4`
- `ModelData.java`: `2cc9406b2504fbd4fae0c0c952aa2d133809e928`
- `Rasterizer3D.java`: `f32216b5e564c6a03a173438e3b19004c27c1c9e`

### Migrated canonical fixture families

#### `model.selection.typed_exact.orientation_4`

Manifest:

`reference-fixtures/manifest/model-selection-typed-orientation-4.yaml`

Owns:

- `MODEL-BUILD-001`
- `MODEL-BUILD-002`

Input preserves two typed entries, requested loc type `4`, `is_rotated = false`, and orientation `4`.

Expected output preserves exact model order and requires:

- selected model ID `200`;
- mirror `true`.

Expected SHA-256:

`e21314f355237d336ac5c5eb1497e477238384824d653e5ca20fff2442c86024`

#### `model.mirror.geometry_winding`

Manifest:

`reference-fixtures/manifest/model-mirror-geometry-winding.yaml`

Owns `MODEL-BUILD-002` and pins `ModelData.method5280`.

The normalized expected geometry proves both behaviors together:

- model-local Z is negated exactly;
- face A/C indices are swapped, preserving the exact reversed winding rather than sorting topology.

Expected SHA-256:

`5880185db6949fb6f6a10feb2b84c0600032cd80073a0bdf24d1e868e8e6bde1`

#### `model.transform.type4_order`

Manifest:

`reference-fixtures/manifest/model-transform-type4-order.yaml`

Owns:

- `MODEL-BUILD-003`
- `COORD-002`

The fixture preserves the order-sensitive M4 case with:

- loc type `4`;
- orientation `5`;
- 256-JAU special transform/recenter path;
- authored-order recolor chain;
- authored-order retexture chain;
- non-identity resize;
- final definition translation;
- signed `i32::MIN` geometry control.

Expected final vertices remain exactly:

1. `(477, 393, -963)`
2. `(20, -7, 186)`
3. `(-1073741799, 3, 45)`

Final color is `12`; final texture ID is `22`.

Expected SHA-256:

`edd5a1c461e48c64b8e6705866f3f3f12076730b2fe53e4888e2a17d18ff9aea`

### Normalization contract

Checkpoint 2 fixtures preserve semantic ordering and integer identity rather than normalizing away the behavior under test.

The first model families explicitly preserve:

- source/model ID ordering;
- signed integer XYZ values;
- face winding/index order;
- raw loc type and orientation;
- authored recolor/retexture order;
- optional texture presence/absence;
- output vertex order.

No renderer-space conversion, epsilon rounding, face sorting, absent/present coercion, or presentation-only normalization is allowed.

### Offline schema/integrity tests

`crates/osrs-reference/tests/m5_normalized_model_fixtures.rs` loads the three explicit manifests through the Checkpoint 1 `FixtureRepository` and therefore verifies before typed parsing that:

- every path stays inside the fixture root;
- each manifest validates;
- each expected output matches its declared SHA-256;
- each input/expected document parses as the declared normalized family;
- the preserved edge-case values match the audited M4 cases.

This is deliberately a targeted Checkpoint 2 acceptance test, not the repository-wide dynamic fixture runner or dedicated M5 CI gate owned by Checkpoint 3.

### Validation

Implementation head `f2c35f5b2be07d83b5e4af6f916d2e3837948406` passed workflow `37899385456`:

- Tier A static quality: PASS
  - architecture dependency check;
  - architecture guard tests;
  - rustfmt;
  - locked workspace check;
  - strict clippy.
- Tier B workspace tests: PASS
  - includes the new hash-verified normalized-fixture tests.
- Tier C existing M3/M4 semantic parity: PASS.

An earlier implementation head failed only rustfmt. The formatter differences were applied exactly; no lint was suppressed and no semantic workaround was introduced.

### Checkpoint 2 diff against Checkpoint 1

Implementation head `f2c35f5b2be07d83b5e4af6f916d2e3837948406` is 15 commits ahead and 0 behind Checkpoint 1 head `237100710b61ce4d92c13099b0fc39596b12c6af`.

The permanent implementation diff is confined to 13 files:

1. `crates/osrs-reference/src/lib.rs`
2. `crates/osrs-reference/src/schema.rs`
3. `crates/osrs-reference/tests/m5_normalized_model_fixtures.rs`
4. `reference-fixtures/manifest/M5-MODEL-MIGRATION-v1.md`
5. `reference-fixtures/manifest/model-mirror-geometry-winding.yaml`
6. `reference-fixtures/manifest/model-selection-typed-orientation-4.yaml`
7. `reference-fixtures/manifest/model-transform-type4-order.yaml`
8. `reference-fixtures/model/mirror_geometry_winding.expected.json`
9. `reference-fixtures/model/mirror_geometry_winding.input.json`
10. `reference-fixtures/model/selection_typed_orientation_4.expected.json`
11. `reference-fixtures/model/selection_typed_orientation_4.input.json`
12. `reference-fixtures/model/transform_type4_order.expected.json`
13. `reference-fixtures/model/transform_type4_order.input.json`

No `osrs-core`, `osrs-cache`, `osrs-scene`, renderer, editor, runtime semantic implementation, workflow, or dependency file changed in Checkpoint 2.

### Explicit non-goals / deferred work

Checkpoint 2 does not implement:

- repository-wide automatic fixture discovery/indexing;
- a generic fixture executor that converts production semantic results into normalized outputs;
- dedicated M5 Tier C commands;
- executable Java/deob regeneration;
- oracle adapters;
- `tools/deob-harness` source-root cleanup;
- historical `deob_golden.txt` indexing/migration;
- normal, lighting, placement, bridge/plane, or priority fixtures;
- parity-matrix links to the new concrete fixture IDs.

Those remain owned by Checkpoints 3-6.

---

## Current milestone boundary

M5 Checkpoint 2 is complete once this documentation-complete branch head passes the normal Tier A/B/C chain and the final Checkpoint-1-to-Checkpoint-2 scope audit confirms no unrelated changes.

No M5 pull request should be opened until the milestone exit checkpoint.
