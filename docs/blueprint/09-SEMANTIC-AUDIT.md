# OSRS Rendering Semantic Audit

Status: **Checkpoint 3A complete, Checkpoint 3 overall in progress**

This document records source-level findings that replace or constrain claims in the pre-blueprint `RUNELITE_*.md` research. It is an audit record, not yet the final atomic-spec set.

Primary public deob source audited in this phase:

`melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc` (2026-01-28)

See `docs/verification/SOURCE-PINS.md` for the exact provenance caveat: the checked-in deob harness still references an unpinned developer-machine tree described only as "Jan 2026". The public commit strongly matches many audited identifiers/behaviors but has not yet been proven byte-identical to that local tree.

## 1. High-impact correction: cross-model normal merging exists

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED` for public commit `1ad572d7...`

The previous research statement that normals are strictly per-model and that modular object joints must retain independent normals is incorrect for the audited deob.

### `ModelData.method5262`

The deob contains a dedicated cross-model normal merge routine:

```text
method5262(ModelData a, ModelData b, dx, dy, dz, hideMatchedFaces)
```

Its behavior is structurally clear:

1. calculate bounds and base vertex normals on both models
2. compare vertices from `a` against vertices in `b` after the supplied translation
3. when positions coincide and the source normal has non-zero magnitude:
   - lazily create a per-vertex merged-normal array on each model
   - copy the original normal into that merged slot
   - accumulate the other model's `(x,y,z,magnitude)` values into it
4. record matched vertices in generation-mark arrays
5. if at least three vertices matched and `hideMatchedFaces == true`, mark faces whose three vertices all matched with face render type `2` on both models

This is not topological welding. The two `ModelData` objects remain separate. What is shared is the lighting-normal result at coincident vertices.

### `ModelData.toModel`

The lighting conversion checks the per-vertex merged-normal array first and falls back to the original vertex normal only when no merged normal exists. Therefore `method5262` directly changes final baked lighting.

### Required Rust behavior

The canonical scene/model pipeline must represent these mechanisms separately:

- mesh/model identity remains separate
- base vertex-normal generation
- cross-model normal accumulation at translated coincident vertices
- optional matched-face suppression via render type `2`
- final lighting conversion using merged normals when present

A renderer that merely draws two independent already-lit wall models will be semantically wrong for affected objects.

## 2. Which objects are eligible to survive as `ModelData`

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED` for public commit `1ad572d7...`

`ObjectComposition.getEntity(...)` is the critical initial static-scene builder.

When a static object's entity is first built:

- `nonFlatShading == false`: `getEntity` converts the transformed `ModelData` immediately with `toModel(ambient+64, contrast+768, -50, -10, -50)` and caches a lit `Model`.
- `nonFlatShading == true`: it stores the same ambient/contrast values on the `ModelData`, calculates its vertex normals, caches `ModelData`, then returns a `copyModelData()` so scene-local mutations do not damage the shared cached entity.
- contouring is applied after this branch, to either the `Model` or copied `ModelData` form as appropriate.

This explains the scene normal-merge gate: only scene renderables that remain `ModelData` reach the later cross-object merge/final-lighting pass.

The implementation must not flatten this into a single "always light model immediately" cache.

## 3. Scene-level normal merge and final lighting pass

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED` for public commit `1ad572d7...`

`Scene.method5585(lightX, lightY, lightZ)` traverses scene tiles and finalizes renderables that are still `ModelData`.

### Boundary objects

For `renderable1 instanceof ModelData`:

- merge it against neighboring eligible walls/game objects via `method5587`
- if the boundary has a second ModelData arm, merge that arm against neighbors too
- merge the two boundary arms directly with zero translation and `hideMatchedFaces=false`
- convert both arms to `Model` with each ModelData's stored `ambient` and `contrast`

This directly covers the type-2 L-corner dual-arm case when those renderables remain ModelData.

### Game objects

For each game object whose renderable is ModelData:

- call `method5587` using the object's tile footprint width/height
- convert the mutated ModelData to its final lit Model

### Floor decorations

For floor decorations whose renderable is ModelData:

- call `method5586`
- that helper checks neighboring floor decorations and calls the normal merge routine with translated tile offsets and `hideMatchedFaces=true`
- then convert to final Model

### Neighbor scan

`method5587` scans:

- the source plane and the plane above, within bounds
- neighboring boundary-object ModelData slots
- neighboring game-object ModelData
- translation offsets derived from relative tile/footprint positions
- a vertical offset derived from the difference between average tile heights

The first plane pass may request matched-face hiding; the second does not.

This behavior must become a dedicated scene-finalization semantic contract, not an incidental renderer optimization.

## 4. Initial region build and pending-spawn replacement are different pipelines

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED` for public commit `1ad572d7...`

