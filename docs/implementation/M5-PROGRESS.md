# M5 Progress: Reference Fixture Infrastructure

Status: **IN PROGRESS**  
Milestone: `M5 - Reference fixture infrastructure`  
Branch: `impl/m5-reference-fixture-infrastructure`  
Baseline: M4 squash merge `4d3d21dbe449cd345cf46ccbab651bec6366d585`

## Milestone purpose

M5 turns `osrs-reference` into a development/test-only reference fixture system before scene, normals, lighting, and other high-risk semantic work depends on differential evidence.

Production crates must not depend on `osrs-reference`. Ordinary verification must remain offline and deterministic. Oracle execution and fixture regeneration are separate development workflows and must never silently rewrite checked-in expected outputs during CI.

## Checkpoint plan

1. **Fixture loader/comparator/provenance foundation** - COMPLETE
2. **Canonical normalized fixture schemas and first migrated source-pinned fixtures** - COMPLETE
3. **Exact fixture inventory/runner integration and dedicated M5 CI gates** - IMPLEMENTATION COMPLETE; FINAL DOCUMENTATION-HEAD CI PENDING
4. Explicit regeneration command and isolated oracle/harness adapters - NOT STARTED
5. Historical `deob_golden.txt` indexing/migration plus additional priority semantic families - NOT STARTED
6. M5 verification closure, parity links, exit audit, and milestone PR - NOT STARTED

Completed checkpoints must not silently pull later responsibilities forward.

---

## Checkpoint 1 - fixture loader/comparator/provenance foundation

Status: **COMPLETE**

Implementation validation head: `2c4455a99c4f6bbc76e964d243f2f0b1244596fe`  
Documentation-complete head: `237100710b61ce4d92c13099b0fc39596b12c6af`  
Implementation CI: `37890081413`  
Documentation-head CI: `37890229954`

Checkpoint 1 established the offline fixture integrity boundary:

- strict manifest schema and provenance validation;
- exact oracle commit/blob identities;
- mandatory harness identity and expected-output SHA-256;
- path-safe `FixtureRepository` loading under a canonical fixture root;
- parent/root/prefix traversal rejection and symlink escape protection;
- expected-output SHA-256 verification before admission;
- exact byte comparison with first-difference diagnostics;
- no network access or regeneration in normal tests.

No production semantic crate changed.

---

## Checkpoint 2 - canonical normalized schemas and first source-pinned model fixtures

Status: **COMPLETE**

Implementation validation head: `f2c35f5b2be07d83b5e4af6f916d2e3837948406`  
Documentation-complete head: `38117bbbb511bafed527fe5acb6bc335bcc9e330`  
Implementation CI: `37899385456`  
Documentation-head CI: `37899649831`

### Normalized schema

`crates/osrs-reference/src/schema.rs` defines normalized schema version `1` for:

- typed/untyped model selection;
- model mirror geometry and face winding;
- exact instance-transform inputs/outputs;
- signed integer model points and triangle indices;
- recolor/retexture replacement pairs;
- scale and translation;
- optional texture IDs with `null` distinct from an integer ID.

Unknown fields and unsupported schema versions fail closed. These types remain development-only inside `osrs-reference`.

### First source-pinned fixtures

Checkpoint 2 introduced three manifest/input/expected families:

1. `model.selection.typed_exact.orientation_4`
   - owns `MODEL-BUILD-001`, `MODEL-BUILD-002`;
   - expected selected model ID `200` and mirror `true`;
   - expected SHA-256 `e21314f355237d336ac5c5eb1497e477238384824d653e5ca20fff2442c86024`.
2. `model.mirror.geometry_winding`
   - owns `MODEL-BUILD-002`;
   - proves exact model-local Z negation and face A/C winding swap;
   - expected SHA-256 `5880185db6949fb6f6a10feb2b84c0600032cd80073a0bdf24d1e868e8e6bde1`.
3. `model.transform.type4_order`
   - owns `MODEL-BUILD-003`, `COORD-002`;
   - source-pinned to `ObjectComposition`, `ModelData`, and `Rasterizer3D`;
   - its manually normalized artifact was later corrected by Checkpoint 3 after executable runner validation exposed that the original v1 vector did not reproduce the M4 test it cited. The corrected canonical details are recorded below.

### Manual migration provenance

`reference-fixtures/manifest/M5-MODEL-MIGRATION-v1.md` records the original manual source-pinned migration. It deliberately does not claim Java/deob executable generation.

Pinned oracle source:

- `melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`
- `ObjectComposition.java` blob `079451cd9a6dcfd2666efd15b0524250eaafe4c4`
- `ModelData.java` blob `2cc9406b2504fbd4fae0c0c952aa2d133809e928`
- `Rasterizer3D.java` blob `f32216b5e564c6a03a173438e3b19004c27c1c9e`

Checkpoint 2 validated manifest/schema/hash integrity but intentionally did not yet execute every normalized fixture through production semantics. Checkpoint 3 owns that stronger gate.

---

## Checkpoint 3 - repository-wide fixture runner and dedicated M5 CI gate

Status: **IMPLEMENTATION COMPLETE; FINAL DOCUMENTATION-HEAD CI PENDING**

Implementation validation head: `eb4d6ee8909a8170374a5e90e419d0639399c5c3`  
Implementation CI workflow: `37901677283`

### Deterministic inventory

`crates/osrs-reference/src/inventory.rs` introduces `FixtureInventory`.

Discovery rules:

1. start under the configured fixture root's `manifest/` directory;
2. recursively discover every `.yaml` manifest;
3. reject symlink entries in the manifest tree;
4. sort manifests deterministically by root-relative path;
5. load every manifest through the Checkpoint 1 `FixtureRepository`;
6. therefore validate paths, manifest provenance, and expected SHA-256 before execution;
7. reject duplicate fixture IDs;
8. reject an empty YAML inventory.

