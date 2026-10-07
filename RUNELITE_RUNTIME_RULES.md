# OSRS Runtime Rendering Rules — Everything the Cache Doesn't Store

The cache stores flat records (loc id + type + orientation + tile). It does
**not** store which edge a wall covers, how far a torch stands off its wall, how
tall a gate is, what color a hill is, or which model variant a corner uses. The
engine computes all of that at scene-build time. Every rule below was verified
first-hand in `RuneLite-melxin/runescape-client/src/main/java/` (line numbers
cited); each entry gives the port formula and the artifact of getting it wrong.

Root builders: `class150.addPendingSpawnToScene` (all loc placement, lines
134–441), `class470` (all terrain color/height, lines 880–1125),
`ObjectComposition.getModelData` (per-loc model build, lines 776–909),
`ModelData` transform ops (1851–1961), `Scene.new*` storage (583–648),
`Tiles.java:51-56` (orientation tables).

---

## A. Loc-type dispatch (the master table the cache implies but never states)

### R1. Type → scene-layer mapping (`class150:266-434`)
`l_x_y` gives `(id, type 0–22, orientation 0–3)` per tile. The engine maps:
- `22` → floor decoration (`newFloorDecoration`), model built as type 22.
- `10`, `11` → game objects with full footprint (`method5564` with
  `sizeX/sizeY`); type 11 passes flag `256` (orientation-swapped variant).
- `0,1,2,3` → boundary walls (`newBoundaryObject`); `4–8` → wall decorations
  (`newWallDecoration`); `9` → 1×1 diagonal wall via the game-object path;
  `≥12` → roofs et al. via the 1×1 game-object path.
- A separate category word (`var2`: 0 boundary, 1 wall-decor, 2 game-object,
  3 floor-decor) selects which layer a pending spawn *replaces*.
- Miss the table and whole categories vanish (e.g. treating 9 as a wall leaves
  diagonal walls unlit/unplaced; treating ≥12 as walls breaks every roof).

### R2. Wall edge flags (`Tiles.java:51-52` + `WallObject` javadoc)
Loc-orientation `0–3` maps to edge bitflags: straight walls `field800 =
{1,2,4,8}` = West,North,East,South; diagonal walls `field804 =
{16,32,64,128}` = NW,NE,SE,SW (bit meanings confirmed by the
`WallObject.getOrientationA/B` javadoc). The flags drive software edge drawing
and collision; the GPU static path does not consume them (wall models arrive
pre-rotated from R9), but the editor's placement preview and any collision
export must. Wrong table = walls on wrong edges, corners that don't meet.

### R3. L-corner dual-model rule (`class150:313-325`) — where dual slots come from
Type 2 builds **two** models: `getModel(2, orient+4)` and
`getModel(2, (orient+1)&3)`, stored as `renderable1/2` with
`field800[orient]` + `field800[(orient+1)&3]`. Type 0/1/3 build one model with
`field800`/`field804` respectively. This is the *origin* of the dual-slot
contract: the second slot is not a spare, it is the second arm of every corner.
Port: always build both arms for type 2, even if one model id is shared.

### R4. Wall-decor standoff vectors (`Tiles.java:53-56`, `class150:362-416`)
- Type 4: flush (`field800[orient]`, offsets 0,0).
- Type 5: offset `(field802[orient]·inset, field798[orient]·inset)` where
  `field802={1,0,-1,0}`, `field798={0,-1,0,1}` (perpendicular unit vectors) and
  `inset` = the **existing wall's** `decorDisplacement` — proven: the code reads
  the wall already on the tile and uses its `int2` field, which defaults to 16
  and decodes from cache opcode 28 (same field FileStore calls
  `decorDisplacement`). Falls back to 16 when no wall is present.
- Types 6/8: diagonal offsets `(field803·k, field805·k)` with
  `field803={1,-1,-1,1}`, `field805={-1,-1,1,1}`, `k = inset/2` (default 8);
  type 6 single-sided (`orientation 256` = two-sided flag), type 8 double
  (second model at `(orient+2)&3`); type 7: opposite side, no offset.