This is another correction to the old research emphasis.

### Initial map/region placement

Decoded locations reach `FriendSystem.addObjects(...)` in the audited public deob.

For a static object (`animationId == -1 && transforms == null`) the initial builder uses:

`ObjectComposition.getEntity(...)`

That distinction matters because `getEntity()` can return `ModelData` for `nonFlatShading` objects, preserving them for scene-level normal merging before final lighting.

### Pending spawn / live replacement

`class150.addPendingSpawnToScene(...)` is a separate remove-and-replace path. For a static replacement it uses `ObjectComposition.getModel(...)`, which returns an already-lit `Model`.

The final spec therefore needs at least two construction paths:

1. **initial scene construction + scene ModelData finalization**
2. **runtime/pending replacement construction**

Treating `class150.addPendingSpawnToScene` as the sole/root scene builder, as older research effectively did, loses the initial normal-merge pipeline.

## 5. Loc-type dispatch rechecked on initial placement

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED` for public commit `1ad572d7...`

The initial builder confirms the broad dispatch table already described by research:

| Loc type | Scene construction |
|---|---|
| 22 | floor decoration |
| 10, 11 | game object using full footprint; type 11 adds orientation/draw flag `256` |
| 0 | single boundary arm, straight orientation table |
| 1 | single boundary arm, diagonal orientation table |
| 2 | two boundary arms: `(type=2, orientation+4)` and `(type=2, (orientation+1)&3)` |
| 3 | single boundary arm using diagonal orientation table |
| 4 | flush wall decoration |
| 5 | wall decoration offset by full existing-wall displacement |
| 6 | diagonal wall decoration offset by half existing-wall displacement |
| 7 | opposite-side diagonal decoration, no displacement |
| 8 | dual decoration, half displacement |
| 9 | 1x1 game-object path |
| >=12 | 1x1 game-object path |

The audited `Tiles` tables are:

```text
straight flags  = {1, 2, 4, 8}
diagonal flags  = {16, 32, 64, 128}
cardinal x       = {1, 0, -1, 0}
cardinal z       = {0, -1, 0, 1}
diagonal x       = {1, -1, -1, 1}
diagonal z       = {-1, -1, 1, 1}
```

Type 5 defaults displacement to 16 when there is no existing boundary object. Types 6 and 8 default to 8 and otherwise use half the boundary object's definition displacement.

## 6. Initial placement writes more than visible scene objects

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED` for public commit `1ad572d7...`

The initial builder also updates semantic side grids according to definition flags:

- collision map
- clipping/shadow bytes (`Tiles_underlays2` in this deob naming)
- model-clipping/occlusion flag grids
- wall displacement metadata
- roof-related flags for loc types in the relevant ranges

These are not all renderer-owned. A map editor that wants correct rebuild/export behavior must preserve enough definition/placement semantics to regenerate them deterministically.

Checkpoint 3B will split visual-only derived fields from collision/scene semantic state instead of copying obfuscated storage structures literally.

## 7. Model selection and transform pipeline

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED` for public commit `1ad572d7...`

`ObjectComposition.getModelData(type, orientation)` confirms:

### Model selection

- if `models == null`, only requested type `10` is accepted; all `modelIds` are loaded/combined
- if `modelIds == null`, return null
- when a typed `models[]` table exists, find the first exact requested type; no match returns null
- there is no generic "fall back to first model" behavior

### Mirror selection

For typed models:

`mirror = isRotated XOR (orientation > 3)`

The mirrored raw-model cache key is separated by adding 65536 before fetching/caching, and the loaded ModelData is mirrored once before entering the shared raw cache.

For the untyped/type-10 multi-model path, the audited code also carries a special inversion when `type == 2 && orientation > 3` before loading the component model list.

### Copy-before-instance-mutation

After raw ModelData is resolved, a constructor creates an instance-working ModelData with sharing/copy choices based on whether rotation, recolor, retexture, resizing, and offsets require mutation.

### Required transform order

1. special type-4 `orientation > 3`: rotate by 256 JAU and translate `(45, 0, -45)`
2. reduce orientation to `orientation & 3`
3. apply 90/180/270-degree orientation rotation
4. recolor all requested pairs
5. retexture all requested pairs
6. resize if any axis differs from 128
7. translate by definition offsets

Changing this order changes world geometry and is a P1 semantic bug.

## 8. Morph resolution

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED` for public commit `1ad572d7...`

