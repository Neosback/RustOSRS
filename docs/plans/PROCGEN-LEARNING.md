# Procgen learning: what WFC cannot see, and the labelled data to teach it

## Accepted implementation sequence — 2026-10-20

The product target is local/offline cache learning followed by reviewed kits, deterministic
multi-room/multi-floor building generation, terrain and modular-object autotiling, validated
preview and one reversible commit. Both multiple models within one definition and multiple
placements composing one semantic assembly are in scope. Inferred suggestions remain separate
from reviewed generation data. Broad cache discovery is not universal generation certification.

1. Establish correctness at existing owners: reproduce occupied scene-slot failures; propagate
   vertical failures; replace generator-owned stair IDs with explicit kit assets; preserve room
   role intent. Do not weaken placement validation.
2. Extend catalogue evidence and reviewed prefab relationships with model composition, assembly
   membership, relative transforms, footprints/clearance, anchors, style and provenance. Preserve
   the canonical model-selection rules; typed alternative models are not automatically an assembly.
3. Connect census/review/training/generator role vocabularies. Build distinct per-floor room graphs,
   paired stairs, landings, openings and support before furnishing. Bind learned patterns to reviewed
   structural assets/assemblies; keep global connectivity outside WFC.
4. Extend canonical terrain-shape fitting and modular edge/socket rules for transitions, corners,
   junctions, fences and roofs. Unknown boundaries remain incomplete evidence.
5. Integrate review/correction and per-room/portal/assembly diagnostics into Studio. Preserve bounded
   workers, stale-result rejection, cancellation, exact undo/redo and reproduction.
6. Certify reviewed house/shop/tavern families using held-out geographic/authored-family groups.
   Run 1,000 fixed supported-site requests per family across supported floor counts; target at
   least 90% valid-candidate yield and zero invalid accepted candidates. Report detection precision,
   recall, confusion and abstention against independent human labels; weak labels are not truth.
   Review a fixed generated gallery for appearance and furnishing quality. Failed families remain
   uncertified. These are acceptance targets, not existing evidence.

The initial correctness checkpoint adds building-kit v2 explicit up/down stair assets, dependency
enumeration and circulation-stage classification, and propagates stair realization errors.
Version-1 kits still load for single-floor generation; multi-floor use requires a v2 kit with
explicit stairs. Existing starter stair IDs are preserved as data, not newly certified assets.
Authored room labels now use `BuildingRoomRole::from_label` and `label`; unresolved labels require
review instead of silently becoming living rooms. This does not implement a trained room classifier,
assembly discovery, distinct authored upper floors, or the certification corpus.

The same checkpoint fixes two scene-slot constraints: optional wall finishing reserves one
decoration slot per source-plane tile (all wall faces share it), and wall matching reserves fixed
corner/pillar slots while keeping different source planes independent. A 48×10, seed-0, full-decoration
fixture failed on competing corner decorations before the fix and passes afterward. Existing authored
objects are not deleted to resolve these conflicts; unresolved structural conflicts still reject.

Dataset v2 gains optional `models` evidence in object records: `combined` carries the ordered raw
model-ID list; `typed_alternatives` carries ordered `{kind, model}` entries. Neither is a discovered
multi-placement assembly. Full-width IDs and duplicates remain intact. Offline/runtime role features
use identical `model_representation:*` and `model_entries:*` channels; absent evidence keeps legacy
datasets/models usable. The role trainer now honors `human:member:<occurrence>:role` before the
definition-wide human label; an explicit empty member correction suppresses that fallback. Inferred
labels do not become training targets. Lab kit labels include explicit stair climb actions.
Kit finishing validates canonical room-role keys through the same room-role owner rather than a
second role-name list. Synthetic structural kit fixtures use the client's correct orientation
numbering. The mixed-success/rejection reproduction test now creates deliberate source occupancy
at a seed-dependent decoration anchor, so it does not depend on accidental generator defects.

### Building-kit v2 compatibility

