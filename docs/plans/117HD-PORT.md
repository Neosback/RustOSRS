# 117 HD port plan

Source: `~/Documents/RLHD-master` (117HD / RLHD, BSD-2 licensed; headers credit Adam, 117, Hooder).
Goal: a full port of the plugin's look into our own wgpu engine as a second render profile next to
the RuneLite-parity **Reference** profile, which stays untouched and remains the default.

## 1. What 117HD is (inventory)

| Area | Where | Size | What it does |
|---|---|---|---|
| Scene shaders | `resources/rs117/hd/scene_vert/geom/frag.glsl` | ~800 lines | Per-vertex normals, material + terrain data, tangent-space normal/parallax mapping, specular, ground blending, wind/character displacement, water, caustics, fog, lights, shadows |
| Shader utils | `resources/rs117/hd/utils/*.glsl` (30 files) | ~4,000 lines | sky, starfield, moon, aurora, tone mapping, colour filters, colour blindness, PCSS/PCF shadow filtering, water, caustics, lights, fog, hashing |
| Other passes | `shadow_*`, `tiled_lighting_*`, `sky_*`, `star_*`, `ui_*`, `gap_filler_frag`, `comp*.glsl` | ~1,500 lines | shadow map, tiled point-light culling, sky/star/nebula bake, UI, hole filler, compute face sorting (legacy path) |
| Data (JSON) | `resources/rs117/hd/scene/*.json` | 16 MB | `materials`, `ground_materials`, `tile_overrides` (underlay/overlay -> material), `model_overrides` (object/npc/item ids -> shading rules), `areas` (named AABBs), `environments` (per-area light/fog/sky), `lights` (~49k lines of placed point lights), `water_types`, `sky_presets`, `gamevals` (name -> id tables, 116k lines) |
| Textures | `resources/rs117/hd/scene/textures` | 194 files | diffuse (`_d`), normal (`_n`), height, flow maps for ~100 materials |
| Java | `java/rs117/hd` | ~47k lines | scene managers (area, environment, light, material, ground material, model override, tile override, water, sky, texture, gameval), `ProceduralGenerator` (terrain normals/blending/water depth), `SceneUploader` (vertex/material packing), two renderers (legacy compute, zone), overlays, ~100 config items |

## 2. Why porting is easier for us

117HD works around the vanilla client: it intercepts uploads, recomputes normals, reverses packed
colours, hashes models for a cache, and sorts faces in compute shaders. We own the whole pipeline
(`osrs-core` normals and lighting, `osrs-scene`, `osrs-world` extraction, `osrs-render`), so:

* normals, tile data (underlay/overlay ids, heights, water depth) and object ids exist at extraction
  time; no geometry shader (`scene_geom.glsl`) is needed, which matters because WGSL/Metal have none;
* no model hashing/caching layer, no vertex snapping workaround, no legacy renderer;
* depth-tested rendering (RuneLite zone renderer model) replaces the compute sorting stage; alpha
  ordering is already ported (`Zone.renderAlpha`);
* material/area/override lookups can happen once per region build on the worker thread.

## 3. Target architecture

Add a **render profile** switch (`Reference` | `HD`) in the renderer; the editor exposes it under
Properties > Shading. Reference keeps the current single-pass pipeline.

```
extraction (osrs-world)           render (osrs-render)
  Reference vertex  ----------->   scene.wgsl            (exists)
  HD vertex (new)   ----------->   hd/scene.wgsl         (new: PBR-lite, materials, shadows, fog)
  material tables   ----------->   hd/material buffer     (new)
  lights, areas     ----------->   hd/light + env buffers (new)
                                   hd/shadow.wgsl, hd/sky.wgsl, hd/post.wgsl
```

New vertex layout (HD) = Reference fields + normal (octahedral i16x2), material index (u16), terrain
data (water depth, ground material blend ids) packed like `vMaterialData` / `vTerrainData`. UVs move from
the 8.8 fixed point to per-material UV generation (`utils/uvs.glsl`).

Rendering stages: shadow map -> opaque scene (HDR target) -> alpha scene -> sky (before, behind) ->
tone map / colour filter / AA resolve -> overlays (egui).

## 4. Phases (each ends with a visible, testable result)

0. **Foundations (engine)** - HDR colour target, multi-pass frame graph, uniform/storage buffer
   layout, material texture-array loader (PNG/JPG -> mip-mapped `texture_2d_array`), render-profile
   switch. Acceptance: Reference output unchanged (pixel-identical on the oracle windows).
1. **Data layer** - Rust loaders for the JSON files with serde, `gamevals` resolution against the
   build-241 cache (the cache already carries gameval tables; verify against `gamevals.json`),
   area/AABB matching, tile/model override resolution. Acceptance: unit tests resolving known names
   (`WATER`, `OVERWORLD_GRASS_1`, `DEFAULT_GRASS` ...) to ids.
