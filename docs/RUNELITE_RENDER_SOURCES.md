# RuneLite Render Sources — Complete File List for Full Map-Scene Rendering

Scope: everything needed for **full rendering fidelity** in the Rust map editor
(`osrs-map-editor` workspace: `editor_core` + `editor_render` + `editor_app`),
hyper-focused on **map scenes / terrain / 3D objects**. Raw cache decoding
(`m_x_y`, `l_x_y`, `flo`/`flu`, models, textures) is **excluded** — that is
`OpenRune-FileStore`'s job (§8 maps exactly what it already covers, so nothing
gets reimplemented). Lost? See **`index.md`** (navigation hub + rule index).

## 0. Source pins (winner — no comparison, just what to use)

- **Primary: `/Users/tylercovalt/Documents/runelite-master`** (snapshot 4 Oct 2026,
  newest). All `runelite-api` + `runelite-client/plugins/gpu` paths below refer to
  this tree unless marked `[melxin]`.
- **Secondary: `/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin`**
  (Jan 2026). Used ONLY for: (a) the 9 compute-shader files missing from the
  primary's disk, (b) deob construction tables (Group E — the primary ships no
  `deobfuscator/` or `runescape-client/` modules at all), (c) 5 small editor
  affordances marked `[melxin]` below.
- **Staged shaders: `reference-shaders/runelite-gpu/`** (workspace root, 22 files).
  Master's 13 + melxin's 9, plus `regions.txt` (master's newer 465-line copy).
  Port WGSL from there, not from the source trees. See its `README.md`.
- **Port companion: `RUNELITE_RUST_PORT_NOTES.md`** — crate map, byte-exact
  Rust vertex structs, GL→wgpu state table, camera spec, orientation rules,
  app shell, defaults, constants index.
- **Deep dives:** `RUNELITE_GPU_PIPELINE.md` (Groups A+B internals),
  `RUNELITE_SCENE_AND_MATERIALS.md` (Groups C+D contracts),
  `RUNELITE_RUNTIME_RULES.md` (R1–R22 unwritten engine rules),
  `RUNELITE_DEOB_READING_GUIDE.md` (deob pins + cleaned pseudocode),
  `RUNELITE_RUST_PORT_NOTES.md` (Rust/wgpu/egui mappings).
- **Hard parts first:** `RUNELITE_HARDEST_PARTS.md` — the 10 riskiest port
  items ranked, with the research to do before renderer code (includes newly
  extracted lighting-bake and camera-projection math).
- **Cache decoders:** `RUNELITE_CACHE_STACK.md` — FileStore-vs-rs-cache
  coverage, object opcode tables, render-relevant defaults, and the hybrid
  verdict (depend on rs-cache, port the gaps).
- **Live reference: `tools/runelite-mcp/`** — stdlib-only MCP server over the
  API sources, shaders, guides, and allowlisted Java/deob files (see its
  `README.md`).
- **Executed deob: `tools/deob-harness/`** — compiles the deob sources
  headless and runs `Dumper` to produce `reference-fixtures/deob_golden.txt`
  (tables, 52 triangulations, color-function sweeps, contour + lighting
  fixtures — all machine-checked). Re-run `run.sh` after snapshot updates and
  diff.
- **Generated API pages: `docs/api/`** — one MD per needed class (49 classes +
  index), emitted by `tools/runelite-mcp/gen_api_docs.py` from the same parser
  the MCP server uses. Re-run after updating the snapshot.

> Name corrections vs older notes: `TilePaint`/`TileModel` do not exist — the real
> files are `SceneTilePaint`/`SceneTileModel`. There is no `comp_hps.glsl` and no
> `geom.glsl` in either tree — do not port ghost files.

---

## Group A — Scene geometry & GPU upload (Java)

Path: `runelite-client/src/main/java/net/runelite/client/plugins/gpu/`

### A1. `SceneUploader.java` (692 L) — zone traversal + static upload
- **Extract:** `zoneSize()` (opaque/alpha face counting incl. `bridge` recursion),
  `uploadZone()` (roof-ID bucketing; level 0 does the `VIS_BELOW` double-pass over
  levels 0–3), `uploadZoneLevel()` + `uploadZoneLevelRoof()` (bridge test
  `(settings[1][msx][msz] & TILE_FLAG_BRIDGE) != 0 → maplevel++`,
  `SCENE_OFFSET` shift iff `WorldView.TOPLEVEL`), `uploadZoneTile()` (**order is
  load-bearing**: paint → model → wall 2 slots → decor 2 slots with
  `xOffset/yOffset` → ground → gameobjects filtered to `min == tile` → bridge
  recursion), `uploadStaticModel()` (JAU rotate via `Perspective.SINE/COSINE`,
  `put22224`/`put2222` packing, `alphaBias = transparency<<24 | bias<<16`,
  `texture+1`), paint quad split (2 tris, UV `(0,0)–(256,256)`), `12345678` skip
  sentinel, `RenderCallbackManager.drawTile/drawObject` gates.
- **Artifact prevented:** inverted winding, missing T-junction second slots,
  floating bridges, roof leaks, decor offsets dropped, alpha faces in the opaque
  buffer, multi-tile objects uploaded N times.
- **Rust:** `editor_render::scene_upload`

### A2. `ModelUploader.java` (829 L) — CPU priority sort + dynamic models
- **Extract:** `uploadSortedModel(rt, proj, model, orient, x,y,z, cameraX,Y,Z, opaque, alpha, prioritySort)` —
  `zsortHead/Tail/Next` depth buckets, `MAX_DIAMETER 6000` guard,
  `orderedFaces[12][4000]` + `avg12/avg34/avg68` interleave (pri-10/11 injected at
  pri 0/3/5 boundaries), backface cull
  `(aX-bX)*(cY-bY)-(cX-bX)*(aY-bY) > 0`; `uploadTempModel()` (animated/dirty-zone
  path); `computeFaceUvs()` (tangent/bitangent/normal + camera-ray projection when
  `project=true`, 0/1 fast path otherwise); `interpolateHSL()` override (skipped
  for textured faces); `faceTransparency(modelTransparency, faceT)` — needs
  `Model.getTransparency()`.
- **Artifact prevented:** priority flicker (rugs/posters/railings), glass/water
  drawn behind opaque, backface leaks, UV swimming on rotated models, dropped
  whole-model alpha.
- **Rust:** `editor_render::priority_sort` (per-thread instance + camera + sort
  flag, matching the §A5 thread model)

### A3. `Zone.java` (777 L) — editor chunk unit
- **Extract:** `VERT_SIZE 20` staging layout (`short vec4 pos + int abhsl + short
  vec4(id,u,v,0)` — note attr0 is **vec4**, not vec3), `float[]` model verts,
  `sizeO/sizeA`, `vboO/vboA`, `levelOffsets[4]`, `rids/roofStart/roofEnd`,
  `alphaModels` + `addTempAlphaModel`, `convertForDraw()` staging→`VAO` step,
  dirty/invalidate/cull flags, `synchronized modelCache` (→ `Mutex` in Rust),
  `renderAlpha(mu, …)` uploader-param form.
- **Artifact prevented:** full-scene rebuild per brush stroke, roof toggles
  destroying buffers, lost alpha models, data races under render threads.
- **Rust:** `editor_render::zone`

### A4. `RegionManager.java` + `regions/Region.java` + `regions/Regions.java` + `regions/regions.txt`
- **Extract:** `Regions` parses `regions.txt` into a region-ID lookup;
  `RegionManager.prepare(scene)` deletes every 8×8 chunk whose region ID differs
  from the center region — but only when `hideUnrelatedMaps` is on and the scene
  is not an instance. There is **no** dirty queue and no
  `expandedMapLoadingZones` logic here; dirty tracking lives on `Zone`
  (`dirty`/`invalidate`/`cull`).
- **Artifact prevented:** foreign-region geometry (e.g. opposite sides of a
  border) rendering inside the viewport; wrong world→zone lookup.
- **Rust:** `editor_render::region_manager` (ID lookup + hide-unrelated cull)

### A5. `GpuPlugin.java` (2220 L) — renderer backbone
- **Extract:** `RenderThread{rts}` pool (per-thread `VAOList`, `tmp[3]`,
  `ModelUploader`), reverse-Z setup (`glClipControl(LOWER_LEFT, ZERO_TO_ONE)`,
  `glDepthFunc(GREATER)`, `glClearDepth(0)`, `GL_DEPTH_COMPONENT32F` depth
  buffer — wgpu mirror: `Greater`, clear `0.0`, `Depth32Float`, reversed
  projection), `SceneContext` per scene (`zones` grid, `projection`,
  camera, `minLevel/level/maxLevel`, `hideRoofIds`), `setupGpuFlags()`
  (`GPU | ZBUF | RENDER_THREADS(n)` packed via the `RENDER_THREADS()` macro `|
  NO_VERTEX_SNAPPING?`), `numThreads` rebuild path, `drawPass()` three-pass
  structure (`PASS_OPAQUE` draws `vaoO` per thread; `PRE_PASS_ALPHA` unmaps
  `vaoA` + sets `entityProj`/`entityTint` from scene overrides; `PASS_ALPHA`
  drops temp models per zone), `useStaticUnsorted` fast path (skipped when the
  scene has HSL overrides), camera view/proj assembly,
  `wvid == WorldView.TOPLEVEL` test, draw-distance box + `setMinLevel`,
  `uniBase`/`uniEntityProj`, `uniColorblindIntensity`,
  `textureAnimations` tick array build (`uv += tick*anim/128`),
  fog uniforms (`uniUseFog/uniFogColor/uniFogDepth`), `smoothBanding` +
  `textureLightMode` toggles, `expandedMapLoadingChunks` uniform. Tile heights
  reach the pipeline through `Scene`/`WorldView` accessors (consumed in `Zone`
  setup and `SceneUploader`), not through a `GpuPlugin` fetch.
- **Frame inputs the plugin does NOT compute** (arrive via `preSceneDraw` from
  client logic — the editor implements them): `minLevel/level/maxLevel` plane
  window (levels outside are skipped in `renderOpaque/renderAlpha`),
  `hideRoofIds` (roof ranges in the set are skipped; sourced from
  `ROOF_FLAG_POSITION/HOVERED/DESTINATION/BETWEEN` mode + player/hovered
  position + `overrides.jsonc` region volumes), camera floats. Dynamic objects
  additionally clamp `plane = min(maxLevel, tileObject.getPlane())` before
  upload — replicate for editor previews.
- **Artifact prevented:** stalls (no threads), depth collapse, edge warp, gamma
  banding, frozen water (tick never fed), fog popping.
- **Rust:** `editor_render::renderer` + `editor_core::camera` (design the thread
  pool + per-thread staging now, even starting with 1 thread)

### A6. `GpuIntBuffer.java` + `GpuFloatBuffer.java` + `GLBuffer.java` + `VBO.java` + `VAO.java`
- **Extract:** `put22224`/`put2222`/`putfff4` packing, **`VAO.VERT_SIZE 24`
  (`float vec3 + int abhsl + short vec4`)**, `FACE_SIZE = (VERT_SIZE>>2)*3`,
  stride/offsets, `GLBuffer` orphaning, `VAO.Range{endpos, projection float[],
  h,s,l,a, renderMethod}` batching + `addRange(projection, scene, renderMode)`
  merging (carries entity HSL overrides + render-mode batches).
- **Artifact prevented:** attribute bleeding (UVs as colors, normals as positions
  → mesh spikes), broken entity tint batches.
- **Rust:** `editor_render::buffers` (bytemuck + wgpu `VertexBufferLayout`)

### A7. `Mat4.java` + `Shader.java` + `template/Template.java`
- **Extract:** `Mat4` (`identity/scale/translate/rotateX/rotateY/projection/mul`),
  `Shader` (`add()` units + `compile()`/link), `Template.process()`
  `#include "file"` expansion (how generated `texture_config`
  (`TEXTURE_COUNT`), `sampling_mode` (`uiScalingMode`), `colorblind_mode` and
  `hsl_to_rgb` get into `vert`/`frag`).
- **Artifact prevented:** MVP mismatch between picking and drawing, silent shader
  failures, missing generated snippets (`TEXTURE_COUNT`, colorblind mode).
- **Rust:** `editor_render::shaders` (glam + wgpu pipeline + a tiny `#include`
  preprocessor mirroring `Template`)

### A8. `TextureManager.java` (269 L)
- **Extract:** texture-array build, palette scaling/brightness, `getTextureProvider()`
  binding, `computeTextureAnimations()` per-tick U-vs-V scroll vectors.
- **Artifact prevented:** frozen water/lava, reversed scroll, washed-out sampling.
- **Rust:** `editor_render::textures`

### A9. `GpuPluginConfig.java` + `config/AntiAliasingMode.java` + `config/UIScalingMode.java` + `config/ColorBlindMode.java`
- Full viewport feature list to port as editor settings: `drawDistance`,
  `hideUnrelatedMaps`, `expandedMapLoadingZones`, `smoothBanding`,
  `antiAliasingMode`, `uiScalingMode`, `fogDepth`, `anisotropicFilteringLevel`,
  `colorBlindMode`, **`colorBlindIntensity`** (default 100), `brightTextures`
  (→`textureLightMode`), `unlockFps`/`vsyncMode`/`fpsTarget`,
  `removeVertexSnapping`, **`numThreads`** (default 3, max 15).
- **Rust:** `editor_app::settings`

### A10. `callback/RenderCallbackManager.java` + `callback/RenderCallback.java` (+ `hooks/DrawCallbacks.java` in `runelite-api`)
- **Extract:** `drawTile(scene, tile)` / `drawObject(scene, obj)` gates called by
  `SceneUploader`; terrain hooks `drawScenePaint` / `drawSceneTileModel`;
  frame hooks `drawScene` / `postDrawScene` / `preSceneDraw` / `postSceneDraw`;
  `drawPass()` with passes (`PASS_OPAQUE/PASS_ALPHA/PRE_PASS_ALPHA`); per-zone
  `drawZoneOpaque(entityProj, scene, zx, zz)` /
  `drawZoneAlpha(entityProj, scene, level, zx, zz)`; `drawDynamic(...)` for
  per-frame models (declared on the interface, answered by `GpuPlugin`); `animate(texture, diff)` tick hook; frustum hooks
  `tileInFrustum(...)` / `zoneInFrustum(...)` (the latter gated by
  `ZBUF_ZONE_FRUSTUM_CHECK`); lifecycle `loadScene` (×2) / `swapScene` /
  `despawnWorldView`.
- Flags: `GPU | HILLSKEW | NORMALS | NO_VERTEX_SNAPPING | ZBUF |
  ZBUF_ZONE_FRUSTUM_CHECK | UNLIT_FACE_COLORS | RENDER_THREADS(n)` (count packed
  via macro). Note: `GpuPlugin` sets only `GPU|ZBUF|RENDER_THREADS|
  NO_VERTEX_SNAPPING?` — it never sets `HILLSKEW`/`NORMALS`, so
  `Model.getUnskewedModel()`, `getVertexNormals*()`, and `getUnlitFaceColors()`
  have no backing on this path: compute normals yourself at build
  (`calculateVertexNormals` equivalent) and skip unskew handling unless
  hillskew artifacts appear.
- Editor stubs these (always-draw + roof/plane filters); keep the interface so
  future plugins/filters plug in without touching upload code. The frustum
  hooks are the natural home for editor zone culling.
- **Rust:** `editor_render::callbacks` (trait with a pass enum + frustum hooks)

---

## Group B — Shaders (staged in `reference-shaders/runelite-gpu/`)

Live path: `vert` + `frag` (+ CPU sorter A2). The `comp*`/`common*`/`cl_types`
files are **priority-math documentation, not compilable units** — verified: they
`#include "to_screen.glsl"` and `"comp_common.glsl"`, which exist on disk in
**neither** tree, and no Java code loads any `comp*` file by name in either tree.
Port the math (`priority_map`, `count_prio_offset`, light model), not the files.

### B1. `vert.glsl` (129 L) — per-vertex transform + fog + UV scroll
- Includes: generated `texture_config` (`TEXTURE_COUNT`), `hsl_to_rgb.glsl`.
  Uniforms: `worldProj`, `entityProj`, `entityTint`, `brightness`, `useFog`,
  `fogDepth`, `drawDistance`, `expandedMapLoadingChunks`, `base`, `tick`,
  `textureAnimations[TEXTURE_COUNT]`.
- Fog is **per-vertex**: scene-edge clamp
  (`FOG_SCENE_EDGE_MIN/MAX` from expanded chunks) intersected with the
  draw-distance box, with `FOG_CORNER_ROUNDING` corner correction →
  `fogFactorLinear(dist, 0, fogDepth*TILE_SIZE)` → `fFogAmount`.
  Depth micro-offset `screenPos.z += bias/128.0` (verified `vert.glsl:96`) keeps
  coplanar priorities apart inside the reverse-Z buffer — the bias byte is
  authored per face in cache (see R21), not computed. UVs scroll by `tick` here.
- **Artifact prevented:** lighting stretch on sheared quads, fog popping at chunk
  borders, frozen/scrolling-wrong water.

### B2. `frag.glsl` (115 L) — lighting mix + alpha test + fog + colorblind
- Includes: generated `colorblind_mode`, `hsl_to_rgb.glsl`, `colorblind.glsl`.
  Uniforms: `textures` (`sampler2DArray`), `brightness`, `smoothBanding`,
  `fogColor`, `textureLightMode`.
- Order: alpha discard (`textureColor0.a < 1`) → `textureLightMode` mix
  (`brightTextures`) → `smoothBanding` mix — note the uniform carries the
  **negation** (`smoothBanding() ? 0 : 1`): 0 → interpolated vertex RGB,
  1 → per-pixel `hslToRgb` → `mix(color, fogColor, fFogAmount)` →
  colorblind correction × `colorblindIntensity/100`.
- **Artifact prevented:** black halos on fences/vegetation, washed-out textures,
  banding, wrong fog color, unscaled colorblind correction.

### B3. `hsl_to_rgb.glsl` (84 L) — canonical color conversion
- `hslToRgb(vec3)`: `hue = x/64 + 1/128`, `sat = y/8 + 1/16`, `lum = z/128` curve.
  The ONLY correct OSRS HSL math — `GpuPlugin` delegates here.
- **Artifact prevented:** banding / dark-grid seams from ad-hoc sRGB conversion.

### B4. `priority_render.glsl` (+ `.cl`) — 12→18 bucket painter's algorithm
- **Extract:** `priority_map(p, distance, min10, avg1..3)` (adjusted 0–17 buckets;
  pri-10/11 placement via distance averages — mirrors A2's
  `avg12/avg34/avg68`), `count_prio_offset()` (draw-order offsets).
- **Artifact prevented:** intra-mesh Z-fighting (hair vs helmet, roof vs beams).
- Port as the documented contract next to A2; the `.cl` twin pins work-group
  assumptions if a compute path is ever revived.

### B5. `comp.glsl` / `comp_unordered.glsl` (+ `.cl`) — opaque / alpha compute passes
- `comp`: `totalNum[12]`/`totalDistance[12]` face census, `renderPris[]` draw
  order, `#include "priority_render.glsl"`, `map_face_priority()` per face.
  `comp_unordered`: the split alpha/translucent path — never merge the two.
- **Artifact prevented:** (when revived) translucent sorting inversion (glass, ice,
  spell FX, water behind opaque).
- Today: read for the light-model + census formulas; the live equivalent is A2.

### B6. `common.glsl` / `common.cl` + `cl_types.cl` — shared math reference
- `rotate_vertex` (JAU orientation), camera-distance fn, `uniform` struct
  (`cameraYaw/Pitch`, `centerX/Y`, …). The `abhsl = alpha<<24|bias<<16|hsl`
  packing + `bias/128` depth rule live with A1/A6 — keep them consistent.
- **Artifact prevented:** CPU/GPU packing drift.

### B7. `colorblind.glsl` (52 L) — correction × intensity
- Use the staged 52-line copy (has the `colorblindIntensity` uniform; older tree
  has 48 without it).
- **Rust:** uniform 0–100 slider.

### B8. `vertui.glsl` / `fragui.glsl` — UI overlay program
- Needed only if the editor draws overlays through the same pipeline; otherwise
  egui handles UI. Pin, port last.

### B9. `scale/*` (bicubic, hybrid, xbr_lv2_*) — upscaling filters. Out of scope
for the viewport (egui/wgpu swapchain handles scaling). Do not port.

---

## Group C — Scene structural contracts (`runelite-api/...`)

### C1. `Scene.java` — scene root
- `getTiles[4][104][104]` vs `getExtendedTiles` (184), `getExtendedTileSettings`,
  `getTileHeights`, `getTileShapes`, `getUnderlayIds/getOverlayIds` (`id+1`,
  0 = none), `getRoofs` + `buildRoofs()` + `setRoofRemovalMode`,
  `getWorldViewId`, `getMinLevel/setMinLevel`, `getDrawDistance`,
  `getBaseX/getBaseY`, `isInstance/getInstanceTemplateChunks`,
  `getMapRegions()` (loaded-region scope for fetch + blend borders),
  `removeTile`, `removeGameObject(GameObject)`, **`getSkybox()`** (preview backdrop source),
  `setDrawDistance()` (viewport culling distance). Height grids run
  size+1 per axis (NE corners read `[x+1][y+1]`).
- **Artifact prevented:** every structural bug (bridge/vis-below/roof handling in
  A1 reads from here).
- **Rust:** `editor_core::scene`

### C2. `Tile.java` — per-tile slots + bridge contract
- `getBridge/getRenderLevel/getPlane` (plane-1 bridge renders in the plane-0
  pass, heights stay plane-1), `getSceneLocation/getLocalLocation/
  getWorldLocation`, `getSceneTilePaint/getSceneTileModel`,
  `getWallObject/getDecorativeObject/getGroundObject/getGameObjects/
  getItemLayer/getGroundItems`, plus **setters the master already ships**:
  `setGroundObject/setSceneTilePaint/setSceneTileModel` (mutate terrain and
  ground layer in place — brush tools use these).
- `[melxin]` extras (absent upstream): `setDecorativeObject/setWallObject`,
  `getPhysicalLevel()` (roof min-plane).
- **Artifact prevented:** floating-bridge disconnect.
- **Rust:** `editor_core::tile`

### C3. `SceneTilePaint.java` — flat underlay quads
- `getSw/Se/Ne/NwColor`, `getTexture`, `getRBG` (the tile's minimap color —
  palette lookup of the jittered underlay HSL at fixed brightness 96;
  `drawTileMinimap` paints it directly), `isFlat`, buffer offsets.
- **Artifact prevented:** diagonal seam cracks from inconsistent quad splits.
- **Rust:** `editor_render::terrain`

### C4. `SceneTileModel.java` — shaped overlay cuts (shapes 0–12)
- `getModelUnderlay/getModelOverlay`, `getShape/getRotation`, `getFaceX/Y/Z`,
  `getVertexX/Y/Z`, `getTriangleColorA/B/C`, `getTriangleTextureId`, `isFlat`.
- `[melxin]` cherry-picks (absent upstream, needed for a paint brush):
  `get/setUnderlaySw/Se/Ne/NwColor` + `get/setOverlaySw/Se/Ne/NwColor`.
- **Artifact prevented:** square paths/shores/rivers.
- **Rust:** `editor_render::terrain`

### C5. `WallObject.java` — dual-slot walls
- `getRenderable1/getRenderable2`, `getOrientationA/getOrientationB` (both models
  instanced on the same tile; never overwrite slot 1).
- `[melxin]`: `getModelA()/getModelB()` if direct model access is wanted.
- **Artifact prevented:** missing T-junctions / inverted corner caps.
- **Rust:** `editor_core::placement`

### C6. `DecorativeObject.java` — wall decor insets
- `getRenderable/getRenderable2`, `getXOffset/getYOffset/getXOffset2/getYOffset2`
  (inset ≈ 16, `ObjectType.decorDisplacement`), orientation from `getConfig`
  bits (`>>>6&3`, same layout as `GameObject`). Rule: `pos += wallNormal × inset`.
- **Artifact prevented:** floating torches / embedded banners.
- **Rust:** `editor_core::placement`

### C7. `GroundObject.java` — floor decals
- `getRenderable`, placed at the sampled tile height with no uploader-added bias
  (neither `SceneUploader` nor the scene builder offsets it) — coplanar
  separation comes from the authored per-face bias byte (`z += bias/128` under
  reverse-Z `GREATER`; see R21) plus CPU bucket order, not epsilon. If shimmer
  appears on a specific decal, bias the preview ghost, not the stored height.
- **Artifact prevented:** coplanar Z-fighting (flowers, cracks, rugs).
- **Rust:** `editor_core::placement`

### C8. `GameObject.java` — multi-tile objects
- `getRenderable`, `getModelOrientation` (JAU — typically **0** for statics
  because models are pre-rotated at build per R9, so the uploader's draw-time
  rotation is a no-op; nonzero only for actors/poses — do not apply both),
  `getOrientation` (loc orientation 0–3), `getConfig`
  (`type = bits&31`, `orient = bits>>>6&3`), `getSceneMinLocation/
  getSceneMaxLocation`, `sizeX/sizeY`, rotation pivot (upload only where
  `min == tile`).
- **Artifact prevented:** 1×2/2×3 objects rotating off-center into wrong tiles.
- **Rust:** `editor_core::placement`

### C9. `TileObject.java` + `Renderable.java` + `DynamicObject.java` + `ItemLayer.java` + `EntityOps.java`
- Base `getId/getX/getY/getZ/getPlane`; `DynamicObject.getModelZbuf()` (animated
  locs — include so doors/ladders never vanish; `[melxin]` adds
  `getAnimationID()` for anim preview); `ItemLayer` stacks (hideable, must not
  crash); `EntityOps` (current menu-op contract from `ObjectComposition.getOps`).
- **Rust:** `editor_core::objects`

### C10. `WorldView.java` — instance vs overworld coordinates
- `TOPLEVEL = 0`, `isTopLevel()`, `getScene()`, `getTileSettings()`,
  `getSizeX/getSizeY`, `getBaseX/getBaseY`, `getPlane()`,
  `SCENE_OFFSET = 40` tiles (`(184−104)/2`; zone shift `>>3` = 5 zones) with
  `base=(mzx−offset)<<10`,
  **`getTileHeight(x, y, maplevel)`** (height queries for picking/placement),
  **`getMainWorldProjection()/getCanvasProjection()`** (the two projections
  the editor viewport needs — world drawing vs canvas overlays),
  `getCollisionMaps()`, `players/npcs/worldEntities`, `getSelectedSceneTile()`
  (hover state for the inspector).
  XTEA keys: implement via FileStore's `OpenRS2.downloadKeysByRevision` (note:
  rev 237+ keys are largely unarchived — encrypted squares need out-of-band keys
  or stay black; data problem, not render code).
