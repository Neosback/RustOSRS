# OSRS tile appearance and path authoring

Baseline: RustOSRS `94d3ed16f4185cbc16f8af8e5e1cc32ff0740c39`, reviewed with this task's local changes.
Primary client: `melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`, inspected at
`/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin`.
The checkout has an unrelated deleted LICENSE; the inspected Java files match the pinned Git blobs.
This report explains source behavior and proposes subsequent improvements. The canonical
[terrain specification](../specs/terrain.md) owns semantic rules; [ADR-0006](../adr/ADR-0006-reference-and-enhanced-render-profiles.md)
separates Reference behavior from enhancements.

## 1. What a map tile can express

The map does not store arbitrary road meshes, layered splats, a path graph, or a blend amount.
It stores heights/settings and one underlay plus one shaped overlay per tile and source plane.
A path authoring tool chooses those values; the client derives geometry and colours afterwards.

| Input | Pinned source behavior | RustOSRS owner |
|---|---|---|
| Terrain stream | `class264.loadTerrain`: unsigned-short opcodes; 0 ends with default height, 1 ends with explicit height byte; 2–49 carry overlay ID, shape `(op-2)/4`, rotation `(op-2+regionRotation)&3`; 50–81 supply settings; larger values supply underlay `op-81` | `osrs-cache` terrain decoder/encoder; `osrs-scene::terrain_load` |
| Floor identity | Map floor IDs are one-based; zero means absent; builder masks raw floor IDs with `0x7fff` and looks up `id-1` | Core floor IDs, cache definitions, world `FloorTable` |
| Height | 128 local units per tile; negative is up. Byte 1 means zero steps. Default upper height is lower height minus 240; explicit upper height is lower height minus eight times the decoded byte | `terrain_load`; semantic `HeightVertexCoord` |
| Overlay shape | Stored shape 0–11 maps to scene row 1–12; scene row 0 is underlay only. Row 1 is the whole overlay tile | `terrain_appearance::shape_row`, terrain topology tables |
| Geometry | `SceneTileModel` owns 13 templates × 4 rotations, face ownership, midpoint/quarter vertices, integer height/colour interpolation and optional overlay texture | `osrs-scene::terrain` and exact shape goldens |