- `WallDecoration.method6262` then nudges ±1 by orientation flag
  (`WallDecoration.java:98-122`). Skip the vectors and every banner floats or
  embeds; skip the wall-lookup and inset is wrong on thick walls.

### R5. Diagonal-decor recenter (`ObjectComposition.java:874-877`)
Type 4 models with `orientation > 3` (the `+4` mirrored variants used by types
6–8) get `rotateJAU(256)` (= 45°) **plus** `changeOffset(45, 0, -45)` *before*
the standard rotation step. This recenters the mirrored asset on its diagonal.
Without it, diagonal torches/signs sit half a tile off.

### R6. Footprint center + height sampling (`class150:226-258`)
- Size swap: orientations 1/3 transpose `sizeX/sizeY` *for placement math*.
- Center: `cx = tile + size/2`, `cz = tile + size/2` (integer halves), clamped to
  the map edge; world pos `(tile<<7)+(size<<6)` — the footprint center, not a corner.
- Height: four heightmap samples at the footprint corners on the
  **bridge-adjusted plane** (R13), averaged `>> 2`.
- Game objects (`method5564`, `Scene.java:640-648`): center
  `(size·64 + tile·128)` per axis. Floor decor: exact tile center.
- Objects placed at corner height or unadjusted plane float/sink on slopes and
  bridges.

## B. Per-loc model build (`ObjectComposition.getModelData`, 776–909)

### R7. Type-gated model selection (776–858)
- `models == null` → only type 10 builds (merges *all* `modelIds`); any other
  type returns null = **renders nothing**.
- Otherwise the engine scans `models[]` for `== type` and builds that
  `modelIds[i]`; no match → null. An object lists which loc types it has
  geometry for — a fence with only type-0 geometry placed as type 10 is
  legitimately invisible.
- Port: never fall back to "first model"; null is a valid answer.

### R8. Mirror rule (790–793, 840–843, `ModelData.method5280`)
`mirror = isRotated ^ orientation > 3` → `modelId += 65536`, load mirrored
cache variant, `z = -z` **and swap winding** (`indices1 ↔ indices3`). The
winding swap is load-bearing: mirror without it and lighting/culling invert.
(Covers the `orient+4` models in R3/R4: they arrive pre-mirrored.)

### R9. Transform pipeline order (873–908 — exact sequence)
1. Copy-construct (shares nothing the caller may mutate).
2. Type-4/orient>3 recenter (R5).
3. `orientation & 3`: `1 → rot90 (x,z)→(z,−x)` (`method5273`), `2 → rot180`
   (`5274`), `3 → rot270` (`5275`); `5276(angle)` is the general JAU rotator.
4. `recolor(from→to)` over **all** faces, then `retexture` (null-safe).
5. `resize(modelSizeX, modelHeight, modelSizeY)` as `v·n/128` — only if any ≠128.
6. `changeOffset(offsetX, offsetHeight, offsetY)` translate — only if any ≠0.
Reordering (e.g. offset before rotation) displaces every resized/offset object
(statues, lamps, cannons).

### R10. Dynamic vs static branch (all types share one pattern)
`animationId == -1 && transforms == null` → baked `getModel(...)` (cached,
shared); else `new DynamicObject(...)` carrying the displaced renderable from
the replaced object. Type 11 additionally passes flag `256` into the
game-object add (its orientation variant) — but static-vs-dynamic still
branches only on animId/transforms. Port both: static cache for the map, live
`DynamicObject` for animating locs — and never contour/pose the cached original
(R16).

## C. Terrain color + light (all in `class470`, all runtime)