- **Artifact prevented:** 40-tile coordinate desync between instance and
  overworld views.
- **Rust:** `editor_core::coords`

---

## Group D — Spatial math & materials (`runelite-api/...`)

### D1. `Perspective.java` — tile math + trig
- `LOCAL_TILE_SIZE 128`, public `SINE/COSINE[2048]` + `SINEF/COSINEF`,
  **`SINE14/COSINE14/SINEF14/COSINEF14[0x4000]`** + `UNIT14` (14-bit rotation
  precision), `ESCENE_OFFSET`, height bilinear interpolation
  (`getTileHeight`, `getFootprintTileHeight`), canvas projection with
  CPU/GPU variants (`localToCanvas*`, `modelToCanvas*` incl. `*Gpu`/`*Cpu` and
  `modelToCanvasProjection`), picking helpers (`getClickbox`,
  `calculateAABB`/`calculate2DBounds`, `getCanvasTilePoly/AreaPoly`).
- **Artifact prevented:** floating/sunken scenery, misaligned tile picking.
- **Rust:** `editor_core::math`

### D2. `Constants.java` — coordinate/flag truth
- `CHUNK_SIZE 8`, `REGION_SIZE 64`, `SCENE_SIZE 104`, `EXTENDED_SCENE_SIZE 184`,
  `TILE_FLAG_BRIDGE 0x2`, `TILE_FLAG_UNDER_ROOF 0x4`, `TILE_FLAG_VIS_BELOW 0x8`,
  `ROOF_FLAG_*`, tick lengths, `>>7`/`>>6` shifts.
