# Rust / wgpu / egui Port Notes — Exact Mappings

Companion to the other `RUNELITE_*.md` guides. This doc answers only one
question: **what does the Rust code look like?** Every value below was read
from source (pins cited); anything still unverified is marked `[VERIFY]`.

## 1. Crate map (where each guide section lands)

| Guide section | Crate | Module(s) | Deps allowed |
|---|---|---|---|
| C1–C2, C9–C10, D1–D3, D6–D7, E, F | `editor_core` | `scene`, `tile`, `objects`, `coords`, `math`, `consts`, `color`, `object_def`, `camera`, `picking`, `collision`, `items` | glam, bytemuck — **no wgpu, no egui** |
| Cache I/O (FileStore is JVM) | `editor_cache` | `archive`, `decoders`, `assets` | Depend on `rs-cache` (verified: builds offline, 30/30 tests pass) for I/O + item/npc/obj/map/loc; port transforms-with-varbit, 32-bit model IDs, models, textures, overlay/underlay/sequence defs from FileStore specs — full plan in `RUNELITE_CACHE_STACK.md` |
| A1–A4, A6, D4, F3–F4 | `editor_render` (tessellator) | `scene_upload`, `priority_sort`, `zone`, `region_manager`, `buffers`, `mesh`, `ghosts`, `anim_preview`, `culling` | + wgpu types for layouts only |
| A5, A7–A9, B | `editor_render` (backend) | `renderer`, `shaders`, `textures`, `callbacks`, `settings` (shared with app) | wgpu, naga (via wgpu) |
| UI, docking, tools | `editor_app` | `app`, `viewport`, `panels`, `theme` | eframe, egui_dock, catppuccin-egui, rfd, web-time |

Rule: `editor_core` must compile for `wasm32-unknown-unknown` with no
platform imports. All threads (`rayon`/`crossbeam`), file dialogs (`rfd`),
and clocks (`web-time`, not `std::time::Instant`) live behind
`#[cfg(not(target_arch = "wasm32"))]` with single-threaded/`wasm-bindgen-futures`
fallbacks — never inline `cfg` spaghetti in render math.

## 2. Vertex layouts as Rust structs (byte-exact)

```rust
use bytemuck::{Pod, Zeroable};

// VAO draw layout — 24 bytes (VAO.VERT_SIZE; attr pointers verified in VAO.java)
#[repr(C)] #[derive(Clone, Copy, Pod, Zeroable)]
pub struct DrawVertex {
    pub pos: [f32; 3],   // +0  float vec3 (world units, heights raw)
    pub abhsl: u32,      // +12 alpha<<24 | bias<<16 | hsl (unpack: hue>>10&63, sat>>7&7, lum&127)
    pub extra: [i16; 4], // +16 (tex+1, u*256, v*256, 0)
}
// wgpu VertexBufferLayout: array_stride 24,
//   location 0 Float32x3 offset 0, location 1 Uint32 offset 12, location 2 Sint16x4 offset 16.
// Canonical mesh rule: keep deob-faithful ints (positions, face colors,
// groups) in editor_core through lighting; convert to f32 only at the render
// boundary. Never drop face metadata (alpha/priority/texture/bias/groups)
// into a generic vertex early — animation groups and material decisions need it.

// Zone staging layout — 20 bytes (Zone.VERT_SIZE; attr0 is vec4 SHORT)
#[repr(C)] #[derive(Clone, Copy, Pod, Zeroable)]
pub struct StageVertex {
    pub pos: [i16; 4],   // +0  short vec4
    pub abhsl: u32,      // +8
    pub extra: [i16; 4], // +12 (id, u, v, 0)
}
```

`FACE_SIZE = (VERT_SIZE >> 2) * 3` ints per triangle (both layouts). Writers:
`put22224`/`put2222` (ints), `putfff4` (floats) — port as `StageBuffer::push_*`.

## 3. wgpu state mapping (GL → wgpu, verified pins)

