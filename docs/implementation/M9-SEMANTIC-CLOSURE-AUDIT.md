# M9 Semantic Parity Closure Audit

Status: **Historical M9 PASS record with post-M9 terrain provenance correction**

Historical branch: `impl/m9-semantic-parity-closure`  
M8/main baseline: `23a2602ed62000a4843d7118b34427569ac176b9`  
Historical final M9 branch head: `91325e219ac3988914ca60885b4606ec40502a04`  
Historical M9 squash/main result: `f7d004471562f723ee8115f5a3e28e937e3a6f5d`

## Purpose

This document preserves the M9 semantic-exit audit while recording one important conclusion that was later proven wrong during M10 foundation review.

M9 remains a valid milestone for coordinate/placement ownership, deterministic semantic-scene identity, and renderer-boundary classification. The historical `TERRAIN-004 = BLOCKED because the builder source is missing` conclusion is superseded.

## Historical M9 closure

M9 closed:

- `COORD-001` cross-layer tile/region/local behavior;
- `LOC-PLACEMENT-005` deterministic side-effect ownership/regeneration inputs;
- renderer-independent `rustosrs-semantic-scene-v1` hashing;
- explicit target capability diagnostics;
- row-by-row separation of semantic gaps from renderer-owned work.

Pinned semantic scene hashes remain unchanged:

| Golden semantic scene | SHA-256 |
|---|---|
| composed terrain + loc scene | `50974f0232192dbd97c623d31bdbe208432ef64a13121c2852230afcc262f4a6` |
| four-plane linked-below scene | `bf7a7876864142f75a29c47836f331da6cfb97b512d500274fd3c8035c3a296c` |

The permanent M9 Tier C tests remain valid and continue to protect these contracts.

## Post-M9 correction: `TERRAIN-004`

### What M9 said

At M9 exit, the repository classified the complete terrain-color builder as blocked because prior source review concluded that obfuscated `class470` in the pinned public source was unrelated text-layout code.

That conclusion was incorrect.

### What the M10 foundation audit proved

At exact public reference:

`melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`

`runescape-client/src/main/java/class470.java` blob:

`1cd9cad5cb4be865dcae94dc633bba821644dc84`

contains final static method:

`method9712(WorldView)`

which is the complete terrain-construction routine.

It includes slope lighting, the placement-derived shadow grid, separable radius-5/11x11 underlay blending, overlay handling, tile emission, `minPlane`, normal finalization, and bridge relinking.

The placement-side shadow writer is independently pinned in:

`FriendSystem.java` blob `b8cf51b6ee673181d8a115e28153a77f5978c390`, symbol `addObjects(...)`.

### Correct current disposition

`TERRAIN-004` is now:

`SOURCE_VERIFIED / IMPLEMENTATION_REQUIRED`

It is **not** implementation-complete. RustOSRS still needs the production builder and an executable exact differential fixture before the row can become `EXISTING`.

The target profile capability state is therefore `source_verified`, not `blocked`.

## Terrain details corrected after M9

The source audit also established:

- random hue/lightness jitter does not change the four underlay corner HSL values used for 3D tile shading;
- jitter feeds separately derived palette/minimap-style RGB values;
- hue jitter wraps with `& 255` while lightness clamps;
- `Tiles.Tiles_underlays2` is placement-derived shadow state, not generic neighboring-ground contrast;
- clipped walls write orientation-specific value `50` shadow cells;
- clipped game objects may write model-height-derived shadow values capped at `30`;
- the terrain builder assigns target `Tile.minPlane` before normal finalization/link-below processing.

## Plane correction after M9

`Tile.minPlane` and `Tile.originalPlane` are target client scene fields, not merely RuneLite GPU upload strategy.

Imported RuneLite `SceneUploader.maplevel` may choose settings/roof-group lookups from another level, but it does not move `tiles[level][x][y]` geometry into a different semantic storage-plane pass.

Current authoritative plane contracts are in `docs/specs/planes-bridges.md`.

## Normal/lighting correction after M9

The canonical M7 implementation was correct. Historical root research prose claiming that `Scene` does not perform wall/object normal merging was not.

Pinned `Scene.method5585/method5587` invokes `ModelData.method5262` across qualifying boundary/game/floor neighbors, while dual boundary arms merge with `hideMatchedFaces=false`.

Object-definition contrast is already decoded as signed opcode-39 byte multiplied by `25`; final loc lighting adds `768` and does not multiply again.

## Renderer provenance correction after M9

M10 foundation review distinguishes:

- software/client priority behavior from pinned `Model.method5946`;
- imported RuneLite GPU path-specific sorting;
- RustOSRS Reference renderer policy.

Imported RuneLite only enables the full priority-queue branch for paths that request it, notably `SORTED_NO_DEPTH`. Ordinary dynamic/static GPU work does not universally reproduce the software priority algorithm.

This does not invalidate M10's exact software priority implementation. It corrects the provenance claim used to justify it.

## M9 provenance limitations retained

The historical developer-machine melxin checkout remains not proven byte-identical to the public source revision.

`C-003`/`C-005` style whole-snapshot limitations therefore remain explicit where a claim still depends on the old checkout as a whole.

The newly corrected terrain builder does **not** depend on that historical equivalence because the relevant public `class470` and `FriendSystem` files are now pinned directly.

## Historical verification record

M9 exact-head pre-PR CI and PR CI both passed Tier A/B/C, including the explicit M9 suite. The branch was scope-audited and squash merged to main at:

`f7d004471562f723ee8115f5a3e28e937e3a6f5d`

The post-M9 provenance correction changes current documentation/profile state, not the fact that the historical M9 code/tests passed their then-declared contract.

## Current non-claims

Even after the correction, the project does not yet claim:

- implemented full `TERRAIN-004` parity;
- external 1:1 visual parity from self-generated screenshots;
- completed GPU texture/material/transparency realization;
- completed renderer `minPlane`/`originalPlane` visibility realization;
- broader skeletal animation execution outside the verified M8 subset.

These remain explicit forward gates rather than hidden renderer compensation.