`rustosrs-building-kit` version 2 adds optional `stairs` with `up` and `down` `KitAsset` records
(`id`, scenery `kind` 10/11, and four orientations in north/east/south/west order). Missing stairs
allow single-floor generation and reject multi-floor generation with `MissingStairs`. Invalid
kinds/orientations reject compilation. Version 1 must omit stairs; unsupported versions reject.
Stair dimensions come from source definitions through existing footprint resolution, not assumed
one-tile declarations. This schema alone does not certify landing clearance or vertical traversal.

`BuildingRoomRole` retains all canonical roles, including guest room, bathroom, armory and cell.
Census aliases resolve explicitly (church → nave, bank → lobby, smithy → workshop, shop → sales
floor, study → office, tavern/pub → common room). `general`, unknown and empty labels stay
unresolved. Unassigned drawn floor remains the existing default living-space intent.

## Earlier rationale and source observations

Plan of record for extending the building generator's WFC from hand-written patterns toward
patterns, weights and small models learned from real maps. Written 2026-10-18; the dataset exporter
now lives in `osrs-editor/examples/building_dataset.rs`, where composition can use both world/cache
inputs and the Lab's canonical learning split. Current implementation status is recorded in
`DEV_HANDOFF.md` and `docs/PROJECT-STATUS.md`; the phases below preserve the original sequence.

## Where the generator stands

- **Macro layout** (BSP rooms, corridor bands, exactly one entrance, validation) is deterministic and
  hand-built. It guarantees global properties (reachable rooms, closed shell) that WFC cannot.
- **WFC today** (`building_wfc.rs`, `building_refinement*.rs`, `building_roof_wfc.rs`) is a generic
  simple-tiled solver over at most 128 hand-written patterns with `u16` sockets. It refines straight
  exterior wall runs into windows and picks roof slope/eave variants. Patterns, sockets and weights
  are authored by hand; nothing about real maps reaches the solver except through kit JSON.
- **Furnishing and finishing** (`building_finish*`, kits) are rule tables measured from the census
  (`docs/building-census-findings.md`), not solved.

## What WFC cannot see

Each row is something a person sees at once and the solver has no handle on, with where a label for
it can come from today.

| Blind spot | Why it hurts | Label source |
|---|---|---|
| What an object is (ids are opaque, 35,350 distinct ones in the buildings) | It cannot tell a bed from a barrel, so it cannot pick a variant or refuse a bad neighbour | `object_semantics` (name tokens, structure, relations), gameval symbol, cache `category`, 117HD `model_overrides` tags |
| Which way a piece faces and what it attaches to | A range must stand against a wall, facing in; a window decoration hangs on a wall edge, on both faces | Measured: decoration orientation minus wall side is 0 in 100% of cases; furniture orientation relative to the wall it stands against (`Furniture.against`) |
| Footprint, clearance and walkability | Multi-tile pieces swap axes on odd turns; doorways must stay clear; a room must stay connected | Object size from the definition; the existing connectivity check |
| Room role | A kitchen and a bank lobby share walls and differ in everything inside | `RoomTag` (word evidence, weak), 117HD named areas (hand-made, sparse) |
| Building archetype and style family | Wall/door/window/roof pieces come in town sets that must not be mixed | 117HD area names, the dominant exterior wall piece, region |
| Edge sockets between tiles | The wall type on each of four edges (wall, door, window, opening, railing) is what a tile's neighbours must agree with; prefabs do not store it | Derived from the placed boundary pieces: `Edges::of_plane` and `WallRecord` |
| Vertical relations | A roof overhangs by one tile and sits on the plane above; stairs pair across planes | `UNDER_ROOF` volumes (`osrs-scene::roofs`), roof pieces by loc kind |
| Ground context | Doorsteps, paths, the ring around the building | `door_aprons`, `ring`, 117HD `ground_materials` |
| Whether the result is *good* | Validation proves closure and reachability, not taste | Human accept / reject in the gallery (not built) |

