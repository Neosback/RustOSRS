# RuneLite GPU Pipeline — How It Works (File-by-File Deep Dive)

Companion to `RUNELITE_RENDER_SOURCES.md` (Groups A + B). This doc explains **what
each file does, how it does it, and why it exists** — the knowledge needed to
reimplement the pipeline in Rust/wgpu/WGSL rather than transliterate Java.

Primary tree: `/Users/tylercovalt/Documents/runelite-master`
(`runelite-client/src/main/java/net/runelite/client/plugins/gpu/`).
Staged shaders: `reference-shaders/runelite-gpu/`.

Notation: `Class.method()` = Java source. `→ Rust` = suggested port home.

---

## 1. Frame lifecycle (the 10,000-ft view)

```
login / region change
  └─ SceneUploader.zoneSize()      → Zone.sizeO / sizeA (buffer pre-size)
  └─ SceneUploader.uploadZone()    → Zone.vboO / vboA (opaque + alpha VBOs)
       ├─ uploadZoneLevel(Roof)    → roof buckets, VIS_BELOW double pass
       └─ uploadZoneTile()         → paint → model → wall×2 → decor×2 → ground
                                      → gameobjects → bridge recursion
  └─ Zone.convertForDraw()         → VAO-ready float layout
every frame
  ├─ GpuPlugin: camera → worldProj/entityProj, fog box, tick, brightness…
  ├─ ModelUploader.uploadSortedModel() → dynamic/anim models (CPU priority sort)
  ├─ VAO.draw()                    → glDrawArrays per Range batch
  └─ Zone.renderAlpha()            → translucent pass (depth-sorted)
```

Two principles explain every oddity below:

1. **The scene is pre-baked, not drawn from objects.** `SceneUploader` flattens
   the whole tile grid into two giant vertex buffers (opaque + alpha) once per
   region change. Per-frame work is uniforms + dynamic models + draw calls.
   Consequence for the editor: brush edits dirty a *zone* (8×8), never the scene.
2. **Priority replaces depth.** OSRS content is authored coplanar (rugs on floors,
   posters on walls). The 12 software priorities are resolved on the CPU
   (`ModelUploader`) and as micro depth offsets (`abhsl` bias), not by the
   depth buffer alone. Consequence: any port that "just uses `Less` depth" will
   Z-fight everywhere. The depth system is reverse-Z end to end — clip control
   `ZERO_TO_ONE` ("1 near 0 far"), depth func `GREATER`, clear depth `0`,
   32-bit float depth — with coplanar separation from the **authored per-face
   bias byte** (`screenPos.z += bias/128`, larger wins under `GREATER`) plus CPU
   bucket order. Full mechanics: `RUNELITE_RUNTIME_RULES.md` R21.

3. **Baked vs per-frame is a hard split.** Baked into VBOs: positions, `abhsl`,
   static emission order, base UVs (plus slope shading and underlay blur, baked
   earlier into HSL bits at scene build). Per frame: dynamic-model sort, zone
   Range walk, fog, lighting conversion, UV scroll, all tint/brightness/fog
   uniforms. Full inventory: R22. The editor must respect the same split —
   brush edits re-bake zones, never poke uniforms to fake geometry.

---

## 2. `GpuPlugin.java` — renderer backbone (what it owns and why)

