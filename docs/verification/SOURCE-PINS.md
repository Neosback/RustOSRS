# Source Pins and Provenance

Status: **M5 provenance record with 2026-10-10 reference correction applied**

This file records exact source identities used by semantic audits and fixtures. A date, local filesystem path, historical research document, generated API page, or obfuscated class name alone is not an acceptable final source pin.

The correction record `docs/implementation/REFERENCE-PROVENANCE-CORRECTION-2026-10-10.md` supersedes earlier conclusions that the pinned public `class470` lacked the terrain builder.

## Repository baseline

Original RustOSRS blueprint baseline:

`489b0603bc7c2c9fabd2614d204f809fb017d0cb`

Historical blueprint branch:

`blueprint/osrs-editor-foundation`

## RuneLite reference import

Imported tree:

`runelite-master/`

RustOSRS tree SHA:

`5afef996a992bacc73655860681269242e90d6e8`

Research identifies this import as an October 2026 snapshot. Until its exact upstream RuneLite commit is recorded, renderer citations use the RustOSRS tree/blob identity.

Selected imported renderer/API file pins:

| File | RustOSRS blob SHA | Use |
|---|---|---|
| `runelite-api/.../Constants.java` | `407831d491b39afd1230552f7475f86ce9e79be2` | scene/chunk sizes and tile flags |
| `runelite-api/.../Perspective.java` | `648a593690b777c1522c7afb16203f547e044b88` | local units and projection reference |
| `runelite-api/.../Texture.java` | `80a4d1a45a51b28c3d5da9f0ad47a03ccf0a482f` | texture animation inputs |
| `runelite-client/.../gpu/SceneUploader.java` | `83ac701f1b2ee7a039879941bcc710527dbc54c1` | terrain upload, bridge/roof lookup, static alpha split, terrain UVs |
| `runelite-client/.../gpu/ModelUploader.java` | `35347963838d1be74deddd59027a73623d7872f3` | dynamic face preparation, conditional priority sorting, model UVs |
| `runelite-client/.../gpu/TextureManager.java` | `e830a518dc13c173f22f006fe52cb25e3c0fc8b3` | 128x128 texture upload, sampler state, animations |
| `runelite-client/.../gpu/frag.glsl` | `0ca7180d50ef90e5083c85f7182e60521ef76baa` | alpha discard, shader brightness, textured lightness |
| `reference-shaders/runelite-gpu/vert.glsl` | `d899cf3180295bd18d30adf901fd7a460e560318` | texture animation and clip-space face bias |

RuneLite GPU behavior is renderer evidence, not automatically OSRS semantic truth.

## Historical local melxin/deob evidence

The original `reference-fixtures/deob_golden.txt` came from a developer-machine checkout historically described by an absolute local path. That path is provenance only, not a reproducible source pin.

Checked-in historical identities are byte-gated as Git blobs:

- `reference-fixtures/deob_golden.txt` -> `49887733ad463572cf61bc059733b7c5f5fd26f4`
- `tools/deob-harness/src/Dumper.java` -> `ceefbd6e97c8e0b09c2ef196b3f9fd2e0f763236`

Classification:

`historical_local_harness_corroborated_by_public_source`

This does not claim the old local checkout was byte-identical to the public revision.

## Current deob harness contract

`tools/deob-harness/run.sh` requires explicit caller-supplied checkout, expected commit, BC provider JAR, output, and optional work directory.

Safety rules:

- checkout must be exactly at the requested Git commit;
- output inside accepted `reference-fixtures/` is rejected;
- existing candidate output is not silently overwritten;
- ordinary CI does not clone/download/regenerate accepted output;
- no automatic acceptance mode mutates manifests or expected hashes.

Tier A validates this boundary through `scripts/test_reference_regeneration.py`.

## Pinned public melxin source

Canonical public semantic/reference revision:

`melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`

Commit tree:

`3d934e2fef3e90130974f4cbd8f125c6af6a9fb5`

The commit message references RuneLite version commit:

`d175041233e4974fecef6081449585ee7af097e8`

### Exact public file pins