The honest summary: WFC needs **symbols** (roles, not ids), **relations** (attaches-to, faces,
against-wall, on-the-other-face-of) and **context** (room role, style family, archetype) as inputs,
and **real adjacency statistics** to replace hand-written sockets.

## Labelled data that exists now

`building_dataset` writes `buildings.jsonl` (one real building per line) and `objects.json`:
position, size, rooms with role tag, floors, furniture (with `against` and `wall_distance`), wall
decoration (with the edge side and inside/outside), every wall edge (piece, direction, exterior,
feature), roof pieces, ring and door-apron ground, height spread, and, with `AREAS=` pointing at
117HD's `scene/areas.json`, the hand-made area names containing each building. Over the OpenRune
build 240 cache: **3,944 buildings, 35,350 distinct objects, 1,888 buildings inside some named
area** (most are whole-region names such as `VARLAMORE`; the specific ones are about a hundred:
21 bank, 9 church, 5 inn, 53 house, 5 kitchen, 11 guild, 11 temple, 49 castle).

First measured disagreement, which is the point of having the data: of the 21 buildings inside a
bank area only **9** have a room the census tags `bank`; of 9 church areas **3** have a `church`
room. The word-evidence tagger is weak, so it is a feature, not the label. The tagger's output
distribution (4,375 `general`, 635 `living`, 426 `bedroom`, 393 `storage` of 6,400 rooms) says the
same.

## Phases

1. **Labels** (next). Add to the exporter the pieces the solver needs as inputs: edge-socket grid
   per building (four edge symbols per tile, the thing a tiled WFC adjoins), object role per piece
   (`object_semantics::infer` output with its confidence), attach relation (decoration to wall,
   furniture to wall side), walkable mask. Store the labels next to a prefab (`labels` section,
   `PREFAB_VERSION` bump) so captured and generated prefabs carry them too.
2. **Learned patterns.** Build the WFC pattern catalog from the edge-socket grids: each distinct
   (floor class, four edge symbols) tuple is a pattern, observed adjacencies become the
   compatibility table, counts become weights, per archetype/style. This replaces hand-written
   sockets with data and keeps the existing solver (`PatternDomain` is `u128`; group by style so
   each catalog stays under 128 patterns or widen the domain; since widened to 512, `MAX_WFC_PATTERNS`).
3. **Gallery as the labelling tool.** The gallery already shows each prefab in 3D. Add: accept /
   reject on the generated result (feeds a quality label), room-role and archetype dropdowns that
   write corrections, and a per-object "what is this" prompt for the opaque ones the role inferrer
   is least sure of. Corrections are stored as `labels` with `source: human`.
4. **Narrow models, trained offline.** Small, boring, inspectable: (a) object role classifier on
   token and structure features, with the human corrections as test set; (b) room-role classifier
   from furniture bag-of-roles plus size and door count; (c) a furniture placement scorer (where a
   role stands relative to walls and doors) replacing the hand `place`/`relative` rule tables;
   (d) a validity critic trained on accepted versus rejected generations. Training stays outside the
   workspace (Python, the exported JSONL); the editor only loads the result as tables or tiny
   weight files in a kit, so inference stays deterministic and wasm-safe.
5. **Close the loop.** Generated buildings are scored by the critic before they are shown; the
   generator keeps the seed with the best score of N.

## Guardrails

- Determinism: the same seed, request, kit and tables give the same plan. Learned tables are data
  versioned with the kit; no floating-point inference in the solver path unless it is quantised.
- Labels carry their source and confidence (`word`, `structure`, `relation`, `area`, `human`); a
  human label always wins and is never overwritten by a later inference run.
- The exporter reads caches and writes files; it never touches a map. Datasets are derived data and
  are regenerated, not committed.
- Area names come from a third-party file (117HD, BSD-2); keep the attribution in `THIRD_PARTY.md`
  if the names are ever stored in this repository rather than joined at export time.

## Reproduce

