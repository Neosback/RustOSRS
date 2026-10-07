# Source Pins and Provenance

Status: **Checkpoint 3 complete provenance record; unresolved equivalence gates remain explicit**

This file records exact source identities used by semantic audits and, equally important, records when an exact identity is not yet known. A human-readable date such as "Jan 2026" is not an acceptable final source pin.

## Repository baseline

RustOSRS blueprint baseline:

`489b0603bc7c2c9fabd2614d204f809fb017d0cb`

Blueprint branch:

`blueprint/osrs-editor-foundation`

## RuneLite reference import

Imported tree in this repository:

`runelite-master/`

RustOSRS baseline tree SHA:

`5afef996a992bacc73655860681269242e90d6e8`

Research notes identify this import as an October 4, 2026 RuneLite snapshot. Until the upstream RuneLite commit corresponding to the import is recorded, specs must cite the RustOSRS tree/blob SHA in addition to the descriptive date.

### Renderer/API file pins used during Checkpoint 3B

| File | RustOSRS blob SHA | Audit use |
|---|---|---|
| `runelite-master/runelite-api/src/main/java/net/runelite/api/Constants.java` | `407831d491b39afd1230552f7475f86ce9e79be2` | scene/chunk/region sizes and tile flags |
| `runelite-master/runelite-api/src/main/java/net/runelite/api/Perspective.java` | `648a593690b777c1522c7afb16203f547e044b88` | local tile size, angle tables, projection reference |
| `runelite-master/runelite-api/src/main/java/net/runelite/api/Texture.java` | `80a4d1a45a51b28c3d5da9f0ad47a03ccf0a482f` | texture animation inputs |
| `runelite-master/runelite-client/src/main/java/net/runelite/client/plugins/gpu/SceneUploader.java` | `83ac701f1b2ee7a039879941bcc710527dbc54c1` | terrain upload, bridge/roof grouping, static alpha split, terrain UVs |
| `runelite-master/runelite-client/src/main/java/net/runelite/client/plugins/gpu/ModelUploader.java` | `35347963838d1be74deddd59027a73623d7872f3` | dynamic priority ordering, transparency composition, object UV reconstruction |
| `reference-shaders/runelite-gpu/vert.glsl` | `d899cf3180295bd18d30adf901fd7a460e560318` | texture animation and RuneLite clip-space face-bias application |

The imported RuneLite API `Scene.java` and `Tile.java` were also audited from this pinned tree for normal-vs-extended scene, render-level, bridge-link, and roof API contracts. Their authority is the imported tree pin unless/until an upstream commit is recorded.

## Local melxin/deob reference used by the existing harness

`tools/deob-harness/run.sh` currently points at a developer-machine source tree:

`/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java`

That absolute path is historical provenance only. It is not a reproducible pin.

The existing research documents describe this source as a January 2026 melxin/deob snapshot.

### Provisional public upstream match

A public `melxin/runelite` commit audited during Checkpoint 3 is:

`1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`

Commit date: January 28, 2026.

Repository: `melxin/runelite`

Commit tree:

`3d934e2fef3e90130974f4cbd8f125c6af6a9fb5`

The commit message itself references RuneLite version commit:

`d175041233e4974fecef6081449585ee7af097e8`

This public source strongly matches many identifiers and behaviors exercised by the checked-in harness, including:

- `ModelData.method5263`
- `ModelData.method5264`
- `class39.method817`
- `class57.method2086`
- `Tiles.field800/field804/field802/field798/field803/field805`
- `ModelData.method5262` normal merging
- the audited `Scene` ModelData merge/final-lighting methods
- object-definition transform/model-build behavior

However, this is currently a **provisional equivalence pin**, not a claim that the developer-machine tree is byte-identical to that commit.

One warning discovered during the audit: the root research documents refer to the terrain builder as `class470`, while `class470` in the public January 28 commit is unrelated text-layout code. Obfuscated class names can move between revisions. Stable behavior, exported names, method bodies, tables, file/blob hashes, and executable fixtures are stronger anchors than an obfuscated class number.

### Public deob file pins used during Checkpoint 3B

