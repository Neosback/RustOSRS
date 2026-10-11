# Minimap, world map, compass and wiki lookup: how the game does it, what we use

## Where the data lives (build 241 cache, verified with `examples/cache_map_assets.rs`)
| Data | Cache location | Notes |
|---|---|---|
| Interface sprites (compass 169, minimap frame `MAPBACK` 12, compass outline 5813, wiki icon 2420/2421, ...) | index 8, group = sprite id | Indexed sprites with palette + optional alpha; decoded by `decode_sprite_group`. All the ids RuneLite's `SpriteID` lists exist. |
| World map ground data | index 18 (2,103 groups) and index 20 (2,103 groups) | Group id = region id `(x << 8) | y` (e.g. 3872 = region 15,32). One entry per map square. |
| World map areas / details | index 19 (54 groups), index 17 (defaults) | Area definitions (`WorldMapArea`: id, internal/external name, origin, zoom, regions, sections). |
| Named groups (`details`, `compositemap`, `compositetexture`, `labels`) | name hashes are **absent** in this cache | The client addresses them by name; our cache has no name hashes, so they cannot be looked up by name. |

The client's world map (`WorldMapRenderer` / `WorldMapRegion` / `WorldMapData_0/1`) draws from per-region
"geography" data: floor underlay/overlay ids per tile, plus decorations (walls etc.) per tile, then
icons (map functions) and labels on top. It does **not** need a prerendered image.

## What we do (implemented)
`osrs_world::assets` renders every map region ourselves from the same terrain data the 3D view uses:
the real terrain builder in the region's own scene window (so underlay blending across region borders
matches), flat tile colours (the game's minimap colours, `SceneTilePaint.rgb` / shape underlay and
overlay colours) with 20% of the lit corner colour mixed back in for relief, texture average colours for
textured floors, and wall edges as dark pixels. 2 px per tile; 56 regions render in about 0.4 s, so the
whole world sweeps in the background in seconds, nearest the camera first.

* World map window: toolbar globe / **M**; drag pans, scroll zooms around the cursor, **double-click
  flies to that tile**; the camera is drawn as a marker with its heading.
* Compass HUD (View > Compass): the game's own compass sprite (169) turned with the camera; click to face
  north.

## Next steps (structure exists, not done)
1. Map functions and labels: `WorldMapElement` definitions (config group) give icon sprite + position per
   function; area labels come from the `labels` data. Draw as sprite icons on the map and as text.
2. Scenery on the map (trees, rocks as the game's small dot sprites), plane switching, per-area maps
   (dungeons, Prifddinas) via the area definitions in index 19.
3. Minimap HUD: the same tile images, a 4 px/tile crop rotated by camera yaw and masked to a circle, framed
   with `MAPBACK`; orbs optional. The rspsi-imgui project does exactly this as an overlay
   (`MinimapHudOverlay`: circular crop, NPC dots, click opens the world map).
4. Higher resolution on demand for the region under the cursor (4 px/tile) while keeping 2 px/tile resident.

## RuneLite specifics worth knowing
* **Wiki lookup** (`WikiPlugin`): a button next to the orbs (`SpriteID.WikiIcon` 2420 deselected / 2421
  selected, widget `InterfaceID.Orbs.WIKI`) and a "Lookup" menu entry. It opens
  `https://oldschool.runescape.wiki/w/Special:Lookup?type=<npc|object|item>&id=<id>&name=<name>&x=&y=&plane=`
  and the wiki redirects to the right page. Implemented here as `wiki.rs` (toolbar button, Object tab
  button, **L**).
* **World map plugin** (`WorldMapPlugin`): adds points (agility shortcuts, fairy rings, transports,
  dungeons...) as `WorldMapPoint`s from data tables, on top of the client's own map. Its data
  (`TeleportLocationData`, `*Location` enums) is a ready list of map overlays to import later.
* The default minimap/orbs layout comes from interface groups `Orbs` / `OrbsNomap`; sprite ids above.
