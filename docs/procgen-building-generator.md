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

- [x] semantic edge model for exterior wall / interior wall / door / window / open
- [x] north/east/south/west edge identity
- [x] physically adjacent seeded exterior entrance
- [x] floor-material intent
- [x] semantic object-role palette
- [x] compile `BuildingPlan` into `MapPatch`
- [x] ensure realization uses exact `ObjectPlacement` values and normal semantic invalidation
- [x] exact undo/redo test for generated buildings

Building realization is intentionally palette-driven. The semantic architecture knows that an edge is an exterior wall, interior wall, door, window, or opening, but does not know cache object IDs. The palette supplies exact object IDs, location kinds, and orientation mappings for each cardinal direction.

The single-floor house planner puts the exterior entrance directly off the end of its hallway. This gives realization one unambiguous shell edge for the exterior door instead of treating a distant boundary tile and hallway tile as if they were one portal.

## Checkpoint 3: WFC engine

Implement simple-tiled WFC over semantic architectural patterns.

- [x] pattern IDs and directional sockets
- [x] compact `u128` bitset domains for up to 128 semantic patterns
- [x] precomputed adjacency compatibility tables
- [x] lowest-domain entropy selection with stable tie-breaking
- [x] deterministic weighted observation
- [x] propagation queue
- [x] contradiction reporting with exact grid coordinate
- [x] bounded backtracking
- [x] fixed / allowed-domain constraints
- [x] tests for determinism, socket compatibility, pinned cells, contradictions, and backtracking recovery

WFC is responsible for local architectural compatibility. It is not responsible for global room reachability or building-program guarantees. The solver is intentionally generic and has no RuneScape object-ID dependency. A higher layer maps semantic WFC pattern IDs to architectural variants before palette realization.

## Checkpoint 4: hybrid building solver

- [x] room/program graph grammar by archetype
- [x] deterministic BSP / constrained rectangular room packing
- [x] reserved connected corridor bands during recursive partitioning
- [x] convert BSP rooms/corridor bands into final semantic building topology
- [x] route program-preferred connections through the corridor network
- [x] exact entrance count and placement constraints for BSP layouts
- [x] first WFC facade pass for straight exterior wall/window runs
- [x] deterministic contradiction retry after each WFC solve exhausts bounded backtracking
- [x] seeded facade reroll without changing macro building geometry
- [ ] WFC corner variants
- [ ] WFC door / portal variants

The CP4 macro solver separates global topology from local appearance. Archetype programs describe required spaces and preferred relationships. BSP packs those spaces without overlap and reserves one-tile circulation bands. Topology conversion turns room and corridor rectangles into a validation-clean `BuildingPlan` with exactly one deterministic exterior entrance, Door portals from semantic rooms into circulation, and Open portals between corridor bands. Program routing then computes a stable shortest corridor path for every preferred room relationship.

Facade refinement runs only after that topology is fixed. Its first pass converts eligible straight exterior-wall edges into isolated semantic windows while preserving corners, doors, openings, rooms, corridors, and reachability. Each WFC attempt receives its normal bounded backtracking budget. Only a true contradiction can trigger a deterministic alternate local seed. Reroll variations derive a facade-only seed from the building seed plus a variation number, so visual rerolls cannot move rooms or corridors.

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

- [x] building-generator panel state (Prefabs rail tab, Build page; no separate `Tool`)
- [x] rectangular site selection (click the map or use the tile selection); polygon sites are not done
- [x] live semantic preview (ghost of the objects added and removed)
- [x] seed field and reroll action (plus a windows/doors-only reroll)
- [x] archetype/style/kit controls (kits are JSON files, `osrs-edit/src/building_kit.rs`)
- [ ] validation diagnostics overlay (errors are shown as one line; no per-room overlay yet)
- [x] Apply as one history transaction (composite route, `app/editing/prefab_commit.rs`)
- [x] save a generated building to the prefab library
- [x] room program sized to the site; rooms at least 3x3; hip roof from real piece semantics (`building_roof_hip.rs`)

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
