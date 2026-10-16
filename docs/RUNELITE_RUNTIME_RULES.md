# OSRS / RuneLite Runtime Rules

Status: **Historical research note reconciled by the M10 foundation audit**

This root document is **not canonical specification text**. Canonical behavior lives in `docs/specs/`, `docs/adr/`, `docs/verification/`, and the implementation audit documents.

The earlier version of this note contained useful research but also several conclusions that are now proven wrong. Git history retains that earlier text. This replacement records only the corrected runtime rules that remain useful as a research index.

## Authoritative source baseline

Primary semantic client reference:

`melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`

Important pinned files:

- `class470.java` `1cd9cad5cb4be865dcae94dc633bba821644dc84`
- `FriendSystem.java` `b8cf51b6ee673181d8a115e28153a77f5978c390`
- `Scene.java` `f15260a63103952fe8f5ffbdb62f5c7c39d94565`
- `Tile.java` `74c221f904f8b4748a7733eeade057934f1e0879`
- `ModelData.java` `2cc9406b2504fbd4fae0c0c952aa2d133809e928`
- `Model.java` `c2aa55c0e8fea89fae0da33d782119f8c109cacf`

Imported RuneLite renderer evidence is pinned separately in `docs/verification/SOURCE-PINS.md`.

## R1. Terrain construction is in `class470.method9712`

The earlier claim that pinned `class470` was unrelated to terrain construction is false.

Most of the class is text-layout code, but its final static `method9712(WorldView)` is the full terrain construction routine. It performs slope lighting, placement-shadow consumption, underlay neighborhood blending, overlay handling, tile emission, `minPlane`, scene normal finalization, and bridge relinking.

Current canonical contract: `TERRAIN-004` in `docs/specs/terrain.md`.

## R2. Terrain shadow state comes from placed clipped locs

`Tiles.Tiles_underlays2` is not generic adjacent-ground contrast.

`FriendSystem.addObjects(...)` writes it from `ObjectComposition.clipped` placement behavior:

- wall cases use orientation-specific value `50` writes;
- game objects may use model height / 4, defaulting to `15` and capped at `30`;
- the larger prior value wins.

Terrain slope/light construction subtracts a weighted neighborhood of this grid.

## R3. Random terrain jitter does not drive the 3D underlay corner HSL

The terrain builder computes non-jittered blended HSL for the four 3D corner colors passed into `Scene.addTile`.

Hue/lightness random walk is used for the separately derived palette/minimap-style RGB values. Hue wraps with `& 255`; lightness clamps.

Do not make 3D terrain color parity depend on invented random seeds.

## R4. Planes are multiple distinct contracts

Do not collapse:

- source/encoded plane;
- collision plane;
- storage plane;
- mutable `Tile.plane`;
- immutable `Tile.originalPlane`;
- `Tile.minPlane`;
- linked-below relation;
- renderer-local roof/settings grouping level.

`Tile.minPlane` participates in target client draw traversal. It is not merely a RuneLite upload optimization.

RuneLite `SceneUploader.maplevel` can select settings/roof lookup from another level while still uploading `tiles[level][x][y]`. It does not move semantic tile geometry to another storage plane.

## R5. Bridge collision and link-below are different operations

The initial loader may use collision plane `encodedPlane - 1` when the bridge bit applies.

`Scene.setLinkBelow` later structurally shifts scene tile storage, updates mutable tile/object plane state, retains linked-below identity, and clears the top slot.

Neither operation is a universal "bridge-adjusted plane" helper.

## R6. Scene does cross-model normal reconciliation

The earlier root-note statement that the engine "never merges walls" was misleading/incorrect.

Correct distinction:

- Scene does **not weld separate wall/object geometry into one mesh**.
- Scene **does reconcile normals across separate ModelData objects** through `Scene.method5585/method5587` and `ModelData.method5262`.
- when the owning merge enables matched-face hiding, fully matched faces become render type `2` and are suppressed later.
- the two arms of a boundary object are directly reconciled with `hideMatchedFaces=false`.

Canonical contract: `NORMALS-002` / `NORMALS-003`.

## R7. Object contrast is already scaled during decode

Object-definition opcode `39` decodes:

```text
contrast = signed_byte * 25
```

Static-object lighting then uses:

```text
ambient  = decodedAmbient + 64
contrast = decodedContrast + 768
light    = (-50, -10, -50)
```

Do not multiply contrast by `25` a second time.

## R8. Raw alpha sentinels act before later draw alpha

Before ModelData lighting:

```text
raw alpha -2 -> render type 3
raw alpha -1 -> render type 2
```

For untextured output:

- type 3 -> gray `128`, flat marker `c == -1`;
- type 2 -> suppressed marker `c == -2`.

Later software draw-alpha handling of `-1 -> 253` is a separate stage. Renderer code must not re-admit `c == -2` faces.

## R9. Software priority and RuneLite GPU priority are different targets

Pinned `Model.method5946` contains the software/client priority 0..11 threshold/special-queue algorithm.

The imported RuneLite GPU does not apply that algorithm universally. Its `ModelUploader` only uses the priority queue when called with `prioritySort=true`, notably for `RENDERMODE_SORTED_NO_DEPTH`. Ordinary static/dynamic paths may rely on depth or simpler depth ordering.

RustOSRS Reference mode currently chooses the software/client ordering contract for priority-sensitive content. That is an explicit project/reference decision.

## R10. Imported RuneLite reverse-Z uses strict greater

Imported `GpuPlugin` uses:

```text
depth clear = 0
reverse Z
GL_GREATER
```

RustOSRS Reference baseline is therefore strict `Greater`, not `GreaterEqual`.

The imported projection has no finite far plane, so the authored `bias / 128` term produces distance-dependent normalized depth separation.

## R11. Do not assume blended alpha means depth writes off

The imported RuneLite snapshot does not globally disable depth writes for alpha geometry. Explicit no-depth render modes are separate.

A conventional depth-read-only transparency path may exist in an Enhanced profile, but must not be mislabeled Reference parity.

## R12. RuneLite texture sampling is not simply nearest

Imported renderer behavior:

- magnification: nearest;
- minification at filter level 0: nearest;
- minification at filter level >=1: `NEAREST_MIPMAP_LINEAR`;
- imported config default level: 1;
- S wrap: clamp-to-edge;
- T wrap: default repeat in the audited setup.

Texture RGB value zero uploads transparent. Shader-side level-0 alpha participates in cutout/discard behavior.

## R13. Self-generated screenshots are regression evidence

A RustOSRS Reference image generated by RustOSRS proves internal regression stability.

It does not independently prove external 1:1 parity. That claim requires a separately generated, provenance-complete client/RuneLite visual oracle.

## Current canonical documents

Use these instead of this research note for implementation decisions:

- `docs/specs/terrain.md`
- `docs/specs/planes-bridges.md`
- `docs/specs/normals-lighting.md`
- `docs/specs/face-materials.md`
- `docs/adr/ADR-0004-reverse-z-raster-conventions.md`
- `docs/adr/ADR-0005-zone-compiled-hybrid-rendering.md`
- `docs/verification/SOURCE-PINS.md`
- `docs/verification/PARITY-MATRIX.md`
- `docs/implementation/M10-FOUNDATION-AUDIT.md`
