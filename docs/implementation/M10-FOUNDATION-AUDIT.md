# M10 Foundation Audit

Status: **Foundation corrections implemented; exact-head CI required before checkpoint closure**  
Date: 2026-10-10  
Branch: `impl/m10-render-extraction-core`  
Audit baseline: M10 CP5 head `ab150cd6393bd2de46f7c157bb6aaaa99249a44a`

## 1. Purpose

Before continuing from M10 structural extraction toward a physical renderer, this audit re-checked the project's semantic and renderer assumptions against the strongest available evidence.

The goal is not to make every row appear complete. The goal is to distinguish:

- implementation that is already exact and protected;
- documentation/provenance conclusions that were wrong;
- source behavior that is now pinned but not implemented;
- renderer decisions that intentionally differ from imported RuneLite GPU behavior;
- forward gates that must remain visible before external parity is claimed.

## 2. Source sets reviewed

### Pinned public semantic/deob source

`melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`

Directly re-audited in this checkpoint:

- `class470.method9712(WorldView)`
- `FriendSystem.addObjects(...)`
- `Scene` plane/min-plane and normal-finalization behavior
- `Tile` plane/originalPlane/minPlane fields
- `ModelData` normal merge and lighting sentinel behavior
- `ObjectComposition` contrast decode
- `Model` priority oracle as already pinned by fixtures

### Imported RuneLite renderer

RustOSRS tree:

`5afef996a992bacc73655860681269242e90d6e8`

Directly re-audited:

- `SceneUploader`
- `ModelUploader`
- `GpuPlugin`
- `Zone`
- `TextureManager`
- `Renderable`
- imported/staged vertex and fragment shader evidence

### RustOSRS implementation/doc layers reviewed

- target profile/capability diagnostics;
- M3-M9 semantic closure state;
- canonical terrain, plane, normal/lighting, face/material specs;
- reverse-Z and zone-renderer ADRs;
- M10 CP1-CP5 renderer code/tests;
- source-pin and parity verification docs;
- historical M9 audit;
- root RuneLite runtime/GPU research notes;
- GPU/verification blueprint contracts.

This is a foundation/contract audit, not a claim that every imported third-party source file was reread line by line.

## 3. Executive result

### No newly discovered P0/P1 semantic implementation defect in already-promoted areas

The audit did **not** find evidence that the existing promoted Rust implementations of these areas are wrong:

- target cache transport/profile mechanics;
- object/floor/model/map/sequence/var decode contracts already covered by M3/M4;
- object model selection/combine/mirroring/transform order;
- placement dispatch, footprints, height centers, initial/runtime ownership;
- terrain topology and flat/shaped surface representation;
- structural bridge link-below behavior;
- base normals and cross-model normal reconciliation;
- final reference object lighting;
- morph resolution, contouring, verified legacy animation subset;
- semantic scene hashing;
- M10 integer renderer rebase and face metadata extraction;
- M10 software/client priority preparation;
- M10 CPU alpha interpretation;
- M10 static/dynamic/ordered classification;
- M10 material handles and reference UV preparation;
- M10 8x8 zone invalidation/generation logic.

### Material foundation errors were documentation/provenance and future-render-policy errors

The audit did find several important incorrect assumptions:

1. `TERRAIN-004` was falsely source-blocked.
2. target `Tile.minPlane`/`originalPlane` were under-documented and partly misclassified as RuneLite upload policy.
3. root notes incorrectly denied cross-wall/object normal reconciliation.
4. docs overgeneralized RuneLite GPU priority sorting.
5. Reference depth compare was set to `GreaterEqual` despite imported RuneLite using strict `GL_GREATER`.
6. transparency docs incorrectly assumed conventional global depth-write disable.
7. texture sampling docs were underspecified and "nearest" would have been incomplete.
8. self-generated screenshots were positioned too close to an external 1:1 oracle.
9. raw alpha sentinel stages were insufficiently distinguished in renderer-facing documentation.

These are now corrected in canonical docs and, where they affected current machine-readable state, in the target profile/tests.

## 4. Terrain audit

### Finding: exact builder exists

Pinned `class470.java` blob:

`1cd9cad5cb4be865dcae94dc633bba821644dc84`

contains `method9712(WorldView)`, the complete terrain builder.

The previous source blocker is invalid.

### Verified builder dependencies/order

The source includes:

- bounded random hue/lightness presentation walk;
- slope-light computation from neighboring height samples;
- consumption of `Tiles.Tiles_underlays2` shadow data;
- radius-5/separable 11x11 underlay weighted-HSL blend;
- underlay/overlay tile construction;
- texture average-color path;
- primary magenta sentinel path;
- secondary overlay color path;
- `Scene.addTile`;
- `setTileMinPlane`;
- scene ModelData normal finalization;
- bridge `setLinkBelow`.

