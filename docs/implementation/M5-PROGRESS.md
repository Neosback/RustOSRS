# M5 Progress: Reference Fixture Infrastructure

Status: **IN PROGRESS**  
Milestone: `M5 - Reference fixture infrastructure`  
Branch: `impl/m5-reference-fixture-infrastructure`  
Baseline: M4 squash merge `4d3d21dbe449cd345cf46ccbab651bec6366d585`

## Milestone purpose

M5 turns `osrs-reference` into a development/test-only reference fixture system before scene, normals, lighting, and other high-risk semantic work depends on differential evidence.

Production crates must not depend on `osrs-reference`. Ordinary verification remains offline and deterministic. Oracle execution and fixture regeneration are separate development workflows and must never silently rewrite checked-in expected outputs during CI.

## Checkpoint plan

1. **Fixture loader/comparator/provenance foundation** - COMPLETE
2. **Canonical normalized fixture schemas and first migrated source-pinned fixtures** - COMPLETE
3. **Exact fixture inventory/runner integration and dedicated M5 CI gates** - COMPLETE
4. **Explicit regeneration command and isolated oracle/harness adapters** - IMPLEMENTATION COMPLETE; FINAL DOCUMENTATION-HEAD CI PENDING
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
   - selected model ID `200`, mirror `true`;
   - expected SHA-256 `e21314f355237d336ac5c5eb1497e477238384824d653e5ca20fff2442c86024`.
2. `model.mirror.geometry_winding`
   - owns `MODEL-BUILD-002`;
   - exact model-local Z negation and face A/C winding swap;
   - expected SHA-256 `5880185db6949fb6f6a10feb2b84c0600032cd80073a0bdf24d1e868e8e6bde1`.
3. `model.transform.type4_order`
   - owns `MODEL-BUILD-003`, `COORD-002`;
   - source-pinned to `ObjectComposition`, `ModelData`, and `Rasterizer3D`;
   - its original manually normalized v1 vector was later corrected by Checkpoint 3 after executable runner validation proved it did not reproduce the cited M4 test.

`reference-fixtures/manifest/M5-MODEL-MIGRATION-v1.md` records the original manual source-pinned migration and deliberately does not claim Java/deob executable generation.

Pinned oracle source:

- `melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`
- `ObjectComposition.java` blob `079451cd9a6dcfd2666efd15b0524250eaafe4c4`
- `ModelData.java` blob `2cc9406b2504fbd4fae0c0c952aa2d133809e928`
- `Rasterizer3D.java` blob `f32216b5e564c6a03a173438e3b19004c27c1c9e`

Checkpoint 2 validated schema, provenance shape, path integrity, and expected hashes. Repository-wide semantic execution was deliberately deferred to Checkpoint 3.

---

## Checkpoint 3 - repository-wide fixture runner and dedicated M5 CI gate

Status: **COMPLETE**

Implementation validation head: `eb4d6ee8909a8170374a5e90e419d0639399c5c3`  
Implementation CI: `37901677283`  
Documentation validation head: `b9c97271e51fbca63b1a0c1910f3389d80d12db8`  
Documentation-head CI: `37901932522`

### Deterministic inventory

`crates/osrs-reference/src/inventory.rs` introduces `FixtureInventory`.

Discovery rules:

1. start under the configured fixture root's `manifest/` directory;
2. recursively discover every `.yaml` manifest;
3. reject symlink entries in the manifest tree;
4. sort manifests deterministically by root-relative path;
5. load every manifest through `FixtureRepository` so path, provenance, and expected SHA-256 checks run before execution;
6. reject duplicate fixture IDs;
7. reject an empty YAML inventory.

A newly checked-in YAML fixture therefore cannot silently escape the repository-wide M5 gate because an author forgot to add it to a hand-maintained list.

### Exact semantic runner

`crates/osrs-reference/src/runner.rs` executes each integrity-verified normalized fixture through production `osrs-core` semantics:

- `model_selection` -> `select_object_model`;
- `model_mirror` -> `mirror_source_model`;
- `model_transform` -> `apply_object_model_instance_transforms`.