Primary sources: [terrain decoding](https://github.com/melxin/runelite/blob/1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc/runescape-client/src/main/java/class264.java#L13),
[scene tile model](https://github.com/melxin/runelite/blob/1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc/runescape-client/src/main/java/SceneTileModel.java),
[scene tile construction](https://github.com/melxin/runelite/blob/1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc/runescape-client/src/main/java/Scene.java#L485).

Shaped tiles partition a single tile between underlay and overlay faces. They do not alpha-fade
one floor texture over another. Full overlay paint has the reference two-triangle split; partial
shapes expose the underlay in the remaining faces. Integer operation order matters at shared vertices.

## 2. Underlay colours: the exact neighbourhood

[FloorUnderlayDefinition](https://github.com/melxin/runelite/blob/1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc/runescape-client/src/main/java/FloorUnderlayDefinition.java)
derives HSL from RGB. It stores weighted hue and a hue multiplier, not just a hue angle.
The builder accumulates weighted hue, saturation, lightness, multiplier and contributor count
separately. It computes `sumHue*256/sumMultiplier`, `sumSaturation/count`, and `sumLightness/count`,
then packs terrain HSL with the client's lightness-dependent saturation reductions.

**Boundary correction:** the [actual rolling loops](https://github.com/melxin/runelite/blob/1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc/runescape-client/src/main/java/class470.java#L915)
add at `x+5` and remove at `x-5` **before** producing tile x; the second axis does the same.
The fully populated interior support is therefore `[x-4,x+5] × [y-4,y+5]`, ten positions per axis.
At map edges it clips to in-range contributing underlays. Earlier “11×11” descriptions are
incorrect for this pinned routine. “Radius 5” remains a conservative reach, not a licence to
substitute a symmetric 11×11 convolution. The production Rust rolling loop already agrees with Java.

Changing an underlay can tint nearby tiles even where the changed tile is covered by an overlay:
the accumulation reads underlay IDs independently of whether overlay faces cover those tiles.
This is why an underlay companion brush could make a road edge look more intentional, but also
why it needs an affected-neighbour preview and wider invalidation than the directly painted tiles.

## 3. Overlay colours, textures and visibility

[FloorOverlayDefinition](https://github.com/melxin/runelite/blob/1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc/runescape-client/src/main/java/FloorOverlayDefinition.java#L132)
defaults to primary RGB 0, texture -1, `hideUnderlay=true`, and secondary RGB -1.
Opcodes 1/2/5/7 set primary RGB/texture/false/secondary RGB. Post-decode retains separate primary
and secondary HSL. The [builder branches](https://github.com/melxin/runelite/blob/1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc/runescape-client/src/main/java/class470.java#L1073) are:

- Texture present: use the texture average packed colour for the tile-level RGB; overlay faces
  retain a texture ID and corner brightness for texture modulation.
- Primary RGB `0xFF00FF`: use the hidden/no-colour sentinel branch; this is not a pink road material.
- Ordinary RGB: use primary HSL with the four corner light values for 3D shading.
- Secondary RGB: override the separately stored tile/minimap RGB; it does not replace primary
  HSL on the ordinary 3D faces.

`hideUnderlay` is **not** a material-blend switch. In this routine it participates in deciding
whether a flat upper tile contributes the `2340` occlusion flags. Underlay/overlay triangle
ownership comes from the shape tables. Rust preserves the definition, but `terrain_build`
explicitly omits this occlusion word because its current renderer does not consume it.

The `12345678` colour sentinel skips the applicable surface/face; texture and sentinel branches
must not be converted into ordinary palette colours. RuneLite's
[GPU SceneUploader](https://github.com/melxin/runelite/blob/1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc/runelite-client/src/main/java/net/runelite/client/plugins/gpu/SceneUploader.java)
consumes the constructed paint/model surface. A GPU upload does not redefine cache semantics.

## 4. Height, light, shadows and planes

The [slope-light calculation](https://github.com/melxin/runelite/blob/1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc/runescape-client/src/main/java/class470.java#L892)
uses neighbour height differences, a synthetic normal with vertical scale 256, direction
`(-50,-10,-50)`, an integer normalization sequence, and base brightness 96. A road can therefore
change apparent brightness when its heights change even though its floor IDs stay identical.

The builder subtracts a weighted local shadow grid: centre `>>1`, west/south `>>2`, east/north
`>>3`. This grid is written by
[FriendSystem.addObjects](https://github.com/melxin/runelite/blob/1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc/runescape-client/src/main/java/FriendSystem.java).
Clipped walls write orientation-specific 50s; clipped game objects write a default 15 or lit
model cylinder-XZ-radius/4 capped at 30, over an inclusive `(size+1)` footprint after successful
placement. These are object shadows, not a terrain-edge contrast filter.

`osrs-world::loc_stage::write_shadow` already implements the writer. The older specification's
claim that the writer is absent is stale; independent differential evidence with non-empty
placement shadows is still a separate acceptance gap. The terrain oracle fixture alone cannot
prove every placement-shadow case.

Random hue/lightness walks affect tile/minimap RGB, not the non-jittered HSL of the four 3D
corners. Palette brightness is another presentation input. Tests must distinguish those outputs.

[Minimum-plane and link-below rules](https://github.com/melxin/runelite/blob/1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc/runescape-client/src/main/java/class470.java#L1129)
use bit 8 to select minimum plane 0; otherwise an upper tile with bit 2 on settings plane 1
uses plane-1. Plane-1 bridge bit 2 later triggers `setLinkBelow`. Low-detail bit-16 visibility and
floor collision are additional branches. Painting a path does not change these flags or collapse
source/storage/collision/render planes into one plane.

## 5. Three different meanings of “blend”

| Operation | Changes map semantics? | What it actually does |
|---|---|---|
| Vanilla underlay neighbourhood blend | Underlay IDs are authored; result is derived | Weighted HSL neighbourhood average, then corner slope/shadow light |
| Triangle colour interpolation | No | Interpolates the face's vertex colours; shaped topology can interpolate intermediate corner inputs |
| Path edge smoothing | Overlay shapes/rotations change | Eiken's area-match score penalizes neighbouring edge disagreement; it does not blur materials |
| Path height smoothing / blend distance | Yes, height vertices | Repeated integer neighbourhood averaging within half-width + 1.5 + blend distance |
| RustOSRS smooth-terrain option | No | Enhanced shared-vertex colour averaging, distinct from the Reference client |
| HD ground blending | No | Separate HD override/material, vertex colour and texture-blend behavior, governed by HD settings and source pins |

HD's `osrs-world/src/hd/terrain.rs` explicitly gates colour/texture blending and handles overlay,
underlay and main override slots. It is not evidence that vanilla paths have a cache-level feather
amount. HD acceptance remains governed by the existing HD manifest and independent visual gates.

## 6. Current path implementation and evidence

The old occupancy/census/wander/per-point fitter has been removed. The sole Eiken fitter uses
8×8 area samples, all source overlay candidates, first-candidate tie breaking and six in-place
edge-refinement passes. Java integer-map iteration is observable behavior: production now models
resizes, tree bins and chain order. A gesture freezes its local origin, rectangular bounds and
available semantic region membership; camera streaming cannot change its fitted tile choices.

[Oracle tooling](../../scripts/path_oracle/run.sh) pins source SHA-256 values and OpenJDK 25.0.4.
It executes the original fitter and extracts `smoothPathHeights` unchanged except for method
visibility; UI, history and redraw adapters are inert. Goldens contain 62 fit cases, six integer
iteration sequences and 16 multi-plane smoothing cases. The test driver is not a second Java
implementation of the smoothing formula. Source files are never modified.

Full precision is held in `MapDocument`, project i32 records, undo history and worker projections.
The source editor's target write divides authored height differences by eight with truncation;
byte 1 decodes as no step. The editor export preflight applies this explicit policy to a scratch
snapshot and reports authored/reloaded differences; strict `export_region` remains available.
Invalid plane ordering or out-of-range deltas still block export. The project is not rounded.

The present preview is a useful approximation of appearance, not a Reference or HD pixel oracle:
it resolves Reference corner colours, uses average texture colour instead of sampled textures,
projects an egui mesh with a small height bias/transparency, and falls back for windows over 160
tiles or missing data. It does not preview the future height-smoothed terrain before laying.
It currently draws painted tiles, not every neighbour whose underlay tint might change.

## 7. Ranked improvements for a subsequent change

These are proposals, not additional features implemented in this replacement.

| Rank | Improvement and benefit | Compatibility / owner | Required acceptance evidence |
|---|---|---|---|
| 1 | Material-aware palette: distinguish solid/textured/hidden overlays; show primary 3D colour separately from secondary/minimap colour; retain an explicit override for intentional hidden materials | UI/read model over existing floor definitions; no change to fitting or cache format | Palette cases for texture, magenta and secondary branches; real-material screenshots |
| 2 | Preview fidelity: build the planned overlay plus optional raw height patch from a semantic snapshot; draw sampled textures and affected neighbours using the existing scene/render pipeline | Editor preview orchestration, scene appearance contract, renderer projection; Reference and HD remain separate | Preview/committed geometry and colour equality; missing-context and long-path cases; independent Reference image where available |
| 3 | Optional underlay companion: author a chosen underlay inside the road or a controlled shoulder to influence vanilla edge tint | Pure `osrs-edit` intent/patch; expose as a new option rather than changing Eiken parity defaults | Exact affected support `[-4,+5]`; border/undo/redo tests; terrain oracle and before/after comparisons |
| 4 | Junction and merge workflow: preview replacement at intersections and offer an explicit combine operation using existing overlay shapes | Pure path planner and controller; one overlay per tile limits combinations; no obstacle routing in the source-parity mode | Cross/T/Y joins, material disagreement, existing water/floors, repeated application and undo; separate behavior specification |
| 5 | Manual edge correction: inspect and rotate/replace a fitted boundary shape using existing Paint controls | Existing semantic Paint owner; same 12 stored shapes, no custom geometry | All 48 stored shape/rotation combinations, neighbour-edge inspection, save/export round-trip |
| 6 | Improved height workflow: compare before/after slopes and smoothing reach before commit, with upper-plane effects visible | Existing pure smoothing planner and preview owner | Single-unit multi-plane differential cases, 250 passes, boundary and undo proofs |

The first two improve confidence and appearance selection without altering Eiken's algorithm.
Underlay shoulders and junction composition should be individually specified and opt-in. Artificial
alpha feathering or arbitrary road meshes would be enhanced rendering features with no equivalent
in the target map format; they need a distinct compatibility contract before implementation.
