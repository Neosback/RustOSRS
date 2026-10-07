# Terrain, Material, Plane, and Renderer Semantic Audit

Status: **Checkpoint 3B complete**  
Checkpoint 3 rendering semantic audit: **complete with explicit revision gates**

This document is the second half of the source-level semantic audit started in `09-SEMANTIC-AUDIT.md`. It focuses on terrain topology, terrain color inputs, bridge/plane behavior, roofs, texture coordinates, transparency/priority behavior, camera/coordinate math, and the decoder fields that must survive into the Rust semantic model.

A completed audit does **not** mean every old research claim was promoted. In several places the correct audit result is `REVISION_SENSITIVE`: the existing prose is plausible, but its claimed deob source pin is not reproducible enough to become a canonical specification.

## 1. Source anchors used in 3B

### Public deob anchor

Repository:

`melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`

Relevant pinned blobs:

| File | Blob SHA | Use |
|---|---|---|
| `runescape-client/src/main/java/Scene.java` | `f15260a63103952fe8f5ffbdb62f5c7c39d94565` | tile storage, bridge relinking, scene object coordinates |
| `runescape-client/src/main/java/SceneTileModel.java` | `ce6a179cfa93e02271af87164e102ee538223718` | terrain shape/rotation topology |
| `runescape-client/src/main/java/FloorUnderlayDefinition.java` | `f06136263590143afeedf5a8e448cd901f960614` | underlay decode and RGB to weighted HSL |
| `runescape-client/src/main/java/FloorOverlayDefinition.java` | `f3a15cc07c53ccae74b2219db88d456b77d88d68` | overlay decode, HSL, texture/hide-underlay fields |
| `runescape-client/src/main/java/Tiles.java` | `e4655da97d297f3d4fb53e9e7ae9816f1bdd5790` | bridge bit consumers and orientation tables |
| `runescape-client/src/main/java/DynamicObject.java` | `3ff5ba2c1c4fb4cc914326cc70223720d5f2e77d` | decoded loc placement and collision-plane selection |

The broader object/model files audited in 3A remain part of the same public commit.

### Imported RuneLite renderer anchor

The imported `runelite-master/` tree is independently pinned in `docs/verification/SOURCE-PINS.md`. Relevant per-file blobs in the RustOSRS branch are:

| File | Blob SHA |
|---|---|
| `runelite-api/.../Constants.java` | `407831d491b39afd1230552f7475f86ce9e79be2` |
| `runelite-api/.../Perspective.java` | `648a593690b777c1522c7afb16203f547e044b88` |
| `runelite-api/.../Scene.java` | source tree pin, see `SOURCE-PINS.md` |
| `runelite-api/.../Tile.java` | source tree pin, see `SOURCE-PINS.md` |
| `runelite-api/.../Texture.java` | `80a4d1a45a51b28c3d5da9f0ad47a03ccf0a482f` |
| `runelite-client/.../gpu/SceneUploader.java` | `83ac701f1b2ee7a039879941bcc710527dbc54c1` |
| `runelite-client/.../gpu/ModelUploader.java` | `35347963838d1be74deddd59027a73623d7872f3` |
| `reference-shaders/runelite-gpu/vert.glsl` | `d899cf3180295bd18d30adf901fd7a460e560318` |

RuneLite GPU behavior is renderer evidence, not automatically OSRS cache/scene semantics.

