# Source Pins and Provenance

Status: **Current through M10 foundation audit; unresolved equivalence gates remain explicit**

A date, local path, historical note, generated API page, or obfuscated class name alone is not an acceptable terminal source pin.

## Repository/reference baselines

RustOSRS blueprint baseline:

`489b0603bc7c2c9fabd2614d204f809fb017d0cb`

Pinned public semantic reference:

`melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`

Public commit tree:

`3d934e2fef3e90130974f4cbd8f125c6af6a9fb5`

The commit message references RuneLite version commit:

`d175041233e4974fecef6081449585ee7af097e8`

## Pinned public deob files

| File | Blob SHA | Canonical use |
|---|---|---|
| `ObjectComposition.java` | `079451cd9a6dcfd2666efd15b0524250eaafe4c4` | object model selection/transforms, decoded lighting fields |
| `ModelData.java` | `2cc9406b2504fbd4fae0c0c952aa2d133809e928` | model transforms, normals, cross-model merge, final lighting |
| `Model.java` | `c2aa55c0e8fea89fae0da33d782119f8c109cacf` | contouring and software face-priority emission |
| `Rasterizer3D.java` | `f32216b5e564c6a03a173438e3b19004c27c1c9e` | exact sine/cosine tables and raster helpers |
| `Scene.java` | `f15260a63103952fe8f5ffbdb62f5c7c39d94565` | scene storage, placement, normal finalization, min-plane draw use, link-below |
| `Tile.java` | `74c221f904f8b4748a7733eeade057934f1e0879` | `plane`, `originalPlane`, `minPlane`, linked-below identity |
| `SceneTileModel.java` | `ce6a179cfa93e02271af87164e102ee538223718` | terrain shape/rotation topology |
| `FloorUnderlayDefinition.java` | `f06136263590143afeedf5a8e448cd901f960614` | weighted underlay HSL conversion |
| `FloorOverlayDefinition.java` | `f3a15cc07c53ccae74b2219db88d456b77d88d68` | overlay fields/defaults/HSL |
| `Tiles.java` | `e4655da97d297f3d4fb53e9e7ae9816f1bdd5790` | bridge/orientation tables |
| `DynamicObject.java` | `3ff5ba2c1c4fb4cc914326cc70223720d5f2e77d` | placement and collision-plane behavior |
| `class470.java` | `1cd9cad5cb4be865dcae94dc633bba821644dc84` | complete terrain builder `method9712(WorldView)` |
| `FriendSystem.java` | `b8cf51b6ee673181d8a115e28153a77f5978c390` | initial `addObjects(...)` placement and shadow-grid writes |

## Terrain-builder correction

The earlier M5/M9 provenance record incorrectly concluded that the pinned public `class470` was unrelated to terrain construction. That inspection stopped at the class's dominant text-layout responsibilities and missed its final static method.

At the exact public pin above, `class470.method9712(WorldView)` contains the complete target terrain construction path, including:

- random hue/lightness presentation walk;
- slope-light calculation;
- consumption of `Tiles.Tiles_underlays2` shadow state;
- separable radius-5/11x11 underlay blend;
- weighted hue/saturation/lightness accumulation;
- overlay texture/magenta/secondary branches;
- `Scene.addTile` calls;
- per-tile `setTileMinPlane`;
- scene normal finalization;
- `setLinkBelow` application.

`FriendSystem.addObjects(...)` independently pins the placement-derived shadow input:

- clipped wall cases write orientation-specific value `50` cells;
- clipped game objects write a model-height-derived value, default `15`, model height / 4 when available, capped at `30`;
- writes preserve the greater prior value.

Therefore `TERRAIN-004` is **source verified, not source blocked**. Production Rust implementation and an executable differential fixture remain required before claiming implementation parity.

## Imported RuneLite renderer tree

Imported tree:

`runelite-master/`

RustOSRS tree SHA:

`5afef996a992bacc73655860681269242e90d6e8`

Research identifies it as an October 4, 2026 snapshot. Until its exact upstream RuneLite commit is recorded, renderer citations use this RustOSRS tree/blob identity.

Selected pins:

| File | RustOSRS blob SHA | Audit use |
|---|---|---|
| `Constants.java` | `407831d491b39afd1230552f7475f86ce9e79be2` | scene/chunk/region sizes and tile flags |
| `Perspective.java` | `648a593690b777c1522c7afb16203f547e044b88` | local units/projection reference |
| `Texture.java` | `80a4d1a45a51b28c3d5da9f0ad47a03ccf0a482f` | animation inputs |
| `Renderable.java` | `bce439b4dd2000a98d83191f3d0348b64ff83b3c` | GPU render-mode constants |
| `SceneUploader.java` | `83ac701f1b2ee7a039879941bcc710527dbc54c1` | terrain upload and roof/maplevel grouping |
| `ModelUploader.java` | `35347963838d1be74deddd59027a73623d7872f3` | path-specific priority/depth sorting and object UVs |
| `GpuPlugin.java` | `8c233cb381e288146af47ac375fa3dfc41a53d76` | reverse-Z state and render-mode dispatch |
| `Zone.java` | `80594f4ca2759a96c9a3a9800008790b91c5d692` | static alpha model/face ordering |
| `TextureManager.java` | `e830a518dc13c173f22f006fe52cb25e3c0fc8b3` | texture pixels, mip/filter/wrap setup |