### R11. Corner slope shading (`class470:893-904`)
Per-tile brightness from the height gradient with the **same sun as model
lighting**: `gx = h[x+1]−h[x−1]`, `gz = h[z+1]−h[z−1]`,
`len = √(gx²+gz²+65536)`, `light = ((−50·gx −50·gz −10·len)/norm) + 96`, minus
a weighted neighbor sum from the `Tiles_underlays2` grid
(`>>3,>>2,>>2,>>3,>>1` — adjacent contrasting ground darkens the tile).
Stored per corner (the `MouseRecorder.field868` array is the brightness grid
despite its name).
This is baked Gouraud shading: hills shade without any light in the shader.

### R12. Underlay 11×11 blur (907–1049) — exact kernel, corrects older 5×5 claims
Separable sliding-window box blur, radius **5 in each axis (11×11 tiles)**:
column sums over `x±5`, then row sums over `z±5`, of `(hue, saturation,
lightness, hueMultiplier, count)` — but only tiles with an underlay id
participate (counted separately). Reduction: `hue = Σhue·256/ΣhueMultiplier`
(**weighted**), `sat = Σsat/count`, `light = Σlight/count`. Then jitter
(`hue+rndHue`, `light+rndLightness`, clamped) and per-corner palette lookup
with the R11 brightness values (`method2086(hsl, cornerLight)`, which keeps
hue/saturation bits and rewrites only lightness, clamped 2–126 — and maps
`hsl == -1` to the `12345678` skip sentinel). The packed ints come from
`method817`, which compresses 8-bit HSL with highlight desaturation
(`sat/=2` above lightness 179/192/217/243; packed as `sat/32<<7 | hue/4<<10 |
light/2`) — reuse it verbatim rather than `JagexColor.packHSL` for terrain.
Port exactly: hue weighting and the count-vs-weight split are what keep deserts
from tinting neighboring grass. Border rule: accumulate over the *extended*
grid (neighbors outside the region contribute) — isolated-region blending
draws dark seams. Jitter source: `Tiles.rndHue/rndLightness` random-walk ±2 per
region load, clamped to [−8,8]/[−16,16] (`Tiles.java:58-59`,
`class470.java:855-871`) — the live client re-rolls slightly every login, so an
editor should pin fixed values (0) for deterministic output.

### R13. Overlay rules (1051–1122)
`shape+1` with rotation from the overlay-rotation grid; `hideUnderlay` overlays
suppress the underlay entirely; `primaryRgb == magenta (16711935)` means hidden
(`-2`, skip); textured overlays use the texture's average RGB; `secondaryRgb`
overrides via the secondary HSL path; all-flat overlay quads get the `2340`
flag OR-ed into the tile's minimap word (read back by minimap paths — confirm
the exact consumer for your build before relying on it).

### R14. Bridge-adjusted plane for heights (`class150:211-214`, also terrain)
`if plane < 3 && settings[1][x][z] & 2: samplePlane = plane+1`. Both loc heights
(R6) and the uploader's geometry pass use the shifted plane while collision
stays put. One bit, two different planes — the defining bridge subtlety.

## D. Identity, replacement, and caches

### R15. Config word + tag (`class150:259-264`)
`config = (orientation<<6) + type`, `+256` when `int3 == 1` (`int3` defaults
from `interactType != 0` in `postDecode` — the engine's supports-items signal,
matching the `bits>>>8&1` slot in the `GameObject.getConfig` javadoc); tag =
`calculateTag(plane, x, y, 2, int1==0, id, worldId)` with those literal
arguments (`int1` is the models/actions-derived interactivity flag from the same
`postDecode`). The editor should write identical tags — downstream tools
(collision, click handling) key off them.

### R16. Replace-by-layer + cache discipline (148–207, 662–694)
Pending spawns first remove the same-category object (freeing its renderable
into the new `DynamicObject`), update collision for the old def, then place.
Model caches split three ways (raw `ModelData`, lit entities, lit models);
smooth (`nonFlatShading`), contour, and pose paths always copy before mutating.
Editor equivalent: undo/replace must restore the exact previous slot content,
and preview mutations must never touch the shared cache.

## E. Walls + wall decor catalog (all types executed, not just read)