- **Artifact prevented:** FileStore↔wgpu coordinate desync.
- **Rust:** `editor_core::consts`

### D3. `JagexColor.java` — HSL truth
- `packHSL/unpackHue/unpackSaturation/unpackLuminance`,
  `rgbToHSL(rgb, brightness)` gamma curve (round-trips through B3 exactly).
- **Artifact prevented:** wrong swatches, underlay-blend drift.
- **Rust:** `editor_core::color`

### D4. `Model.java` + `ModelData.java` + `Mesh.java` — mesh truth
- `Mesh` (which `Model` extends): `getVerticesX/Y/Z` as **`float[]`** (+
  `getVerticesCount`), `getFaceIndices1/2/3`, `getFaceTransparencies`,
  `getFaceTextures`, `getFaceCount`. `Model` adds: `getFaceColors1/2/3`,
  `getFaceRenderPriorities[0–11]`,   `getFaceBias`, `getTextureFaces/
  getTexIndices1/2/3`, `getDiameter/getRadius`, `getTransparency()`
  (whole-model alpha), `getOverride*` (skipped for textured faces),
  `getVertexNormals*`, `getUnlitFaceColors()`. (`getUnskewedModel()` exists on
  the API but nothing on the GPU path calls it — skip unless hillskew artifacts
  appear.) `ModelData` adds: `cloneVertices/cloneColors/cloneTextures/
  cloneTransparencies`, `calculateVertexNormals`, `recolor/retexture/resize/
  changeOffset`, `toModel`. Buffer-upload bookkeeping lives on the objects
  themselves (`getBufferOffset/setBufferOffset`, `getUvBufferOffset`,
  `getBufferLen` on `Model`/`SceneTilePaint`/`SceneTileModel`) — the uploader
  writes these so draws can address ranges; the editor's buffers need the same
  fields. (`drawFrustum`/`drawOrtho`/AABB helpers are debug-only.)
