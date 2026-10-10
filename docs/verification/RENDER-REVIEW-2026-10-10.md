# Render review — tiles, walls, decorations, planes, GPU state (2026-10-10)

Scope: source-level review of tile rendering (all shapes), wall merging and normals, wall
decorations (offsets, mirrors), object placement/scale/multi-tile shading, plane visibility, and the
GPU state/shader configuration. Sources:

* **Pinned client** (`melxin/runelite@1ad572d7`, January 2026 deob): placement, lighting, normal
  merge, terrain, scene draw rules. A revision *behind* build 241 for objects/models.
* **`runelite-master`** (October 2026): build-241-accurate GPU plugin (`SceneUploader`, `Zone`,
  `GpuPlugin`, shaders, `TextureManager`, config defaults) and cache loaders.

"Byte-exact data" does not guarantee "visually exact"; this review therefore also covers draw
state, ordering and configuration, not only scene data.

## Method

* Terrain: pinned-client terrain oracle (`tools/deob-harness/run-terrain-oracle.sh`) run on extra
  windows and diffed against `osrs_world::oracle_dump` by
  `crates/osrs-reference/tests/terrain_oracle_diff.rs`, which now accepts `RUSTOSRS_ORACLE_IN` /
  `RUSTOSRS_ORACLE_OUT` to diff any window. The oracle JVM often does not exit after writing its
  (complete) output; kill it.
* Placement / normal merge / lighting: line-by-line reading of `FriendSystem.addObjects`,
  `DynamicObject.method2054/getModel`, `Tiles` placement call, `Scene.newWallDecoration`,
  `WallDecoration.method6262`, `Scene.method5576/5585/5586/5587/draw`, `ModelData.method5262`,
  `ObjectComposition.getEntity/getModelData`, `GrandExchangeOfferUnitPriceComparator.method8811`
  against `osrs-world::loc_stage`, `osrs-scene::normal_finalization`, `osrs-core::normals`,
  `osrs-core::model_construction`, `osrs-core::animation`.
* GPU: `SceneUploader`, `ModelUploader.computeFaceUvs`, `Zone.renderOpaque/renderAlpha/flush`,
  `GpuPlugin` state setup and draw callbacks, `vert.glsl`/`frag.glsl`, `GpuPluginConfig`,
  `TextureManager` against `osrs-render`.
* Experiments on the real cache: `examples/tile_gaps`, `overlay_probe`, `loc_finder`,
  `merge_probe`, `region_sweep`, `animation_bench`.

## Verified equal to the reference

| Area | Evidence |
| --- | --- |
| Terrain loader + builder: every shape/rotation, skipped faces, bridges, min planes, default/noise heights, empty-region fill | Oracle diff passes line for line on Lumbridge (committed fixture), Yanille east coast, Varrock, Falador, Karamja, an underground window and Ardougne (see the window table). |
| Normal-merge traversal (`method5585/5586/5587`) | Plane range `p..p+1`, x-start decrement, corner exclusion, translation formulas, boundary primary/secondary order, floor-decoration neighbourhood, "still `ModelData`" ≙ pending: identical. |
| `ModelData.method5262` | Bounds constants, base-normal contributions, strict match test, `>= 3` matched pairs hide rule: identical (but see *Wall-merge seams*). |
| Instance transforms (mirror, type-4 `256` + `(45,0,-45)`, rotate, recolor, retexture, resize, offset) | Same order as `getModelData`. Mirror = negate z + swap indices 1/3. |
| Placement tables (`Tiles.field800..805`), wall-decoration nudge (`method6262`), type 4/5/6/7/8 offsets | Identical. |
| Loc plane vs collision plane on bridges | Only the collision plane is lowered; the scene plane is not. Matches `loc_stage`. |
| Placement height sampling, shadow (`Tiles_underlays2`) writes, game-object capacity | Identical to `addObjects`. |
| Terrain/model upload: vertex order, UVs, `-1/-2` face colours, bias, alpha split, `computeFaceUvs` | Identical to `SceneUploader`/`ModelUploader`. |
| Pipeline state: reverse-Z, `GREATER`, back-face cull CCW, blend `SRC_ALPHA,1-SRC_ALPHA / ONE,ONE`, `noperspective centroid` HSL | Identical to `GpuPlugin`/`vert.glsl`. |
| Shader maths (hsl→rgb, bias, texture anim, cutout at mip 0, texture lighting `hsl/127`) | Identical to `vert.glsl`/`frag.glsl`. |
| Texture array: 128², 8 mips, nearest mag, `NEAREST_MIPMAP_LINEAR` min (AF default 1), clamp S / repeat T, animation direction→(u,v)·speed | Identical to `TextureManager`. |

