# Reference Fixture Contracts

Status: **M5 reference fixture infrastructure contract**

This document defines how source-derived fixtures are organized, regenerated, normalized, reviewed, and consumed by Rust tests.

## 1. Goals

Reference fixtures must be:

- reproducible;
- source-pinned;
- deterministic;
- small enough to diagnose;
- diffable where practical;
- mapped to one or more canonical spec IDs;
- consumable without requiring the reference implementation at ordinary test time.

## 2. Fixture family layout

Canonical layout:

```text
reference-fixtures/
  manifest/
  historical/
  placement/
  model/
  normals/
  lighting/
  morph/
  animation/
  contour/
  terrain/
  planes/
  priority/
  alpha/
  textures/
  scenes/
  images/
```

M5 has populated normalized model, normals, planes, and priority families and has indexed the useful historical `deob_golden.txt` evidence without rewriting its provenance.

Not every later family must be populated before its owning production milestone, but the first required M5 families are now present either as normalized manifests or indexed historical evidence.

## 3. Fixture ID convention

Use stable semantic IDs such as:

```text
placement.loc_type_02.orientation_0
model.mirror.typed.rotated_true.orientation_4
normals.merge.coincident_triangle.hide_false
priority.all_0_11.threshold_crossing
terrain.shape_12.rotation_3
planes.link_below.four_plane_column
```

Fixture IDs must not contain transient Java line numbers or opaque generated filenames.

## 4. Manifest contract

Every canonical normalized fixture has a YAML manifest.

Example:

```yaml
fixture_id: normals.merge.coincident_triangle.hide_false
schema_version: 1
owned_specs:
  - NORMALS-002
parity_level: P1
execution: evidence_only
oracle:
  kind: melxin-deob-source
  repository: melxin/runelite
  commit: 1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc
  files:
    - path: runescape-client/src/main/java/ModelData.java
      blob: 2cc9406b2504fbd4fae0c0c952aa2d133809e928
      symbol: method5262
harness:
  path: reference-fixtures/manifest/M5-SEMANTIC-EVIDENCE-MIGRATION-v1.md
  revision: 2e1ac34d309ce6f9c0c401e462746e31377cacec
input: normals/merge_coincident_triangle_hide_false.input.json
expected: normals/merge_coincident_triangle_hide_false.expected.json
expected_sha256: 35cb9988c6511c8bb6f9a1e592c5f54c80c109dbc17b3a7b80617d17339b4a3b
normalization:
  - preserve separate left and right model identity
  - preserve exact integer translation and vertex order
  - preserve absent face render type arrays when hiding is disabled
notes: Source-pinned evidence. Production normal merging belongs to M7.
```

Every manifest must record:

- stable fixture ID;
- schema version;
- owned specs;
- parity level;
- execution classification;
- exact oracle repository/commit/file/blob/symbol provenance;
- exact harness or normalization revision;
- input and expected paths;
- expected-output SHA-256;
- explicit normalization rules.

When a fixture is sourced from an imported renderer/API tree rather than pinned deob semantic code, the manifest must say so explicitly.

## 5. Execution classifications

### `semantic`

Use when a production RustOSRS semantic executor already exists.

The M5 runner must execute the normalized input through production code and compare exact typed output.

M5 semantic kinds:

- `model_selection`;
- `model_mirror`;
- `model_transform`.

### `evidence_only`

Use only when the owning production executor deliberately belongs to a later milestone.

The runner still validates:

- normalized input/output schemas;
- input/expected kind equality;
- exact manifest provenance;
- expected-output SHA-256;
- deterministic inventory inclusion.

M5 evidence-only kinds:

- `base_normals`;
- `normal_merge`;
- `plane_link_below`;
- `priority_order`.

An implemented semantic kind cannot be downgraded to `evidence_only`. This is permanently regression-tested.

Evidence-only fixtures are contracts for later owners, not claims that the production behavior is already implemented.

## 6. Input normalization