- `contourGround()` is **not** on the RuneLite API — implement it from the deob
  pseudocode (see E5).
- Normals stay per-model (no cross-model welding — the joint crease is authentic;
  see `RUNELITE_RUNTIME_RULES.md` R17, which corrects earlier welding claims).
- Normals stay per-model (no cross-model welding — the joint crease is authentic;
  see `RUNELITE_RUNTIME_RULES.md` R17, which corrects earlier welding claims).
- **Artifact prevented:** lighting seams, ignored priorities, tinted textures,
  dropped translucency.
- **Rust:** `editor_render::mesh`

### D5. `Texture.java` + `TextureProvider.java` — materials
- `Texture` exposes only `getAnimationDirection()` (U vs V axis) +
  `getAnimationSpeed()`; `TextureProvider` adds `getBrightness()`,
  `getTextures()`, `getDefaultColor()`. Tinting does not live here — it comes
  from scene HSL overrides (`entityTint`), `textureLightMode`, and palette
  scaling in `TextureManager`. Runtime rule `uv += tick × speed / 128`
  (`TEXTURE_ANIM_UNIT` in `vert.glsl`).
- **Artifact prevented:** reversed/too-fast scroll, grayscale materials, frozen
  water/lava.
- **Rust:** `editor_render::textures`

