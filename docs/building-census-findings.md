# How real OSRS buildings are made (measured)

Measured with the `building_census` example (`osrs-scene::building_census`, run over every map
square of the OpenRune revision-240 cache; about 7 seconds). 3,944 enclosed building shells were
found, 470 of them roofed (30% or more of their tiles have a roof piece on the plane above). Read
the numbers as evidence for the generator's kits, not as exact statistics: a building that spans
two map squares is cut at the border, and fenced yards and ruins pass for buildings when they have
no roof.

Reproduce: `cargo run --release -p osrs-world --example building_census -- <OpenRune root>`;
`building_ascii_real <root> x0 y0 x1 y1 [plane]` prints any real area as text.
`CENSUS_EXCLUDE=user_world_defs.json` (the osrs-wiki-maps file of underground/instanced areas) skips
those map squares; only 10 of its squares exist in this cache, so it changes almost nothing.

## Walls, inside and outside

- Enclosed rooms are found by flood fill across wall edges. A gap of one or two edges in an
  otherwise straight wall run is an **open doorway** (no door object): it keeps the room closed.
  Banks and shops are full of them; counting them is what made banks detectable at all.
- Exterior wall edges 13,723, interior 2,885 (roofed buildings). Exterior: 8.0% windows, 2.3%
  doors. Interior: 8.2% doors. Most buildings have one room; 78 have two, 41 have three.
- The most used exterior walls: `brickwall` 1902, `timberwall` 1602, `oldwall` 1631, then town
  styles (`fai_varrock_*`, `fai_falador_*`, `draynor_walls_ext`, `kr_seers_village_wall`).
  Interior walls are usually the same piece as the exterior, plus railings (`woodenrailing` 994,
  `pier_rail` 814) for open balconies.
- Doors: `poordoor` 1535 is the standard door; `poshdoor` 1540 next; town styles have their own.
  Interior doors are the same objects as exterior ones.

## Windows are decoration on both faces

- In brick and timber towns a window is **a wall plus a wall decoration on each face**: outside
  `window` 1821 / `poshwindow` 1823 / `poshbigwindow` 1828 / `shutterwindow` 1838, inside
  `curtainedwindow` 1822 / `curtainedposhwindow` 1824. Town styles (Falador, Varrock, Dorgesh-Kaan)
  instead use a **window wall piece** (`fai_falador_interior_window`, `deal_wall_window`).
- Every wall decoration (loc kinds 4 and 5) is attached to the wall edge on the side of its own
  tile that its orientation names: orientation minus wall side is 0 in 100% of 3,000+ cases, inside
  and outside. Inside the decoration stands on the room tile; outside on the tile beyond the wall.
- Interior decoration: torches (`torch` 196, `torch_grubby` 197), `hanginglantern` 3474, and
  town-specific candlesticks, oil lamps, shelves, paintings. Outside: `lantern` 205 and signs.

## Roofs

- A roof is one object used with several loc kinds: 12 slope, 15 inner slope corner, 16 outer slope
  corner, 17 flat top (object e.g. `roof_redslate` 1924, `roof_wooden` 1933, `roof_thatched` 1926,
  `fai_varrock_roofkit_slate` 15552) and a second object for the edge trim: 18 straight, 19 outer
  corner, 20 inner corner (`roofedge_redslate` 1790, `roofedge_wooden` 1793).
- A roof lies on the **plane above** the walls and **overhangs the walls by one tile** on every
  side. The layers (edge ring, slope ring, flat top) follow from the footprint; a test reproduces a
  real Lumbridge roof piece for piece (`osrs-edit/src/building_roof_hip.rs`).

## Floors

Dominant interior floor overlays: 5 (wood planks, texture 3), 81 (dark brown 80,64,32), 14 (dark
wood), 119 (92,84,60), 171 (stone 120,112,96), 173, 123 (pale stone, Falador), 20 (marble, texture
15). Banks: 81 and 171 (Varrock), 20 and 10. Kitchens, bedrooms, living rooms and taverns: mostly
overlay 5. Churches: 171. An earlier starter kit used overlay 10 (flat grey 80,80,80), which is why
its interiors looked like asphalt.

## Furniture

Against-wall share is 100% for beds, ranges, sinks, shelves, drawers, bookcases, chests, anvils and
armour; stools and chairs are 60 to 90%. Orientation relative to the wall side it stands against
(0 = orientation equals the side number, west 0, north 1, east 2, south 3): ranges, sinks, cooking
shelves, bookcases, shelves, counters: 0; beds: mostly 3; chests: 1; anvils: 2. Many items are
2 tiles (range 1x2, sink 1x2, bed 1x2, counter 2x1) and anchor on their south-west tile; the
footprint swaps its sides on odd orientations.

Banks (Varrock West, from the ASCII dump): a hall with a counter line of `banktable`/
`bank_booth_private` objects, `bank_chest` and `deposit box` pieces, **skirting boards**
(`fai_varrock_bank_skirting_board` 2131, 100% against a wall, one orientation) along every wall,
chandeliers, potted plants, stone floors. The generator's bank lobby uses a row of `bankbooth` 10355
against a wall; skirting boards, counters and the staff side are not generated yet.

## The ground

- Authored terrain heights are flat under real buildings and in the ring around them (median
  spread 0 in both) wherever the map authors set heights; roofed buildings sit on levelled ground.
  The generator therefore grades the site: the footprint to one level, blended to the original
  height across the margin to the site edge.
- The ground just outside the walls is ordinary natural ground (underlays 50, 63, 48, 64);
  in front of doors it is 40 to 60% ordinary ground, the rest paths. There is no special plinth
  or doorstep material.

## Styles (roofed buildings grouped by exterior wall)

| Style | Roof | Doors | Windows | Floors |
|---|---|---|---|---|
| brickwall 1902 | roof_wooden 1933, roof_redslate 1924, thatched 1926 | poordoor 1535, poshdoor 1540 | decor 1821 out, 1822 in | overlay 5 |
| timberwall 1602 | roof_wooden, thatched_timberroof 1604, thatched 1926 | poshdoor 1540, poordoor 1535 | decor 1828/1823 out, 1824/1822 in | overlay 5 |
| oldwall 1631 | oldroof_wooden 1640, greyslate 1643 | poordoor 1535, poordoor_cross 1537 | shutterwindow 1838, broken shutters | overlay 14, 5 |
| fai_varrock_* | fai_varrock_roofkit_slate 15552 | fai_varrock_door 11775 | window wall pieces | overlay 81, 171 |
| fai_falador_* | fai_falador_roofkit_slate 24125 | fai_falador_poor_door 24050 | window wall pieces | overlay 123 |

## What the generator does with this

`osrs-edit/src/building_finish.rs` (floors by room role, window decoration on both faces, wall
decoration, furniture with real orientation rules, rows of booths, grading) and
`building_kit.rs` (kit files). The built-in kits `lumbridge-brick` and `timber-town` are built from
the rows above. Not yet done: skirting boards, counters with a staff side, multi-floor buildings,
town-specific wall/door/window piece sets, upper-floor interiors, exterior decoration such as
lanterns and signs, and per-archetype layout rules (a bank's hall and vault, a church's nave).
