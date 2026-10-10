# Reference Provenance Correction, 2026-10-10

Status: **Normative correction to pre-M10 provenance conclusions**

This record corrects source-audit conclusions discovered after M9 merged. It does not rewrite milestone history. Where an older research note, M5/M9 audit statement, source-pin note, or blueprint statement conflicts with this correction, this record and the active atomic specs take precedence.

## Pinned sources rechecked

Public client/deob source:

`melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`

Important exact files:

- `class470.java` blob `1cd9cad5cb4be865dcae94dc633bba821644dc84`
- `Scene.java` blob `f15260a63103952fe8f5ffbdb62f5c7c39d94565`
- `ModelData.java` blob `2cc9406b2504fbd4fae0c0c952aa2d133809e928`
- `ObjectComposition.java` blob `079451cd9a6dcfd2666efd15b0524250eaafe4c4`

Imported RuneLite renderer evidence remains pinned by RustOSRS blob/tree identity in `docs/verification/SOURCE-PINS.md`.

## C-2026-01: terrain builder source gate

**Corrected conclusion:** the exact public-pinned `class470.java` does contain the terrain builder. The method is `class470.method9712(WorldView)`.

The method performs, in source order:

- client terrain hue/lightness random-walk updates;
- slope-derived tile lighting;
- subtraction of the weighted `Tiles_underlays2` shadow/clipping grid;
- the separable radius-5, 11x11 underlay neighborhood accumulation;
- weighted underlay HSL construction;
- overlay texture/sentinel/secondary-color handling;
- `Scene.addTile(...)` calls;
- `Scene.setTileMinPlane(...)` calls;
- scene normal/finalization call `method5494(-50, -10, -50)`;
- bridge `Scene.setLinkBelow(...)` calls.

The previous statement that the pinned `class470` was unrelated text-layout code was caused by an incomplete inspection of the file. Most of the file is unrelated, but its final large static method is the terrain builder.

### TERRAIN-004 status

The source/provenance gate is now **VERIFIED**.

The RustOSRS production implementation is still **REQUIRED**. No milestone may claim executable `TERRAIN-004` parity until the builder is ported and an exact fixture exercises the production implementation.

The target-profile `terrain_color_builder` capability state is `source_verified`. That state means the exact source oracle is pinned; it does **not** mean the Rust production implementation exists. Production readiness remains tracked separately by the `TERRAIN-004` production status and executable verification coverage.

## C-2026-02: terrain jitter scope

`class470.method9712` builds two related underlay color values:

- an unjittered packed HSL used with the four corner light values passed to `Scene.addTile`;
- a jittered HSL used to derive the additional RGB/minimap-style field.

Therefore the ordinary 3D terrain corner-color path consumed by the audited RuneLite uploader does not require random jitter to reproduce its four corner colors. Jitter still exists in the client builder and still affects the additional derived RGB field. Documentation must not claim either that jitter is absent or that all 3D terrain colors require a random seed.

Hue jitter uses bit wrapping with `& 255`; lightness jitter is clamped to `0..255` before repacking.

## C-2026-03: `Tiles_underlays2` is a shadow/clipping contribution

The grid subtracted by the terrain slope-light calculation is not "adjacent contrasting ground".

The pinned initial object-placement path writes `Tiles.Tiles_underlays2` from loc clipping/shadow behavior. Clipped boundary objects write value `50` at orientation-dependent cells. Clipped game objects can write a height-derived value over their covered footprint.

This creates an ordering dependency: terrain lighting/color construction consumes scene/object-derived shadow state. A Rust port must model this input explicitly rather than deriving it from neighboring underlay color contrast.

## C-2026-04: tile minimum plane is client scene behavior

`class470.method9712` explicitly calls `Scene.setTileMinPlane` with this rule:

1. tile flag `8` on the current plane -> minimum plane `0`;
2. otherwise, when plane > 0 and plane-1 bridge condition is represented by tile-settings plane 1 bit `2` -> minimum plane `plane - 1`;
3. otherwise -> current plane.

This is source-client scene behavior, not solely a RuneLite GPU upload policy.

The captured RuneLite `SceneUploader` has a related but distinct renderer rule. It begins with `maplevel = level`, increments `maplevel` for a bridge tile, uses that derived level for `VIS_BELOW` and roof lookup, and still uploads `tiles[level]`. Documentation must not say bridge geometry is physically reassigned to the lower uploader pass by this variable alone.

## C-2026-05: cross-model wall/object normal reconciliation

The root runtime research statement that the engine never merges wall meshes/normals is false.

Pinned `Scene.method5585`, `method5586`, and `method5587` invoke `ModelData.method5262` while finalizing eligible scene ModelData. The canonical `NORMALS-002` and `NORMALS-003` distinction remains correct:

- objects are not mesh-welded;
- vertex/face/object identity remains separate;
- coincident eligible vertices can receive merged normal state;
- qualifying fully matched faces can be marked render type `2` when the caller enables matched-face hiding;
- final lighting consumes merged normals.

Older root prose that says "no wall merging" is superseded specifically for normal reconciliation. It remains correct that this process is not boolean mesh welding.

## C-2026-06: software-client priority and RuneLite GPU priority are different targets