### D6. `ObjectComposition.java` — object definitions (render-relevant subset)
- `getId/getName/getActions`, `EntityOps getOps()`, `getSizeX/getSizeY`,
  `getImpostorIds/getImpostor/getVarbitId/getVarPlayerId` (multiloc morph:
  crops, doors; ID spaces are `gameval/VarbitID` + `gameval/VarPlayerID`,
  tagged by the `@Varbit`/`@Varp` annotations), map icon/scene ids. Full field decode (ambient/contrast,
  `clipType`, `nonFlatShading`, `decorDisplacement`, …) already lives in
  FileStore's `ObjectType` — only the morph-resolution + footprint logic ports here.
- **Artifact prevented:** wrong variant (seedling vs mature), wrong footprints.
- **Rust:** `editor_core::object_def`

### D7. `Projection.java` (+ `IntProjection`/`FloatProjection`) + `Client.java` (subset) + `coords/{LocalPoint,WorldPoint,Angle,Direction,WorldArea}.java`
- `Projection.project(x,y,z,tmp)` (A2 culls `p[2] < 50`), `Client`
  `.getTileHeights/getCamera*/getTextureProvider/getWorldView` accessor shapes
  (editor stubs the client but honors the contract), coordinate/angle math.
- **Artifact prevented:** CPU-cull vs GPU-draw mismatch, camera-dependent UV errors.
- **Rust:** `editor_core::camera` + `editor_render::culling`

