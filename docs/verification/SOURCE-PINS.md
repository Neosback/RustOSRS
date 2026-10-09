# Source Pins and Provenance

Status: **M5 closure provenance record; unresolved equivalence gates remain explicit**

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

## Historical local melxin/deob reference and current harness contract

The original `deob_golden.txt` was produced from a developer-machine melxin/deob checkout historically described by the path:

`/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java`

That absolute path is historical provenance only. It is not a reproducible source pin, and the current harness no longer depends on it.

As of M5, `tools/deob-harness/run.sh` requires all source/dependency/output locations explicitly from the caller:

- `--checkout PATH`;
- `--expected-commit SHA`;
- `--bcprov JAR`;
- `--output FILE`;
- optional `--work-dir DIR`.

The checkout must be a Git worktree exactly at the requested commit. Candidate output inside `reference-fixtures/` is rejected, existing candidate output is not overwritten, and ordinary CI does not clone/download or accept regenerated expected output.

The checked-in historical artifacts are independently identity-gated by recomputing their Git blob IDs from bytes:

- `reference-fixtures/deob_golden.txt` -> `49887733ad463572cf61bc059733b7c5f5fd26f4`;
- `tools/deob-harness/src/Dumper.java` -> `ceefbd6e97c8e0b09c2ef196b3f9fd2e0f763236`.

The historical evidence classification is:

`historical_local_harness_corroborated_by_public_source`

This classification deliberately does not claim the original local checkout was byte-identical to the public source below.

### Pinned public upstream source

The public `melxin/runelite` revision used by M5 normalized fixtures and public corroboration is:

`1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`

Commit date: January 28, 2026.

Repository: `melxin/runelite`

Commit tree:

`3d934e2fef3e90130974f4cbd8f125c6af6a9fb5`

The commit message itself references RuneLite version commit:

`d175041233e4974fecef6081449585ee7af097e8`

This public source strongly matches many identifiers and behaviors exercised by the checked-in historical harness, including:

- `ModelData.method5263`;
- `ModelData.method5264`;
- `class39.method817`;
- `class57.method2086`;
- `Tiles.field800/field804/field802/field798/field803/field805`;
- `ModelData.method5262` normal merging;
- the audited `Scene` ModelData merge/final-lighting methods;
- object-definition transform/model-build behavior.

It remains wrong to infer whole-tree equality between the historical developer-machine checkout and this public revision. M5 instead pins the exact public files used by each normalized fixture or historical-evidence family.

One warning discovered during the earlier audit remains important: root research documents refer to the terrain builder as `class470`, while `class470` in the public January 28 commit is unrelated text-layout code. Obfuscated class names can move between revisions. Stable behavior, exported names, method bodies, tables, file/blob hashes, and executable fixtures are stronger anchors than an obfuscated class number.

### Public deob file pins used by M3-M5

| File | Blob SHA | Audit / fixture use |
|---|---|---|
| `runescape-client/src/main/java/ObjectComposition.java` | `079451cd9a6dcfd2666efd15b0524250eaafe4c4` | typed/untyped model selection and object model transform ordering; M5 `model.selection.typed_exact.orientation_4` and `model.transform.type4_order` |
| `runescape-client/src/main/java/ModelData.java` | `2cc9406b2504fbd4fae0c0c952aa2d133809e928` | mirror/winding, model transforms, normal merging, lighting; M5 `model.mirror.geometry_winding`, `model.transform.type4_order`, and historical lighting corroboration |
| `runescape-client/src/main/java/Model.java` | `c2aa55c0e8fea89fae0da33d782119f8c109cacf` | contouring and reference face-priority emission; M5 `priority.all_0_11.threshold_crossing` plus historical contour corroboration |
| `runescape-client/src/main/java/Rasterizer3D.java` | `f32216b5e564c6a03a173438e3b19004c27c1c9e` | exact sine/cosine tables used by the M5 type-4 transform fixture |
| `runescape-client/src/main/java/Scene.java` | `f15260a63103952fe8f5ffbdb62f5c7c39d94565` | tile storage, bridge relinking, object coordinates, floor-decoration storage; M5 historical placement corroboration |
| `runescape-client/src/main/java/SceneTileModel.java` | `ce6a179cfa93e02271af87164e102ee538223718` | exact terrain shape/rotation topology; M5 historical terrain-gallery corroboration |
| `runescape-client/src/main/java/FloorUnderlayDefinition.java` | `f06136263590143afeedf5a8e448cd901f960614` | underlay RGB-to-weighted-HSL conversion |
| `runescape-client/src/main/java/FloorOverlayDefinition.java` | `f3a15cc07c53ccae74b2219db88d456b77d88d68` | overlay fields, defaults, decode, primary/secondary HSL |
| `runescape-client/src/main/java/Tiles.java` | `e4655da97d297f3d4fb53e9e7ae9816f1bdd5790` | bridge bit consumer and orientation tables |
| `runescape-client/src/main/java/DynamicObject.java` | `3ff5ba2c1c4fb4cc914326cc70223720d5f2e77d` | decoded loc placement and bridge-adjusted collision-plane selection |