| GL (`GpuPlugin.java`) | wgpu |
|---|---|
| `glClipControl(LOWER_LEFT, ZERO_TO_ONE)`, "1 near 0 far" (:369) | Nothing — wgpu NDC depth is always 0..1. Emit clip `z` with 1 = near. |
| `glDepthFunc(GREATER)` (:1027), `glClearDepth(0)` (:1041/1047) | `CompareFunction::Greater`, clear `0.0` |
| `GL_DEPTH_COMPONENT32F` MSAA renderbuffer (:804) | `TextureFormat::Depth32Float`, `sample_count` from AA mode |
| `glBlendFuncSeparate(SRC_ALPHA, ONE_MINUS_SRC_ALPHA, ONE, ONE)` (:1024, scene) | color: `(SrcAlpha, OneMinusSrcAlpha, Add)`; alpha: `(One, One, Add)` |
| UI passes (`ONE, ONE_MINUS_SRC_ALPHA` / `SRC_ALPHA, ONE_MINUS_SRC_ALPHA`) | Same table per pipeline; UI overlay last |
| `GL_CULL_FACE` on during scene draw (:1020), off after (:1082); CPU keeps `(aX-bX)*(cY-bY)-(cX-bX)*(aY-bY) > 0` | `cull_mode: Back`, `front_face: Ccw` `[VERIFY with a T-junction render test]` |
| 128×128×N texture array, RGBA8, 8 mips (`TextureManager:42,57,64`) | `D2`, `128×128×256`, `Rgba8Unorm`, `mip_level_count: 8` |
| Anisotropy level int (`anisotropicFilteringLevel`) | `SamplerDescriptor.max_anisotropy` (clamp 1..16) |
| `TEXTURE_COUNT = 256`, `TEXTURE_ANIM_UNIT = 1/128` | `textureAnimations: [[f32;2];256]` uniform + `tick: u32` |

Depth-stencil attachment + color target (RGBA surface, sRGB view for egui
interop — see §6) with `sample_count` shared; resolve to the swapchain texture.

Uniforms: one buffer with the std140 `uniforms` camera block
(`cameraYaw/Pitch/X/Y/Z` = 5 floats, hence `UNIFORM_BUFFER_SIZE`) plus plain
`worldProj`/`entityProj` mat4x4, `entityTint` ivec4, `base` ivec3, scalars
(`brightness`, `useFog`, `fogDepth`, `drawDistance`, `expandedMapLoadingChunks`,
`tick`, `smoothBanding`, `textureLightMode`, `colorblindIntensity`),
`fogColor` vec4, and `textureAnimations[256]` vec2 array. Mirror the exact
set — every row of the pipeline uniform table must have a counterpart.

## 4. Camera spec (orbit, JAU-compatible)

- Axes: world +x = east, +y = north (tile units); local z follows scene tile-Y
  (SceneUploader maps tileY → lz 1:1). Model/world y is up; heights raw.
- JAU compass (verified `Direction.java`): 0 = south, 512 = west, 1024 = north,
  1536 = east (cardinal bands ±256). Orbit state: target + yaw + pitch (JAU) +
  distance. Pitch clamp **[128, 383]**, reset default 128; yaw free (`& 2047`)
  — verified client bands, use as the editor orbit defaults.
- **Mirroring is the risk**: with a custom axis mapping the map can render
  mirrored (walls on wrong edges, text backwards) while still "looking right".
  Arbiter: an asymmetric landmark render test (e.g. Lumbridge Castle front vs
  the `Direction` bands above) before locking the view matrix. View matrix
  itself is editor-defined (native projection math is injected, not in
  mixins) — only the clip contract below is load-bearing.
- Clip contract: x right, y up, z ∈ [0,1] with **1 = near** (reverse-Z +
  `Greater`).
- Projection: replicate `Mat4.projection(w, h, n)` semantics
  (`[2/w, -2/h, 0/1 depth row, 2n]`); the `-2/h` Y row compensates GL clip —
  `[VERIFY whether to keep or drop it under wgpu NDC with a render test]`.
- `Perspective.getTileHeight` / `getFootprintTileHeight` drive picking and the
  placement ghost; `localToCanvas`/`modelToCanvas` are canvas-overlay helpers
  (port only if the editor draws canvas markers).

## 5. Orientation + rotation rules (no double rotation)

- Loc orientation 0–3 → build-time 90° steps (`rot90/180/270` equivalents) applied
  in `getModelData` order (R9); wall edges from `field800/field804` bitflags
  (1=W,2=N,4=E,8=S, 16=NW,32=NE,64=SE,128=SW).
- `GameObject.getModelOrientation()` is JAU for the **draw-time** rotation and
  is typically 0 for statics (models pre-rotated at build) — apply exactly one
  of the two, selected by `modelOrientation == 0`.
- Editor gizmo: display loc-orientation 0–3 + compass edge; store both.

## 6. `editor_app` shell (eframe + egui + wgpu)

- Embed the 3D view with `egui_wgpu::CallbackTrait`: allocate the central-panel
  rect each frame, submit a paint callback holding an
  `Arc<RwLock<SceneRenderer>>`. Theme once per update:
  `catppuccin_egui::set_theme(ctx, MOCHA)`.