Reference and Rust implementations must consume equivalent logical input.

Normalization rules belong in the manifest or pinned migration/harness adapter, not hidden inside expected-output post-processing.

Examples:

- stable vertex/face input ordering;
- explicit signed/unsigned byte interpretation;
- explicit orientation values;
- explicit tile/local coordinate origin;
- deterministic varbit/varp state;
- deterministic animation tick/frame;
- deterministic terrain jitter input;
- explicit camera projection state for renderer oracles.

Do not normalize away behavior under test.

For example, a normal-merge fixture must not sort vertices by position after output if original vertex identity is part of the contract.

## 7. Output normalization

Output normalization is permitted only when the reference implementation includes irrelevant unstable presentation details.

Allowed examples:

- remove Java object identity/hash strings;
- sort a diagnostic map whose iteration order is not semantic;
- normalize path separators in provenance strings.

Not allowed:

- sort face emission order in a priority fixture;
- epsilon-round integer geometry;
- collapse `None`/missing model into a default object;
- renumber scene planes to make outputs match;
- ignore matched-face render type changes.

## 8. Regeneration workflow

Fixture regeneration is explicit and reviewable.

Required sequence:

1. select the exact oracle source/ref;
2. verify source pins against `SOURCE-PINS.md`;
3. build/run the generator in a clean environment where practical;
4. generate output into a candidate location outside `reference-fixtures/`;
5. compare against checked-in expected output;
6. inspect semantic differences;
7. update manifests/hashes only when the change is intended and reviewed;
8. run Rust parity tests against the accepted fixture diff;
9. commit logically coupled source/fixture changes together.

CI never regenerates and accepts fixtures automatically.

## 9. Harness isolation and M5 regeneration boundary

Reference harnesses remain development/test tooling.

`tools/reference-fixtures/regenerate.py` and `tools/deob-harness/run.sh` now provide the M5 candidate-only workflow.

The deob adapter requires explicit:

- source checkout;
- expected Git commit;
- dependency JAR;
- output path;
- optional work directory.

It verifies the checkout commit and rejects accepted-fixture output locations and silent overwrite.

Ordinary CI does not require Java, the public source checkout, network access, or regeneration.

Tier A validates this safety boundary with `scripts/test_reference_regeneration.py`.

## 10. Historical deob evidence migration

`reference-fixtures/deob_golden.txt` remains historical local-harness evidence and is not relabeled as if it were generated from the pinned public source.

M5 indexes it through:

`reference-fixtures/historical/deob_golden.index.json`

Classification:

`historical_local_harness_corroborated_by_public_source`

The index provides stable IDs for terrain topology, contour control, lighting control, wall/decor/floor placement, and game-object footprint/capacity evidence.

`scripts/test_deob_golden_index.py` verifies exact checked-in golden/harness Git blob identities and the index structure entirely offline.

## 11. Normal fixture family

### M5 canonical evidence fixtures

#### `normals.base.smooth_triangle`

Owns `NORMALS-001` evidence.

Expected:

- exact base vertex normals;
- exact magnitude counts;
- absent face-normal storage for the smooth case.

#### `normals.base.flat_triangle`

Owns `NORMALS-001` evidence and proves the flat/smooth distinction.

Expected:

- zero vertex accumulators;
- exact flat face normal.

#### `normals.merge.coincident_triangle.hide_false`

Owns `NORMALS-002` evidence.

Expected:

- both models remain separate;
- exact merged normal components/magnitudes on matched vertices;
- absent face-render-type arrays remain absent.

#### `normals.merge.coincident_triangle.hide_true`

Owns `NORMALS-002` evidence.

Expected fully matched faces become render type `2`.

#### `normals.merge.translated_negative`

Owns `NORMALS-002` negative evidence.

Expected no matches and no merged-normal storage.

All five are `evidence_only` in M5. M7 must execute the same normalized contracts through production normal code before the owning specs advance.

