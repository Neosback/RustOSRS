# Procedural Building Generator Roadmap

Branch: `feat/procgen-building-generator`

This branch owns the complete procedural-building effort. Work stays on this branch and advances through checkpoint commits so architecture and behavior can be reviewed incrementally.

## Architectural rules

- Procedural generation is headless and semantic. It must not depend on egui, wgpu, render workers, cache transport, or editor widget state.
- A building is planned before it is realized as RuneScape terrain/object edits.
- Global guarantees are solved before local WFC detail.
- The same seed + request + palette + source map state must produce the same plan.
- Preview does not mutate `MapDocument`.
- Apply produces one reversible semantic transaction.
- Generation failures are explicit. Broken or partially valid buildings are never silently committed.

## Target pipeline

```text
BuildingRequest
  -> site analysis
  -> building program / room graph
  -> macro layout
  -> local WFC
  -> portals / stairs / roof
  -> furnishing / decoration
  -> validation + repair
  -> BuildingPlan
  -> asset realization
  -> MapPatch
  -> MapDocument / history
```

## Checkpoint 0: semantic foundation

- [x] deterministic building request
- [x] rectangular generation site
- [x] building archetype/style vocabulary
- [x] room IDs, room rectangles, room types, and explicit connections
- [x] occupancy grid
- [x] first deterministic connected single-floor house planner
- [x] tests for deterministic output, room bounds, connectivity, and occupancy bounds

The first planner deliberately uses a simple hallway-and-room layout. It exists to lock down global building contracts before WFC is introduced.

## Checkpoint 1: generation snapshot + plan validation

- [x] read selected terrain/height state through semantic query boundaries
- [x] build a generation-side occupancy snapshot
- [x] track existing object footprints supplied by an adapter
- [x] validate selected site bounds and required height vertices
- [x] BFS room reachability validator
- [x] doorway clearance / adjacency validator
- [x] room-overlap and corridor-overlap validator
- [x] deterministic diagnostic reporting

`GenerationSnapshot` is deliberately bounded to the requested site. Terrain and height state come through `MapQuery`; cache/catalog code adapts object definitions into `OccupiedFootprint` values before the solver sees them. This keeps procedural generation independent of cache and renderer infrastructure.

## Checkpoint 2: semantic realization

- [ ] semantic edge model for exterior wall / interior wall / door / window / open
- [ ] floor-material intent
- [ ] semantic object-role palette
- [ ] compile `BuildingPlan` into `MapPatch`
- [ ] ensure realization uses exact `ObjectPlacement` values and normal semantic invalidation
- [ ] exact undo/redo test for generated buildings

Checkpoint 2 must also make exterior entrances and interior portals edge-oriented. Checkpoint 1 validates room-level doorway adjacency, but RuneScape realization needs an explicit north/east/south/west wall edge so opposite-facing doors on one tile cannot be conflated.

## Checkpoint 3: WFC engine

Implement simple-tiled WFC over semantic architectural patterns.

- [ ] pattern IDs and directional sockets
- [ ] bitset domains
- [ ] adjacency compatibility tables
- [ ] entropy selection
- [ ] deterministic weighted observation
- [ ] propagation queue
- [ ] contradiction reporting
- [ ] bounded backtracking
- [ ] fixed cells / pinned constraints
- [ ] tests for determinism and contradiction recovery

WFC is responsible for local architectural compatibility. It is not responsible for global room reachability or building-program guarantees.

## Checkpoint 4: hybrid building solver

- [ ] room/program graph grammar by archetype
- [ ] BSP / constrained rectangular room packing
- [ ] corridor routing
- [ ] exact entrance count and placement constraints
- [ ] WFC refinement of walls/corners/windows/door variants
- [ ] local repair followed by backtracking
- [ ] seeded reroll without changing request geometry

## Checkpoint 5: multi-floor + roofs

- [ ] vertical anchors
- [ ] stair-up/stair-down paired constraints
- [ ] per-floor solver passes around fixed vertical anchors
- [ ] upper-floor support validation
- [ ] roof coverage solver
- [ ] roof edge/corner WFC variants
- [ ] atrium/void support

## Checkpoint 6: interiors

- [ ] room-specific furnishing grammars
- [ ] asset footprint and clearance masks
- [ ] wall-mounted object sockets
- [ ] weighted furniture variants
- [ ] sparse decoration sampling
- [ ] walkability preserved after furnishing

## Checkpoint 7: terrain fitting

- [ ] Reject / Fit / Grade / Terrace policies
- [ ] foundation-height sampling
- [ ] deterministic grading patch
- [ ] blend ring around foundation
- [ ] slope/height constraints for entrances

## Checkpoint 8: editor integration

- [ ] building-generator tool state
- [ ] rectangular/polygon site selection
- [ ] live semantic preview
- [ ] seed field and reroll action
- [ ] archetype/style/palette controls
- [ ] validation diagnostics overlay
- [ ] Apply as one history transaction

## Checkpoint 9: building instances and regeneration

- [ ] generation manifest / building instance ID
- [ ] plan hash
- [ ] palette version
- [ ] select generated building
- [ ] regenerate from seed
- [ ] restyle / refurnish
- [ ] remove generated building safely

## Checkpoint 10: production hardening

- [ ] cross-region sites
- [ ] stress/property tests across many seeds
- [ ] contradiction-rate metrics
- [ ] performance profiling
- [ ] pathological-site tests
- [ ] deterministic snapshots/golden fixtures
- [ ] CI quality gates