Object definition transform/morph resolution is:

1. selector from `transformVarbit` when present
2. otherwise selector from `transformVarp` when present
3. if selector is in `[0, transforms.length-2)`, choose `transforms[selector]`
4. otherwise choose the final transform entry as fallback
5. selected id `-1` means no active object definition

The opcode decode for transforms includes both 77 and 92 semantics, including the opcode-92 fallback id.

The editor preview state therefore needs explicit varbit/varp inputs. Morphs cannot be treated as a one-time import-time model choice.

## 9. Dynamic objects resolve morph and animation at model time

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED` for public commit `1ad572d7...`

`DynamicObject.getModel()`:

- loads the original object definition
- resolves its transform chain on the current varbit/varp state each time
- returns no model when the transform resolves to null
- computes footprint size from the transformed definition, including orientation 1/3 size swap
- resamples average tile height and world center
- advances animation state by client cycles
- requests `getModelDynamic(type, orientation, heights, position, sequence, frame)`

The constructor can preserve animation state from a replaced DynamicObject when animation id/restart rules allow it.

`getModelDynamic` starts from the cached lit static Model, applies a shared/copy sequence transform when needed, then applies contouring to the resulting working model.

Implication: static ModelData normal merging and dynamic animated-model generation are distinct pipelines. Do not try to force every object through one mutable-model representation.

## 10. Contouring

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED` for public commit `1ad572d7...` and already covered by the checked-in deob fixture

`Model.contourGround` uses integer bilinear sampling in 128-unit tile space.

- `clip == 0` fully warps every vertex by `sampledGround + originalY - baseHeight`
- nonzero clip computes a vertical depth ratio from `(-vertexY << 16) / modelHeight` and only blends vertices below the clip threshold
- source model can be copied or mutated according to the call contract
- bounds are reset after mutation
- fast paths return the original when the height range is out of bounds or all relevant corner heights equal the supplied base height

This remains an exact integer contract. It is not a GPU displacement feature.

