# RuneLite Scene Contracts & Materials — How They Work (File-by-File Deep Dive)

Companion to `RUNELITE_RENDER_SOURCES.md` (Groups C + D + E). Covers the
**data contracts** the pipeline consumes: what each object *means*, how its
fields are produced, and which ones the editor may mutate. All `runelite-api`
paths below are in `/Users/tylercovalt/Documents/runelite-master`
unless marked `[melxin]`; construction internals live in the melxin
`runescape-client` tree (see `RUNELITE_DEOB_READING_GUIDE.md`).

---

## 1. `Scene.java` — the world container

**What it is:** four planes of 104×104 tiles plus the *extended* 184×184 arrays
the engine actually indexes (margin = `(184-104)/2`, exposed as `ESCENE_OFFSET`
in `Perspective`). Everything the uploader reads comes from here.

**Key accessors and what they carry:**
- `getTiles()` vs `getExtendedTiles()` / `getExtendedTileSettings()` — the
  uploader iterates *extended* coordinates; the settings byte per tile carries
  `BRIDGE/VIS_BELOW/UNDER_ROOF`. Rendering the 104-grid without the margin
  breaks bridge math and underlay blending at region borders.
- `getTileHeights()` — int heights per plane/tile; vertex Y is derived from
  these, never stored on tiles. `getTileHeight(x, y, level)` interpolates.
- `getTileShapes()` + `getUnderlayIds()` / `getOverlayIds()` (`id+1`, 0 = none)
  — shape/rotation select the cut (Group E tables); ids index floor definitions.
- `getRoofs()` + `buildRoofs()` + `setRoofRemovalMode()` — roof-ID volumes;
  the uploader buckets geometry by these so hiding is a range skip.
- `getWorldViewId()`, `getMinLevel()/setMinLevel()`, `getDrawDistance()`,
  `getBaseX()/getBaseY()`, `isInstance()/getInstanceTemplateChunks()`,
  `getSkybox()` — view identity, plane window, culling distance, world origin,
  instancing, preview backdrop.

**Why it matters:** misreading any one array shifts, holes, or unlights the
whole scene. → Rust: `editor_core::scene` owns extended arrays + heights as
the single source of truth.

---

## 2. `Tile.java` — one cell, seven slots

**What it is:** the per-cell record: `paint` (flat) *or* `model` (shaped) terrain
plus up to one `WallObject`, one `DecorativeObject`, one `GroundObject`, an
`ItemLayer`, several `GameObject`s, and an optional `bridge` tile.

**How the slots interact:** terrain is exclusive (paint xor model); wall/decor/
ground are independent layers that all render; game objects are shared across
the tiles they cover (uploader filters `min == tile`). `getRenderLevel()` is the
plane the tile *draws* on; `getPlane()` is where it *lives* — they differ exactly
for bridges. `[melxin]` adds `setDecorativeObject/setWallObject` (editor
mutation) and `getPhysicalLevel()` (roof min-plane for culling).

→ Rust: `editor_core::tile`; brush tools mutate through the built-in setters
(`setGroundObject/setSceneTilePaint/setSceneTileModel`) plus melxin's
(`setDecorativeObject/setWallObject`).

---

## 3. `SceneTilePaint.java` — flat terrain quads

**What it is:** four corner HSL colors + optional texture + `isFlat`. The uploader
emits two triangles with full-tile UVs. Gouraud interpolation across the quad is
what makes hills look smooth; `isFlat` (all corners equal) lets the renderer
take fast paths.

**Why it matters:** the quad's diagonal split must match everywhere or seams
crack along tile diagonals. → Rust: `editor_render::terrain::paint`.

## 4. `SceneTileModel.java` — shaped terrain (the 0–12 cuts)

**What it is:** the triangulated overlay for paths, shores, and corners: shape +
rotation select vertex/face/color/texture arrays (built by the Group E
constructor). `[melxin]` adds per-corner underlay/overlay color getters+setters —
the paint-brush API.

**Why it matters:** this is the most geometry-dense contract in the scene; every
shoreline and path edge flows through it. → Rust: same module as paint, plus the
melxin corner setters.

---

## 5. Loc objects — `WallObject`, `DecorativeObject`, `GroundObject`, `GameObject`

**`WallObject` — the dual-slot junction system.** Walls are *not* merged meshes:
each tile holds two renderable slots (`getRenderable1/2`) with independent
orientations (`getOrientationA/B` 0=West,1=North,2=East,3=South). A lone straight
wall fills slot 1; a T-junction or corner fills slot 2 on the *same* tile.
`[melxin]` adds `getModelA/B`. Miss slot 2 and half of all rooms lose corners.

**`DecorativeObject` — wall furniture with standoff.** Torches, banners, and
signs don't sit on the tile center: `getXOffset/getYOffset` (and the `…2`
twins, ≈ 16 units = `decorDisplacement`) push the model perpendicular to the
wall face, with orientation read from `getConfig` bits (`>>>6&3`).

**`GroundObject` — floor decals with lift.** Flowers, cracks, rubble, and rugs
render slightly above the tile (+1/+2 height units) so coplanar terrain never
Z-fights them. No lift = shimmering floors.

