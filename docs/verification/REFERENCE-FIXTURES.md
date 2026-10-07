# Reference Fixture Contracts

Status: **Checkpoint 7 verification plan**

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

Preferred future layout:

```text
reference-fixtures/
  manifest/
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

The existing `deob_golden.txt` remains valid historical evidence but should gradually be indexed/split into these families as implementation work begins.

## 3. Fixture ID convention

Use stable IDs such as:

```text
placement.loc_type_02.orientation_0
model.mirror.typed.rotated_true.orientation_4
normals.merge.coincident_triangle.hide_false
priority.all_0_11.threshold_crossing
terrain.shape_12.rotation_3
planes.link_below.four_plane_column
```

Fixture IDs are semantic and must not contain transient Java line numbers or opaque generated filenames.

## 4. Manifest contract

Every canonical fixture family has a manifest entry.

Recommended fields:

```yaml
fixture_id: normals.merge.coincident_triangle.hide_false
schema_version: 1
owned_specs:
  - NORMALS-002
parity_level: P1
oracle:
  kind: melxin-deob
  repository: melxin/runelite
  commit: 1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc
  files:
    - path: runescape-client/src/main/java/ModelData.java
      blob: 2cc9406b2504fbd4fae0c0c952aa2d133809e928
      symbol: method5262
harness:
  path: tools/deob-harness
  revision: <RustOSRS commit or harness hash>
input: normals/merge_coincident_triangle.input.json
expected: normals/merge_coincident_triangle.expected.json
expected_sha256: <hash>
normalization:
  - preserve integer order
  - stable object ids assigned by input order
notes: positive normal accumulation without matched-face hiding
```

When a fixture is sourced from the imported October RuneLite renderer rather than deob semantic code, the manifest must say so explicitly.

## 5. Input normalization

Reference and Rust implementations must consume equivalent logical input.

Normalization rules belong in the fixture manifest or harness adapter, not hidden inside expected-output post-processing.

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

## 6. Output normalization

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

## 7. Regeneration workflow

Fixture regeneration is an explicit, reviewable workflow.

Required sequence:

1. select the exact oracle source/ref;
2. verify its source pins against `SOURCE-PINS.md`;
3. build/run the fixture generator in a clean environment where practical;
4. generate outputs into a temporary directory;
5. compare against checked-in expected outputs;
6. inspect semantic differences;
7. update manifests/hashes if the change is intended;
8. run Rust parity tests against the regenerated fixtures;
9. commit source/fixture changes together when they are logically coupled.

CI never regenerates and accepts fixtures automatically.

## 8. Harness isolation

Reference harnesses must remain development/test tooling.

Recommended design:

```text
osrs-reference/
  oracle/
    deob_adapter
    runelite_renderer_adapter
  fixture/
    schema
    loader
    comparator
  cli/
    regenerate
    compare
