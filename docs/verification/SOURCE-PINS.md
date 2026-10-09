# Source Pins and Provenance

Status: **M5 closure provenance record; unresolved equivalence gates remain explicit**

This file records exact source identities used by semantic audits and fixtures. A date, local filesystem path, historical research document, generated API page, or obfuscated class name alone is not an acceptable final source pin.

## Repository baseline

RustOSRS blueprint baseline:

`489b0603bc7c2c9fabd2614d204f809fb017d0cb`

Blueprint branch:

`blueprint/osrs-editor-foundation`

## RuneLite reference import

Imported tree:

`runelite-master/`

RustOSRS tree SHA:

`5afef996a992bacc73655860681269242e90d6e8`

Research notes identify this import as an October 4, 2026 RuneLite snapshot. Until the exact upstream RuneLite commit corresponding to the import is recorded, semantic citations must use the RustOSRS tree/blob identity in addition to any descriptive date.

Selected imported file pins used by renderer/API audits:

| File | RustOSRS blob SHA | Audit use |
|---|---|---|
| `runelite-master/runelite-api/src/main/java/net/runelite/api/Constants.java` | `407831d491b39afd1230552f7475f86ce9e79be2` | scene/chunk/region sizes and tile flags |
| `runelite-master/runelite-api/src/main/java/net/runelite/api/Perspective.java` | `648a593690b777c1522c7afb16203f547e044b88` | local tile size, angle tables, projection reference |
| `runelite-master/runelite-api/src/main/java/net/runelite/api/Texture.java` | `80a4d1a45a51b28c3d5da9f0ad47a03ccf0a482f` | texture animation inputs |
| `runelite-master/runelite-client/src/main/java/net/runelite/client/plugins/gpu/SceneUploader.java` | `83ac701f1b2ee7a039879941bcc710527dbc54c1` | terrain upload, bridge/roof grouping, static alpha split, terrain UVs |
| `runelite-master/runelite-client/src/main/java/net/runelite/client/plugins/gpu/ModelUploader.java` | `35347963838d1be74deddd59027a73623d7872f3` | dynamic priority ordering, transparency composition, object UV reconstruction |
| `reference-shaders/runelite-gpu/vert.glsl` | `d899cf3180295bd18d30adf901fd7a460e560318` | texture animation and RuneLite clip-space face-bias application |

## Historical local melxin/deob evidence

The original `reference-fixtures/deob_golden.txt` was produced from a developer-machine checkout historically described by:

`/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java`

That absolute path is historical provenance only. It is not a reproducible source pin and the current harness does not depend on it.

Checked-in historical identities are byte-gated as Git blobs:

- `reference-fixtures/deob_golden.txt` -> `49887733ad463572cf61bc059733b7c5f5fd26f4`;
- `tools/deob-harness/src/Dumper.java` -> `ceefbd6e97c8e0b09c2ef196b3f9fd2e0f763236`.

Historical evidence classification:

`historical_local_harness_corroborated_by_public_source`

This classification deliberately does not claim that the old developer-machine checkout was byte-identical to the public source revision used by M5.

## Current deob harness contract

`tools/deob-harness/run.sh` requires explicit caller-supplied inputs:

- `--checkout PATH`;
- `--expected-commit SHA`;
- `--bcprov JAR`;
- `--output FILE`;
- optional `--work-dir DIR`.

The checkout must be a Git worktree exactly at the requested commit.

Candidate generation safety:

- output inside `reference-fixtures/` is rejected;
- existing candidate output is not silently overwritten;
- ordinary CI does not clone/download or regenerate accepted output;
- no automatic acceptance mode edits manifests or expected hashes.

Tier A validates this boundary through `scripts/test_reference_regeneration.py`.

## Pinned public melxin source

Canonical public M5 reference revision:

`melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`

Commit tree:

`3d934e2fef3e90130974f4cbd8f125c6af6a9fb5`

The commit message references RuneLite version commit:

`d175041233e4974fecef6081449585ee7af097e8`

### Exact public file pins