| File | Blob SHA | Audit use |
|---|---|---|
| `runescape-client/src/main/java/Scene.java` | `f15260a63103952fe8f5ffbdb62f5c7c39d94565` | tile storage, bridge relinking, object coordinates, floor-decoration storage |
| `runescape-client/src/main/java/SceneTileModel.java` | `ce6a179cfa93e02271af87164e102ee538223718` | exact terrain shape/rotation topology |
| `runescape-client/src/main/java/FloorUnderlayDefinition.java` | `f06136263590143afeedf5a8e448cd901f960614` | underlay RGB-to-weighted-HSL conversion |
| `runescape-client/src/main/java/FloorOverlayDefinition.java` | `f3a15cc07c53ccae74b2219db88d456b77d88d68` | overlay fields, defaults, decode, primary/secondary HSL |
| `runescape-client/src/main/java/Tiles.java` | `e4655da97d297f3d4fb53e9e7ae9816f1bdd5790` | bridge bit consumer and orientation tables |
| `runescape-client/src/main/java/DynamicObject.java` | `3ff5ba2c1c4fb4cc914326cc70223720d5f2e77d` | decoded loc placement and bridge-adjusted collision-plane selection |

Checkpoint 3A additionally audited the public commit's `ObjectComposition`, `ModelData`, `Model`, `Scene`, `FriendSystem`, and pending-spawn path for model construction, normal merging, transforms, morphs, dynamic objects, contouring, lighting, face priority, and alpha behavior.

### Required closure before final spec promotion

Before a spec relies on the local melxin/deob tree as exact provenance, do one of the following:

1. identify its upstream Git commit and record it here, or
2. vendor a source manifest containing hashes for every audited deob file, or
3. prove the relevant file/method body is byte/semantic equivalent to the public source used by the spec.

Until then, specs may state that a behavior is **VERIFIED for public commit `1ad572d7...`** while separately recording local-harness corroboration. They must not describe the entire local snapshot as exactly pinned.

### Terrain-builder gate

The complete terrain color/build routine described by older research remains unresolved at the exact-source level because the cited obfuscated `class470` does not match the public Jan 28 source.

Therefore claims such as the complete 11x11 underlay-blur loop, slope-lighting builder, random color walk, overlay sentinel path, and related builder-side writes remain `REVISION_SENSITIVE` until the actual source method is pinned or an end-to-end executable fixture proves them.

Terrain topology itself is independently verified through `SceneTileModel` plus `reference-fixtures/deob_golden.txt` and is not blocked by this gate.

## OpenRune FileStore

Imported RustOSRS baseline tree:

`OpenRune-FileStore-main/`

Tree SHA:

`55f571db4b23f2d528786e1cdfbcba0061fd201a`

Treat as independent decoder/tooling implementation evidence. It is not the OSRS oracle by itself.

## rs-cache

Imported RustOSRS baseline tree:

`rs-cache-master/`

Tree SHA:

`fae41f98352fc804e5d13d9bd2e836ab1e2635cd`

Treat as candidate Rust cache dependency/reference. Its revision lineage and known decoder gaps remain explicit compatibility concerns.

## Shader reference tree

RustOSRS path:

`reference-shaders/runelite-gpu/`

Tree SHA:

`d33651c6b87ec5d61c43ab2e1e69d15ed6083877`

The directory is mixed provenance: newer live vertex/fragment material plus older compute/priority references and optional presentation shaders. Individual files must be classified and pinned rather than citing the directory as one semantic source.

## Fixture provenance

`reference-fixtures/deob_golden.txt` is generated by `tools/deob-harness/src/Dumper.java` via `tools/deob-harness/run.sh`.

The fixture is executable evidence only to the extent that its source deob snapshot and harness revision are recorded. New fixtures added during the blueprint must record both.

Current fixture coverage includes exact terrain shape/rotation topology, wall/decor storage, color helpers, contouring, and selected lighting/placement behavior. It does **not** yet close the new normal-merge, morph, transform-order, priority, alpha, and complete terrain-color builder fixture requirements recorded by Checkpoint 3.

## Rule

A final atomic OSRS spec must never use only a date, local filesystem path, root research document, generated API page, or obfuscated class name as its terminal source citation.