```

Production crates cannot depend on Java, reference source trees, or fixture generators.

## 9. Existing deob harness migration

The current `tools/deob-harness` remains useful, but implementation should improve it before relying on newly generated canonical fixtures.

Required improvements:

- remove absolute developer-machine source path assumptions;
- accept an explicit source root or pinned source checkout;
- record source commit/file hashes in generated manifests;
- support fixture-specific commands rather than only one monolithic dump;
- emit structured data suitable for exact Rust comparison;
- provide a deterministic harness-version identity;
- keep regeneration separate from test execution.

Until this is done, `deob_golden.txt` remains partially reproducible historical evidence as documented by `SOURCE-PINS.md`.

## 10. Normal fixture family

Minimum canonical fixtures:

### `normals.base.smooth_triangle`

Owns `NORMALS-001`.

Expected:

- base vertex normals;
- magnitude counts;
- face-normal state where applicable.

### `normals.base.flat_triangle`

Proves flat/smooth distinction.

### `normals.merge.coincident_triangle.hide_false`

Owns `NORMALS-002`.

Expected:

- both models remain separate;
- merged normal components/magnitudes on all matched vertices;
- face render type unchanged by hiding.

### `normals.merge.coincident_triangle.hide_true`

Expected fully matched faces become render type `2`.

### `normals.merge.translated_negative`

No matches, no merged state.

### `normals.scene.dual_wall`

Owns `NORMALS-003`.

Proves scene finalization order and dual-arm merge.

### `normals.scene.floor_decor_hide`

Proves matched-face hiding through floor-decoration neighbor processing.

### `lighting.merged_normal_changes_output`

Owns `NORMALS-004` + `LIGHTING-001`.

Proves the merged normal changes final baked lighting relative to the control.

## 11. Model fixture family

Minimum cases:

- typed model exact hit;
- typed model miss returns absence;
- type-10 untyped combine;
- `isRotated` / orientation mirror truth table;
- mirrored winding;
- special type-4 256-JAU recenter;
- each ordinary orientation;
- combined recolor/retexture/resize/translation where reordering would change output;
- cache-sharing immutability.

Expected outputs should retain integer vertices, triangle indices, colors/textures, and relevant metadata, not only a rendered screenshot.

## 12. Priority/alpha fixture family

Priority fixtures must make the special algorithm observable.

A canonical crafted model should include:

- at least one face in each priority `0..11`;
- depths designed so `avg12`, `avg34`, and `avg68` differ;
- priority-10 and priority-11 faces above/below each threshold;
- opaque and alpha faces;
- unique face IDs so exact emission order is unambiguous.

Expected output:

```text
ordered_face_ids: [...]
```

Optionally include intermediate threshold values for diagnosis.

Do not use final pixels as the primary oracle for `FACE-002`.

## 13. Bridge fixture family

The canonical four-plane bridge fixture should assign stable labels to every tile and game object before `setLinkBelow`.

Expected output records:

- storage slot -> original tile label;
- stored tile `plane`;
- game-object stored plane;
- linked-below label;
- cleared top slot;
- source/encoded plane retained outside storage relinking.

This avoids ambiguous verification based only on what appears visually above/below another surface.

## 14. Terrain fixture family

### Existing topology

All `13 x 4` shape/rotation outputs remain exact canonical fixtures.

### Flat terrain

Add a four-distinct-height/color flat tile so the diagonal split is visible in data.

### Floor definitions

Add underlay/overlay decode fixtures around RGB/HSL clamp boundaries and secondary-color behavior.

### Terrain-color builder

Do not generate a canonical complete-builder fixture from the old stale `class470` attribution.

`TERRAIN-004` fixture creation remains blocked until:

- the correct builder is pinned, or
- a separate exact executable oracle with reproducible provenance is accepted.

## 15. Morph/animation fixtures

Morph fixtures encode both input selector state and selected output definition.

Required cases:

- varbit path;
- varp path;
- fallback;
- null;
- footprint-changing transform.

Animation fixtures must pin sequence/frame/tick inputs. Until complete sequence-frame semantics are promoted, fixtures should test only the ownership/model-resolution behavior currently specified by `ANIMATION-001`.

## 16. UV/material fixtures

Model UV fixtures should record:

- face indices;
- texture triangle indices;
- semantic texture ID;
- input camera ray/projection state for the projected reference case;
- expected UVs.

Exact comparison is preferred where the reference computation is deterministic and the selected Rust implementation is intended to reproduce the same float result. If implementation math legitimately differs in floating operation ordering, document a narrowly bounded numeric tolerance at this V2/reference-math layer rather than relying on a screenshot.

## 17. Fixture review checklist

Before accepting a new or updated fixture:

- Is the owning spec listed?
- Is the oracle source exact?
- Is the harness revision recorded?
- Is the input minimal enough to diagnose?
- Is output normalization justified?
- Is exact comparison used where possible?
- Does the fixture accidentally encode a renderer/editor policy as OSRS semantics?
- Is any cache data redistributable/licensed appropriately for repository inclusion?
- Can ordinary Rust tests consume it offline?

## 18. Asset-distribution rule

Do not commit proprietary game cache archives or unnecessary raw assets merely to make tests convenient.

Prefer:

- small synthetic semantic fixtures;
- source-derived numeric outputs;
- legally redistributable metadata;
- developer-local cache integration tests that are opt-in and fingerprint-gated.

Canonical repository fixtures should contain only the minimum data needed to prove the contract.