| File | Blob SHA | Semantic/reference use |
|---|---|---|
| `ObjectComposition.java` | `079451cd9a6dcfd2666efd15b0524250eaafe4c4` | model selection/transforms, ambient/contrast decode including opcode 39 scale |
| `ModelData.java` | `2cc9406b2504fbd4fae0c0c952aa2d133809e928` | mirror/winding, transforms, normals, cross-model merge, lighting, alpha sentinels |
| `Model.java` | `c2aa55c0e8fea89fae0da33d782119f8c109cacf` | contouring, software face-priority emission, draw-time alpha |
| `Rasterizer3D.java` | `f32216b5e564c6a03a173438e3b19004c27c1c9e` | exact sine/cosine tables |
| `Scene.java` | `f15260a63103952fe8f5ffbdb62f5c7c39d94565` | scene storage, link-below, ModelData finalization/normal reconciliation |
| `SceneTileModel.java` | `ce6a179cfa93e02271af87164e102ee538223718` | terrain shape/rotation topology |
| `FloorUnderlayDefinition.java` | `f06136263590143afeedf5a8e448cd901f960614` | underlay weighted HSL |
| `FloorOverlayDefinition.java` | `f3a15cc07c53ccae74b2219db88d456b77d88d68` | overlay fields/defaults/HSL |
| `Tiles.java` | `e4655da97d297f3d4fb53e9e7ae9816f1bdd5790` | bridge flags and orientation tables |
| `DynamicObject.java` | `3ff5ba2c1c4fb4cc914326cc70223720d5f2e77d` | dynamic construction context |
| `class470.java` | `1cd9cad5cb4be865dcae94dc633bba821644dc84` | `method9712(WorldView)`: full terrain builder, tile min-plane writes, scene finalization, link-below phase |

The public file identities above are the canonical exact-source anchors. Older whole-snapshot local evidence remains corroborating only unless separately proven equivalent.

## Terrain-builder correction

The previous M5/M9 provenance conclusion said the pinned public `class470` was unrelated and that the terrain builder source was missing. That conclusion was incorrect because inspection stopped before the file's final large static method.

`class470.method9712(WorldView)` at the exact public pin is the terrain builder and directly contains:

- slope lighting;
- `Tiles_underlays2` shadow/clipping subtraction;
- separable radius-5 / 11x11 underlay accumulation;
- weighted HSL construction;
- client hue/lightness jitter;
- overlay sentinel/texture/secondary-color handling;
- `Scene.addTile` calls;
- tile minimum-plane writes;
- scene ModelData finalization;
- `setLinkBelow` calls.

Therefore the **source gate for `TERRAIN-004` is closed**.

Production Rust `TERRAIN-004` remains `REQUIRED` until the builder is implemented and exact executable fixtures pass. The target capability may remain operationally blocked while implementation is absent, but it must not say the oracle is unknown.

## M5 normalized semantic fixtures

The M5 normalized production fixtures remain valid for:

1. typed model selection;
2. mirror geometry/winding;
3. type-4 transform order.

Evidence-only M5 families remain useful for normal generation/merge and face-priority ordering, with later milestones owning production promotion.

Important normalized/historical fixture IDs include:

- `normals.base.smooth_triangle`
- `normals.base.flat_triangle`
- `normals.merge.coincident_triangle.hide_false`
- `normals.merge.coincident_triangle.hide_true`
- `normals.merge.translated_negative`
- `planes.link_below.four_plane_column`
- `priority.all_0_11.threshold_crossing`
- `terrain.shape_gallery.all_13x4`
- `contour.synthetic.flat_slope`
- `lighting.synthetic_triangle.loc_rig`
- `placement.wall_types.orientation_matrix`
- `placement.decor_types.orientation_matrix`
- `placement.floor_type22.storage`
- `placement.game_object.footprint_and_capacity`

## Historical whole-snapshot equivalence gate

Before a specification relies on the old developer-machine melxin tree as exact whole-snapshot provenance, one of these is still required:

1. identify its upstream commit;
2. vendor source hashes for every audited file; or
3. prove the relevant method body equivalent to a pinned public source.

The terrain correction does not remove this general provenance rule. It closes `TERRAIN-004` source provenance specifically because its actual method is now pinned in the public revision.

## OpenRune FileStore

Imported tree:

`55f571db4b23f2d528786e1cdfbcba0061fd201a`

Treat as independent decoder/tooling evidence, not the OSRS semantic oracle by itself.

## rs-cache

Imported tree:

`fae41f98352fc804e5d13d9bd2e836ab1e2635cd`

Treat as candidate Rust cache dependency/reference. Revision lineage and decoder gaps remain explicit concerns.

## Shader reference tree

`reference-shaders/runelite-gpu/` tree SHA:

`d33651c6b87ec5d61c43ab2e1e69d15ed6083877`

This directory has mixed provenance. Individual files must still be classified/pinned rather than treating the whole directory as one semantic source.

## Final rule

A final atomic OSRS specification must terminate in a pinned primary source, executable oracle/fixture with exact provenance, or explicit accepted project decision. It must not terminate only in research prose, a date, a local path, or an unstable obfuscated class name.