---

## Group E — Construction tables (melxin-only paths; authoritative, minimal)

The primary tree ships no deob modules — every path here is
`RuneLite-melxin/runescape-client/src/main/java/…` (+ one mixin). Port tables and
formulas only, never the software rasterizer.

- **E1. `Scene.java:29-33, 285-286`** — `tileShape2D` (13×16) + `tileRotation2D`
  (4×16) matrices, consumed at `1164-1169`. Verbatim copy. (Square paths if wrong.)
- **E2. `SceneTileModel.java:10-13, 79-95-270`** — `triangleTextureIndices` ragged
  table + constructor emitting `vertex/face/color/textureId` arrays. Match 1:1.
  (Cracks on shape cuts if wrong.)
- **E3. `Scene.java:480-518 `addTile``** — three branches (flat paint / textured
  paint with `corners-equal → isFlat` / model), `SceneTilePaint` arg order,
  `12345678` skip sentinel. (Flat/shaped misclassification if wrong.)
- **E4. `Scene.java:583+ `newBoundaryObject`` + `newFloorDecoration`** — dual-slot
  writes, `x*4096+xOffset` / `y*64+…` offset formulas (software draws at
  1918/1936/2185/2296-2301 use the same offsets A1 reads). (Lost junctions,
  floating decor if wrong.)