## 11. Lighting rig and HSL helpers

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED` for public commit `1ad572d7...`; existing harness has exact cases

Static object conversion uses the familiar loc light rig:

```text
ambient  = objectAmbient + 64
contrast = objectContrast + 768
light    = (-50, -10, -50)
```

`ModelData.toModel` computes the light-vector magnitude and derives the per-normal contrast divisor using integer shift semantics.

The public snapshot also matches the harnessed terrain/color helpers:

- `class39.method817`: packed terrain HSL with repeated saturation halving above lightness 179, 192, 217, 243
- `class57.method2086`: brightness-adjust packed HSL, clamp lightness 2..126, `-1 -> 12345678`
- `ModelData.method5263/method5264`: model-lighting HSL/lightness helpers used by `toModel`

These matching identifiers/behaviors increase confidence that the public Jan 28 source is closely related to the local deob harness snapshot, but do not replace the exact-source-pin requirement.

## 12. Face priority and alpha: preliminary verification

**Domain:** `OSRS_SEMANTIC` plus later `RENDERER_POLICY` implementation choice  
**Status:** source behavior verified; final canonical split pending Checkpoint 3B

The software `Model` confirms that face priorities are not just generic sort keys.

When per-face priorities are absent, visible faces are drawn from far depth buckets to near depth buckets.

When priorities exist:

- faces are first depth-bucketed
- priorities 0..11 have distinct queues
- average depth thresholds are computed for priority groups `(1,2)`, `(3,4)`, and `(6,8)`
- priority 10 and 11 faces are interleaved against the ordinary priority pass at the 0, 3, and 5 boundaries according to those depth thresholds
- priorities 0..9 are otherwise emitted in priority order, with faces already stored far-to-near within their priority queue

The October 2026 imported RuneLite `ModelUploader` reproduces the same priority structure (`avg12`, `avg34`, `avg68`, priority 10/11 dynamic queue), which is strong independent corroboration.

Software face alpha behavior also confirms that alpha is semantic face metadata. The rasterizer receives `0` when no face-alpha array exists; a stored signed byte `-1` is treated specially as 253 in the normal face draw path, otherwise the byte is interpreted unsigned.

The Rust renderer does not have to copy RuneLite's buffer implementation, but it must reproduce the reference ordering/alpha contract on crafted fixtures.

## 13. Current audit matrix

| Area | Status after 3A | Next action |
|---|---|---|
| loc type dispatch | verified on initial builder | turn into atomic placement specs later |
| wall/decor offsets | verified | fixture expansion for all build paths |
| model selection | verified | atomic model-build spec |
| mirroring | verified source path | add winding fixture |
| transform order | verified | add differential transform matrix |
| contouring | verified + existing fixture | preserve exact integer cases |
| morph selection | verified | add varbit/varp fixture |
| dynamic model resolution | verified | sequence fixture needed |
| normal generation | verified source | add dedicated fixture |
| cross-model normal merge | verified, old research disproven | add positive/negative/hide-face fixtures |
| scene ModelData finalization | verified source | identify exact initial finalization call in local snapshot |
| model lighting | verified + existing fixture | broaden flat/smooth/texture cases |
| face priority | verified algorithm | craft exact ordering fixture |
| face alpha | verified source | craft alpha/sentinel fixture |
| terrain topology | existing executable evidence, source re-audit not finished | Checkpoint 3B |
| underlay/overlay terrain construction | research/harness partially strong | Checkpoint 3B, repair obfuscated source pin |
| bridges/planes | partially verified | Checkpoint 3B |
| textures/UV | not fully audited | Checkpoint 3B |
| roof semantic construction | not fully audited | Checkpoint 3B |
| camera semantic math | not fully audited | Checkpoint 3B |
| cache revision-sensitive fields | not fully audited | Checkpoint 3B/C |

## 14. Required new verification fixtures

The existing harness must be expanded before final spec promotion with at least:

### Normals

- two coincident ModelData triangles where merge changes merged-normal magnitude/components
- same models translated so no vertices match
- `hideMatchedFaces=false` proves normals merge without face render-type suppression
- `hideMatchedFaces=true` with >=3 matches proves matched faces become render type 2
- scene-level boundary/game-object merge case where practical

### Model transforms

- mirror geometry and winding
- every orientation
- type-4 diagonal recenter
- recolor + retexture + resize + translate in one order-sensitive fixture

### Dynamic/morph

- varbit selection
- varp selection
- fallback transform
- null transform
- footprint change after morph
- one animation frame transform and copy-ownership check

### Face ordering

- priorities 0..11 with crafted depths around all three interleave thresholds
- no-priority depth-only baseline
- face alpha `0`, ordinary nonzero, `255/-1` special handling
- transparent/opaque interaction required by the chosen renderer implementation

## 15. Old research statements superseded by this audit

The following claims must not survive into canonical specs:

1. "Normals are strictly per-model."
2. "The editor must not merge normals."
3. "Separate wall meshes imply authentic hard lighting creases."
4. "`class150.addPendingSpawnToScene` is the sole/root construction path for map objects."
5. Any source pin that identifies the deob only as "Jan 2026" without a commit/file hash.

The more accurate rule is:

> Separate scene/model topology can remain separate while OSRS still reconciles lighting normals across translated coincident vertices for eligible static ModelData during scene finalization.

That distinction is foundational to the Rust scene architecture.

## 16. Checkpoint 3B scope

Checkpoint 3 is not complete yet. The next sub-checkpoint audits:

- terrain build source and the stale `class470` pin
- underlay 11x11 construction and region-border contribution
- overlay rules and tile shape ownership
- bridge/render/collision plane distinctions
- texture/material/UV semantics
- priority/alpha fixture requirements in more detail
- roof and tile-plane construction semantics
- semantic camera/coordinate math versus editor-camera policy
- revision-sensitive decoder fields needed by the renderer

No canonical atomic spec should be written until the relevant 3A/3B audit row is closed or explicitly marked revision-gated.