The runner normalizes only the fields owned by each fixture family and requires exact typed equality with the checked-in expected document. It introduces no tolerance, sorting, hidden mirror/placement stage, renderer-space conversion, or fallback acceptance.

`crates/osrs-reference/tests/m5_fixture_runner.rs` asserts the deterministic current inventory:

1. `model.mirror.geometry_winding`
2. `model.selection.typed_exact.orientation_4`
3. `model.transform.type4_order`

and requires all three to execute successfully.

### Dedicated Tier C gate

`.github/workflows/ci.yml` now runs:

`cargo test --locked -p osrs-reference --test m5_fixture_runner`

as `Run M5 repository-wide normalized fixture runner` after the existing M3/M4 semantic gates.

Ordinary CI remains offline with respect to oracle execution and regeneration.

### Runner-detected correction to the Checkpoint 2 transform migration

The first repository-wide execution correctly failed `model.transform.type4_order` while M4 production transform tests remained green. The failure proved the Checkpoint 2 v1 transform artifact was hash-consistent but not an exact normalization of the executable M4 case it cited.

The cited M4 case was re-read from commit `384b1274d40bbc8b4acd383ee31364b82a2e08cc`, test `combined_pipeline_matches_reference_order_exactly`.

Correct normalized input:

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

`reference-fixtures/manifest/M5-MODEL-MIGRATION-v2.md` records the correction explicitly. Its Git blob is `146c3c5fac2611d8cb9968353de30436defc1a85`. The type-4 manifest points to that v2 record; v1 remains checked in as historical provenance.

No production transform code changed. The runner remained strict. The artifact was corrected to the actual cited executable source case rather than weakening equality or accepting the runner's previous observed output as a new oracle.

### Validation

Implementation head `eb4d6ee8909a8170374a5e90e419d0639399c5c3` passed workflow `37901677283`:

- Tier A: PASS
- Tier B: PASS, including repository-wide fixture execution
- Tier C: PASS, including the dedicated M5 runner plus all existing M3/M4 gates

Documentation validation head `b9c97271e51fbca63b1a0c1910f3389d80d12db8` passed the same full chain in workflow `37901932522`.

Checkpoint 3 was then closed by documentation-only commit `cda2dd13214fd342b1f0c1b6e17f2e65c32e6b5e`, which also passed the full Tier A/B/C chain in workflow `37902148092`.

### Checkpoint 3 implementation diff against Checkpoint 2

Implementation head was 14 commits ahead and 0 behind Checkpoint 2 documentation head `38117bbbb511bafed527fe5acb6bc335bcc9e330`.

Permanent implementation changes were confined to:

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

---

## Checkpoint 4 - explicit candidate regeneration and isolated deob adapter

Status: **IMPLEMENTATION COMPLETE; FINAL DOCUMENTATION-HEAD CI PENDING**

Implementation validation head: `a6364411c473f1c4e847fe43dc6ee8bb9ae9a604`  
Implementation CI: `37903583746`

### Explicit regeneration entry point

`tools/reference-fixtures/regenerate.py` is now the public development-only regeneration command.

The initial adapter is:

`melxin-deob-golden`

It pins the public oracle checkout exactly to:

`melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`

The command accepts explicit paths for:

- the local pinned source checkout;
- the caller-supplied compile-only `bcprov-jdk15on-1.52.jar`;
- an output candidate outside the checked-in fixture tree;
- an optional isolated work directory.

It does not clone source repositories, download dependencies, update manifests, update expected hashes, or accept generated output into the repository.

### Candidate-only acceptance boundary

Regeneration cannot write directly into `reference-fixtures/`.

Both the public Python entry point and the lower-level shell harness reject candidate paths under the checked-in fixture root. Existing candidate paths are also rejected instead of silently overwritten.

There is intentionally no `--accept` mode.

Promoting regenerated evidence requires a separate reviewed change that explicitly updates the expected artifact, `expected_sha256`, and source/harness provenance when those identities changed. The repository-wide M5 runner must then pass against the checked-in result.