## 2. Terrain topology is verified independently of the stale terrain-builder class name

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED`

The stale `class470` terrain-builder reference does not invalidate the terrain topology evidence.

`SceneTileModel` itself contains the 13 terrain shape templates (`0..12`), their face templates, rotation remapping, midpoint/quarter-point vertex construction, per-face underlay/overlay color selection, and optional triangle texture assignment.

The checked-in deob fixture independently executes every shape for all four rotations and records exact:

- vertex counts
- vertex X/Y/Z
- face indices
- per-corner/per-face colors
- texture ids
- flatness

This is enough to make tile **topology** a canonical exact contract even while the higher-level terrain-color builder remains revision-gated.

### Flat `SceneTilePaint`

The imported RuneLite uploader confirms the flat-paint split as two triangles with four terrain corner heights and fixed full-tile UVs.

The emitted triangles are equivalent to:

1. NE, NW, SE
2. SW, SE, NW

The tile is omitted when the paint's NE color is the sentinel `12345678`.

### Shaped terrain

RuneLite's static uploader consumes the `SceneTileModel`'s already-generated vertices/faces and skips any face whose first color is `12345678`. Terrain UVs are derived from the vertex's X/Z position within the tile, mapping 128 local units to 256 packed UV units.

The Rust implementation must preserve the OSRS topology first. Whether those vertices are uploaded as one static buffer, a meshlet, or another wgpu structure is renderer policy.

## 3. Terrain color construction: verified inputs, revision-gated builder

**Domain:** `OSRS_SEMANTIC`  
**Status:** `REVISION_SENSITIVE` for the complete builder

### Verified underlay definition behavior

`FloorUnderlayDefinition` verifies:

- opcode 1 supplies RGB
- post-decode converts RGB to HSL
- saturation and lightness are clamped to `0..255`
- `hueMultiplier` depends on saturation and whether lightness lies above or below 0.5
- `hueMultiplier` is clamped to at least 1
- stored `hue` is scaled by that multiplier rather than simply stored as an independent 0..255 hue

That weighted-hue representation is semantically important for any neighborhood blending algorithm.

### Verified overlay definition behavior

`FloorOverlayDefinition` verifies:

- primary RGB default `0`
- texture default `-1`
- `hideUnderlay` default `true`
- secondary RGB default `-1`
- opcode 1: primary RGB
- opcode 2: texture id
- opcode 5: `hideUnderlay = false`
- opcode 7: secondary RGB
- post-decode calculates secondary HSL first when present, then restores/calculates primary HSL

### What is **not** yet source-pinned strongly enough

The root research attributes the complete terrain-build loop to `class470` and claims exact behavior for:

- corner slope lighting
- the separable radius-5 / 11x11 underlay blur
- hue weighting and neighborhood count rules
- random hue/lightness walk
- overlay magenta sentinel handling
- `hideUnderlay` interaction
- texture average-RGB fallback
- secondary-color override
- some terrain occlusion/minimap flag writes

The public Jan 28 `class470` is unrelated text-layout code. Therefore the old class/line citations are invalid for this public pin.

Some pieces are strongly corroborated by:

- the HSL helper functions already matched in 3A
- the underlay/overlay definition structures above
- generated terrain outputs in the current fixture
- downstream RuneLite handling of `12345678`

But the **complete 11x11/color-build algorithm remains `REVISION_SENSITIVE` until its actual source method is pinned or an executable terrain-color fixture proves it end-to-end.**

The old prose may remain research input, but it must not be copied verbatim into `specs/terrain-colors.md` as `VERIFIED`.

## 4. Random terrain color variation is not editor semantic state

**Domain:** mixed OSRS behavior / `EDITOR_POLICY`  
**Status:** source behavior requires final terrain-builder pin; deterministic editor policy is clear

The research describes client-side random hue/lightness variation during scene construction. Even if that exact behavior is reverified, it must be represented as an input to terrain color construction, not silently embedded in canonical map data.

For the editor:

- decoded terrain definitions remain deterministic
- semantic terrain state does not mutate because the editor launched again
- parity rendering may expose an OSRS-jitter profile when exact behavior is required
- normal editing/golden tests use fixed deterministic jitter inputs, normally zero unless a fixture states otherwise

This policy prevents screenshot/golden tests from changing across runs.

## 5. Bridge behavior is a set of distinct plane operations, not one universal rule

**Domain:** `OSRS_SEMANTIC` plus RuneLite renderer policy  
**Status:** `VERIFIED` for the mechanisms below

The old research phrase "bridge-adjusted plane" is too broad. At least five separate concepts exist and must be modeled separately.

### 5.1 Decoded loc placement plane

The audited decoded-loc loader reads the location's encoded plane and passes that original plane to the initial object builder.

### 5.2 Collision plane

Before calling the initial builder, the loader checks tile settings plane 1 for bridge bit `2`.

When set:

```text
collisionPlane = encodedPlane - 1
```

when that result remains valid.

The render/scene placement call still receives the original encoded plane. Therefore bridge collision adjustment is **not** evidence that every height/render lookup should also use `plane - 1` or `plane + 1`.

### 5.3 Scene tile relinking

`Scene.setLinkBelow(x, y)` physically shifts tile references:

- old plane 1 becomes scene plane 0
- old plane 2 becomes scene plane 1
- old plane 3 becomes scene plane 2
- moved tile `plane` values are decremented
- qualifying game objects rooted on that tile have their stored plane decremented
- the previous plane-0 tile becomes `linkedBelowTile`
- top plane slot is cleared

This is structural scene state, not merely a shader visibility bit.

### 5.4 Tile render level

The imported RuneLite `Tile` API explicitly distinguishes `getPlane()` from `getRenderLevel()`, describing render level as the plane from which tile heights are taken. It also exposes the linked bridge tile separately.

The Rust scene model therefore needs separate concepts for:

- semantic/source plane
- scene/storage plane after bridge relinking
- render/height level
- collision level
- linked-below tile

Do not compress those into one integer plus ad-hoc `+1` calls.

### 5.5 RuneLite roof/VIS_BELOW map level

The imported RuneLite `SceneUploader` checks bridge bit `2` and increments a local `maplevel` before evaluating `VIS_BELOW` and roof IDs. This is a RuneLite upload/grouping rule. It must not be generalized into the OSRS object-placement contract.

### Pending-spawn warning

The live/pending-spawn builder has its own bridge-aware sampling behavior. That path must remain a separate spec from initial decoded region placement, as established in 3A.

## 6. Ground-decoration lift claim is superseded

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED` correction for public commit `1ad572d7...`