| File | Blob SHA | M5 / semantic use |
|---|---|---|
| `runescape-client/src/main/java/ObjectComposition.java` | `079451cd9a6dcfd2666efd15b0524250eaafe4c4` | typed/untyped model selection and object-model transform ordering |
| `runescape-client/src/main/java/ModelData.java` | `2cc9406b2504fbd4fae0c0c952aa2d133809e928` | mirror/winding, transforms, base normals, cross-model normal merge, lighting |
| `runescape-client/src/main/java/Model.java` | `c2aa55c0e8fea89fae0da33d782119f8c109cacf` | contouring and reference face-priority emission |
| `runescape-client/src/main/java/Rasterizer3D.java` | `f32216b5e564c6a03a173438e3b19004c27c1c9e` | exact sine/cosine tables for type-4 transforms |
| `runescape-client/src/main/java/Scene.java` | `f15260a63103952fe8f5ffbdb62f5c7c39d94565` | scene storage, game-object placement, bridge relinking through `setLinkBelow` |
| `runescape-client/src/main/java/SceneTileModel.java` | `ce6a179cfa93e02271af87164e102ee538223718` | exact terrain shape/rotation topology |
| `runescape-client/src/main/java/FloorUnderlayDefinition.java` | `f06136263590143afeedf5a8e448cd901f960614` | underlay RGB-to-weighted-HSL conversion |
| `runescape-client/src/main/java/FloorOverlayDefinition.java` | `f3a15cc07c53ccae74b2219db88d456b77d88d68` | overlay fields, defaults, decode, primary/secondary HSL |
| `runescape-client/src/main/java/Tiles.java` | `e4655da97d297f3d4fb53e9e7ae9816f1bdd5790` | bridge bit consumer and orientation tables |
| `runescape-client/src/main/java/DynamicObject.java` | `3ff5ba2c1c4fb4cc914326cc70223720d5f2e77d` | decoded loc placement and bridge-adjusted collision-plane selection |

M5 revalidated the public blob identities used by normalized fixtures against the exact pinned upstream revision.

## M5 normalized semantic fixtures

The following M5 fixtures execute production semantic code in ordinary offline Tier C verification:

1. `model.selection.typed_exact.orientation_4`
   - source: `ObjectComposition.java`;
   - symbols covering exact typed selection / orientation mirror semantics;
   - production executor: `select_object_model`.
2. `model.mirror.geometry_winding`
   - source: `ModelData.java`;
   - production executor: `mirror_source_model`.
3. `model.transform.type4_order`
   - source: `ObjectComposition.java`, `ModelData.java`, `Rasterizer3D.java`;
   - production executor: `apply_object_model_instance_transforms`.

Each fixture manifest records exact repository, commit, file blob, symbol, normalization record, input/output paths, and expected-output SHA-256.

## M5 evidence-only normalized fixtures

Evidence-only fixtures are accepted only when the corresponding production executor does not yet exist. Their manifests still require exact source pins, normalization revision, expected output, and expected-output SHA-256.

### Base normals

Fixtures:

- `normals.base.smooth_triangle`;
- `normals.base.flat_triangle`.

Source:

- `ModelData.java` blob `2cc9406b2504fbd4fae0c0c952aa2d133809e928`;
- symbol `calculateVertexNormals`.

The crafted triangle preserves exact integer cross-product/normalization behavior and distinguishes smooth vertex accumulation from flat face-normal storage.

Production owner: M7. `NORMALS-001` is not promoted to `EXISTING` by M5 evidence.

### Cross-model normal merge controls

Fixtures:

- `normals.merge.coincident_triangle.hide_false`;
- `normals.merge.coincident_triangle.hide_true`;
- `normals.merge.translated_negative`.

Source:

- `ModelData.java` blob `2cc9406b2504fbd4fae0c0c952aa2d133809e928`;
- symbol `method5262`.

These fixtures make exact coincident positive matching, face hiding, and a translated no-match control reviewable before M7 production implementation.

Production owner: M7. `NORMALS-002` remains `REQUIRED`.

### Plane/link-below structural relinking