### Shadow writer pinned

`FriendSystem.java` blob:

`b8cf51b6ee673181d8a115e28153a77f5978c390`

proves `Tiles.Tiles_underlays2` is placement-derived shadow state.

Clipped wall cases write `50`; clipped game objects can write model height / 4, default `15`, capped at `30`.

### Jitter correction

Non-jittered blended HSL drives the 3D underlay corner colors. Jitter feeds separately derived palette/minimap-style RGB. Hue wraps with `& 255`.

### Current disposition

`TERRAIN-004 = SOURCE_VERIFIED / IMPLEMENTATION_REQUIRED`

The target profile now records `source_verified` instead of `blocked`.

**Gate before full terrain parity:** production port + exact differential fixture.

## 5. Plane/bridge audit

Pinned `Tile` includes:

- mutable `plane`;
- `originalPlane`;
- `minPlane`;
- linked-below relation.

Pinned terrain construction writes `minPlane`, and pinned `Scene` uses it in draw traversal/eligibility.

Therefore `minPlane` is target client scene state, not merely RuneLite GPU policy.

Imported RuneLite `SceneUploader.maplevel` changes settings/roof grouping lookup while geometry still comes from `tiles[level]`.

### Current disposition

- source/collision/storage planes: implemented and exact-tested;
- structural link-below: implemented and exact-tested;
- `minPlane/originalPlane`: source verified, not yet fully represented/consumed end to end by renderer visibility.

**Gate before roof/plane Reference parity:** exact semantic representation and grouping tests without scene-hash mutation.

## 6. Normals/lighting audit

Pinned `Scene.method5585/method5587` calls `ModelData.method5262` across qualifying boundary/game/floor neighbors.

Correct rule:

- no geometry welding;
- real cross-model normal reconciliation;
- matched faces may be marked render type `2` when hide is enabled;
- dual boundary arms merge directly with hide disabled;
- same-plane versus plane-above traversal does not use one universal hide flag.

Existing M7 implementation/tests already follow this model.

### Contrast decode

Pinned opcode `39` performs signed byte `* 25`.

Rust `osrs-cache` already performs the multiplication and final lighting adds `768`. No code bug was found here.

### Alpha sentinels

Existing `osrs-core::lighting` correctly applies:

```text
alpha -2 -> effective render type 3
alpha -1 -> effective render type 2
```

and produces suppressed baked `c == -2` where required.

M10 now exposes `RenderMesh::face_is_suppressed(...)` so later draw-packet code has an explicit no-draw semantic boundary.

## 7. Priority and alpha renderer audit

### Software/client priority

M10 CP2 correctly targets pinned `Model.method5946` and its 0..11 queues/thresholds/special stream.

### Imported RuneLite GPU

RuneLite GPU only runs priority queues when `prioritySort=true`, notably for `SORTED_NO_DEPTH`. Ordinary static/dynamic work is not universally priority sorted. `Zone` alpha work uses distance/depth ordering.

### Decision

RustOSRS Reference mode keeps the stronger software/client priority contract for priority-sensitive content.

A future RuneLite-GPU comparison profile, if desired, must reproduce its render-mode-specific behavior separately.

## 8. Reverse-Z/raster audit

Imported RuneLite uses:

```text
clear depth = 0
GL_GREATER
```

ADR-0004 has been corrected from `GreaterEqual` to strict `Greater` for Reference mode.

Imported projection has no finite far plane. Authored `bias / 128` is applied before perspective division, so normalized-depth separation is distance dependent.

The imported renderer does not establish a universal transparency depth-write-off rule. Explicit no-depth render modes are separate.

### Gates before Reference GPU parity

- strict equal-depth `Greater` fixture;
- no-far projection fixture;
- authored bias at materially different distances;
- alpha depth-write/no-depth fixture.

## 9. Texture/material audit

M10 CP4 structural work remains sound:

- full-width `TextureId` is retained;
- renderer handles are distinct from semantic IDs;
- deterministic material-table ordering;
- canonical/explicit/projected model UV preparation;
- decoded animation input -> deterministic render-tick velocity.

Imported RuneLite GPU additionally proves renderer behavior that remains for later GPU work:

- 128x128 image construction for this snapshot;
- RGB zero -> transparent;
- level-0 alpha cutout/discard;
- shader-side brightness;
- textured face lightness behavior;
- nearest magnification;
- nearest minification only at filter level 0;
- `NEAREST_MIPMAP_LINEAR` at level >=1;
- imported default level 1;
- S clamp-to-edge;
- T default repeat in audited setup.