This is stronger than merely asking developers not to rewrite expected output during regeneration: the command itself cannot perform that mutation.

### Isolated deob harness adapter

`tools/deob-harness/run.sh` no longer contains Tyler's historical absolute local RuneLite paths and no longer writes `reference-fixtures/deob_golden.txt` directly.

The harness now requires:

- `--checkout PATH`;
- `--expected-commit SHA`;
- `--bcprov JAR`;
- `--output FILE`;
- optional `--work-dir DIR`.

Before compiling it validates:

- the expected commit is exactly 40 hexadecimal characters;
- the supplied checkout is a Git checkout;
- checkout HEAD exactly equals the expected commit;
- the required `runescape-client` and `injection-annotations` source roots exist;
- the caller-supplied bcprov jar exists;
- candidate output does not already exist;
- candidate output is outside `reference-fixtures/`.

The harness no longer downloads Maven artifacts itself. It does not mutate the reference checkout. When no work directory is supplied it creates and removes an isolated temporary build directory.

After successful execution it writes only the candidate artifact and prints its SHA-256 and byte length for review.

### Offline tooling gate

`scripts/test_reference_regeneration.py` verifies without Java, RuneLite, Maven, or network access that:

- the public adapter uses the exact pinned deob commit;
- checked-in candidate paths are rejected;
- external candidate paths are accepted;
- the lower-level command receives the exact pin and candidate path;
- the legacy machine-specific `/Users/tylercovalt` path is absent;
- direct `reference-fixtures/deob_golden.txt` writes are absent;
- hidden `curl` dependency acquisition is absent;
- the shell harness retains candidate-path rejection.

Tier A now runs this test as `Test reference regeneration safety`.

Ordinary CI still never executes the Java/deob oracle.

### Documentation

- `tools/reference-fixtures/README.md` documents the candidate-only workflow and explicit promotion steps.
- `tools/deob-harness/README.md` documents the parameterized low-level harness, exact checkout validation, no-network behavior, and Checkpoint 5 migration boundary.

The three current normalized M5 model fixtures remain honestly classified as source-pinned manual normalizations. Checkpoint 4 does not retroactively claim they were generated by the Java harness.

### Validation

Implementation head `a6364411c473f1c4e847fe43dc6ee8bb9ae9a604` passed workflow `37903583746`:

- Tier A: PASS
  - architecture boundaries;
  - architecture guard;
  - offline reference-regeneration safety tests;
  - rustfmt;
  - locked workspace check;
  - strict clippy.
- Tier B: PASS
  - complete workspace tests.
- Tier C: PASS
  - all existing M3/M4 semantic gates;
  - M5 repository-wide normalized fixture runner.

No oracle execution, source checkout, Java compilation, Maven access, or expected-output mutation occurs in CI.

### Checkpoint 4 implementation diff against Checkpoint 3

Implementation head is 6 commits ahead and 0 behind Checkpoint 3 closeout head `cda2dd13214fd342b1f0c1b6e17f2e65c32e6b5e`.

Permanent implementation changes are confined to six files:

1. `.github/workflows/ci.yml`
2. `scripts/test_reference_regeneration.py`
3. `tools/deob-harness/README.md`
4. `tools/deob-harness/run.sh`
5. `tools/reference-fixtures/README.md`
6. `tools/reference-fixtures/regenerate.py`

No `osrs-core`, `osrs-cache`, `osrs-scene`, renderer, editor, normalized expected artifact, or production semantic implementation changed.

### Deferred work

Checkpoint 4 does not:

- index or migrate `deob_golden.txt` into canonical fixture manifests;
- add normals, lighting, loc placement, bridge/plane, or priority normalized fixture families;
- automatically accept candidate oracle output;
- change current fixture provenance from manual normalization to executable generation;
- promote later semantic parity rows.

Those remain owned by Checkpoints 5-6.

---

## Current milestone boundary

M5 Checkpoint 4 implementation is complete. The documentation-complete branch head must pass the full Tier A/B/C chain before Checkpoint 4 is closed.

No M5 pull request should be opened until the milestone exit checkpoint.