## Shader reference tree

`reference-shaders/runelite-gpu/` is mixed-provenance renderer evidence. Individual files remain pinned:

- `vert.glsl` -> `d899cf3180295bd18d30adf901fd7a460e560318`
- `frag.glsl` -> `0ca7180d50ef90e5083c85f7182e60521ef76baa`

The shader tree is not promoted wholesale as semantic truth.

## Renderer findings fixed by the M10 foundation audit

Imported RuneLite evidence establishes:

- reverse-Z clear `0` and strict `GL_GREATER`;
- no universal `GreaterEqual` reference rule;
- software-style priority queues only when the imported GPU caller enables `prioritySort`, notably `SORTED_NO_DEPTH`;
- ordinary dynamic/static paths do not universally reproduce `Model.method5946`;
- static alpha work in `Zone` uses distance/depth ordering;
- texture RGB zero becomes transparent;
- staged fragment alpha-cutout uses level-0 alpha;
- magnification is nearest;
- minification is nearest at filtering level 0 and `NEAREST_MIPMAP_LINEAR` at level >= 1;
- imported config defaults filtering level to 1;
- S wraps clamp-to-edge while T remains default repeat in the audited setup.

These are renderer-reference facts, not OSRS cache semantics.

## Historical local melxin/deob evidence

The original `reference-fixtures/deob_golden.txt` came from a developer-machine checkout historically described by an absolute path. That path is provenance only, not identity.

Checked-in historical identities remain byte-gated:

- `reference-fixtures/deob_golden.txt` -> Git blob `49887733ad463572cf61bc059733b7c5f5fd26f4`;
- `tools/deob-harness/src/Dumper.java` -> Git blob `ceefbd6e97c8e0b09c2ef196b3f9fd2e0f763236`.

Classification:

`historical_local_harness_corroborated_by_public_source`

The old whole checkout is **not** claimed byte-identical to the public pin.

## Current deob harness contract

`tools/deob-harness/run.sh` requires explicit caller-supplied checkout, expected commit, bcprov jar, output, and optional work directory.

Safety rules remain:

- checkout must be exactly at the requested Git commit;
- accepted fixture directories cannot be direct regeneration output;
- accepted output is never silently overwritten;
- ordinary CI does not clone/download/regenerate accepted fixtures;
- promotion remains an explicit review step.

## Normalized fixture provenance

M5 production semantic fixtures remain source-pinned for:

- typed model selection;
- mirror geometry/winding;
- type-4 transform order.

Evidence fixtures remain pinned for base normals, translated/cross-model normal merging, four-plane link-below, and face-priority thresholds. Later milestones promote those behaviors only when production Rust tests execute the owning contract.

The historical evidence index continues to expose:

- terrain shape gallery;
- contour flat/slope;
- synthetic lighting;
- wall/decor orientation matrices;
- floor-decoration storage;
- game-object footprint/capacity.

## Historical whole-snapshot equivalence gate

Before any claim relies on the complete old developer-machine melxin tree as exact provenance, one of these remains required:

1. identify its upstream commit;
2. vendor hashes for every audited source file; or
3. prove the relevant method bodies equivalent to pinned public source.

Current promoted semantic contracts do not require whole-snapshot equivalence where exact public file/method pins now exist.

## Independent decoder/tooling imports

OpenRune FileStore tree:

`55f571db4b23f2d528786e1cdfbcba0061fd201a`

rs-cache tree:

`fae41f98352fc804e5d13d9bd2e836ab1e2635cd`

Both remain corroborating decoder/tooling evidence unless separately promoted. They are not the terminal OSRS semantic oracle by themselves.

## External visual oracle status

No externally captured, provenance-complete visual golden is currently promoted as the V3 oracle for RustOSRS.

RustOSRS-generated Reference screenshots are valid regression evidence but cannot alone prove external 1:1 visual parity. Before that claim is made, the project must add at least one independently generated RuneLite/client render artifact with exact scene, camera, renderer configuration, and source identity.

## Final rule

Every atomic normative claim must terminate in one of:

- pinned primary source;
- executable fixture/oracle with exact provenance;
- explicit project decision.

Historical prose, dates, local paths, self-generated screenshots, and unstable obfuscated names alone are not sufficient.