Fixed RuneLite texture count remains an implementation detail, not semantic truth.

## 10. M10 CP1-CP5 code audit

### CP1 extraction/rebase

Result: **PASS**

- integer semantic placement before rebase;
- immutable generation identity;
- optional face arrays preserved as optional;
- original face render types retained;
- no renderer dependency leaks into semantic crates.

Foundation addition: explicit suppressed-face query.

### CP2 priority/alpha preparation

Result: **PASS with provenance clarification**

The algorithm is correct for the selected software/client oracle. Documentation no longer calls it universal RuneLite GPU behavior.

### CP3 classification/draw plan

Result: **PASS**

Static/dynamic/ordered classification is renderer policy based on stability/reasons rather than semantic object type.

Future GPU pass realization must use corrected Reference depth/alpha rules.

### CP4 material/UV

Result: **PASS structurally**

CPU handoff is compatible with corrected texture evidence. Pixel construction/sampler realization remains later GPU work.

### CP5 zones

Result: **PASS**

8x8 semantic tile zone identity, negative-coordinate division, cross-zone bounds, dirty ordering, partial generation reuse, and stale changed-zone ticket rejection remain sound.

Zone identity correctly remains independent of render-origin movement.

## 11. Verification architecture audit

Internal RustOSRS screenshots are now classified as `V3R` regression evidence.

External visual parity is `V3` and requires an independently generated oracle artifact.

Current status:

- exact semantic and renderer-structural parity evidence: strong and merge-gated;
- internal visual golden capability: planned for GPU milestones;
- independent external visual oracle: **OPEN**.

No future "1:1" claim is permitted from self-generated images alone.

## 12. Historical documentation cleanup

Corrected/replaced in this checkpoint:

- target profile terrain capability state;
- target profile digest expectation;
- M9 terrain capability regression;
- terrain spec;
- plane/bridge spec;
- normals/lighting spec;
- face/material spec;
- reverse-Z ADR;
- zone/hybrid renderer ADR;
- GPU data/pass blueprint;
- verification architecture blueprint;
- source pins;
- parity matrix;
- historical M9 audit with post-M9 correction;
- terrain/material/plane audit;
- root RuneLite runtime research rules;
- root RuneLite GPU pipeline research note;
- stale `floor_color` module source-blocker comment.

## 13. Current foundation risk register

| Risk | State | Required closure |
|---|---|---|
| complete terrain-color Rust implementation | OPEN, source verified | port `method9712` semantics + exact fixture |
| placement shadow side-grid production mutation | PARTIAL ownership metadata only | narrow source-pinned implementation contract needed by terrain builder |
| `minPlane/originalPlane` semantic representation/visibility | OPEN | exact scene representation + renderer grouping tests |
| suppressed face reaches draw packet | STRUCTURAL GUARD ADDED | future packet builder must filter using explicit state and test it |
| strict-Greater/no-far GPU realization | OPEN | M11 exact raster logic/offscreen tests |
| bias distance behavior | OPEN | near/far fixture |
| imported RuneLite alpha depth/no-depth parity | OPEN | explicit render-mode fixture |
| texture pixels/cutout/filter/wrap realization | OPEN | M12 material/sampler fixtures |
| external visual oracle | OPEN | independent capture/harness with provenance |
| old local deob whole-tree equivalence | REVISION_SENSITIVE | only required where a claim still lacks a public exact source pin |
| broader skeletal animation | DEFERRED | separate owning milestone/spec, not hidden in renderer |

## 14. Foundation judgment

The project is on a **sound semantic foundation**, with one important qualification:

The old terrain-color blocker hid missing implementation behind a provenance claim. The source is now known, so the missing work is visible and actionable rather than ambiguous.

The audit found no reason to discard or redesign M0-M9 or M10 CP1-CP5. The architectural separation of cache -> semantic model -> scene -> renderer extraction remains appropriate.

The correct next approach is to keep that architecture and close the explicit risk register rather than compensating in shaders or silently treating RuneLite GPU behavior as identical to the software client.

## 15. Exit criteria for this audit checkpoint

This audit checkpoint is complete only when:

1. all corrected code/docs are committed on the M10 branch;
2. target profile digest/test state is internally consistent;
3. suppressed-face boundary test passes;
4. Tier A formatting/check/clippy passes;
5. Tier B full workspace tests pass;
6. Tier C M3-M9 semantic parity passes with corrected terrain capability state;
7. branch-vs-CP5 diff contains only foundation corrections/audit work;
8. no M11/wgpu implementation is introduced.

After those gates pass, M10 may resume from the corrected foundation, but `TERRAIN-004` and the other forward gates remain explicit work items.