The software-client `Model` priority algorithm remains the canonical exact oracle for `FACE-002` semantics.

The captured October RuneLite GPU implementation does not apply that priority sorter universally. `GpuPlugin` enables `prioritySort` only for `RENDERMODE_SORTED_NO_DEPTH`; `ModelUploader` otherwise uses its ordinary depth/alpha paths, and static zone upload does not run the full software priority queue.

Therefore docs must distinguish:

- **software-client/reference semantic ordering**: exact priorities `0..11`, threshold groups, and 10/11 interleave;
- **captured RuneLite GPU operational behavior**: conditional use of that sorter based on render mode.

RustOSRS Reference profile currently chooses the stronger software-client ordering contract for renderables classified as requiring ordered reference behavior. That is a deliberate renderer policy and must not be described as an exact copy of RuneLite's operational dispatch.

## C-2026-07: reverse-Z depth and alpha pass differences

The captured RuneLite GPU renderer uses strict `GL_GREATER`, not `GreaterEqual`.

Accordingly, the RustOSRS Reference profile should use strict `Greater` for the parity baseline. `GreaterEqual` may exist only as an explicitly documented enhanced/alternative policy.

The captured renderer also does not establish a universal "all alpha draws disable depth writes" contract. RuneLite render modes include no-depth behavior and alpha ordering, but a RustOSRS alpha pass must not claim exact RuneLite parity merely from the generic conventional-transparency rule.

Authored face-bias application still requires an equal/coplanar depth fixture, including distance-dependent projection behavior.

## C-2026-08: texture upload and sampling details

Captured RuneLite texture upload provides concrete renderer evidence:

- texture array storage is `128x128`, `GL_RGBA8` in this snapshot;
- source pixel value `0` becomes zero RGBA, while nonzero RGB becomes alpha `255`;
- level-0 sampled alpha below `1.0` is discarded by the fragment shader;
- upload temporarily sets the client texture-provider brightness to `1.0`, then shader brightness is applied with `pow`;
- textured-face light contribution uses the 7-bit lightness `fHsl / 127` unless the configured texture-light mode selects RGB tinting;
- magnification is `GL_NEAREST`;
- minification starts as `GL_NEAREST`, but when mipmaps/anisotropy are enabled it becomes `GL_NEAREST_MIPMAP_LINEAR`;
- S wrap is explicitly `GL_CLAMP_TO_EDGE`;
- T wrap is not overridden in the captured setup and therefore relies on the GL default repeat behavior.

These are RuneLite renderer-reference details, not cache semantics. RustOSRS material/texture specs should preserve them as a Reference-profile oracle without encoding the historical `TEXTURE_COUNT = 256` as semantic truth.

## C-2026-09: alpha sentinel preprocessing

Before final model lighting, pinned `ModelData` interprets face-alpha sentinels:

- alpha `-2` selects render type `3`, whose untextured color result is gray/lightness `128`;
- alpha `-1` selects render type `2`, whose final face marker suppresses the ordinary untextured face path.

Separately, the normal software `Model.drawFace` path interprets raw signed alpha `-1` as unsigned `253` where that raw alpha reaches the draw path.

Specs must keep preprocessing and draw-time interpretation distinct.

## C-2026-10: object contrast decode scale

Pinned `ObjectComposition` opcode `39` stores:

`contrast = readByte() * 25`

before the later static-object lighting offset is applied. Therefore `LIGHTING-001` must state that `objectDefinition.contrast` is already decoder-scaled by `25`, then the model-lighting path adds `768`.

## C-2026-11: reference visual oracle limitation

A screenshot generated only by RustOSRS's own Reference profile is a regression golden, not an independent external proof of visual parity.

P3 may be called externally corroborated only when the expected image or equivalent raster artifact comes from a separately pinned client/RuneLite capture, executable oracle, or independently reproduced fixture with provenance. Self-generated images remain useful for deterministic regression but cannot by themselves substantiate a "1:1" claim.

## Immediate implementation consequences

1. Correct active terrain/source documentation from `REVISION_SENSITIVE/BLOCKED BY MISSING SOURCE` to `VERIFIED SOURCE / IMPLEMENTATION REQUIRED`.
2. Keep the target capability state at `source_verified` while source provenance is the dimension it reports; do not use that state as a substitute for production implementation coverage.
3. Port `class470.method9712` in a dedicated semantic checkpoint before claiming `TERRAIN-004` executable parity.
4. Include clipping/shadow-grid inputs and tile minimum-plane output in that fixture family.
5. Keep M10 priority preparation as the software-client Reference-profile oracle, while documenting that captured RuneLite GPU dispatch uses the sorter conditionally.
6. Change Reference-profile depth compare from `GreaterEqual` to `Greater` before M11 GPU implementation.
7. Add exact texture sampler/upload rules and an authored-bias distance fixture before M12 parity claims.
8. Treat RustOSRS-generated screenshots as regression goldens until an external visual oracle is added.

## Historical milestone note

M5 and M9 are not retroactively re-authored. Their `TERRAIN-004` blocked conclusions were based on an incomplete source inspection. This correction supersedes that provenance conclusion while preserving the milestone record. The implementation gap remains real and is now represented as REQUIRED work rather than unknown source behavior.