Premise correction first: **the engine never merges wall meshes.** There is no
welding, no boolean union, no shared-edge stitching anywhere in the scene
build or the GPU upload. Adjacent walls abut purely because every wall object
is stored at the **tile center** (`BoundaryObject.x = tileX*128+64`,
`y = (tileY*128+64)*64` fixed-point) with geometry authored out to the edges,
and corner types supply the second arm on the same tile (R3). A "merged" look
is authored content + shared center, nothing else. (Fixes the merging premise
in older notes.)

Storage-scale note (executed): boundary/wall-decor positions use
`x = tile*128+64`, `y = tile*8192+4096` (mixed fixed-point), while floor
decor uses `tile*8192+4096` on **both** axes (`floor t=22 x=86016` vs wall
`x=1344` on tile (10,20)). API accessors present world units; the port must
not mix raw scales. All values below are executed outputs in
`reference-fixtures/deob_golden.txt` (`wall`/`decor`/`floor`/`nudge` lines).

### R23. Wall types 0, 1, 2, 3 (+9): what each is and where each goes

All four store one `BoundaryObject` per tile with `orientationA/B` edge flags;
rendering draws `renderable1` (+`renderable2` when present) at the stored
center — the model carries the edge geometry, pre-rotated at build (R9).
- **Type 0 — straight wall.** One model (`getModel(0, orient)`), single slot,
  `orientationA = field800[orient]` (1/2/4/8 = W/N/E/S). The common case.
- **Type 1 — diagonal wall.** One model, `orientationA = field804[orient]`
  (16/32/64/128 = NW/NE/SE/SW). Same single-slot shape as type 0; only the
  flag domain (and authored geometry) differs.
- **Type 2 — L-corner.** Two models on the same tile (`orient+4` mirrored arm
  + `(orient+1)&3` arm, R3), `orientationA/B = field800[orient],
  field800[(orient+1)&3]`. This is the entire dual-slot mechanism: no other
  wall type fills slot 2.
- **Type 3 — square cap/junction.** One model, `field804` flags (same domain
  as type 1; geometry is the cap, not a span).
- **Type 9 — diagonal span as game object.** Built and stored through the
  game-object path (`method5564` 1×1; executed: center = tile center in
  128-units, edge mask 0), **not** the boundary path — so it gets footprint
  treatment and `addGameObject` collision instead of edge flags and wall
  collision (`method6340`). Rendered the same way downstream (one model at
  tile center).
- No type merges, no hidden variants, no exceptions: types outside 0–3 go
  through game-object or decor paths (R1). A tile holds at most one
  `BoundaryObject` — a second wall placement on the same tile *replaces*
  slot content per the R16 layer rule (pending spawns remove same-category
  first), except type 2 which fills both slots in one call.

### R24. Wall-decor types 4–8: offsets, nudges, and the 256 flag

All five store one `WallDecoration` with two renderable slots and two offset
pairs; slot 1 draws at `+offset`, slot 2 at `+offset2` (mixin clickbox/hull
code confirms: `getX()+getXOffset()` vs raw `getX()`). Model is always built
as type 4 (`getModel(4, …)`); orientation variants select mirroring (R8).
- **Type 4 — flush decor.** `orientation = field800[o]`, offsets (0,0) → but
  `method6262` still applies its ±1 nudge (executed: `decor t=4 o=2` yields
  `xOff=-1`, orientation flag 4 decrements). Nothing is ever truly zero.
- **Type 5 — stood-off decor.** Offsets `(field802[o]·inset,
  field798[o]·inset)` with wall-lookup inset (R4); then the nudge
  (executed: `t=5 o=0` → `xOff=17` from input 16, flag 1 increments).
- **Type 6 — diagonal single.** Model mirrored (`orient+4`),
  `orientation = 256`, `orientation2 = orient`, offsets `(field803·k,
  field805·k)`, `k = inset/2` (executed `t=6` → `(8,8)`-class diagonals).