**What it is:** the plugin entry point. It owns the GL context state, all shader
programs, all uniforms, the render-thread pool, VAO lists, and the per-frame
update path. It does *not* tessellate (that's `SceneUploader`/`ModelUploader`).

**How it works, piece by piece:**

- **Render threads (`RenderThread[] rts`).** Each thread has its own
  `VAOList vaoO/vaoA`, scratch `tmp[3]`, and `ModelUploader`. `config.numThreads`
  (default 3, max 15) controls the pool; changing it frees and rebuilds every
  thread's VAOs, then re-issues `setupGpuFlags()` with
  `DrawCallbacks.RENDER_THREADS(n)`. Why threads exist: dynamic-model sorting
  (`uploadSortedModel`) is pure CPU math and parallelizes cleanly; the static
  zone buffers are shared read-only.
  → Rust: a staging-thread pool even if you start with 1 thread; per-thread
  scratch + sorter instances from day one.

- **GPU flags (`setupGpuFlags()`).** Tells the (injected) client to route draws
  through the GPU path: `GPU | ZBUF | RENDER_THREADS(n)` (count packed by the
  `RENDER_THREADS()` macro, not a plain bit) plus optional
  `NO_VERTEX_SNAPPING`. `removeVertexSnapping` trades pixel-jitter authenticity
  for smooth camera motion — an editor viewport almost certainly wants it ON.
  → Rust: a `RendererFlags` bitset with the same layout; no client needed.
- **Scene contexts + draw passes.** One `SceneContext` per scene holds the
  `zones` grid, its `projection`, camera, `minLevel/level/maxLevel`, and
  `hideRoofIds`. Each frame answers three passes: `PASS_OPAQUE` (draw every
  thread's `vaoO`; non-top-level views get `IDENTITY` entity projection),
  `PRE_PASS_ALPHA` (unmap `vaoA`, upload `entityProj` + `entityTint` from the
  scene's HSL overrides), `PASS_ALPHA` (drop temp models per zone, then
  `Zone.renderAlpha`). The `useStaticUnsorted` fast path skips resorting static
  alpha unless the scene carries HSL overrides.

- **Camera + uniforms (per frame).** Builds `worldProj` (static scene) and
  `entityProj` (tinted/animated entities) via `Mat4`, uploads `base` (scene
  origin for fog math), `tick` (texture scroll clock), `drawDistance`,
  `expandedMapLoadingChunks`, `brightness`, fog triple
  (`useFog = fogDepth > 0`, `fogDepth`, `fogColor`), `smoothBanding`,
  `textureLightMode` (`brightTextures`), `entityTint` (HSL overrides),
  colorblind pair. Why so many: each maps 1:1 to a `vert`/`frag` uniform —
  the uniform table in §6 *is* the `GpuPlugin`→shader contract.
  → Rust: one `CameraUniforms` struct uploaded per frame + one `FrameUniforms`.

- **Texture animations.** `textureManager.computeTextureAnimations()` builds the
  `textureAnimations[TEXTURE_COUNT]` vec2 array from `TextureProvider`: each entry
  is the per-tick U/V scroll vector. `tick` advances it in-shader.
  Why an array, not per-material time: one uniform update animates all water,
  lava, and conveyors coherently.
  → Rust: same array; advance `tick` on game-tick, not wall-clock (pausing the
  editor must freeze water).

- **Top-level test.** `wvid == WorldView.TOPLEVEL` (value `0` in this tree)
  selects the `SCENE_OFFSET` shift. Old code compared against `-1`; the constant
  changed, the logic didn't.
  → Rust: `if world_view == TOPLEVEL { base -= SCENE_OFFSET }`.

**Why it matters:** every visual feature in `GpuPluginConfig` bottoms out here.
If a setting has no uniform behind it, it does nothing — use §6 as the checklist.

---

## 3. `SceneUploader.java` — the flattener (how static geometry is baked)

**What it is:** a one-way compiler from the tile grid to two vertex buffers.
No GL calls inside (it writes into `GpuIntBuffer` views over the zone VBOs).

**How `uploadZone` works (and why the order is load-bearing):**

1. `zoneSize()` walks every tile of the 8×8 zone and counts faces into
   `sizeO` (opaque) / `sizeA` (alpha): paint = 2 tris, model = `faceX.length`,
   each renderable split by `faceTransparencies`. Why pre-size: GL buffers are
   allocated once; over/under-run corrupts neighbors.
2. Roof census: collect roof IDs per level into `rids/roofStart/roofEnd` ranges
   so roof hiding later is a *range skip*, not a rebuild.
3. Level 0 is special: after its own pass it re-uploads levels 1–3 with
   `visbelow=true`, so geometry under upper floors (caves, dungeons) is drawn
   as part of the ground pass. Levels 1–3 upload only their own pass.
4. `uploadZoneLevelRoof()` applies the **bridge shift**: if
   `settings[1][x][z] & BRIDGE`, `maplevel++` — the tile's *geometry* joins the
   lower plane's pass while heights/collision stay upstairs. Then
   `visbelow != VIS_BELOW` tiles are skipped, roof ID must match the bucket, and
   `uploadZoneTile` runs.

**How `uploadZoneTile` works (order = draw-priority contract):**
`SceneTilePaint` quad → `SceneTileModel` shape mesh → `WallObject` slot 1, then
slot 2 → `DecorativeObject` renderable + renderable2 with `xOffset/yOffset` →
`GroundObject` → each `GameObject` whose `min == this tile` (multi-tile objects
appear on every covered tile's list; the filter guarantees single upload) → then
`tile.getBridge()` recursed. Each object is gated by
`renderCallbackManager.drawTile/drawObject` (editor: roof/plane filters plug in
here). The paint path writes the 2-triangle quad with corner HSL + full-tile UVs;
the model path converts scene-local verts to zone-local and skips `12345678`
faces.

**How `uploadStaticModel` works:** JAU-rotate verts (`SINE`/`COSINE`, `>> 16`),
translate by object origin, then emit each face as 3× (`pos`, `alphaBias|color`,
`tex+1`, `u*256`, `v*256`). Transparency and depth-bias ride in the color int
(`alphaBias`), texture `+1` reserves 0 for "untextured".

→ Rust: `scene_upload::{size_zone, upload_zone, upload_tile, upload_model}`;
keep the function boundaries — they map to unit tests (bridge map, T-junction,
multi-tile gate).

---

## 4. `ModelUploader.java` — the sorter (how dynamic/priority drawing works)

**What it is:** the CPU half of the priority system, used for animated, dirty, or
otherwise non-baked models (players, NPCs, animating locs, temp edits).

**How `uploadSortedModel` works:**

1. Rotate + translate verts to world; project each through `Projection`; **cull
   the whole model if any `p[2] < 50`** (behind/inside camera — cheap reject).
2. Backface-cull per triangle in *projected* space
   (`(aX-bX)*(cY-bY)-(cX-bX)*(aY-bY) > 0`), bucket survivors by mean depth into
   `zsortHead/Tail/Next` chains over a `MAX_DIAMETER 6000` range.
3. If the model has no priorities (or sorting disabled): emit far→near.
   Otherwise the 12-bucket interleave: faces accumulate in
   `orderedFaces[pri]` with distance sums in `lt10` and exact distances for
   pri-10/11 in `eq10/eq11`; running averages `avg12/avg34/avg68` decide where
   the pri-10, then pri-11, faces inject (at the pri-0/3/5 boundaries).
   Each face lands in the opaque or alpha buffer by its transparency bit.
4. UVs come from `computeFaceUvs` (tangent/bitangent/normal frame + camera-ray
   projection for textured faces; constant 0/1 triangle otherwise), and colors
   pass through `interpolateHSL` unless textured, then `faceTransparency`
   merges model-wide and per-face alpha.

**Why two sorters exist conceptually:** static geometry is pre-sorted at bake
time; anything that moves must be re-sorted per frame against the camera. The
editor needs both: baked zones for the map, the dynamic path for ghosts,
previews, water planes, and animated locs.

→ Rust: `priority_sort::{sort_model, compute_face_uvs, interpolate_hsl}`;
the `priority_render.glsl` `priority_map` function is the same algorithm in GLSL
form — keep them side by side and test with identical inputs.

---

## 5. `Zone.java` + `RegionManager.java` — the streaming layer

**What `Zone` is:** one 8×8 chunk's worth of GPU buffers plus the metadata to
draw subsets: `vboO/vboA` (opaque/alpha), `levelOffsets[4]` (per-plane draw
ranges), roof ranges, `alphaModels` (translucent models with packed faces for
the alpha pass), and `dirty/invalidate/cull` lifecycle flags.

**How it works:** upload fills the two VBOs in staging layout (`VERT_SIZE 20`:
short vec4 pos + int `abhsl` + short vec4 id/uv); `convertForDraw()` swizzles to
the `VAO` draw layout (`VERT_SIZE 24`: float vec3 + int + short vec4). Draws use
the level/roof ranges so plane toggles and roof hiding never re-upload. The
alpha-model cache is mutex-guarded because render threads share zones.

**What `RegionManager`/`Region`/`Regions` are:** the region-ID lookup plus the
`hideUnrelatedMaps` pruner. `Regions` parses `regions.txt`; `prepare(scene)`
deletes every 8×8 chunk whose region ID differs from the center region (skipped
for instances or when the setting is off). There is no dirty queue here —
dirty tracking lives on `Zone` (`dirty`/`invalidate`/`cull`).

→ Rust: `region_manager::{RegionManager}` (lookup + hide-unrelated cull); the
dirty→reupload→`convertForDraw` cycle *is* the brush-edit pipeline.

---

## 6. Buffers, math, and shader plumbing

**Buffers (`GpuIntBuffer`, `GpuFloatBuffer`, `GLBuffer`, `VBO`, `VAO`).**
`GpuIntBuffer.put22224/put2222` and `putfff4` are the *only* legal writers —
they encode the ABI: position ints or floats, then `abhsl`, then `(tex+1, u, v)`.
`GLBuffer`/`VBO` own GL memory (orphaning on resize); `VAO` owns attribute
pointers + the `Range` batch list that merges consecutive draws sharing
`(projection, renderMethod)`. Get one stride wrong and UVs decode as colors.

**`Mat4` / `Shader` / `Template`.** `Mat4` builds view/proj (plus `IDENTITY`);
`Shader` compiles/links (`add()` units + `compile()`); `Template.process()`
expands `#include "file"` lines, which is how generated snippets
(`texture_config` with `TEXTURE_COUNT = 256`, `sampling_mode` from
`uiScalingMode`, `colorblind_mode`) and `hsl_to_rgb.glsl` land inside
`vert`/`frag`. A WGSL port needs the same tiny preprocessor.

**`TextureManager`.** Builds the texture array from `TextureProvider` (palette
scaling/brightness included) and the per-tick scroll vectors. Frozen water =
this file not ticked; reversed scroll = U/V direction flipped; gray materials =
tint not applied.

**Uniform table (the full contract — every row needs a Rust counterpart):**

| Uniform | Producer | Consumer | Effect if wrong |
|---|---|---|---|
| `worldProj` / `entityProj` | `GpuPlugin` via `Mat4` | `vert` | everything misplaced / tinted entities unprojected |
| `entityTint` | `GpuPlugin` (HSL overrides) | `vert` | override entities wrong color |
| `base` | `GpuPlugin` (scene base coords — added to **every** vertex position as `vert = vertf + base`, and anchors the fog box; location fetched as `uniBase`, confirm the write path during port) | `vert` positions + fog | whole scene offset / fog anchored wrong |
| `tick` + `textureAnimations[]` | `TextureManager` + tick clock | `vert` UVs | frozen/reversed water, lava, conveyors |
| `drawDistance`, `expandedMapLoadingChunks` | config | `vert` fog box | popping edges, fog over unloaded chunks |
| `useFog`, `fogDepth`, `fogColor` | config | `vert`→`frag` | no fog / wrong color / depth ignored |
| `brightness` | config | `vert`+`frag` | dark/bright mismatch between terrain and textures |
| `smoothBanding` | config | `frag` | color banding on gradients |
| `textureLightMode` | `brightTextures` | `frag` | washed-out or muddy textures |
| `colorblindIntensity` | config | `frag` via `colorblind.glsl` | over/under-corrected output |

---

## 7. Config features and their rendering meaning (`GpuPluginConfig`)

- `drawDistance` + `expandedMapLoadingZones` + `hideUnrelatedMaps`: resident-set
  size and the fog box that hides its edge. Editor needs all three (large-map
  editing without fog walls).
- `fogDepth` (`0` = off), `smoothBanding`, `brightTextures`,
  `antiAliasingMode`, `anisotropicFilteringLevel`, `uiScalingMode`: quality
  ladder — implement in this order.
- `colorBlindMode` + `colorBlindIntensity`: post-mix correction; intensity
  defaults to 100.
- `removeVertexSnapping`: smooth camera vs authentic jitter — default ON for an
  editor.
- `numThreads`: render-thread count (3 default, 15 max).
- `unlockFps`/`vsyncMode`/`fpsTarget`: presentation only; still wire them so the
  viewport behaves.
