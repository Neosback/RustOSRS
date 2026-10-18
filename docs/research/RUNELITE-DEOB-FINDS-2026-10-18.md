# What the RuneLite fork, the deobfuscated client and 117HD offer this project (2026-10-18)

Read for the roof work; extended with everything else worth using that turned up. Research only:
current code and repository state win over this note (see `index.md`).

## Roofs: how the client and RuneLite decide, and what the editor does

**Vanilla client** (`class33.java` in the deobfuscated sources, the camera update). The client draws up
to plane 3 (`var12 = 3`) unless one of two things lowers the top plane to the player's plane:

1. the "roofs hidden" preference (`ClientPreferences.isRoofsHidden`), or
2. the tile under the camera, or a tile on the line from the camera to the focal point, has the
   `UNDER_ROOF` setting (`tileSettings & 4`), checked only while the camera pitch is below 310.

So in the game roofs are normally visible from the ground and disappear when you stand or look
under one. `tile.minPlane` (`terrain_build::tile_min_plane`) decides what a plane shows; the tile
setting `8` (`VIS_BELOW`) makes a tile always visible.

**RuneLite roof removal plugin** (`RoofRemovalPlugin`, `RSSceneMixin`): `Scene.buildRoofs()` labels
connected components (eight neighbours) of `UNDER_ROOF` tiles per plane. The GPU plugin's
`SceneUploader` buckets each tile's geometry under the roof id of the tile below it
(`roofs[plane - 1][x][y]`, none on plane 0 or where `VIS_BELOW` is set), so hiding a roof is a range
skip (`hideRoofIds`). The plugin chooses which ids to hide from the player tile, the hovered tile,
the destination tile and the tiles between camera and player. `overrides.jsonc` (63 KB, by map
square and plane) adds `UNDER_ROOF` to regions whose authors never set the flag (Polnivneach,
dungeon "roofs" and so on).

**This editor** (`osrs-scene::roofs`, `osrs-render::gpu::RoofView`, `app/roof_view.rs`):

- The renderer's geometry groups carry a roof id; `RoofView { top_plane, hidden }` replaces every
  `min_plane <= view_plane` test. The viewed plane still decides what can be edited and picked.
- Setting `roofs`: **Hidden** (what the editor did before), **Visible** (every plane), **Cutaway**
  (default: every plane except the roof volumes over the camera, the orbit pivot, the hovered tile
  and the selection). A toolbar button cycles it.
- Roof ids are derived from world coordinates (the component's south-west-most tile), so every
  streamed window and the editor name the same building the same without sharing a window.
  Unflagged tiles touching a roof (the one-tile overhang) join it.
- Generated buildings now set `UNDER_ROOF` on their floors, like real ones, so Cutaway works on
  them.
- Not done: RuneLite's `overrides.jsonc` regions, roofs larger than the 145-tile labelling window,
  "between camera and focus" cutting for low pitches, per-object roof visibility for picking.

## Found while there

- **117HD used the wrong flag**: `osrs-world/src/hd/extract.rs` had `TILE_FLAG_VIS_BELOW = 4`
  (that is `UNDER_ROOF`); the client, RuneLite and `terrain_build` use `8`. The effect was HD terrain
  deciding "visible from below" for roofed tiles. Corrected to `8`.

## Other things in these sources worth using

| Source | What | Use here |
|---|---|---|
| 117HD `scene/areas.json` | 1,218 named rectangles in world coordinates: 33 banks, 7 churches, 45 houses, inns, kitchens, guilds, temples, towers ... | Free weak labels for the building census: join each census building to the rectangle that contains it and take the archetype from the name. See `docs/plans/PROCGEN-LEARNING.md`. |
| 117HD `scene/tile_overrides.json`, `ground_materials.json` | Ground material by area and by underlay/overlay | A ground-material label per tile (grass, dirt, sand, path, snow ...) beyond raw underlay ids; also tells the generator what the ground around a building should be. |
| 117HD `scene/gamevals.json` (4.9 MB), RuneLite `GameValDefinition` | Symbolic names for npcs, objects, items | Same signal the OpenRune gamevals already give `object_semantics`; a cross-check for opaque names. |
| 117HD `scene/model_overrides.json` | Per-object tags (hide fake shadows, recolour, wind) | Which objects are foliage, lit, translucent or animated: a physical-property label. |
| RuneLite `roofremoval/overrides.jsonc` | Hand-authored roof volumes | Ground truth for "this building has a roof" where the flag is missing. |
| RuneLite `cache` module (`ObjectManager`, `AreaManager`, `WorldMapManager`, `region/`) | Reference readers for map squares, objects, world-map areas | Cross-checks for `osrs-cache` decoders; the world-map areas are named regions (towns, dungeons) usable as map-level labels. |
| Object definition `category` and `params` (already decoded: `ObjectDefinition.category`) | Cache-native object groupings | Labels the cache itself provides; not used by `object_semantics` yet. |
| `user_world_defs.json` (osrs-wiki-maps) | Underground and instanced areas | Already used as `CENSUS_EXCLUDE`. |
| RuneLite `groundmarkers`, `tileindicators`, `instancemap` | Tile marking and instance templates | Nothing needed now; instance template chunks matter if the editor ever edits instanced areas. |

## Not found

No building or room labels exist in RuneLite or the deobfuscated client: the client does not know
what a building is. The only human-made building labels are the 117HD area names above and the
named rectangles in the world map.