### Later normal/lighting production fixtures

Still required by M7:

- `normals.scene.dual_wall`;
- `normals.scene.floor_decor_hide`;
- `lighting.merged_normal_changes_output`.

These require actual scene finalization and production lighting ownership and were not fabricated in M5.

## 12. Model fixture family

Current normalized M5 semantic fixtures include:

- typed exact selection with orientation/mirror behavior;
- mirrored geometry/winding;
- special type-4 transform order with recolor/retexture/resize/translation.

Permanent M4 tests additionally cover broader typed/untyped selection, combine, orientations, and immutability cases.

Expected model outputs retain integer vertices, triangle indices, colors/textures, and relevant metadata rather than only screenshots.

## 13. Priority/alpha fixture family

M5 canonical fixture:

`priority.all_0_11.threshold_crossing`

It includes:

- at least one face in each priority `0..11`;
- distinct `avg12`, `avg34`, `avg68` thresholds;
- priority-10 and priority-11 queue cases around thresholds;
- signed alpha metadata;
- unique face IDs;
- exact ordered face IDs.

It is `evidence_only` until the renderer-owned production priority executor exists. Final pixels are not the primary oracle for `FACE-002`.

## 14. Bridge fixture family

M5 canonical fixture:

`planes.link_below.four_plane_column`

The input assigns stable labels to every tile and game object around `Scene.setLinkBelow`.

Expected output records:

- storage slot -> original tile label;
- stored tile plane;
- qualifying game-object stored-plane decrement;
- non-qualifying object negative controls;
- linked-below label;
- cleared top slot.

It is `evidence_only`. M6 must execute the same contract through production scene code before `PLANES-003` advances.

## 15. Terrain fixture family

### Existing topology

The historical index preserves all `13 x 4` shape/rotation outputs as exact evidence for `TERRAIN-001`.

### Flat terrain

A later production milestone still needs a four-distinct-height/color flat tile so the diagonal split is visible in data.

### Floor definitions

M3 already contains exact underlay/overlay decode and color-helper coverage.

### Terrain-color builder

Do not generate a canonical complete-builder fixture from the stale `class470` attribution.

`TERRAIN-004` remains blocked until the correct builder is pinned or a separate exact executable oracle with reproducible provenance is accepted.

## 16. Morph/animation fixtures

Morph fixtures must encode both input selector state and selected output definition.

Required cases include:

- varbit path;
- varp path;
- fallback;
- null;
- footprint-changing transform.

Animation fixtures must pin sequence/frame/tick inputs. Until complete sequence-frame semantics are promoted, fixtures should test only behavior owned by the current milestone.

## 17. UV/material fixtures

Model UV fixtures should record:

- face indices;
- texture triangle indices;
- semantic texture ID;
- input camera ray/projection state for projected reference cases;
- expected UVs.

Exact comparison is preferred where deterministic. If implementation math legitimately differs in floating operation ordering, document a narrowly bounded numeric tolerance at the V2/reference-math layer rather than relying on screenshots.

## 18. Fixture review checklist

Before accepting a new or updated fixture:

- Is the owning spec listed?
- Is execution classification correct?
- Is the oracle source exact?
- Is the harness/migration revision exact?
- Is the input minimal enough to diagnose?
- Is output normalization justified?
- Is exact comparison used where possible?
- Does the fixture accidentally encode renderer/editor policy as OSRS semantics?
- Is any cache data redistributable/licensed appropriately for repository inclusion?
- Can ordinary Rust tests consume it offline?
- If evidence-only, does the production executor genuinely not exist yet?

## 19. Asset-distribution rule

Do not commit proprietary game cache archives or unnecessary raw assets merely to make tests convenient.

Prefer:

- small synthetic semantic fixtures;
- source-derived numeric outputs;
- legally redistributable metadata;
- developer-local cache integration tests that are opt-in and fingerprint-gated.

Canonical repository fixtures should contain only the minimum data needed to prove the contract.