- **Type 7 — opposite-side decor.** Model `(orient+2&3)+4`, `orientation =
  256`, `orientation2 = (orient+2)&3`, offsets (0,0) (nudge still applies by
  flag domain — 256 falls in `default`, so no nudge: executed `xOff=0`).
- **Type 8 — double diagonal decor.** Two models (`orient+4` and
  `(orient+2&3)+4`, second fed the displaced renderable), same offsets as
  type 6. The only decor type filling slot 2.
- **The 256 flag** means "no single edge" (diagonal/two-sided); it selects the
  `default` (no-nudge) branch of `method6262` (executed: all `o=256` rows pass
  inputs through unchanged).
- **`method6262` nudge table** (executed, 27 rows: flags
  1/2/4/8/16/32/64/128/256 × inputs (16,0)/(0,−16)/(8,−8)): flag 1 → x+1,
  flag 4 → x−1, flag 2 → second-axis −1, flag 8 → second-axis +1, everything
  else → unchanged. Inputs are stored raw in `yOffset/field3194`; nudged
  copies land in `xOffset/field3196`.

## F. Correctness guards (do-not-port items + lighting/depth reference)

### R17. No cross-model normal welding — seams are authentic
`ModelData.calculateVertexNormals` (1964+) loops **only its own faces** into its
own `vertexNormals`; `Scene.java` contains zero normal code and the GPU
uploader none. Adjacent wall/cliff pieces keep independent normals, so the
lighting crease along modular joints is the *correct* look. Earlier drafts of
these docs prescribed welding — that was wrong and is corrected here: do not
average normals across models. (Supersedes the welding lines formerly in
`RUNELITE_RENDER_SOURCES.md` §9 and `RUNELITE_SCENE_AND_MATERIALS.md` §9 —
both now corrected.)

### R18. No software occluder culling on the GPU path
Occluders/`visibleTiles`/`method5801` appear only in the software draw walk.
`SceneUploader` never reads them. The editor replaces this with frustum
culling per zone — do not reimplement occluders.

### R19. No shadow maps, no specular, and other non-render flags

Vanilla shading = R11 slope brightness + per-vertex sun diffuse + fog. Shadows
beyond baked brightness don't exist; adding them is an HD-mode decision, not a
parity requirement. Likewise do not port for rendering: `modelClipped`
(opcode 23), `isHollow` (opcode 74), object sounds (`createObjectSound`,
ambient/sound fields), or collision writes (`method6340/6345`,
`addGameObject`, `setBlockedByFloorDec`) — no scene-draw consumers found
(collision feeds movement, not pixels), though the editor will want collision
later as a separate system.

### R20. Light-constant scales (decode opcodes 29/39 + all `toModel` sites)
Cached `ambient` decodes raw (`readByte`), but cached `contrast` decodes
**×25** (`readByte()*25`) — then lighting calls `toModel(ambient+64,
contrast+768, sun=(-50,-10,-50))`. The `+64/+768` offsets and the sun vector are
engine constants shared by the static, smooth, and dynamic paths (662–768).
On the FileStore question: FileStore is **byte-faithful by design** — its codec
keeps the raw byte and round-trips TOML exactly, which is correct behavior for
a cache library. The ×25 is engine behavior at model-lighting time, so it
belongs in the Rust lighting code (`contrast*25` before `+768`), not in a
FileStore fix. Get the scale wrong and every object renders one contrast step
off — a global, easy-to-miss brightness shift.

### R21. Depth system: reverse-Z + authored bias (no polygon offset, no epsilon)
The GPU path runs **reverse-Z end to end** (`GpuPlugin.java` pins):
`glClipControl(GL_LOWER_LEFT, GL_ZERO_TO_ONE)` ("1 near 0 far", line 369),
`glDepthFunc(GL_GREATER)` (1027), `glClearDepth(0)` (1041/1047 — 0 is far),
32-bit float depth renderbuffer (`GL_DEPTH_COMPONENT32F`, 804–805), and the
custom `Mat4.projection` (depth-friendly, not standard perspective). Precision
sits near the camera — required for coplanar decals across a 104-tile scene.
A wgpu port must mirror this (`CompareFunction::Greater`, clear 0.0,
`Depth32Float`, reversed projection) — `Less`/clear-1.0 reintroduces every
Z-fight the bias below was authored to fix.