## Differences found and fixed

1. **Missing `Scene.method5576`** (retroactive wall-decoration rescale). When a type-0/type-2
   boundary, a type-9 diagonal wall or any type >= 12 object with `decoration_displacement != 16`
   is placed on a tile that already holds a wall decoration, the client re-sets the decoration's
   offsets to `displacement * base / 16`. Not ported; 4-118 decorations per window were
   mis-offset. Fixed (`LocStageStats::decorations_rescaled`).
2. **Wall-decoration slot overwrite.** `newWallDecoration` replaces the tile's previous wall
   decoration; only boundaries and floor decorations were modelled. Fixed.
3. **Alpha pass state.** `Zone.flush` draws alpha with `glDepthMask(false)`; ours wrote depth.
   Alpha is now drawn after all opaque geometry, level by level, farthest zone first, without
   depth writes.
4. **Shading mode.** RuneLite's `smoothBanding` uniform is inverted (`config ? 0 : 1`). Default
   config ("Remove color banding" on) shades with vertex RGB; ours converted interpolated HSL per
   pixel (the banded CPU look). Default now matches; the banded mode is a checkbox.
5. **MSAA.** RuneLite defaults to 2x MSAA; the scene pass is now 4x MSAA resolved into the
   viewport target.
6. **Whole-region build failure.** A non-flat *wall decoration* is a `ModelData` the client never
   reconciles (it draws nothing); we left it "pending", which aborted the region. Now omitted as
   in the client. All 2,934 existing map squares build; missing map groups are "empty".
7. **Plane visibility.** `Scene.draw` iterates every plane from `minPlane` and draws a tile when
   `tile.minPlane <= Scene_plane`; RuneLite uploads `TILE_FLAG_VIS_BELOW` tiles of upper levels
   with level 0. We additionally required `level <= plane`, hiding e.g. the Lumbridge battlements.
   Now only `min_plane <= plane` is tested.
8. **Animated objects** (new): legacy-frame sequences are posed per frame at 50 Hz (5,310 of
   5,418 animated object defs); morph objects show their default (all variables zero) state.
   Deviation: a sequence without a loop-back step is reset by the client after one pass
   (`method8811` flags `1 & 8` → `AnimationSequence.reset`); placed scenery is expected to keep
   cycling, so such sequences (289 defs) restart at frame 0.

## The coastline gaps (user screenshot, Yanille east coast)

Not a renderer bug. The pinned client itself produces skipped faces there: shaped water-overlay
tiles (overlay 442, shapes 1-5) whose underlay id is 0, so the underlay half of the tile has no
colour (`ca=12345678` in the oracle output for tile `0 41 45` of the Yanille window). Our output
matches the oracle line for line, and RuneLite skips those faces too; the clear colour shows
through. RuneLite's default sky colour is black (`client.getSkyboxColor()`, set only by the
optional Skybox plugin, which ships `skybox.txt` region colours). The editor clears to black by
default with a colour picker; porting `skybox.txt` is not done.

## Wall-merge seams (objects 1904 / 1907, Yanille garden wall)

Object 1904 has `translation.y = 1`, 1907 has `0` (same models 599-603; 1907 adds one recolor).
The diagonal wall alternates type 9 (1904) and type 1 (1907) pieces. For the pair at (2585,3085)
type 9 / (2584,3085) type 1 there are 18 x/z-coincident vertices, **0** strict-equal in `y`,
9 within 2 units (`examples/merge_probe.rs`). The January client's condition is strict
(`var11 == var1.verticesY[var14]`), so by that source the pieces do not merge, leaving a lighting
seam and visible coincident end caps. A third-party write-up and a project validated against the
game show no such seam in OSRS, so the deob is probably behind build 241 here.

Decision: opt-in, non-reference `TerrainPresentation::wall_merge_tolerance` (library default 0,
exact client). The editor enables 2 by default ("merge wall seams"; toggling rebuilds loaded
regions). Needs confirming against a 241 client if one becomes available.