- **E5. `ObjectComposition.java:116-117, 461, 669-763` + `Model.java:473-545
  contourGround`** — `clipType` (opcodes 21/81, `×65536` scale, `>= 0` guard) =
  contouredGround bilinear bottom-vert warp; `nonFlatShading` = mergeNormals.
  (Floating cliffs / lighting creases if wrong.)
- **E6. `ObjectComposition.java:218-219, 585-594, 926-927`** — `transforms[]`
  (+2 sentinel, `65535 → -1`) multiloc indexing; out-of-range varbit/varp
  falls back to `transforms[last]` (the default variant). (Wrong variants if
  wrong.)
- **E7. `FloorUnderlayDefinition.java:31-56, 111-180 `setHsl``** — `rgb →
  hue/sat/light/hueMultiplier` gamma conversion, verbatim. The spatial kernel is
  **solved** (`class470.java:880-1049`): separable 11×11 sliding-window blur
  (radius 5 per axis), hue weighted by hueMultiplier, sat/light by count, plus
  jitter and per-corner slope brightness — exact algorithm in
  `RUNELITE_RUNTIME_RULES.md` R11–R12 (supersedes older 5×5/8×8 notes). (Harsh
  swatches / dark border seams if wrong.)
- **E8. Mixin wiring (read-only):** `runelite-mixins/…/RSObjectCompositionMixin`,
  `RSModelMixin::copy$contourGround`, `RSSceneMixin` + `RSTileMixin`,
  `RSSceneTileModelMixin` + `RSSceneTilePaintMixin`, `RSWorldViewMixin` — confirm
  `@Export` mappings only.

---

---

## Group F — Editor-support contracts (`runelite-api/...`)

Not scene content, but the editor cannot function without them: picking,
collision display, ghosts, animation/FX preview, and ground items.

### F1. Picking + bounds: `model/Jarvis.java`, `geometry/SimplePolygon.java`, `Point.java`, `AABB.java`
- `Jarvis.convexHull()` builds clickboxes (`GameObject`/`WallObject`/
  `DecorativeObject`/`GroundObject.getConvexHull[(2)]` consume it;
  `Perspective.getClickbox` + `calculateAABB`/`calculate2DBounds` are the
  picking path);
  `SimplePolygon` is the hull container; `Point` (`distanceTo` + coords) flows
  through every footprint API; `AABB` (center + extremes) backs frustum/bounds
  checks (`Model.getAABB` feeds it).
- **Without:** no tile/object picking, no selection boxes, no frustum culling.
- **Rust:** `editor_core::picking` (+ `editor_render::culling` uses `AABB`)

### F2. Collision overlay: `CollisionData.java` + `CollisionDataFlag.java`
- Per-tile collision words + flag constants. Bit layout (verified):
  movement block 8 directions `0x1–0x80` (NW/N/NE/E/SE/S/SW/W),
  `BLOCK_MOVEMENT_OBJECT 0x100`, `BLOCK_MOVEMENT_FLOOR_DECORATION 0x40000`,
  `BLOCK_MOVEMENT_FLOOR 0x200000` (water etc.),
  `BLOCK_MOVEMENT_FULL` = the three combined; line-of-sight copies are the
  movement bits `<< 9` (`0x400–0x10000`), `BLOCK_LINE_OF_SIGHT_FULL 0x20000`.
- The editor renders these as the placement-validity overlay and exports them
  with the map.
- **Without:** blind placement (objects in walls), no clipping preview.
- **Rust:** `editor_core::collision`

### F3. Ghosts + markers: `RuneLiteObject.java` + `RuneLiteObjectController.java`
- `setModel/setLocation/setAnimation/setActive` — client-independent scene
  objects: exactly the mechanism for placement ghosts, measurement markers, and
  preview entities without touching the real scene.
- **Rust:** `editor_render::ghosts`

### F4. Animation + FX preview: `Animation.java`, `AnimationController.java`, `GraphicsObject.java`
- `Animation` (id, frames, duration, restart mode) + `AnimationController.tick()
  /animate(Model)` pose models for `DynamicObject` preview;
  `GraphicsObject` (id, location, level, start cycle) previews spotanims.
- **Without:** every animated loc previews T-posed; spell/decor FX invisible.
- **Rust:** `editor_render::anim_preview` (static map stays T-posed by design)

### F5. Ground items + textures: `ItemComposition.java`, `TileItem.java`, `SpritePixels.java`
- `TileItem` (a `Renderable` on `ItemLayer`, whose `getHeight/getBottom/
  getMiddle/getTop` stack the pile vertically) + `ItemComposition` (item models
  for ground rendering); `SpritePixels` (width/height/offsets/pixels) is the
  pixel container `TextureManager` consumes when building the texture array
  (see also its `load()` lifecycle + `setBrightness()`).
- Verified gap: `ItemLayer` has **no static upload** (zero references in
  `SceneUploader`/`Zone`) — ground items render through the dynamic path
  only. The editor draws piles dynamically too (or extends the zone path
  deliberately, keeping priority behavior identical).
- **Without:** ground-item piles render as nothing; texture upload has no pixel
  source contract.
- **Rust:** `editor_core::items` + `editor_render::textures`

### F6. Morph + instance helpers: `VarbitComposition.java`, `InstanceTemplates.java`
- `VarbitComposition` (index, LSB/MSB) is the bit-extract half of multiloc
  morph resolution (values come from editor preview state);
  `InstanceTemplates.findMatch(chunkData)` classifies instanced chunks for
  area copy/paste.
- **Rust:** `editor_core::object_def` (morph) + `editor_core::scene` (templates)

### Deliberately excluded (audited 2026-10-07, full 293-file sweep)
- **Entities:** `Actor/NPC/Player/GraphicsObject-consumers`, `NPCComposition`,
  `Projectile` (needs live actors + trajectory sim), hitsplats/healthbars —
  runtime world state, not map content.
- **UI/widgets/menus/overlays** (`widgets/*`, `Menu*`, `overlay/*`,
  `worldmap/*` 2D map, `FontTypeFace`, `SpriteID`) — client UI, not the 3D scene
  (egui replaces all of it).