Fixture:

`planes.link_below.four_plane_column`

Source:

- `Scene.java` blob `f15260a63103952fe8f5ffbdb62f5c7c39d94565`;
- symbol `setLinkBelow`.

The expected artifact records storage-slot shifts, tile-plane decrements, linked-below identity, top-slot clearing, qualifying anchored type-2 game-object plane decrements, and negative controls.

Production owner: M6. `PLANES-003` remains `REQUIRED`.

### Face-priority order

Fixture:

`priority.all_0_11.threshold_crossing`

Source:

- `Model.java` blob `c2aa55c0e8fea89fae0da33d782119f8c109cacf`;
- symbol `method5946`.

It preserves priorities `0..11`, threshold groups, priority-10/11 queue behavior, signed alpha metadata, and exact ordered face IDs.

Production owner: renderer milestone. `FACE-002` remains `REQUIRED`.

## M5 semantic-evidence normalization record

`reference-fixtures/manifest/M5-SEMANTIC-EVIDENCE-MIGRATION-v1.md`

Git blob:

`2e1ac34d309ce6f9c0c401e462746e31377cacec`

This migration records the source derivation for base normals, normal-merge controls, and `Scene.setLinkBelow` structural relinking.

It does not claim executable RustOSRS parity for those later-owned semantics.

## Indexed historical evidence

`reference-fixtures/historical/deob_golden.index.json` exposes stable IDs for useful historical families while retaining their historical provenance classification:

- `terrain.shape_gallery.all_13x4`;
- `contour.synthetic.flat_slope`;
- `lighting.synthetic_triangle.loc_rig`;
- `placement.wall_types.orientation_matrix`;
- `placement.decor_types.orientation_matrix`;
- `placement.floor_type22.storage`;
- `placement.game_object.footprint_and_capacity`.

`scripts/test_deob_golden_index.py` verifies historical fixture/harness Git blob identities, classification, IDs, public pin shape, exact-line uniqueness, and expected prefix counts entirely offline.

## Historical whole-snapshot equivalence gate

Before any specification relies on the old developer-machine melxin/deob tree as exact whole-snapshot provenance, one of the following is required:

1. identify and record its upstream Git commit;
2. vendor a source manifest containing hashes for every audited deob file; or
3. prove the relevant file/method body is byte/semantic equivalent to a pinned public source.

Until then, specs may state that a behavior is verified for the exact public commit while separately recording historical local-harness corroboration. They must not describe the entire old local checkout as exactly pinned.

## Terrain-builder gate

The complete terrain color/build routine described by older research remains unresolved at the exact-source level because the cited obfuscated `class470` does not match the public January 28 source.

Therefore the full builder-side 11x11 blur, slope-lighting builder, random color walk, overlay sentinel path, and related writes remain `REVISION_SENSITIVE` until the actual source method is pinned or an exact executable oracle proves them.

`TERRAIN-004` remains blocked. Terrain topology itself is independently pinned through `SceneTileModel` and historical indexed evidence.

## OpenRune FileStore

Imported tree:

`OpenRune-FileStore-main/`

Tree SHA:

`55f571db4b23f2d528786e1cdfbcba0061fd201a`

Treat as independent decoder/tooling evidence, not as the OSRS semantic oracle by itself.

## rs-cache

Imported tree:

`rs-cache-master/`

Tree SHA:

`fae41f98352fc804e5d13d9bd2e836ab1e2635cd`

Treat as candidate Rust cache dependency/reference. Revision lineage and known decoder gaps remain explicit compatibility concerns.

## Shader reference tree

RustOSRS path:

`reference-shaders/runelite-gpu/`

Tree SHA:

`d33651c6b87ec5d61c43ab2e1e69d15ed6083877`

This directory has mixed provenance. Individual files must be classified and pinned rather than treating the whole directory as one semantic source.

## Final rule

A final atomic OSRS specification must terminate in a pinned primary source, executable oracle/fixture with exact provenance, or explicit accepted project decision. It must not terminate only in research prose, a date, a local path, or an unstable obfuscated class name.
