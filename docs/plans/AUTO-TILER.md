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
| overlay shape | same opcode | `shape = (opcode - 2) / 4` (0..=11), plus 1 internally to index the 13-row shape tables |
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

**Refinement from Terraini (see `TERRAINI-IDEAS.md`):** instead of a 4-bit corner index, rasterise all
12 shapes x 4 rotations into N x N sub-tile masks (N = 8 is plenty) from the real `FACE_CODES`, paint at
that resolution, and choose the (shape, rotation) with the smallest Hamming error; edge masks of each
candidate give "portals" so neighbouring tiles connect, and a frequency table harvested from the real map
(8-neighbour pattern -> shape/rotation) breaks ties. This replaces the hand-written case table in step 2
above and handles painting finer than one tile.

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

## 6. How the other editors handle it (comparison)
* **Terraini**: the most developed. Sub-tile mask atlas + edge portals + frequency grammar (see
  `TERRAINI-IDEAS.md`), driven by *road/path tools* (curve -> tiles) rather than free painting.
* **OpenRune Studio** (`RSPSi-master/docs/ROADMAP.md`): plans an **AutoTile service** that "centralizes OSRS
  overlay shape/rotation/topology logic" shared by all tools (tile painter, roads, streams, fences,
  bridges); also lists overlay flood fill with a tile cap, rotate/mirror paste that remaps overlay shape
  rotation, and closed-room detection to suggest roof flags. Its rule: no per-tool math stacks.
* **rspsi-imgui**: no auto-tile; paints overlay/underlay from swatch panels, with ramp/spline/prefab tools
  and a radial brush menu for quick brush switching.
* Our choice: one `osrs-edit::autotile` service (pure, tested against `FACE_CODES` and the real map census),
  used by the tile painter, road tool, fill and "retile selection".

## 7. Status (2026-10-11)

Built in `crates/osrs-editor/src/autotile.rs` (to move into `osrs-edit`): sub-tile mask rasterisation from the
real `FACE_CODES` tables (file shape + 1 indexes the builder's rows), Hamming-error choice, edge-portal passes,
and the spline band. Used by the Spline tool. Not yet: the real-map frequency table for tie-breaking, multiple
overlays with priorities, retile-selection, and flood fill.

## 8. Review of outside notes (2026-10-11)

Already built, differently: sub-tile masks from the real tables (16 x 16, not 8 x 8, so the 12.5% and 19%
pieces separate cleanly; there is no 4-corner table anywhere); edge continuity (each tile's four edge profiles are
compared with its neighbours' over three or four relaxation passes, with the shape error counted once and a
mismatched edge cell twice); complements are ordinary candidates (4, 5, 8, 10 against 2, 3, 7, 9).

Corrected: underlay id 0 is skipped by the blend, so a full overlay over "uninitialised" underlay does not
bleed; silently inheriting the neighbours' underlay would break the keep-what-is-there rule, so it stays an
explicit choice. Heights need a +-1 invalidation window (the blend sums underlay definitions, not shaded colours),
not +-7.

Worth doing next, in order: (1) paired underlays harvested from the real map (the most common underlay beneath
each overlay) offered as one click; (2) the census prior: a histogram over all map squares of (3 x 3 same-overlay
neighbour pattern) -> (shape, rotation), used as `- lambda * ln(1 + count)` to break saddle ties; (3) overlay
priority arbitration at borders (the lower tile keeps its facing shape and takes the higher overlay's paired
underlay); (4) an eraser that cuts partial gaps with the complement shapes; (5) per-channel invalidation reach in
the edit layer (overlay, shape and flags need only their own tile, underlay +-5, height +-1).

## 9. Census-driven tiler (2026-10-11)

The 16 x 16 mask fit alone gave a diagonal sliver on every edge tile ("jagged ears"). A census of the whole
b241 map (`osrs-world` example `shape_census`, output kept in `crates/osrs-editor/data/shape-census.txt`:
`pattern shape rotation count`, pattern = which of the 8 neighbours carry the same overlay id) shows how
people actually lay shapes:

* 6.2 M of 7.8 M overlay tiles have all eight neighbours the same and are whole (99.9%).
* On a straight edge (five neighbours in a row) the tile is whole 87% of the time; shape 6 (half tile) is
  8%. Shapes concentrate at corners: a convex corner is cut with shape 1 (the diagonal) about 30%, whole
  about 55%.
* Rotating the neighbourhood a quarter turn clockwise (bits move two places) turns the shape one step, so
  every observation is counted in all four orientations.

`autotile::generate` first tried to follow the census closely (occupancy by half coverage, shapes only on
occupied tiles, census weight dominant). That reproduced the whole-tile staircase of straight edges but turned
every slanted road into sawteeth, so the weights were reversed after looking at renders
(`cargo test -p osrs-editor draw_paths -- --ignored` writes `/tmp/autotile-*.png`: shapes filled, ideal band
outlined): every tile the band touches (at least 10 of 256 cells) picks nothing, whole or a shape; score =
30 x area mismatch + 3 x portal mismatch (only partly covered edges count: whole against empty is just the
outline running along a tile border) + 0.25 x census cost. Six relaxation passes. With the fit leading, the
shape vocabulary (quarter-point triangles, half diagonals) follows slanted and curved edges within a quarter
tile. Existing whole tiles are not rewritten, and old partial pieces that end up surrounded become whole, so a
new path flows into an old one. Limits: thin paths (width 2) show small teeth, the prior sees one ring of
neighbours, existing neighbours are only retiled when they end up surrounded, and underlay pairs are not
modelled.