- **Event bus** (`events/*Spawned/Despawned`, ticks, `hooks/Callbacks`) — live
  client sync; a headless editor polls state instead of subscribing.
- **Software raster** (`Rasterizer`, `BufferProvider`, `MainBufferProvider`,
  `TileFunction.drawTile`, `IndexedSprite` blitting) — CPU drawing path.
- **Definition params** (`ParamHolder/ParamID/StructComposition`,
  `EnumComposition`, `ColorTextureOverride` item overrides) — no scene-draw
  consumers (params ride along in FileStore, unused by the renderer).
- **Systems:** sounds, friends/clan/chat, GE, skills, quests, scripts, packets,
  collections (`Node/Deque/HashTable`), preferences — no rendering role.

---

## 8. What OpenRune-FileStore already covers (do not reimplement)

- `definition/osrs/.../ObjectCodec.kt` — opcodes 21 (`clipType=0`), 22
  (`nonFlatShading`), 28 (`decorDisplacement`), 74 (`isHollow`), 81
  (`clipType`), + `multiVarBit/Varp/Default/transforms`; `ObjectType.kt`
  carries `sizeX/sizeY/ambient/contrast/clipType/nonFlatShading` equivalents.
  Scale note: the engine multiplies cached contrast by 25 at decode
  (`contrast = readByte()*25`, deob opcode 39; ambient is raw, opcode 29) while
  FileStore keeps the raw byte — apply ×25 when computing light, matching the
  `toModel(ambient+64, contrast+768, …)` call sites.
- `OverlayCodec` / `UnderlayCodec` (+ `OverlayType`/`UnderlayType` with
  `hideUnderlay`), `TextureCodec`, `ModelCodec`/`ModelDecoder`/`ModelType`,
  `FullMapDefinition`, `tools/.../worldmap/` (`Landscape`, `MapFlags`:
  `LINK_BELOW 0x2` bridge / `VISIBLE_BELOW 0x8`, bridge-aware
  `WorldMapBlockBuilder`, `WorldMapGeography`).
- `tools/.../OpenRS2.kt` — fetch any rev cache + keys from
  `archive.openrs2.org`; `displee/` — modern cache read/write;
  `definition/.../game/render/` — FileStore's own software-rasterizer kit
  (`Rasterizer3D.kt` with texture `averageRgb` for textured-overlay preview,
  palette gamma notes; `JagexColor.kt` HSL helpers) — reusable math reference
  for the Rust port.
- Rev-bump maintenance tool (not runtime): melxin
  `deobfuscator/` (`Deobfuscator`, `UpdateMappings`, `MappingDumper`,
  `clientver/`) re-pins Group E after each Jagex update.

---

## 9. Porting constants checklist (`editor_core::consts`, all tested)

- [ ] `1 tile = 128`; heights pass through raw — no ×8 or negation anywhere on
  the GPU path (verified; see R22); verts `f32` (`putfff4` path; int path legacy)
- [ ] `BRIDGE 0x2`, `UNDER_ROOF 0x4`, `VIS_BELOW 0x8`
- [ ] `CHUNK 8`, `REGION 64`, `SCENE 104`, `EXTENDED 184`, `ESCENE_OFFSET = (184-104)/2`
- [ ] Top-level test `wvid == TOPLEVEL` with `TOPLEVEL = 0` + `SCENE_OFFSET` shift
- [ ] Render-thread pool + per-thread staging (`numThreads` 3/15); zone alpha
      cache behind a mutex
- [ ] `abhsl = (alpha&ff)<<24 | (bias&ff)<<16 | hsl&ffff`; depth
  `z += bias/128` under reverse-Z (`GREATER`, clear `0.0`, `Depth32Float`);
  bias is authored per face, never a global epsilon (R21)
- [ ] Priorities `0–11` → 18 buckets via `avg12/avg34/avg68`; `diameter 6000`
      guard; `12345678` skip sentinel
- [ ] Texture `id+1` (0 = none); underlay/overlay `id+1` (0 = none)
- [ ] `uv += tick × speed / 128` along U or V per `Texture`
- [ ] Decor inset ≈ 16 perpendicular (read the wall's own `decorDisplacement`;
  opcode 28, default 16); floor decals at sampled height, no bias;
  `contourGround` bilinear warp; per-model normals, never welded (joint creases
  are authentic — R17)
- [ ] Underlay blur 11×11 separable (radius 5), hue weighted by hueMultiplier,
      sat/light by count, plus per-corner slope brightness and hue/light jitter
      (exact kernel: `RUNELITE_RUNTIME_RULES.md` R11–R12)
- [ ] Fog: per-vertex `fFogAmount` with scene-edge clamp ∩ draw-distance box;
      `fogColor` mix in frag; `useFog = fogDepth > 0`
- [ ] `smoothBanding` mix, `textureLightMode` (`brightTextures`) mix,
      colorblind × `intensity/100`

---

## 10. Build order + verification scenes

1. `editor_core` from Groups C + D + E + F (no wgpu dep).
2. `editor_render` buffers/mesh (A6, D4) → `scene_upload` (A1) + `priority_sort`
   (A2, with B4 contract) → `zone`/`region_manager` (A3/A4) → `renderer` (A5) +
   `textures` (A8) → WGSL from B1–B3 + B7 (B5/B6 as documented reference).
3. Verify: Lumbridge bridge (bridge shift), room corners/T-junctions (dual slot),
   rug-on-floor (priorities), torch-on-wall (inset 16), shoreline path (shape
   cuts), water/lava (tick scroll), 2×3 gate rotation (pivot + `min == tile`),
   cave with depth + animated water (fog + perf), multiloc crop rows (morph),
   roof toggle (roof buckets + region→rect roof-removal volumes from
   `roofremoval/overrides.jsonc`).
