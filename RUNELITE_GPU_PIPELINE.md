# RuneLite GPU Pipeline Research Index

Status: **Historical deep-dive reconciled by M10 foundation audit**

This root document is research context, not a canonical RustOSRS renderer specification. The earlier version contained several overbroad conclusions about priority sorting, bridge/maplevel grouping, depth comparison, alpha depth writes, and sampling. Git history retains the full historical deep dive. Current implementation decisions must use the canonical docs listed at the end of this file.

## Exact imported source identity

Imported RuneLite tree:

`5afef996a992bacc73655860681269242e90d6e8`

Key blobs:

- `SceneUploader.java` `83ac701f1b2ee7a039879941bcc710527dbc54c1`
- `ModelUploader.java` `35347963838d1be74deddd59027a73623d7872f3`
- `GpuPlugin.java` `8c233cb381e288146af47ac375fa3dfc41a53d76`
- `Zone.java` `80594f4ca2759a96c9a3a9800008790b91c5d692`
- `TextureManager.java` `e830a518dc13c173f22f006fe52cb25e3c0fc8b3`
- `Renderable.java` `bce439b4dd2000a98d83191f3d0348b64ff83b3c`
- `vert.glsl` `d899cf3180295bd18d30adf901fd7a460e560318`
- `frag.glsl` `0ca7180d50ef90e5083c85f7182e60521ef76baa`

Until the exact upstream RuneLite commit for this imported tree is recorded, cite these RustOSRS tree/blob identities.

## 1. Render modes

The imported API defines:

```text
RENDERMODE_DEFAULT           = 0
RENDERMODE_SORTED            = 1
RENDERMODE_SORTED_NO_DEPTH   = 2
RENDERMODE_UNSORTED          = 3
RENDERMODE_UNSORTED_NO_DEPTH = 4
```

These modes materially affect sorting/depth treatment. Do not describe RuneLite GPU as one universal model-rendering path.

## 2. Priority sorting is path-specific

`ModelUploader.uploadSortedModel(..., prioritySort)` only performs the 0..11 priority threshold/special-queue algorithm when `prioritySort` is true.

The imported plugin enables that for `SORTED_NO_DEPTH`. Ordinary dynamic upload passes `false`; static opaque upload does not globally reproduce the software priority algorithm.

Static alpha work in `Zone` sorts models by distance and faces by depth bucket rather than universally running the software priority queues.

Therefore:

- pinned OSRS `Model.method5946` remains the software/client priority oracle;
- imported RuneLite GPU is a separate render-mode-specific oracle;
- RustOSRS Reference mode may deliberately choose the software algorithm for priority-sensitive content.

## 3. Static versus dynamic upload

RuneLite demonstrates a useful architectural split between reusable uploaded scene geometry and runtime/dynamic model upload.

RustOSRS adopts the architectural lesson, not every incidental implementation limit:

- static/dynamic describes render stability;
- full semantic face metadata remains retained;
- renderer handles/material indices are not semantic texture IDs;
- fixed RuneLite texture-array capacity is not cache truth.

## 4. Bridge/maplevel correction

RuneLite `SceneUploader` computes a local `maplevel` to choose tile-settings/roof grouping information.

The geometry tile is still fetched from `tiles[level][x][y]`.

A bridge therefore does **not** mean the tile geometry itself "joins the lower plane's upload pass." The grouping/settings lookup can reference another level while semantic storage identity remains unchanged.

Target-client `Tile.minPlane` and `Tile.originalPlane` are separate semantic scene fields and must not be reduced to this RuneLite uploader strategy.

## 5. Reverse-Z

Imported `GpuPlugin` establishes:

```text
clear depth = 0
comparison  = GL_GREATER
```

The Reference comparison is strict greater, not greater-or-equal.

Imported projection is reverse-Z with no finite far plane. Authored face bias is applied before the perspective divide, so normalized depth separation decreases with distance.

## 6. Alpha/depth correction

The imported renderer does not establish a universal "alpha pass disables depth writes" rule.

Ordinary depth-tested work retains normal depth writes unless the owning no-depth render mode/range says otherwise.

RustOSRS must test this explicitly for any profile claiming RuneLite-like Reference behavior.

## 7. Texture image construction

Imported `TextureManager` uses 128x128 source texture images for this snapshot and produces RGBA data where:

- source RGB `0` -> transparent;
- nonzero RGB -> alpha `255`.

The staged fragment shader uses level-0 alpha for cutout/discard behavior.

The 128 size and imported count/capacity are renderer implementation facts, not universal cache-semantic limits.

## 8. Texture filtering and wrap

Correct audited summary:

```text
MAG                         = NEAREST
MIN at filtering level 0    = NEAREST
MIN at filtering level >=1  = NEAREST_MIPMAP_LINEAR
imported default level      = 1
S wrap                      = CLAMP_TO_EDGE
T wrap                      = OpenGL default repeat in audited setup
```

Calling this simply "nearest filtered" is incomplete.

## 9. Texture lighting/brightness

Brightness/color processing occurs in the shader-side renderer path. Textured model faces consume a baked lightness payload rather than recalculating OSRS semantic normal lighting in the shader.

RustOSRS semantic lighting remains upstream. Renderer profiles may transform the resulting color only according to a separately tested material contract.

## 10. UV preparation

`ModelUploader.computeFaceUvs` remains the imported differential oracle for:

- canonical no-selector mapping;
- explicit texture-triangle basis mapping;
- projected/dynamic texture-plane behavior.

M10 CPU preparation already targets these structural contracts. Physical texture sampling remains later GPU work.

## 11. What this import can and cannot prove

The imported RuneLite tree can prove facts about that imported renderer snapshot.

It does not automatically prove:

- exact OSRS software renderer behavior;
- behavior of a different RuneLite upstream commit;
- that historical shader files in another staged tree are actually loaded by this imported October plugin;
- external visual parity of RustOSRS without an independently generated capture.

## Canonical RustOSRS renderer documents

Use these for implementation decisions:

- `docs/specs/face-materials.md`
- `docs/specs/planes-bridges.md`
- `docs/adr/ADR-0004-reverse-z-raster-conventions.md`
- `docs/adr/ADR-0005-zone-compiled-hybrid-rendering.md`
- `docs/blueprint/12-GPU-DATA-PASSES.md`
- `docs/blueprint/16-VERIFICATION-ARCHITECTURE.md`
- `docs/verification/SOURCE-PINS.md`
- `docs/verification/PARITY-MATRIX.md`
- `docs/implementation/M10-FOUNDATION-AUDIT.md`
