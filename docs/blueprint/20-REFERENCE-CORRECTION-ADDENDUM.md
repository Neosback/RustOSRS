# Reference Correction Addendum

Status: **Normative blueprint addendum, 2026-10-10**

This addendum supersedes conflicting statements in blueprint documents 10, 12, and 16, and in root `RUNELITE_*.md` research notes. It does not replace unaffected sections of those documents.

Primary correction record:

`docs/implementation/REFERENCE-PROVENANCE-CORRECTION-2026-10-10.md`

## 1. Terrain builder and tile color construction

Contrary to the earlier checkpoint-3B audit, the exact public-pinned `class470.java` contains the complete terrain builder in `method9712(WorldView)`.

Effective blueprint rule:

- terrain topology remains verified as before;
- terrain-color **source semantics are now verified**;
- production Rust terrain-color construction remains required;
- the old "class470 is unrelated" conclusion is superseded;
- `Tiles_underlays2` is an object-derived shadow/clipping contribution, not a neighboring-ground color contrast grid;
- random hue/lightness affects the additional derived RGB/minimap-style field, while the four corner HSL/light values passed to `addTile` use the unjittered underlay HSL;
- hue jitter wraps with `& 255`.

M11/M12 must not claim reference terrain-color parity until the production builder exists.

## 2. Plane and bridge interpretation

`class470.method9712` also writes tile minimum-plane state. This is source-client scene behavior.

RuneLite `SceneUploader.maplevel` remains renderer policy. For bridge-tagged tiles the captured uploader increments the local map level for VIS_BELOW/roof lookup but still uploads `tiles[level]`.

Effective rule:

- source plane;
- collision plane;
- storage plane;
- render/height level;
- tile minimum plane;
- linked-below state;
- RuneLite roof/VIS_BELOW lookup level

remain distinct concepts.

Any earlier wording that bridge `maplevel` itself causes geometry to "join the lower plane's pass" is superseded.

## 3. Scene ModelData reconciliation

Root research text claiming `Scene` never performs wall/object normal reconciliation is superseded.

Pinned `Scene.method5585`, `method5586`, and `method5587` invoke `ModelData.method5262` across eligible neighboring ModelData.

The correct architectural rule remains:

- merge normal state where reference positions coincide;
- optionally mark fully matched faces render type `2` when the caller requests hiding;
- do not weld topology or object identity.

Canonical `NORMALS-002` and `NORMALS-003` own this behavior.

## 4. Software priority vs RuneLite GPU dispatch

The software-client `Model` priority algorithm remains the exact FACE-002 ordering oracle.

The captured RuneLite GPU implementation does not apply it universally. Priority sorting is enabled for `RENDERMODE_SORTED_NO_DEPTH`; other static/dynamic paths use different depth/alpha machinery.

Therefore:

- RustOSRS must preserve priority metadata universally;
- the Reference ordered path may intentionally use the exact software-client algorithm;
- this is not described as a one-for-one clone of RuneLite render-mode dispatch;
- a future RuneLite-specific profile may expose explicit `UNSORTED`, `SORTED`, and `SORTED_NO_DEPTH` behavior if desired.

## 5. Reverse-Z amendment

Reference baseline depth comparison is:

```text
depth clear   = 0.0
depth compare = Greater
near          = larger depth
far           = smaller depth
```

This supersedes blueprint-12's `GreaterEqual` Reference baseline.

`GreaterEqual` may be an enhanced/alternative policy only when named as such.

Authored face bias still uses the audited clip-space adjustment initially and requires both equal-depth and distance-sensitive fixtures.

## 6. Transparency depth-state qualification

Blueprint-12's generic statement that conventional translucent faces disable depth writes is not an exact RuneLite reference rule.

Reference implementation must derive alpha/no-depth state from explicit renderer contracts and fixtures. Captured RuneLite behavior is render-mode/path specific.

An enhanced renderer may use conventional depth-test-on/depth-write-off alpha blending, but that is a product policy unless proven equivalent for the reference scene.

## 7. Texture reference state

Captured RuneLite renderer facts for the Reference profile:

- 128x128 RGBA8 texture-array layers;
- source RGB `0` uploads as transparent zero RGBA;
- nonzero RGB uploads with alpha `255`;
- fragment shader discards when level-0 sampled alpha is below `1.0`;
- source textures are uploaded with provider brightness temporarily set to `1.0`;
- shader applies brightness with `pow`;
- textured faces use `fHsl / 127` lightness unless texture-light mode selects RGB tinting;
- magnification filter is nearest;
- minification is nearest at level 0 configuration and can become nearest-mipmap-linear when mipmaps/anisotropy are enabled;
- S wrap is clamp-to-edge;
- T wrap is not explicitly changed by the captured setup and follows default repeat behavior.

These details refine blueprint-12's prior generic "reference-compatible sampler" wording. Historical `TEXTURE_COUNT = 256` remains a RuneLite implementation limit, not semantic truth.

## 8. Alpha sentinel boundary

FACE-003 must distinguish:

- ModelData alpha `-2` selecting render type `3` and the gray/lightness-128 untextured result;
- ModelData alpha `-1` selecting render type `2` and suppressed/hidden untextured face result;
- later normal software draw-time raw alpha `-1` interpretation as unsigned `253` when that raw value reaches the draw path.

Do not flatten these into one sentinel rule.

## 9. Object contrast scale

`ObjectComposition` opcode `39` stores `readByte() * 25`. Later lighting adds `768` to that already-scaled field.

Any blueprint prose or test description treating the stored contrast as the raw byte is superseded.

## 10. Visual oracle terminology

A screenshot generated by RustOSRS's own Reference profile is a deterministic **regression golden**.

It is not, by itself, independent evidence of visual parity.

A verified external visual-parity claim requires a separately pinned oracle such as:

- captured client framebuffer/reference image with exact scene/camera/profile provenance;
- independently rendered RuneLite capture with pinned renderer/source identity and deterministic scene inputs;
- another executable raster oracle whose provenance is independent of the RustOSRS renderer under test.

Blueprint-16 remains correct that P3 is the visual layer, but self-generated images cannot be the sole evidence for a "1:1" claim.

## 11. Roadmap impact

M10 CP1-CP5 remain valid. Nothing in this correction invalidates their render-extraction, priority-oracle, classification, material/UV, or zone ownership work.

Before the first viewport is allowed to claim reference terrain appearance, add a semantic closure checkpoint for `TERRAIN-004`:

1. port `class470.method9712` behavior into the semantic scene pipeline;
2. model the object-derived shadow/clipping input explicitly;
3. expose tile minimum-plane output;
4. create source-pinned exact fixtures;
5. add ordinary Tier C execution;
6. only then mark the terrain builder capability supported.

M11 can still be the first GPU viewport milestone, but its reference-terrain exit gate now depends on that semantic closure rather than a permanently blocked capability.