M5 independently revalidated the exact public blob IDs used by its normalized fixtures and historical corroboration against the pinned upstream revision.

### Required closure before historical-local equivalence claims

Before a spec relies on the old local melxin/deob tree as exact whole-snapshot provenance, do one of the following:

1. identify its upstream Git commit and record it here, or
2. vendor a source manifest containing hashes for every audited deob file, or
3. prove the relevant file/method body is byte/semantic equivalent to the public source used by the spec.

Until then, specs may state that a behavior is **VERIFIED for public commit `1ad572d7...`** while separately recording historical local-harness corroboration. They must not describe the entire old local snapshot as exactly pinned.

### Terrain-builder gate

The complete terrain color/build routine described by older research remains unresolved at the exact-source level because the cited obfuscated `class470` does not match the public Jan 28 source.

Therefore claims such as the complete 11x11 underlay-blur loop, slope-lighting builder, random color walk, overlay sentinel path, and related builder-side writes remain `REVISION_SENSITIVE` until the actual source method is pinned or an end-to-end executable fixture proves them.

Terrain topology itself is independently verified through `SceneTileModel` plus the indexed historical `reference-fixtures/deob_golden.txt` evidence and is not blocked by this gate.

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

## Fixture provenance after M5

M5 separates three evidence classes rather than treating all fixtures as equivalent.

### Normalized semantic fixtures

The checked-in YAML manifest inventory currently includes three normalized semantic fixtures that execute against production semantics in the offline M5 runner:

- `model.selection.typed_exact.orientation_4`;
- `model.mirror.geometry_winding`;
- `model.transform.type4_order`.

Each manifest records exact public source repository/commit/file/blob/symbol provenance, an exact harness/migration revision, normalized input/output paths, and the expected-output SHA-256.

### Evidence-only normalized fixture

`priority.all_0_11.threshold_crossing` is source-pinned to `Model.method5946` and checks priorities `0..11`, three threshold bands, priority-10/11 queue behavior, and signed alpha metadata. It is deliberately `execution: evidence_only` because the renderer-owned production priority executor does not yet exist.

This fixture strengthens the future `FACE-002` contract without promoting `FACE-002` to `EXISTING` early.

### Indexed historical evidence

`reference-fixtures/historical/deob_golden.index.json` gives stable IDs to seven useful historical families while preserving the distinction between historical local harness output and pinned public corroboration:

- `terrain.shape_gallery.all_13x4`;
- `contour.synthetic.flat_slope`;
- `lighting.synthetic_triangle.loc_rig`;
- `placement.wall_types.orientation_matrix`;
- `placement.decor_types.orientation_matrix`;
- `placement.floor_type22.storage`;
- `placement.game_object.footprint_and_capacity`.

`scripts/test_deob_golden_index.py` verifies the checked-in historical fixture/harness Git blob identities, classification, IDs, public pin shape, exact-line uniqueness, and expected prefix counts entirely offline.

### Regeneration boundary

`tools/reference-fixtures/regenerate.py` and `tools/deob-harness/run.sh` produce candidate artifacts only. They cannot write accepted output into `reference-fixtures/`, cannot overwrite an existing candidate silently, and expose no automatic acceptance path. Ordinary CI validates checked-in fixtures but does not regenerate and accept them.

## Rule

A final atomic OSRS spec must never use only a date, local filesystem path, root research document, generated API page, or obfuscated class name as its terminal source citation.