Coplanar faces are resolved by exactly two mechanisms, nothing else:
1. **Authored per-face bias** — a real cache byte per face (deob
   `ModelData.field2718`, read at line 738–741) surfaced as `Model.faceBias`
   and baked into `abhsl` bits 16–23 at upload (`bias<<16`). Software uses it
   as `faceBias*2` (`Model.java:1855,2012`); the shader uses
   `screenPos.z += bias/128.0` (`vert.glsl:96`). Under reverse-Z, larger bias =
   nearer = wins. Rugs/posters/railings ship nonzero bias against their
   background faces. There is no `glPolygonOffset` and no priority-epsilon term
   anywhere in `Zone`/`VAO`/`GpuPlugin` (verified by grep) — do not invent one.
2. **CPU bucket draw order** (A2/R-priority rules) — required for translucency
   and for faces whose bias is equal; with the depth test on, order alone never
   separates exactly-coplanar faces, so (1) is load-bearing. If a decal
   shimmers, the fix is its authored bias (or a content-side nudge recorded as
   data), never a global epsilon.

### R22. Baked-at-upload vs resolved-per-frame (the split to preserve)

Baked into VBOs (immutable until the zone re-uploads): world-space positions
(heights flow raw — see correction below), `abhsl` (alpha, bias, HSL),
static-opaque emission order (priority buckets flattened), base UVs.
Resolved per frame: priority sort of dynamic models (A2), zone Range walk with
roof/plane skips, fog (`fFogAmount` from uniforms), lighting (`hslToRgb` of the
baked HSL), UV scroll (`tick`), tint/brightness/fog/colorblind uniforms.
Slope shading (R11) and the underlay blur (R12) are baked one step earlier, at
scene build, into the HSL lightness bits — the shader never sees a light.

Correction to older notes: **no ×-8 or negation exists on the GPU path.**
`SceneUploader`, `Zone`, and `vert.glsl` apply zero height transform (verified
by grep — the only height-adjacent ops are chunk shifts `<<3`/`>>3`); heights go
`tileHeights → buffer → +base → view/proj` untouched. Any scale lives at scene
build, not upload — do not add one.


### R25. Roof pieces (types 12–21): game objects with type-selected models

There is no roof-specific storage or merging. Every roof piece goes through
the 1×1 game-object path (`method5564`, same as R23 type 9): center at tile
center in 128-units (executed `roof t=14`: `(1344, 2624)` on tile (10,20)),
edge mask 0, footprint span of one tile. What makes a roof piece a roof piece
is entirely upstream: the model selected by **type** (`getModel(type, …)`
picks the `models[]` entry matching the loc type, R7) plus orientation (R9).
A pitched roof is N adjacent 1×1 pieces whose authored models form the slope —
the engine contributes no grouping, no ridge logic, no shared geometry. Port
consequence: roofs need no special code path, only correct per-type models;
an editor "roof brush" that paints multi-tile roof runs is a content-side
convenience, and each tile stays an independent 1×1 game object.

### R26. Tile capacity + edge masks (placement limits the editor must enforce)

- **Max 5 game objects per tile**: `newGameObject` returns false at
  `gameObjectsCount >= 5` (executed: 6th call rejected, count stays 5).
  `Tile.gameObjectEdgeMasks` is preallocated `[5]` to match. Brush tools must
  refuse or replace beyond five — same rule the engine applies.