The old `RUNELITE_SCENE_AND_MATERIALS.md` says ground objects/floor decorations are lifted by roughly +1/+2 height units to avoid Z-fighting.

That claim is not present in the audited source path:

- `Scene.newFloorDecoration(...)` stores `z = suppliedHeight` unchanged
- initial floor-decoration placement supplies the computed tile/footprint height
- RuneLite's `SceneUploader` uploads a ground object using `groundObject.getZ()` directly

No generic +1/+2 lift exists in these audited paths.

If a particular asset visually sits above terrain, that must come from its model geometry, its source definition/transforms, or another explicitly proven path. The final spec must not invent a global ground-decoration lift.

## 7. Roof semantics must be split from RuneLite roof-removal infrastructure

**Domain:** mixed  
**Status:** `VERIFIED` separation; complete OSRS roof/occluder construction remains `REVISION_SENSITIVE`

### OSRS semantic side

Initial loc placement confirms that loc types `>=12` use the game-object scene path, and relevant type ranges can write occlusion/model-clipping state. `Scene` also owns explicit occluder structures independently of RuneLite's later roof-removal feature.

Those facts belong to OSRS scene semantics.

### RuneLite renderer/product side

The imported RuneLite API adds/exposes:

- `Scene.buildRoofs()`
- `Scene.getRoofs()`
- `setRoofRemovalMode()`
- roof-removal mode flags such as position, hovered tile, destination, and camera-between-player

`SceneUploader` groups zone-buffer ranges by those roof IDs so a renderer can omit ranges efficiently.

Those roof IDs and removal controls are **not cache semantics** and must not be embedded in `osrs-core` as if Jagex map data directly contains them.

