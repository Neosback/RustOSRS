# Auto-tiler: how the OSRS tile system works, and how to automate it

Sources: client `class264.loadTerrain` (map file decode), `Scene`/`SceneTilePaint`/`SceneTileModel`
construction, our ports `osrs-scene/src/terrain*.rs`, `osrs-world/src/terrain_stage.rs`, and the editor's
picker data (`TileInfo`).

## 1. What a map tile is

Per tile, per plane (4 planes, 64x64 tiles per region):

| Field | Source in map file | Meaning |
|---|---|---|
| height | opcode 1 (u8) | `-value * 8`, special-cased `1 -> 0`; no value -> procedural noise (plane 0) or `plane-1 height - 240` |
| underlay id | opcode > 81 | `id = opcode - 81`; colour of the whole tile, blended (below) |
| overlay id | opcode 2..=49 | followed by the overlay id (u16) |
| overlay shape | same opcode | `shape = (opcode - 2) / 4` (0..=11), plus 1 internally -> 13 shape tables |
| overlay rotation | same opcode | `(opcode - 2 + region_rotation) & 3` |
| settings flags | opcode 50..=81 | `value = opcode - 49`: bit 1 blocked, bit 2 bridge (draw one plane lower), bit 8 visible-below (`min_plane = 0`) |
| shadow | separate lighting data | baked into corner lightness (`field868`) |

A tile is `underlay` only, or `overlay` (with shape and rotation) on top of an underlay.

## 2. How a tile is built (`terrain_build.rs`, a port of the client)

1. **Underlay blending**: every tile's underlay colour is the average over an 11x11 window
   (`BLEND_RADIUS = 5`) of the hue/saturation/lightness of the underlays around it, weighted by
   `hue_multiplier`. This is why grass fades into sand over 5 tiles and why changing one tile recolours its
   neighbourhood. Hue/lightness jitter is added per scene.
2. **Corner light**: slope light from height gradients (`SLOPE_LIGHT_DIVISOR 213`) minus baked shadow,
   per tile corner; the underlay lightness is adjusted per corner, so terrain shading follows the heights.
3. **Overlay shape 0** (internal shape 1) -> `SceneTilePaint`: a flat quad (two triangles) in the overlay
   colour/texture. **Shapes 1..=11** -> `SceneTileModel` from the `VERTEX_CODES`/`FACE_CODES` tables
   (4-6 vertices per shape: corners 1,3,5,7 + edge midpoints 2,4,6,8 + inner points 9-16), with rotation applied
   to the vertex/face codes. Each face draws either underlay or overlay colours, which is what produces
   diagonal shoreline cuts, paths with rounded ends, and "L" shaped overlays.
4. Textured overlays use the texture's average colour for minimap/lighting; magenta overlays are the
   "do not draw" sentinel (holes).

## 3. The shape alphabet (what an auto-tiler chooses between)

The map file stores `shape = 0..=11`; the builder adds 1 and indexes the tables in `terrain.rs`
(rows 0 and 1 are "all underlay" / "all overlay"; file shape 0 is the plain paint quad). Overlay share of
the tile, computed from `FACE_CODES` (vertex codes 1,3,5,7 are the corners, 2,4,6,8 edge midpoints,
9-16 inner points at quarter positions):

| file shape | table row | overlay share | geometry |
|---|---|---|---|
| 0 | 1 | 100% | whole tile (paint) |
| 1 | 2 | 50% | diagonal half (corner to corner) |
| 2, 3 | 3, 4 | 25% | corner triangle using the opposite edge midpoint (mirror pair) |
| 4, 5 | 5, 6 | 75% | complement of 2/3 (mirror pair) |
| 6 | 7 | 50% | half cut through two edge midpoints (straight edge) |
| 7 | 8 | 12.5% | small corner triangle between two edge midpoints |
| 8 | 9 | 87.5% | complement of 7 |
| 9 | 10 | 34% | rounded concave piece using inner points 11/12 |
| 10 | 11 | 66% | complement of 9 |
| 11 | 12 | 19% | small rounded piece using inner points 13/14 |

Rotation (0-3) turns any of these by 90 degrees: edge/corner codes rotate in steps of two, inner points
in steps of one (`rotated_vertex_code`). So the alphabet is 11 shapes x 4 rotations plus "full" and
"none", about 45 distinct tiles, of which the complements pair up (a shape and its complement are the
same cut seen from the other overlay).

## 4. Auto-tiler design

Input: a desired **overlay mask** (which tiles belong to overlay X) painted by the user, plus underlay mask.
Output: per tile `(overlay id, shape, rotation)` and underlay ids such that the result looks hand-made.

**Algorithm (marching-squares style, per overlay id):**
1. Sample the mask at the four tile *corners* (a corner is inside if the tile region around it is painted):
   a 4-bit index per tile (16 cases).
2. Table: case -> (shape, rotation), derived from the shape alphabet. 0000 -> no overlay, 1111 -> shape 0;
   single corner -> corner triangle; two adjacent corners -> half-tile or edge shape; opposite corners ->
   ambiguous (choose by neighbours: connect diagonal or split into two corner pieces); three corners ->
   concave piece.
3. Resolve ambiguity with a smoothing pass (prefer rounder shapes, avoid single-tile spurs), the same
   preference the hand-authored maps show.
4. Multi-overlay borders (water next to sand next to path): process overlays in priority order; each tile
   holds one overlay, so the loser keeps its shape facing the winner and the underlay shows through.
5. **Underlay painting** needs no shape; remember the 5-tile blend: previews must recompute the blend
   window, not just the painted tile. Offer "blend preview" = the editor's real build on a dirty rectangle.
6. **Heights**: tiles share corner heights; painting a flat area should offer a plateau/ramp tool using
   the same corner model (`heights[sw,se,ne,nw]` per tile, shared with neighbours).
7. **Settings flags** are separate layers (blocked, bridge, visible-below) with tool-assisted defaults:
   water overlays -> blocked, bridges -> flag 2 plus plane-above floor.

**Validation:** re-run `build_terrain` on the edited rectangle (it is deterministic) and diff against the
client oracle (`terrain_oracle_diff`) to prove that automatically produced tiles are valid client output.
Reverse direction for learning: statistics of (neighbour mask -> shape, rotation) over the whole real map
(4.9 M locs, ~170 k zones) give the true tileset usage; use it to rank ambiguous cases.

## 5. Plan
1. `osrs-scene::tile_rules`: pure functions `corner_mask -> Option<(shape, rotation)>` plus the inverse
   `(shape, rotation) -> covered corners`, with exhaustive tests over all 12 shapes x 4 rotations.
2. Census example: over every region of the cache, count (neighbour overlay pattern -> shape/rotation) and
   report cases where the rule table disagrees with real maps (finds hidden conventions).
3. Region-scoped `TileEdit` command (overlay/underlay/height/flags) with undo; dirty-rectangle rebuild
   (window = edit +- 6 tiles because of the blend radius and normal merging).
4. Editor tool on top of the plugin trait: brush with preview, mask-to-tiles, and a "retile selection"
   command that re-derives shape/rotation for existing maps (also a fixer for hand-made inconsistencies).
5. Optional Lua rule sets (see `FOUNDATION.md`) for project-specific tilesets.