2. **Materials and normals** - vertex format, normal reconstruction (`ProceduralGenerator`: terrain
   normals, ground blending, water depth), material data packing, diffuse + normal map + specular in
   the HD shader, ground blending. Acceptance: Lumbridge terrain/brick/wood match 117HD screenshots.
3. **Lighting and environment** - per-area environments (ambient, directional, fog, sky colour),
   fog modes, default sky, vanilla-shadow removal, day/night cycle. Reuses the area matcher.
4. **Shadows** - directional shadow map pass, PCF/PCSS filtering, roof/terrain shadow options,
   pop-in removal.
5. **Water and caustics** - water types, flow maps, normal maps, fresnel, foam, underwater caustics,
   water depth clamp at the draw-distance border.
6. **Dynamic lights** - placed lights from `lights.json`, tiled light culling (compute or fragment
   path), NPC/projectile lights (editor has none yet; placed lights only at first).
7. **Sky** - sky shader, starfield, nebulae, moon, aurora, seasonal themes, real-time cycle.
8. **Post and settings** - tone mapping, contrast/saturation, colour filters, colour blindness,
   MSAA/FXAA options, wind/character displacement, parallax; full settings UI sections
   (Lighting, Shadows, Environment, Textures, Miscellaneous) mirroring `HdPluginConfig`.

## 5. Risks and decisions

* **No geometry shader**: everything `scene_geom.glsl` does (barycentric/texBlend, per-face data) must
  move to vertex data or `@builtin(vertex_index)` + storage-buffer fetches (the zone renderer already
  fetches per-face data through `textureFaces`).
* **Metal limits (macOS, our dev target)**: texture-array layer counts, 16 sampled textures per stage,
  no storage-texture read-write on all formats. Budget the material array (~100 layers) and the tiled
  lighting image accordingly.
* **Attribution/licence**: BSD-2 requires keeping copyright notices in ported shaders and a notice
  file for the JSON/texture data. Add `THIRD_PARTY.md` in phase 0.
* **Data sync**: keep the JSON files verbatim under `assets/117hd/` with a recorded upstream commit so
  updates diff cleanly; do not hand-edit.
* **Parity**: HD is a stylised look, not a parity target. Correctness checks are unit tests on the
  data layer plus screenshot comparisons against RuneLite with 117HD at fixed camera positions.

## 5b. Status (2026-10-11)

* Done: upstream resources vendored verbatim in `assets/117hd/` (68 shaders, scene JSON, 194 textures,
  schemas, settings guide; notices in `THIRD_PARTY.md`). Phase 1 data layer: `crates/osrs-hd` loads and
  cross-checks materials, ground materials, tile/model overrides, areas (names, boxes, regions, nested
  areas), environments, water types, sky presets, lights and the gameval tables (tests in
  `crates/osrs-hd/tests`). `DIRT` and `UNDERWATER_GENERIC` ground materials are defined in the plugin's code
  and must be added when `GroundMaterial` is ported.
* Done (phase 0, 2026-10-11): `RenderProfile::{Reference, Hd}` in `osrs-render::gpu` (editor: Properties >
  Shading > Render profile, persisted as `render_profile` in `editor.json`). `encode_frame` builds a small
  frame graph (`gpu/frame_graph.rs`): Reference is one scene pass resolving into the caller's view (pinned
  by `crates/osrs-world/tests/reference_profile_pixels.rs` against `tests/golden/*.png`); HD renders the scene
  into a 4x MSAA `Rgba16Float` target, resolves it and runs the placeholder resolve pass (`gpu/hd/post.wgsl`:
  exposure + vignette). Groundwork in `gpu/hd/`: `GpuMaterial` storage-buffer layout with its WGSL struct
  (checked on a real device), `LayerPlan` + `DecodedLayers` + `MaterialTextureArray` (PNG/JPG ->
  flipped, bicubic-resampled, sRGB mip-mapped `texture_2d_array`; 385 layers at 256 px for the vendored set,
  so the device must allow >256 array layers: `hd::raise_limits`, applied in the editor's wgpu setup and
  `new_headless`). Not wired into a shader yet: phase 2 consumes it. Vanilla (cache) texture layers are left
  empty for phase 2 to fill via `MaterialTextureArray::write_layer`.
* Next: phase 2 (vertex format with normals
  and material ids, `ProceduralGenerator`), then 3-8 as listed above.

## 6. First concrete steps

1. Copy `resources/rs117/hd/scene/*.json` (without textures) into `assets/117hd/` and write the serde
   models + loaders (phase 1) with tests; no renderer changes.
2. Add the render-profile enum and an HDR offscreen path behind it (phase 0), keeping Reference as is.
3. Port `ProceduralGenerator` normals as an `osrs-world` extraction option feeding the new vertex.

The editor chrome already has a **Shading** section in the Properties dock where the profile switch and
the HD settings sections belong.