Diagonal decorations (shape 8 on diagonal wall 1902): type-9 diagonal walls are *game objects*,
so `getBoundaryObjectTag` is 0 and the displacement is the default `8`; offsets `8 * (±1, ±1)`
and the second entity at orientation `+2` match our plan. The write-up's "plane bias / roof pull"
is not in RuneLite (face bias only; all levels share one depth buffer) and is not applied.

## Diagonal wall decorations vs type-9 diagonal walls (user report: mirrored window)

Earlier analysis of this section was wrong in one respect and is corrected here.

A type-9 wall is a thin **slab** (thickness 16 along the tile diagonal, flat outer face on the
diagonal through the tile centre; only the vertices a face references count, the model also holds
unused vertices). A type-8 decoration has two plates: `orientation + 4` drawn at the offset
`8 * (field803, field805)` and `orientation + 6` drawn at the plain centre. Measured with the real
models (`decor_matrix`, `decor_probe`, `dump_model`):

* When the decoration orientation equals the wall orientation (the usual authoring), the offset
  plate lands flush on the slab's inner face and the plain plate flush on the outer face:
  **the client rule is correct and gives a window visible from both sides.**
* When the orientations differ by 2 (6 of 64 decorations in the Yanille window, e.g. object 1821 at
  (2609..2613, 3076..3080) and torch 1183 at (2596,3085)) the same rule leaves one plate buried in
  the slab and the other floating 11 units off a face — the reported "not flush" case.

An earlier option that zeroed the offsets fixed those 6 but buried the inner plates of the 58
others; it was removed. `TerrainPresentation::flush_diagonal_decorations` now *snaps* each plate
along the slab normal so its base plane coincides with the slab face it faces
(`snap_diagonal_decorations`): identical to the client for the 58 matched cases (checked with
`decor_snap_report`), and corrected for the 6 mismatched ones. It is non-reference (the 241 client
may behave differently: RuneLite's API carries a second offset pair, `getXOffset2/getYOffset2`,
absent in the January deob).

`decor_audit` over region (40,48): straight-wall decorations (types 4/5) are overwhelmingly
flush (50 flush / 6 embedded / 1 floating for type 5), so the straight path matches the client.

### Recessed decoration plates hidden by the depth buffer (black arrow-slit crosses)

