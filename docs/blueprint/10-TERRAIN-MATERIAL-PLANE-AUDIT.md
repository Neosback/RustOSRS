# Terrain, Material, and Plane Audit

Status: **Superseded and corrected by M10 foundation audit**  
Decision class: `SOURCE_AUDIT`

This document originally recorded the pre-implementation audit of terrain, materials, and planes. The M10 foundation audit found that several conclusions in that version were based on incomplete source inspection. Git history preserves the original analysis; this current version records the corrected findings that implementation may rely on.

Canonical specifications are:

- `docs/specs/terrain.md`
- `docs/specs/planes-bridges.md`
- `docs/specs/face-materials.md`
- `docs/specs/normals-lighting.md`
- `docs/verification/SOURCE-PINS.md`
- `docs/verification/PARITY-MATRIX.md`

## 1. Terrain topology

Terrain shape/rotation topology remains correctly pinned to `SceneTileModel` and exact 13x4 fixtures.

Flat paint triangle order, the `12345678` skipped-surface sentinel, shaped-face ownership, and the existing integer coordinate conventions remain valid.

No correction was required to implemented `TERRAIN-001/002` topology.

## 2. Complete terrain builder

### Corrected finding

The old statement that public pinned `class470` was unrelated to terrain building is false.

At:

`melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`

`class470.java` blob:

`1cd9cad5cb4be865dcae94dc633bba821644dc84`

contains `method9712(WorldView)`, the complete terrain construction routine.

It includes:

- random hue/lightness presentation walk;
- slope-light calculation;
- placement-derived shadow subtraction;
- separable radius-5/11x11 underlay blend;
- weighted underlay HSL;
- overlay texture/magenta/secondary-color handling;
- tile emission;
- `Tile.minPlane` assignment;
- scene normal reconciliation/finalization;
- bridge link-below processing.

Therefore `TERRAIN-004` is no longer source-blocked.

Current status:

`SOURCE_VERIFIED / IMPLEMENTATION_REQUIRED`

## 3. Terrain jitter correction

The old audit overestimated random jitter's effect on 3D terrain.

The builder uses non-jittered blended underlay HSL plus per-corner slope/shadow light values for the four 3D underlay corner colors passed to `Scene.addTile`.

Jittered hue/lightness is used for separately derived palette/minimap-style RGB values.

Consequences:

- 3D corner-color parity does not require random-seed pinning;
- hue presentation jitter wraps with `& 255`;
- presentation lightness clamps `0..255`;
- any palette/minimap RGB implementation must still model jitter explicitly.

## 4. Shadow-grid correction

`Tiles.Tiles_underlays2` is placement-derived shadow state.

Pinned `FriendSystem.addObjects(...)` blob:

`b8cf51b6ee673181d8a115e28153a77f5978c390`

proves:

- `clipped` walls write orientation-specific shadow value `50`;
- clipped game objects use a model-derived value, default `15`, height/4 when a concrete model exists, capped at `30`;
- the larger existing shadow value is retained.

Terrain-color implementation must therefore run against the correct placement-derived shadow inputs. Describing this grid as generic neighboring-ground contrast is prohibited.

## 5. Plane audit correction

Target `Tile` has distinct:

- mutable `plane`;
- `originalPlane`;
- `minPlane`;
- `linkedBelowTile`.

The terrain builder sets `minPlane` from visibility/bridge bits, and `Scene` consumes it in tile traversal/draw eligibility.

Therefore `minPlane` is semantic target-client scene state, not only a RuneLite upload strategy.

Imported RuneLite `SceneUploader.maplevel` remains renderer-derived grouping/settings lookup. It may choose bridge-adjusted settings/roof arrays, but geometry still comes from `tiles[level]`; it does not rewrite semantic storage identity.

## 6. Normal-finalization correction

The terrain builder calls `Scene.method5585(-50, -10, -50)` after tile construction.

Pinned `Scene.method5585/method5587` performs actual cross-object ModelData normal reconciliation before final lighting.

This does not weld meshes, but it does merge lighting-normal state and may suppress fully matched faces when the owning call enables hiding.

The canonical M7 Rust implementation was already aligned with this behavior. The error existed in stale research prose, not the M7 code.

## 7. Material/texture audit correction

The imported RuneLite texture system provides renderer evidence, not semantic capacity limits.

Confirmed imported behavior:

- 128x128 texture image construction for this snapshot;
- RGB zero -> transparent;
- level-0 alpha participates in shader cutout;
- shader-side brightness/color handling;
- nearest magnification;
- minification nearest at level 0, `NEAREST_MIPMAP_LINEAR` at level >=1;
- imported config default level 1;
- S clamp-to-edge;
- T default repeat in the audited setup.

Do not encode fixed RuneLite texture count/array capacity into `TextureId` semantics.

M10's full-width material handles and exact CPU UV preparation remain sound.

## 8. Face-ordering audit correction

Pinned software/client `Model.method5946` and imported RuneLite GPU are separate targets.

The software algorithm has the priority 0..11 queues, average thresholds, and priority-10/11 special stream.

Imported RuneLite GPU only enables its priority-queue branch where `prioritySort=true`, notably `SORTED_NO_DEPTH`. Other static/dynamic/alpha paths can rely on depth/distance sorting.

M10's software-priority implementation remains valid for RustOSRS Reference policy. The prior claim that RuneLite GPU universally reproduces the same algorithm is withdrawn.

## 9. Reverse-Z audit correction

Imported RuneLite uses:

```text
clear depth = 0
compare     = GL_GREATER
```

RustOSRS Reference profile now uses strict `Greater`, not `GreaterEqual`.

The imported projection has no finite far plane. Authored bias is applied before perspective division, so its normalized-depth effect shrinks with distance.

Reference alpha rendering must not assume global depth-write disable merely because blending is enabled; imported no-depth behavior is represented separately.

## 10. Current implementation disposition

Already solid and exact-tested:

- cache/decode foundation;
- model selection/mirroring/transforms;
- placement/footprints/heights;
- terrain topology;
- bridge structural relinking;
- normal reconciliation/final lighting;
- morph/contour/verified legacy animation scope;
- M10 extraction/rebase;
- software priority preparation;
- alpha interpretation inputs;
- render classification;
- material/UV CPU preparation;
- 8x8 zone invalidation/generation ownership.

Open foundation gates:

1. port `TERRAIN-004` and add an executable differential fixture;
2. represent/test target `minPlane/originalPlane` consumption before relying on roof/plane visibility;
3. explicitly prevent baked `c == -2` faces from entering Reference draw packets;
4. implement strict-Greater/no-far/bias fixtures for GPU work;
5. implement/reference-test texture pixel/sampler behavior;
6. add an independent external visual oracle before making measured 1:1 claims.

## 11. Authority rule

Where this corrected audit conflicts with historical root research or pre-M10 blueprint prose, current canonical specs/ADRs and the M10 foundation audit win.