```sh
AREAS=/path/to/RLHD/src/main/resources/rs117/hd/scene/areas.json \
CENSUS_EXCLUDE=user_world_defs.json \
cargo run --release -p osrs-editor --example building_dataset -- "<OpenRune root>" out/
```

## Learning to lay paths (implemented)

A **path lesson** is a rectangle of real map (ground, blocked tiles, heights, objects on the ground
plane) with the floor overlays that are paths. `osrs-lab::paths` hides the paths (a blank canvas that
keeps everything else) and asks a learner to draw them back; the lesson's real paths are the hidden
target, scored by the lab. Nothing is copied: the learner is a 26-input network (about 700 numbers)
that scores how likely each *open* tile is to be path from what surrounds it, trained on many places
and checked on places it never saw.

What it sees (`paths/sense.rs`, `FEATURE_INFO` has the plain-language sentence for each): blocked
ground and solid objects, buildings (rooms from the building census) and how near they are, the tile
just outside each exterior door, free-standing walls and fences, slope, ridges, trees, rocks, lamps,
market clutter, plants, narrow gaps, streets between buildings, other floor types (plazas, shores),
wide-open ground and the edge of the area. It never sees the path overlay, and does not see the
underlay (road shoulders would leak the answer). Tiles that are inside buildings or already another
floor are not decisions and are excluded from training and from every score.

Two ways to use it: **given the ends** of each path (doors, gates, where it leaves the area) the
router joins them by least cost over `-ln(chance)` with branches joining the network, and the result
is laid with the existing path tool (`Action::PathLay`, undoable on the map through
`apply_terrain_path`); **alone**, it paints the tiles it is sure of (threshold tuned on the training
areas).

Measured on the pinned build-241 map (907 areas of 64 by 64 tiles with at least 40 path tiles,
whole 8 by 8 map-square blocks held out, `train-paths ... 24 150 1500 40`):

| | model | straight line | shortest walk | paint every open tile |
|---|---|---|---|---|
| tile ranking on unseen areas (AUC) | 0.92 | | | 0.5 |
| drawing between the real ends (on a real path, covers the centre line within 2) | 0.80 | 0.70 | 0.74 | |
| painting with nothing given (F1, 1 tile of slack) | 0.76 | | | 0.69 |

AUC rises with data and size (logistic 0.86, 12 hidden on 150 areas 0.90, 24 hidden on 900 areas
0.92), which is what generalising looks like. Honest limits: the labels are "an overlay that is long
and thin", not hand-marked roads; only 56% of door fronts have a path within 2 tiles and only 12%
of path pieces touch a door, while 66% touch the edge of their 64 by 64 area (roads continue past
it), so "alone" cannot know where roads enter an area and is much weaker than "given the ends";
the gap over the shortest walk is 6 points, not a different league; one imported area teaches mostly
its own quirks (the Workshop says when only the eastern third was held out).

Reproduce: `cargo run --release -p osrs-world --example path_lessons -- "<OpenRune root>" out/lessons`,
then `target/release/osrs-lab train-paths out/lessons out/model.json 24 150 1500 40 world-paths
"Paths of the game world (starter)"` (about 25 s). `paths-show`, `paths-census` and `paths-stats`
print a lesson as letters, the overlays the guesser takes for paths, and door and edge statistics.
`crates/osrs-lab/data/world-paths.model.json` is the model that ships (the Workshop's "comes with the
editor" model).

### Place names (`map_labels.json`)

The world-map labels (863 names: towns, regions, mines, dungeon mouths; `textScale` 0/1/2 for site,
town, region) are kept with each path lesson (`places`, within about 200 tiles) and used to name
imported areas and jump to a place in the Workshop. Tested as a learner input (distance to the
nearest town and to any named site as two extra numbers): no change on unseen areas (tile ranking
AUC 0.91 with and without, drawing 79% and 75% both ways), so they are not part of the model.
`MAP_LABELS=<file>` makes the `path_lessons` example embed them.