Diagonal (`256`-flag) wall-decoration plates are produced by an integer 45-degree rotation plus a
`(45, 0, -45)` shift, which leaves some plates (all that were measured) up to 0.7 units *behind*
the wall face they sit on (decoration 1938 on wall 17088: black slot faces at n in [-0.7, 7.8]).
The CPU client paints decorations over their wall regardless of depth, so the black slot shows;
a depth-tested renderer hides the part behind the face (the slot filled with brick texture), and
the straight-wall path avoids it only through the `+-1` nudge, which diagonal flags (256) do not
have. `snap_diagonal_decorations` now also lifts every diagonal plate 1 unit along its outward
normal (and keeps the client's offsets when the host is not a type-9 wall). With it the black
slots render. Non-reference (option "wall fixes").

Same section: wall 17088 carries `decoration_displacement = 32`, so `Scene.method5576` doubles the
offsets of decorations placed before it (`(-8,-8)` becomes `(-16,-16)`), which matches the thick
(32-unit) slab of that wall family.

Cross-check against the 317-based client at `~/Desktop/ub3r-monorepo/game-client`: it stores one
renderable per wall decoration and bakes the offsets into the draw call (`53 = 45 + 8` for the
`256` flag, `45` for `512`, type 8 = `768` chooses the flag by camera side), which is the same
geometry the 241 model-based path produces (`(45, 0, -45)` plus `8 * field`). It uses a fixed
displacement of 8 for types 6-8 and never rescales existing decorations.

Lighting was re-checked against `ModelData.toModel`/`calculateVertexNormals`: the face-normal
sign, the `/ (var7 * magnitude) + ambient` form, light vector `(-50, -10, -50)`, flat vs smooth
faces and the `2..126` lightness clamp are identical in `osrs-core::normals`/`lighting`. Objects
with `ambient = 0, contrast = 0` (for example the bay window 1852) are lit with
`ambient + 64` / `contrast + 768`, which makes east-facing faces nearly black by design.

### Terrain tile steps ("tiles show up strongly")

`examples/seam_stats` over the Lumbridge window: of 15,426 vertices shared by two flat untextured
tiles, 36% differ in hue, 10% in saturation and 28% in lightness (worst 29/128) between the two
tiles. This is client data, not a renderer error: each tile gets its own blended hue/saturation
(radius-5 underlay average) and per-corner lightness plus the object shadow grid, and the terrain
oracle matches line for line. Option `TerrainPresentation::smooth_terrain` (editor checkbox,
default off, non-reference) averages colours at shared vertices; the grass blotches soften.

Picker fix: models may carry hidden (`-2` / render type 2) faces used for texture axes (wall 997
has one reaching 80 units past the tile); they inflated pick boxes (so a wall could win a hover
meant for a booth) and outlines. Bounds and outlines now skip them, and outlines of morph
objects (bank booth 10356) use the default-state definition (they were empty).

Picker bounds now ignore unused model vertices; selection/hover highlight draws the picked
model's own triangles (ghosted fill + edges), rebuilt on the worker from the definition.

The RuneLite commit "cache: rev 241" (87616aa) only adds loader changes (object opcode 42
`fullRecolor`, opcode reorder, item/npc/spotanim fields); `runelite-master` already contains it.

## RuneLite / client behaviour we know about but do not reproduce yet

* **Alpha ordering.** `Zone.renderAlpha` sorts alpha models by squared distance (far first) and,
  for zones within 2048 units of the camera, counting-sorts each model's alpha faces back to front
  by view depth (`ALPHA_ZSORT_CLOSE`); farther zones use unsorted ranges. Alpha is drawn per level
  after all opaque geometry. We only order zones.
* **Hidden roofs.** RuneLite assigns roof ids per tile (`Scene.getRoofs`), keeps per-roof draw
  ranges, and skips hidden roof ids above the current level (roof-removal). We have no roof ids.
* **Draw order.** Opaque order matters only for exact depth ties (first drawn wins under
  `GREATER`) and alpha. RuneLite emits per tile (paint, model, wall, decorative, ground, game
  objects, then the bridge tile) in x-then-z order per level; we emit a zone's terrain first and
  then its locs.
* **Software-client wall decoration culling.** The CPU client draws a decoration only when its
  orientation flag faces the camera, and for the `256` forms draws *one* of the two renderables
  chosen by camera quadrant; RuneLite's GPU draws both and relies on back-face culling (ours too).
  `getXOffset2/getYOffset2` exist in the RuneLite API; the deob draws the second renderable at the
  plain centre (offset 0), which is what we do.
* **Dynamic objects** use two `AnimationSequence`s (primary, and a carried-over secondary used
  while the primary is in its first loops); object replacement carries state. Not ported (we play
  one sequence). `randomizeAnimStart` (frame/cycle randomised at construction) and
  `deferAnimChange` object flags are not applied. Dynamic models use the static face order, not
  `uploadSortedModel`'s priority path. Skeletal sequences (108 objects) render static.
* **Fog / draw distance / expanded map loading.** RuneLite default fog depth is 0 (off); draw
  distance 50 tiles, 3 extra chunks. Not implemented (streaming radius replaces them).
* **Skybox plugin** (see above); **anisotropic filtering** levels >1; **colour-blind modes**;
  **brightTextures** option (`textureLightMode`).

## Open data questions

* `ground_raise` (object opcode 96) and `full_recolor` (opcode 42): `runelite-master` only stores
  them; nothing in the repo consumes them. Semantics unverified.
* No whole-scene oracle for objects: the January deob cannot decode build-241 object definitions
  (62,384 decode but 57,661 leave trailing bytes), so placement/normal/lighting parity for objects
  rests on source reading plus the opt-in seam tolerance above.
* Overlay/underlay data holes (e.g. overlay 442 shore tiles) are authentic client output.

## Work log: terrain oracle windows

Windows are regenerated with `export_terrain_oracle_input` + `run-terrain-oracle.sh`; the
committed fixture is unchanged. "Pass" = every height, setting, paint/model, face colour, min
plane and link-below line equal to the pinned client. (The underground and Ardougne windows are
sparse, so the test's "fixture should exercise a real scene" size assertion — which runs *after*
the line-for-line comparison — reports failure; the comparison itself passed.)

| Window (scene base) | Result |
| --- | --- |
| Lumbridge (3176,3176), committed | pass |
| Yanille east coast (2592,3048) | pass |
| Varrock (3112,3368) | pass |
| Falador (2920,3304) | pass |
| Karamja (2792,3112) | pass |
| Underground (3168,9536) | pass (comparison); size assertion only |
| Ardougne (2560,3248) | pass (comparison); size assertion only |