- Renderer owns `wgpu::Device/Queue` (created from the same adapter eframe
  uses where possible); the egui callback receives the shared device/queue —
  do not create a second device.
- Surface color target must match egui's (sRGB view); MSAA resolve happens
  inside the callback's command encoder before egui composites.
- Panels: left toolbox (brushes bind directly to `editor_core` brush state),
  right inspector (tile/loc properties from `editor_core::scene`), bottom
  console optional. `egui_dock` for layout persistence.
- Platform gates: `rfd` file dialogs, `web-time` clocks, threads/native-only
  chunk loading behind `cfg(not(wasm32))`; `wasm-bindgen-futures` + Trunk for
  the web target. Fonts (Inter) + icons under `assets/`.

## 7. Startup defaults (verified)

`drawDistance` 50 (uniform ×128), `fogDepth` 0 (fog off),
`numThreads` 3 (max 15), `colorBlindIntensity` 100, `tick = gameCycle & 127`
(frozen while loading), `brightness` from texture provider,
`fogColor = skyboxColor` per frame (use skybox color or a fixed default in the
editor), `removeVertexSnapping` ON for the viewport, jitter pinned to 0
(deterministic; live client random-walks, R12), orbit pitch default 128
within [128, 383], yaw free.

## 8. Constants index (all verified)

`CHUNK 8`, `REGION 64`, `SCENE 104`, `EXTENDED 184`, `SCENE_OFFSET 40`,
`NUM_ZONES 23`, `MAX_WORLDVIEWS 4096`, UBO `5 floats` (camera block),
`MAX_DISTANCE 184`, `MAX_FOG_DEPTH 100`, `TEXTURE_COUNT 256`,
`TEXTURE_SIZE 128`, `MAX_DIAMETER 6000`, `MAX_FACES_PER_PRIORITY 4000`,
`TILE_FLAG_*` 0x1/0x2/0x4/0x8/0x10, sentinel `12345678`, texture/underlay
`id+1` (0 = none), sun `(−50,−50,−10)`, `ambient+64`, `contrast×25 then +768`.

## 9. Testing + parity harness (adopted)

- **Exact tests** (integer equality): decoded vertices/indices, face metadata
  (colors/alpha/priority/texture/bias), camera matrices, projected coordinates,
  sprite pixels, blur-kernel outputs on fixtures, sort orders on crafted
  models. Tolerance tests only for final screenshots, depth edges, and
  animation positions — never for decode math.
- **Golden scenes**: pin one cache revision for all golden data; reference
  screenshots at fixed camera/scene conditions (bridge, T-junction, rug,
  torch, shoreline, water, gate, cave, multiloc row, roof toggle — main MD §10).
- **Decoded-artifact cache keys** must include cache revision +
  decoder-schema version, so a parser change never serves stale geometry.
- **Property + fuzz**: `proptest` for parser invariants (no huge alloc from a
  2-byte count; unknown opcodes don't panic); `cargo-fuzz` on model/sprite/
  definition-opcode/map decoders; `cargo-nextest` runner; Criterion benches for
  sort/upload/blur; `tracing` spans per frame stage (decode, prep, GPU, egui).
- Budgets to validate by profiling (initial): 60 FPS viewport minimum,
  <4 ms frame prep, <10 ms GPU world+UI, zone re-upload off the critical path.

## 10. Editor overlays + input (adapted overlay model)

No game widgets or CS2 exist in this project — but the overlay *pattern* still
applies to gizmos, markers, footprints, and tile highlights:

```rust
pub trait EditorOverlay {
    fn id(&self) -> OverlayId;
    fn layer(&self) -> OverlayLayer;   // UnderSceneGizmos / AboveScene / AlwaysOnTop
    fn priority(&self) -> f32;
    fn draw(&mut self, scene: &SceneView, out: &mut OverlayCommands);
}
```

Scene-space primitives (tile polygons, footprints, outlines) render in the
world pass via a small overlay-primitive renderer; panels/config stay in egui.
Input routes top-down: egui chrome → interactive overlay → viewport
(camera/world). Transparent overlays must never consume pointer input (ground
markers overlapping clickable tiles is the classic bug).

## 11. Explicit non-scope (decided, not deferred)

This project builds a map editor, not a client. Out of scope by design:
networking/login/packets, CS2 + game widgets (egui + world-space primitives
cover all editor UI), plugin systems/sandboxing, sounds, entities (NPC/player
state), official-world connectivity of any kind. The editor loads the user's
local cache and never redistributes game assets. A `--safe-mode` flag
(default settings, conservative backend, diagnostics on) mirrors RuneLite's
recovery story for driver/config failures.
