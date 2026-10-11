# Ideas from Terraini (`Java Deob/Terraini-deob`)

Terraini is an RSPSi-derived map editor (JavaFX, with OSRS/317/530/718 plugins). Decompiled sources under
`src/com/rspsi`. These are ideas to evaluate, not designs to copy; the decompilation is of someone else's
product, so we read it for *concepts* and re-derive the code from the client data model.

## Auto-tiler (most relevant)
* **`OverlayShapeAtlas`**: rasterises every overlay shape x rotation (12 x 4) into an N x N sub-tile bit
  mask (N = 2..16) using the real `ShapedTile` vertex/face tables, and stores coverage, the four *edge
  masks* ("portals": which parts of each tile edge are overlay) and a ranking function
  `candidates(mask)` that sorts all 48 (shape, rotation) pairs by Hamming error against a painted mask.
  Our `AUTO-TILER.md` plan used a 4-bit corner index; the sub-tile mask is strictly better: the user can
  paint finer than a tile, and ambiguous cases resolve by minimum error instead of hand rules.
  Their coverage table `[1, .5, .25, .25, .75, .75, .5, .125, .875, .34375, .65625, .1875]` matches the
  shares we computed from `FACE_CODES`, which cross-checks both.
* **Portals for continuity**: neighbouring tiles agree when the facing edge masks match; the atlas exposes
  `edgeMask(shape, rotation, N/E/S/W)`, so a road or shoreline is chosen as a path through tiles whose
  portals line up.
* **`RoadShapeGrammar`**: a table keyed by the 8-neighbour occupancy pattern (canonicalised under 90-degree
  rotation: 48 classes) holding the observed frequency of each (shape, rotation) in real maps, e.g.
  "pattern 1: shape 0 rot 0 at 41.7%, shape 10 rot 2 at 6.5%...". This is the "census" we planned; do it
  from our own cache so it matches build 241, store it as data, and use it to break ties and add variety
  (seeded weighted choice).
* **Path tools**: `PathPlanner` (A* on a `CostGrid` with obstacle routing), `SplinePathCompiler` /
  `PointPathCompiler` (drag a curve, compile to tiles), `StrokeResampler` (even sampling of a mouse stroke),
  `PathProfile` (width, edge style, material per profile). Roads/rivers as *curves* that compile to tiles.
* **`IslandGenerator`** with `CoastTemplateLibrary` and noise: coast styles Beach / Cliff / Mixed where
  "mixed" was "harvested from real coastlines" (data-driven templates). Same trick works for terrain
  transitions generally: harvest templates from the real map instead of hand-writing rules.
* **`BuildingGenerator`** (kit catalogue, faces, plans): procedural buildings from wall/roof kits.

## Editing model
* **Layers** (`MapLayer`, `LayerManager`, `LayerCompositor`, `ProjectManager`): non-destructive layers
  holding tile changes, object changes and path networks; a compositor applies visible layers over the
  base cache. This is the right shape for our `EditCommand` + undo plan: layers are saved projects,
  commands are the in-session history; export flattens visible layers into a cache.
* **Selection**: modes Replace / Add / Subtract / Toggle; shapes Rectangle / Brush (`TileSelectionModel`,
  `SelectionEdges` for marching-ants outlines).
* **Tile paint**: modes Paint / Erase / Sample; target Material+shape / Material only / Shape only;
  flag painting with Replace/Add/Remove/Toggle (`FlagPaintMode`).
* **Height brush**: Raise / Lower / Flatten / Smooth (Laplacian) / Smooth (slope limit), with falloffs
  Smooth / Linear / Gaussian / Sharp / Hard (`HeightBrushEngine`, `HeightFalloff`).
* **Stamps / map pieces**: copy a region as a *map piece* (clipboard + library + manifest + codec +
  preview renderer) and paste with an anchor (origin/centre), height mode (relative / absolute / keep
  target) and conflict policy (merge / skip occupied / replace). A piece library is a very productive
  feature for content creators.
* **Object placement policy** (`ObjectPlacementPolicy`, `ObjectTypeFilter`): filters which loc types a
  tool may place or remove, with a `PlacementResult` report of what was skipped and why.
* **Key actions** (`KeyActions`): tiny enum (add-to-selection, inverse flag set, overlay-only paint,
  rotate cw/ccw): confirms that modifier-based tool variants are the convention.

## What to take first
1. Sub-tile mask + atlas + portals for the auto-tiler (pure, testable against `FACE_CODES`).
2. Census-derived grammar table from our cache (replaces hand rules, gives natural variety).
3. Layers + command history as the edit model.
4. Selection modes and the paint/height/stamp option sets as the first tool set once the tool trait exists.