RustOSRS may implement comparable roof grouping/removal because it is excellent editor functionality, but ownership belongs to derived scene/render/editor state unless later source evidence proves a lower-level semantic requirement.

## 8. Texture and UV semantics

### 8.1 Terrain UVs

**Status:** `VERIFIED` for imported RuneLite uploader behavior

Flat tile paint uses canonical full-tile UV corners:

```text
SW (0,0)
SE (1,0)
NE (1,1)
NW (0,1)
```

packed as `0..256` values.

Shaped tile models derive UV from each terrain vertex's X/Z offset inside the 128-unit tile. This keeps overlay/underlay texture orientation consistent across tile cuts.

This is strong renderer-reference behavior and should become a terrain fixture.

### 8.2 Object-model UV reconstruction

**Status:** `VERIFIED` for RuneLite renderer reference

`ModelUploader.computeFaceUvs` uses:

- face vertex indices
- optional model `textureFaces`
- texture-triangle indices (`texIndices1/2/3`)
- a tangent/bitangent basis from the texture triangle

When an explicit texture face exists, U/V are reconstructed by projecting face vertices into that texture-triangle basis. The dynamic/sorted path can first project the face vertices from the camera ray onto the texture plane to match client perspective mapping.

When no texture face is supplied, the calculation reduces to canonical triangle UVs:

```text
A = (0,0)
B = (1,0)
C = (0,1)
```

The final Rust renderer does not need Java's temporary arrays, but it does need equivalent input semantics and differential fixtures.

### 8.3 HSL override rule

RuneLite's model uploader deliberately does **not** apply model HSL override tinting to textured faces. That is a renderer-reference behavior to preserve when implementing a RuneLite-parity profile.

### 8.4 Texture animation

The imported `Texture` API exposes animation direction and speed. The staged live vertex shader applies animation as a tick-scaled UV offset using a `1/128` texture-animation unit.

The semantic boundary is:

- animation direction/speed are decoded texture/material data
- converting them to a 2D animation vector is material/render preparation
- tick-driven UV offset is renderer behavior

Do not hard-code RuneLite's texture-array capacity as a cache invariant.

## 9. Face priority, transparency, and authored depth bias

**Domain:** OSRS face metadata plus renderer implementation  
**Status:** priority reference algorithm `VERIFIED`; Rust strategy remains later `RENDERER_POLICY`

### 9.1 Dynamic/sorted RuneLite path

The imported October renderer independently reproduces the software-client priority algorithm audited in 3A:

- depth buckets first
- priority queues 0..11
- averages for `(1,2)`, `(3,4)`, `(6,8)`
- priority 10/11 interleaving at priority boundaries 0, 3, and 5

This is strong corroboration that priority values are semantically meaningful and cannot be replaced by a simple `sort(priority, depth)` tuple.

### 9.2 Static zone path is different

RuneLite's static scene uploader does **not** run the same priority sorter while baking a zone. It walks stored model faces, skips `color3 == -2`, flattens `color3 == -1`, computes UVs, and separates faces into opaque vs alpha buffers.

Therefore RuneLite itself has path-specific rendering machinery. RustOSRS must not infer that "RuneLite static upload ignores priority" means face priority is irrelevant. The semantic model must preserve priority metadata; the chosen wgpu strategy must be tested against reference scenes.

### 9.3 Transparency

The dynamic sorted uploader combines model-level transparency with per-face transparency via `faceTransparency(...)`. Static scene upload keys its alpha-buffer split from per-face transparency in that path.

A transparent face is therefore more than a material flag; ordering and path context matter.

### 9.4 Authored face bias

RuneLite preserves each face's `faceBias` byte in packed vertex data. The live vertex shader extracts that byte and offsets clip-space Z by `bias / 128`.

The **existence/value of face bias** is semantic model metadata. The exact clip-space application is RuneLite renderer policy and will require a wgpu ADR in Checkpoint 5.