- **Edge masks** (`newGameObject`, `Scene.java:732-762`): each covered tile
  records which footprint edges pass through it — bit 1 set when the tile is
  past the footprint min-X, 4 when before max-X, 8 when past min-Y, 2 when
  before max-Y (executed 2×1 gate: min tile mask 4, max tile mask 1); each
  tile OR-accumulates into `gameObjectsEdgeMask`. The uploader's `min == tile`
  filter (R6/A1) keys off the same span (`startX/endX`), so masks and upload
  agree by construction. Single-tile objects always record 0.

### R27. Item pile slots (exact stack rendering)

`ItemLayer` holds **three** visible renderables (`first/second/third`) plus a
`height` lift and tag at the tile position — newer pickups beyond three are
tracked in data but not drawn. Render order is first→second→third at the
stored height; there is no per-item offset table. Port consequence: draw at
most three item models per pile, stacked by slot order, and cap the editor's
pile preview the same way.

### R28. Complete loc-type catalog 0–22 (every type, one line each)

The dispatch (`class150:266-434`) is exhaustive over integers — `== 22`,
`>= 12`, `10/11`, then `0,1,2,3,9,4,5,6,7,8` — so any value routes somewhere,
but shipped content uses exactly 0–22 (no per-type logic exists outside this
list; FileStore agrees: its only magic type constant is the mapscene
exclusion at 22):

| Type | Kind | Storage | Model rule | Notes |
|---|---|---|---|---|
| 0 | straight wall | boundary slot 1, `field800` | `getModel(0, o)` | R23 |
| 1 | diagonal wall | boundary slot 1, `field804` | `getModel(1, o)` | R23 |
| 2 | L-corner | boundary slots 1+2, both flags | `getModel(2, o+4)` + `getModel(2, (o+1)&3)` | R3/R23 |
| 3 | square cap | boundary slot 1, `field804` | `getModel(3, o)` | R23 |
| 4 | flush decor | wall-decor, offsets 0,0 | `getModel(4, o)` | R24 |
| 5 | stood-off decor | wall-decor, wall-lookup inset | `getModel(4, o)` | R4/R24 |
| 6 | diagonal decor | wall-decor, 256-flag, half inset | `getModel(4, o+4)` | R24 |
| 7 | opposite decor | wall-decor, 256-flag, no offset | `getModel(4, (o+2&3)+4)` | R24 |
| 8 | double diagonal decor | wall-decor slots 1+2, half inset | two `getModel(4, …+4)` | R24 |
| 9 | diagonal span | **game-object** 1×1 path | `getModel(9, o)` | R23 |
| 10 | game object | footprint `sizeX×sizeY` | type-matched models merged | R1/R6 |
| 11 | game object, rotated | footprint swapped, flag 256 | same as 10 | R1/R10 |
| 12–21 | roof pieces | game-object 1×1 path each | `getModel(type, o)` per type | R25 |
| 22 | floor decoration | `newFloorDecoration`, tile center | `getModel(22, o)` or DynamicObject | R1/R6 |

(o = loc orientation 0–3 throughout; `+4` = mirrored variant per R8.)

### R29. Face render types 0–3 (model-level, not loc-level)

Separate numbering from loc types — per-face bytes decoded from cache
(`ModelData` line 635) with merge/copy propagation; meaning fixed in
`toModel` (2095–2257):
- **0 — smooth shaded**: per-vertex normals, three `faceColors` via
  `method5263` (textured faces: bare lightness via `method5264`).
- **1 — flat shaded**: single face normal, one color (`faceColors1`,
  `faceColors3 = -1`).
- **2 — skip/alpha path**: untextured faces get `faceColors3 = -2` (culled
  downstream); also forced by alpha −1.
- **3 — magic gray**: `faceColors1 = 128`, `faceColors3 = -1`; forced by
  alpha −2.
Tile overlay shapes 0–12 are a third numbering (terrain cuts, E2) — the three
"type" systems (loc / face / shape) are independent; never mix them.

## G. Remaining gap

- **Second-slot lighting order** — confirm no special-casing in
  `getModelData` beyond the `orient+4` args (already read as none, re-verify
  by execution if a cache becomes available).