A newly checked-in YAML fixture therefore cannot silently escape the repository-wide M5 test merely because an author forgot to add it to a hand-maintained list.

### Exact semantic runner

`crates/osrs-reference/src/runner.rs` executes each integrity-verified normalized fixture through the real production semantic functions while keeping the runner itself in the development-only reference crate.

Current mappings:

- `model_selection` -> `osrs_core::model_construction::select_object_model`;
- `model_mirror` -> `osrs_core::model_construction::mirror_source_model`;
- `model_transform` -> `osrs_core::model_construction::apply_object_model_instance_transforms`.

The runner then normalizes only the fields owned by that fixture family and requires exact typed equality with the checked-in expected document. It does not introduce tolerance, sorting, hidden mirror/placement stages, renderer-space conversion, or fallback acceptance.

`crates/osrs-reference/tests/m5_fixture_runner.rs` asserts the deterministic current inventory:

1. `model.mirror.geometry_winding`
2. `model.selection.typed_exact.orientation_4`
3. `model.transform.type4_order`

and requires all three to execute successfully.

### Dedicated Tier C gate

`.github/workflows/ci.yml` now includes:

`cargo test --locked -p osrs-reference --test m5_fixture_runner`

as `Run M5 repository-wide normalized fixture runner` in Tier C after the existing M3/M4 semantic gates.

Ordinary CI remains offline with respect to oracle execution and regeneration.

### Checkpoint 3 caught and corrected a Checkpoint 2 migration defect

The first repository-wide execution run correctly failed `model.transform.type4_order` while all M4 production transform tests remained green. The failure proved that the Checkpoint 2 v1 transform artifact was hash-consistent but was not an exact normalization of the executable M4 case it cited.

The M4 source case was re-read from commit `384b1274d40bbc8b4acd383ee31364b82a2e08cc`, test `combined_pipeline_matches_reference_order_exactly`.

The corrected normalized input is now exactly:

- vertices `(128, 64, 0)`, `(0, 0, 0)`, `(0, 128, 0)`;
- face color `100`;
- face texture `7`;
- requested loc type `4`;
- orientation `5`;
- recolors `100 -> 200`, then `200 -> 300`;
- retextures `7 -> 8`, then `8 -> 9`;
- scale `(256, 64, 128)`;
- translation `(10, -5, 20)`.

Correct expected output:

- vertices `(-262, 27, -115)`, `(-80, -5, -25)`, `(-80, 59, -25)`;
- face color `300`;
- face texture `9`;
- expected SHA-256 `e40dcd4a67048e4d05d7baee3340d8aa4d96f1ae13bf4f3af4bae71d0f2b78fc`.

`reference-fixtures/manifest/M5-MODEL-MIGRATION-v2.md` records this correction explicitly. Its Git blob identity is `146c3c5fac2611d8cb9968353de30436defc1a85`.

The type-4 manifest now points to this v2 record. The v1 record is retained as historical provenance rather than rewritten.

No production transform code changed. The runner remained strict. The correction changed the fixture to the actual cited executable source case instead of weakening equality or accepting a newly observed output as an oracle.

### Validation

Implementation head `eb4d6ee8909a8170374a5e90e419d0639399c5c3` passed workflow `37901677283`:

- Tier A: PASS
  - architecture boundaries;
  - architecture guard;
  - rustfmt;
  - locked workspace check;
  - strict clippy.
- Tier B: PASS
  - full workspace tests;
  - repository-wide M5 runner passes all three fixtures.
- Tier C: PASS
  - M3 P0 decode fixtures;
  - M3 deterministic decoder fuzz smoke;
  - M4 fixture inventory;
  - M4 exact model construction/instance transforms;
  - M4 model decoder fuzz smoke;
  - dedicated M5 repository-wide normalized fixture runner.

### Checkpoint 3 implementation diff against Checkpoint 2

Implementation head is 14 commits ahead and 0 behind Checkpoint 2 documentation head `38117bbbb511bafed527fe5acb6bc335bcc9e330`.

Permanent implementation changes are confined to:

1. `.github/workflows/ci.yml`
2. `crates/osrs-reference/src/inventory.rs`
3. `crates/osrs-reference/src/lib.rs`
4. `crates/osrs-reference/src/runner.rs`
5. `crates/osrs-reference/tests/m5_fixture_runner.rs`
6. `crates/osrs-reference/tests/m5_normalized_model_fixtures.rs`
7. `reference-fixtures/manifest/M5-MODEL-MIGRATION-v2.md`
8. `reference-fixtures/manifest/model-transform-type4-order.yaml`
9. `reference-fixtures/model/transform_type4_order.expected.json`
10. `reference-fixtures/model/transform_type4_order.input.json`

No `osrs-core`, `osrs-cache`, `osrs-scene`, renderer, editor, or production semantic implementation changed.

### Explicit non-goals / deferred work

Checkpoint 3 does not implement:

- Java/deob executable regeneration;
- an oracle source checkout/downloader;
- external source-root mutation;
- automatic expected-output rewriting;
- historical `deob_golden.txt` migration;
- normal, lighting, placement, bridge/plane, or face-priority semantic fixtures;
- parity-matrix promotion for those later semantic families.

Those remain owned by Checkpoints 4-6.

---

## Current milestone boundary

Checkpoint 3 implementation is complete. The documentation-complete branch head must pass the full Tier A/B/C chain before Checkpoint 3 is closed.

No M5 pull request should be opened until the milestone exit checkpoint.