## 10. Coordinate and camera semantics

**Domain:** mixed OSRS/reference math and editor policy  
**Status:** foundational constants `VERIFIED`

The imported `Constants`/`Perspective` source establishes stable reference conventions:

- tile size: 128 local units
- half tile: 64
- chunk: 8 tiles
- region: 64 tiles
- normal scene: 104 tiles
- imported RuneLite extended scene: 184 tiles
- world planes: 0..3
- classic model/orientation sine/cosine table: 2048 units, scaled by 65536
- camera helper also exposes 16384-unit (`0x4000`) sine/cosine tables
- CPU projection rejects points nearer than depth 50

The final architecture must distinguish:

### Semantic/reference math

- 128-unit tiles
- JAU/model orientation transforms
- world/local/scene coordinate conversion
- height interpolation
- source camera/projection math needed by a parity/reference profile

### Renderer policy

- wgpu clip-space mapping
- reverse-Z decision
- near/far representation
- floating-point upload representation

### Editor policy

- orbit/pan/fly controls
- default pitch/yaw/distance
- smoothing
- focus behavior
- gizmo feel

An editor camera can be better than the OSRS camera without changing OSRS object placement semantics.

## 11. Extended scene is a RuneLite runtime capability, not a map-file dimension

**Status:** `VERIFIED` ownership correction

RuneLite exposes a normal `4x104x104` scene and a larger 184x184 extended scene. The GPU uploader operates on extended tiles and 8x8 zones.

That does **not** mean the cache stores maps as 184x184 scenes. The canonical Rust model should represent regions/chunks/world coordinates independently and allow a scene window to materialize whatever margin the renderer/editor needs.

Therefore:

- `104` and `184` are useful compatibility/reference dimensions
- `osrs-core` must not make 184 the universal world/map size
- region-border underlay verification should use enough neighbor context to reproduce the target builder, rather than depending on a hard-coded RuneLite scene container

## 12. Decoder contract required by the renderer

**Domain:** `OSRS_SEMANTIC`  
**Status:** field ownership identified; opcode widths/defaults remain revision-sensitive where noted

The renderer/scene pipeline cannot be correct unless `osrs-cache` preserves at least the following semantic inputs.

### Object definitions

- model ids
- optional model-type table
- size X/Y
- interaction/collision flags needed to rebuild side state
- `clipped`
- `modelClipped`
- `nonFlatShading`
- `isRotated`
- ambient
- contrast
- decor displacement
- animation id
- contour/clip type
- recolor pairs
- retexture pairs
- model resize X/height/Y
- model offsets X/height/Y
- transform varbit
- transform varp
- transform target list including fallback/null
- relevant booleans/defaults used by placement and animation

### Model data

- integer vertices and winding
- face indices
- face colors
- face render types
- per-face priority or model default priority
- face alpha/transparency
- face textures
- texture-face mapping and texture triangle indices
- vertex/face skin data needed by animation
- authored face bias when present in the target revision

### Terrain definitions/state

- underlay ids and RGB-derived HSL components, including hue multiplier
- overlay ids
- overlay primary/secondary RGB/HSL
- texture id
- hide-underlay flag
- terrain shape
- terrain rotation
- tile settings/flags
- tile heights

### Texture/material definitions

- texture identity
- source pixels/sprites as required by the selected decoder path
- average/default color where target behavior uses it
- animation direction
- animation speed

### Revision rule

Do not encode a 16-bit model-id or fixed texture-count assumption into the semantic type system merely because an older/public source masks an id with `65535` or a particular RuneLite renderer allocates a fixed texture array. Width/capacity must follow the target cache revision.

## 13. Claims superseded or downgraded by 3B

The following old claims must not enter canonical specs unchanged:

1. **`class470` is the pinned terrain builder.** False as a source pin for public Jan 28; class identity is stale/mismatched.
2. **Ground decorations receive a generic +1/+2 lift.** Not present in the audited placement/storage/upload path.
3. **Bridge bit means one universal adjusted render/height plane.** Overbroad; collision, storage, render level, linked-below state, pending-spawn sampling, and RuneLite roof/VIS_BELOW grouping are separate operations.
4. **RuneLite roof IDs/removal mode are OSRS map semantics.** They are derived/runtime renderer-product infrastructure unless separately proven otherwise.
5. **Extended 184x184 scene is a cache/world invariant.** It is a RuneLite scene/runtime representation.
6. **A fixed texture array/count is a cache invariant.** Renderer capacity is not decoder truth.
7. **Static and dynamic models use one identical priority/transparency path.** RuneLite itself uses different upload strategies.

## 14. Checkpoint 3 final audit matrix

| Area | Audit result | Promotion rule |
|---|---|---|
| loc dispatch/walls/decor | `VERIFIED` | may become atomic specs |
| model selection/mirror/transforms | `VERIFIED` | may become atomic specs + fixtures |
| morph/dynamic model resolution | `VERIFIED` | may become atomic specs + fixtures |
| contouring | `VERIFIED` | may become exact integer spec |
| model lighting | `VERIFIED` | may become exact reference spec |
| cross-model normal merge | `VERIFIED` | old no-merge claim superseded; fixture required |
| terrain shape topology 0..12 | `VERIFIED` | may become exact topology spec |
| flat terrain split | `VERIFIED` renderer reference | promote with parity-profile ownership |
| underlay definition HSL | `VERIFIED` | may become exact decoder spec |
| overlay definition decode/HSL | `VERIFIED` | may become exact decoder spec |
| complete 11x11 terrain-color builder | `REVISION_SENSITIVE` | source pin/end-to-end fixture required before normative promotion |
| bridge scene relink | `VERIFIED` | atomic scene-plane spec |
| bridge collision selection | `VERIFIED` | atomic placement/collision spec |
| RuneLite bridge roof/VIS_BELOW grouping | `VERIFIED RENDERER_REFERENCE` | renderer profile only |
| ground-decoration lift | `OBSOLETE/REFUTED` for audited path | do not implement generically |
| object model UV reconstruction | `VERIFIED RENDERER_REFERENCE` | differential fixture before wgpu implementation |
| terrain UVs | `VERIFIED RENDERER_REFERENCE` | terrain renderer fixture |
| texture animation input | `VERIFIED` | decoder/material spec |
| RuneLite tick UV animation | `VERIFIED RENDERER_REFERENCE` | renderer policy/profile |
| software face priority structure | `VERIFIED` | exact ordering fixtures required |
| RuneLite dynamic priority replication | `VERIFIED RENDERER_REFERENCE` | implementation reference |
| static alpha split | `VERIFIED RENDERER_REFERENCE` | do not mistake for semantic definition |
| face bias metadata | `VERIFIED` | preserve in model contract |
| clip-space face-bias formula | `VERIFIED RENDERER_REFERENCE` | wgpu ADR later |
| coordinate constants | `VERIFIED` | core/reference math |
| editor camera behavior | `PROJECT_DECISION` | editor blueprint later |
| revision-sensitive id widths/opcodes | `REVISION_SENSITIVE` | target-revision fixtures required |

## 15. Checkpoint 4 handoff

Checkpoint 3 is complete as an **audit**.

Checkpoint 4 may now create canonical OSRS specifications, but promotion is evidence-gated:

- verified rows can be converted into atomic specs with source pins and test ownership
- renderer-reference rows must be labeled as parity-profile/reference behavior, not OSRS cache truth
- project decisions belong in ADRs
- `REVISION_SENSITIVE` rows remain non-normative until the source/fixture gate is satisfied

The first terrain-color spec should therefore explicitly leave the complete blur/color-build algorithm gated rather than disguising the stale `class470` citation as certainty.