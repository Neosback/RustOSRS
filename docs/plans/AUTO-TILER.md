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
   (up to 6 vertices: corners 1,3,5,7 + edge midpoints 2,4,6,8 + inner points 9-16), with rotation applied
   to the vertex/face codes. Each face draws either underlay or overlay colours, which is what produces
   diagonal shoreline cuts, paths with rounded ends, and "L" shaped overlays.
4. Textured overlays use the texture's average colour for minimap/lighting; magenta overlays are the
   "do not draw" sentinel (holes).

## 3. The shape alphabet (what an auto-tiler chooses between)

The shape tables are a tiny tileset over the four corners/edges of a tile. With rotations they describe
every way an overlay region can cover part of a tile:

| shape | covered area (overlay) | typical use |
|---|---|---|
| 0 | whole tile | interior of a path/water/floor |
| 1 | one corner triangle | outer corner of a diagonal edge |
| 2 | half tile (diagonal) | straight diagonal edge |
| 3 | corner triangle + ... | smooth corner variants |
| 4-6 | quads/trapezoids on the tile edge | straight edges with a notch |
| 7-9 | combinations using edge midpoints | rounded joins |
| 10-12 | inner-corner / concave pieces | concave corners, single-tile islands |

(Exact face lists: `FACE_CODES` in `terrain.rs`. Our tests render all 13 shapes x 4 rotations.)

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
   `(shape, rotation) -> covered corners`, with exhaustive tests over all 13 x 4.
2. Census example: over every region of the cache, count (neighbour overlay pattern -> shape/rotation) and
   report cases where the rule table disagrees with real maps (finds hidden conventions).
3. Region-scoped `TileEdit` command (overlay/underlay/height/flags) with undo; dirty-rectangle rebuild
   (window = edit +- 6 tiles because of the blend radius and normal merging).
4. Editor tool on top of the plugin trait: brush with preview, mask-to-tiles, and a "retile selection"
   command that re-derives shape/rotation for existing maps (also a fixer for hand-made inconsistencies).
5. Optional Lua rule sets (see `FOUNDATION.md`) for project-specific tilesets.