**`GameObject` — everything else, with footprints.** Trees, stalls, gates, and
houses carry `sizeX/sizeY`, `getSceneMin/MaxLocation`, orientation, and a config
word (`type = bits&31`, `orient = bits>>>6&3`, item-support bit). Rotation pivots
around the footprint center, and the uploader's `min == tile` rule guarantees
one upload per object. Rotate around a corner and 2×3 buildings will teleport.

→ Rust: `editor_core::placement` with `footprint()`, `pivot()`, `wall_normal()`
helpers; the placement ghost previews all four types with their real offsets.

**Base types:** `TileObject` (id/position/plane), `Renderable` (drawable marker),
`DynamicObject` (`getModelZbuf()` + `getAnimation/getAnimFrame/getAnimCycle`
+ `getRecordedObjectComposition`, plus `[melxin]` `getAnimationID()` —
animated locs must resolve to *some* model every frame or doors vanish),
`ItemLayer` (ground-item stacks: hideable, never fatal), `EntityOps` (current
menu-op contract).

---

## 6. `WorldView.java` + coordinates — where "where" means

**What it is:** overworld-vs-instance identity. `TOPLEVEL = 0`; anything else is
an instance whose coordinates shift by `SCENE_OFFSET`. `getTileHeight(x, y,
level)` interpolates heights for picking and ghost placement. XTEA keys for
encrypted map squares come from FileStore's `OpenRS2` (rev 237+ keys are largely
unarchived — plan for keyless regions).

**Coordinate types** (`coords/`): `LocalPoint` (scene units), `WorldPoint`
(global tiles), `Angle` (orientation), `Direction`, `WorldArea` (footprint
rects). Keep all editor math in tile+sub-tile (1 tile = 128 units) and convert
at the render boundary.

---

## 7. `Perspective.java` + `Constants.java` — math constants

`Perspective` owns rotation and interpolation: `LOCAL_TILE_SIZE 128`,
`SINE/COSINE[2048]` (+ float twins), 14-bit `SINE14/COSINE14[0x4000]` tables with
`UNIT14`, `ESCENE_OFFSET`, the bilinear height formula (`getTileHeight`,
`getFootprintTileHeight`), canvas projection (`localToCanvas`, `modelToCanvas`,
`getCanvasTilePoly`). `Constants` owns sizes (`CHUNK 8`, `REGION 64`, `SCENE 104`,
`EXTENDED 184`), tile flags (`BRIDGE 0x2`, `UNDER_ROOF 0x4`, `VIS_BELOW 0x8`,
`ROOF_FLAG_*`), and tick lengths. Between them they fix every coordinate space;
desync here shows up as objects sinking into hills or picking the wrong tile.

---

## 8. `JagexColor.java` — the only correct HSL

**What it is:** pack/unpack (`hue<<10 | sat<<7 | lum`, maxima 63/7/127) plus
`rgbToHSL` gamma conversion. It shares the packed layout `hsl_to_rgb.glsl`
decodes (note: terrain colors go through the deob `method817` compressor with
highlight desaturation instead — same layout, different rounding; see
`RUNELITE_RUNTIME_RULES.md` R12).

---

## 9. `Model.java` / `ModelData.java` / `Mesh.java` — mesh truth

**What they are:** `Mesh` (vert arrays as **`float[]`**, counts), `Model`
(renderable: faces, colors ×3, priorities 0–11, transparencies, biases,
textures + tex-face frames, diameter/radius, `getTransparency()`, HSL
overrides, vertex normals), `ModelData` (mutable: `clone*`, `calculateVertexNormals`,
`toModel(ambient+64, contrast+768, light(-50,-10,-50))`, `contourGround`).

**How lighting works:** Gouraud, not PBR — per-vertex diffuse from a fixed sun
vector, modulated by per-object `ambient`/`contrast` (FileStore `ObjectType`
fields). `nonFlatShading` objects keep smooth normals; others bake flat.
Textured faces skip HSL overrides. Normals are strictly per-model
(`calculateVertexNormals` never crosses models) — the crease along modular
joints is the authentic look, so the editor must **not** weld normals
(`RUNELITE_RUNTIME_RULES.md` R17).

---

## 10. `Texture.java` / `TextureProvider.java` — animated materials

`Texture` carries only animation direction (U vs V axis) and speed;
`TextureProvider` adds brightness, the texture array, and default colors.
Material tinting lives elsewhere (scene HSL overrides, `textureLightMode`,
palette scaling). Runtime law: `uv += tick × speed / 128`
(`TEXTURE_ANIM_UNIT` in `vert.glsl`). Waterfalls, lava, and conveyors are all
the same mechanism with different vectors.

---

## 11. `ObjectComposition.java` — which variant, how big

Render-relevant surface: identity/actions, `EntityOps getOps()`,
`sizeX/sizeY` footprints, and the morph chain
(`getImpostorIds/getImpostor/getVarbitId/getVarPlayerId` — resolve per preview
state or crops/doors render wrong). Everything else about the definition
(ambient, contrast, `clipType`, `nonFlatShading`, `decorDisplacement`, recol,
params) already decodes in FileStore's `ObjectType` — this file contributes
only variant resolution + footprint logic to the renderer.
