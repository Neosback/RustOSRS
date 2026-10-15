# Object Zone Invalidation Plan

Status: O1-O4 complete and certified; O5 adversarial coverage next.

Branch: `refactor/object-zone-invalidation`

Base `main`: `a7b9fb47835ab00b25db7c02e1aa3ca3157a054b`

## Goal

Narrow finalized Object edit projection work from conservative region rebuilds to exact 8x8 output zones only when the runtime can prove every affected producer output.

`MapDocument` / `EditorSession` remains authoritative semantic state. World scenes, location replay logs, picker data, animation instances, geometry, and GPU resources remain disposable projections.

## Non-goals

This subsystem does not:

- move Object semantic authority into `WorldRuntime`
- infer correctness from renderer-only state
- guess one universal Object radius
- weaken accepted-build freshness gates
- narrow contextual scene assembly
- mix InteractionRuntime, renderer parity, 117HD feature work, or cache export work into this branch

## Safety invariant

For every finalized Object transaction, 8x8 zone invalidation is permitted only when the selected output zones are a proven superset of every producer output that can change because of the exact semantic Object mutations.

A proof must account for additions and removals as mutations, not merely for currently visible placements.

If the proof is absent, stale, incomplete, unsupported, or cannot represent mutation-order/counterfactual behavior, the complete conservative producer-region fallback is mandatory.

## O1 audit findings

### 1. The O1 semantic footprint was too lossy

`MapOp::AddObject` and `MapOp::RemoveObject` already carry exact `ObjectPlacement` identity:

- object id
- source plane
- world anchor tile
- loc kind
- orientation

At the O1 baseline, `EditFootprint` collapsed Object mutations to anchor `MapTile` values and lost mutation polarity: removed versus added.

O2 now preserves canonical full placement identity and mutation polarity through commit receipts and semantic Undo/Redo replay.

### 2. Renderer ownership is anchor-zone based, but scene dependencies are not

Reference and HD static loc extraction emit a loc into the output zone containing the loc anchor tile.

That does not make Object edits anchor-local, because the full contextual scene is built before zone emission.

An Object edit can alter another output through scene construction even when the edited object's own geometry belongs to only one anchor zone.

### 3. Game-object capacity is order dependent

Game objects occupy every tile in their storage footprint and each tile has a five-object capacity.

The worker replays Object edits over decoded locations, then processes placements in scene order. Added edits are appended to their source region's replay list. Removing or adding one game object can therefore change whether later overlapping game objects are accepted.

That creates causal propagation through placement order and overlap. A simple radius around the edited anchor is not a sufficient proof.

A current-scene map of `visible placement -> output zones` is also insufficient for removal, because a removed placement can cause a previously rejected later placement to become visible.

### 4. Fixed scene slots have replacement behavior

Boundary, wall-decoration, and floor-decoration slots are last-writer wins within their tile slot.

Removing the current winner can reveal an earlier placement. Adding a new placement can replace the prior winner.

The dependency proof must therefore model slot replacement history/effects, not just the final occupant.

### 5. Wall-decoration placement has same-tile host dependencies

Boundary/roof placements can rescale a previously placed wall decoration's displacement on the same tile.

The optional diagonal-decoration presentation pass also derives wall-decoration offsets from a type-9 host model on the same tile.

Changing the host can therefore change a different object's emitted geometry.

### 6. Loc shadows modify terrain output

Clipped boundaries write shadow-grid cells adjacent to their anchor. Clipped type-10/11 game objects write shadow values over their storage footprint plus the reference extra edge.

Terrain slope lighting subtracts the shadow grid using the current cell plus neighboring cells. The resulting corner light feeds terrain geometry colors.

When smooth-terrain presentation is enabled, shared untextured flat-terrain vertex colors are averaged after terrain build, extending the visible effect to adjacent terrain tiles sharing those vertices.

Object edits can therefore change terrain output zones as well as loc geometry zones.

### 7. ModelData normal reconciliation modifies neighboring models

Scene normal reconciliation scans neighboring boundary and game-object ModelData using the source object's storage footprint and the reference traversal, including the next storage plane where applicable.

Matched models receive merged normal contributions and may hide matched faces.

An Object edit can therefore change the finalized geometry/lighting of neighboring object models whose anchors differ from the edited placement.

### 8. Dynamic and omitted placements must remain fail-closed unless explicitly proven

Morphing, animation, absent-model, capacity-rejected, overwritten, and otherwise omitted placements still participate in input ordering and/or scene-side decisions.

Object dependency metadata must not equate "not currently emitted" with "has no dependency effect."

### 9. Producer-region context crosses map-region boundaries

Each producer builds a 104x104 scene window around its owned 64x64 output. The current layout provides 16 context tiles before the owned region and 24 after it.

Therefore an Object anchored in one map region can participate in a neighboring producer's scene and change that neighboring producer's terrain or reconciled-model output.

Before O1, Object-only semantic impact (`LOCATIONS | COLLISION | BOUNDS`) caused the generic fallback to rebuild only the touched anchor region. That was not conservative with respect to the worker's contextual scene model.

O1 hardens the fallback to the complete 3x3 producer-region ring around every touched Object region. This matches the set of neighboring region producers whose contextual windows can include anchors from the touched region under the current streaming architecture.

## O1 production change

`projection_invalidation` treats any unproven Object footprint as requiring `expand_regions(touched_regions, 1)`.

This means:

- Object-only edits rebuild the touched region and all eight neighboring producer regions when proof cannot be established
- mixed Object + Height/Paint edits also fail closed to the same producer ring when Object proof cannot be established
- Height-only and Paint-only behavior is unchanged
- the O1 ring remains the mandatory fallback after O4 runtime narrowing was added

Focused tests assert that unproven Object receipts resolve to the 3x3 ring and continue to override Height zone proofs.

O1 exact-head certification:

- head: `643f760a7f318d5519fc9897a588d732afe0c293`
- CI: `38699017109` / #1264 - SUCCESS
- Tier A/B/C all green

## O2 canonical Object mutation footprint

O2 upgrades finalized semantic footprints without changing Object projection invalidation behavior.

`EditFootprint` now retains two canonical semantic sets:

- `object_removed: Vec<ObjectPlacement>`
- `object_added: Vec<ObjectPlacement>`

Each placement retains exact:

- object id
- source plane
- world anchor tile
- loc kind
- orientation

`object_tiles` remains only as a derived compatibility summary for the existing O1 region fallback. It is no longer the only retained Object mutation identity.

Canonical Object placement sets are ordered deterministically by:

`(source plane, x, y, object id, loc kind, orientation)`

Exact duplicate placements are deduplicated independently inside the removed and added sets.

`EditFootprint::from_patch` preserves semantic patch polarity directly:

- `RemoveObject` -> `object_removed`
- `AddObject` -> `object_added`

Semantic history replay reconstructs the same exact placements from the canonical Object history route:

- Redo preserves the original removed/added polarity
- Undo swaps the original added/removed sets

Touched-region derivation continues to use the derived anchor-tile summary, so O1's 3x3 producer-region fallback remains unchanged.

Worker Object projection and replay validation remain unchanged. O2 does not move semantic authority and does not enable 8x8 Object zone narrowing.

Focused coverage verifies:

- exact placement identity survives finalized Object footprints
- canonical ordering is deterministic
- exact duplicate placements collapse
- removed and added polarity remains separate
- Undo reverses mutation polarity
- Redo preserves original polarity
- source plane, kind, orientation, object id, and anchor survive replay footprint reconstruction
- legacy anchor-tile region routing remains derived from those canonical placements

O2 code-head certification:

- code head: `fed985ea59db7737067b6d19d53844d2d4e9429a`
- CI: `38700886057` / #1275 - SUCCESS
- Tier A architecture/format/workspace/Clippy/WASM green
- Tier B full workspace tests green
- Tier C M3-M9 semantic/reference parity green

O2 final documentation-bearing certification:

- final O2 head: `d61d9169d271c1dde9009707ff59d73cb8001716`
- CI: `38701473420` / #1277 - SUCCESS
- Tier A/B/C all green

## O3 producer-output proof contract

O3 selected **producer output comparison** rather than a guessed causal radius or a current-placement dependency table.

The contract compares exact static producer output generated after full contextual scene assembly. This makes the proof sensitive to the actual consequences of:

- placement-order capacity changes
- fixed-slot replacement/reveal
- wall-decoration host effects
- terrain shadow/lighting changes
- ModelData normal reconciliation
- additions, removals, and counterfactual reveal/rejection behavior

`ObjectOutputSnapshot` records one producer region, explicit 8x8 zone coverage, and exact per-zone `SceneGeometry` output. A complete full snapshot covers all 64 owned output zones of the producer's 64x64 region, including explicitly covered empty zones.

Scoped snapshots carry explicit coverage and are incomplete unless the retained accepted state already proves the uncovered zones. Missing coverage is never interpreted as unchanged output.

The proof contract provides:

- deterministic full/scoped snapshot construction
- same-producer validation
- explicit complete/incomplete state
- exact per-zone output comparison using the existing structural `Eq` representation
- exact detection of added, removed, and changed output zones
- scoped merge into retained accepted state

Editor-side `ObjectOutputState` binds one accepted snapshot to:

- producer region
- projection epoch
- presentation generation

Lookup is explicitly one of:

- stale
- incomplete
- current with exact changed zones

O3 itself did not enable runtime Object zone narrowing; it established the proof and freshness contract used by O4.

O3 exact-head certification:

- head: `a555e07796a938676ddb3c4f7d0320dd480041aa`
- CI: `38703366564` / #1285 - SUCCESS
- Tier A/B/C all green

## O4 accepted-build runtime integration and planner

O4 wires the O3 proof into the accepted worker-result path without changing semantic authority or contextual scene construction.

`ObjectOutputStore` retains only disposable accepted producer output plus pending pre-edit proof stamps. Trusted Object output state is installed only after the existing worker result gates accept the request identity, projection epoch, and presentation generation.

Trusted state is cleared or removed at projection lifecycle boundaries:

- source/streamer replacement
- full presentation rebuild
- region unload
- accepted full absent-region result
- invalid snapshot/store update

For one finalized Object edit, runtime planning now:

1. derives the same O1 conservative 3x3 producer-region universe around the touched Object regions
2. considers every resident non-empty producer in that universe
3. requires a current, complete accepted pre-edit snapshot for every such producer
4. if any required producer is missing, stale, or incomplete, abandons proof planning and uses the O1 full-region fallback
5. otherwise queues one **full producer build** for each required producer, preserving the full 104x104 contextual scene
6. retains the exact pre-edit accepted stamp while the normal invalidation path advances the producer projection epoch
7. accepts the candidate only if the existing `BuildTracker` request/epoch and presentation-generation gates still pass
8. compares the accepted full post-edit static output against the retained pre-edit snapshot
9. installs only the exact changed 8x8 zones when comparison is current and complete
10. falls back to a full producer install on missing, stale, incomplete, mismatched, or invalid proof state

A later semantic invalidation cancels pending Object proof state and/or advances the projection epoch, so an already-running candidate build cannot be trusted after the world has advanced.

An exact empty static diff is valid: static GPU upload becomes a no-op while region-complete picker/inspection, animation, Height dependency, and Object proof freshness state still advances from the accepted worker result.

O4 intentionally does **not** narrow:

- contextual scene assembly
- worker source replay
- region-complete picker/inspection replacement
- animation projection replacement
- Height dependency replacement

No worker protocol change was required. The worker still builds the complete contextual producer result; narrowing is selected only after the accepted result is compared with the current trusted baseline.

O4 exact code-head certification:

- head: `52eadc6284e71d9bc00de878d89cc67a0f0c7628`
- CI: `38705182639` / #1294 - SUCCESS
- Tier A architecture/guard/reference/format/workspace/Clippy/WASM green
- Tier B full workspace tests green
- Tier C M3-M9 semantic/reference parity green

## Required Object dependency contract

The implemented narrowing mechanism proves **mutation effects** through exact producer-output comparison rather than relying on current render ownership.

The semantic footprint still retains exact removed/added placement identity for history, routing, diagnostics, and future validation, while the runtime proof asks the stronger question: did any static output zone produced from the complete contextual scene actually change?

The proof remains fail-closed unless:

- the producer region matches
- accepted baseline coverage is complete
- projection epoch is current
- presentation generation is current
- the full candidate worker result passes normal stale-build gates
- exact output comparison succeeds

## Chosen proof strategy

O3 chose the previously documented **producer output comparison** strategy.

This was selected over a causal dependency graph because the existing producer build already computes the complete contextual consequences of capacity, replacement, host, shadow, lighting, and normal-reconciliation behavior. Exact structural zone comparison therefore proves changed static output without duplicating those scene semantics in an editor-side dependency graph.

The important tradeoff is deliberate: Object edits still perform full contextual CPU producer builds for the conservative producer universe, but exact comparison can narrow final static GPU installation to changed 8x8 zones. O5 must adversarially validate that this boundary remains sound.

## Checkpoints

### O1 - dependency audit and conservative fallback hardening

Status: COMPLETE and CERTIFIED.

Delivered:

- audited semantic Object identity and O1 receipt lossiness
- audited replay ordering and game-object capacity behavior
- audited fixed-slot replacement
- audited wall-decoration host/displacement effects
- audited loc-shadow to terrain-light propagation
- audited ModelData normal reconciliation
- audited Reference/HD anchor-zone emission versus full-scene context
- identified cross-region under-invalidation in the previous Object-only fallback
- hardened Object fallback to the 3x3 producer-region ring
- retained zero Object zone narrowing at O1

Certification: `643f760a7f318d5519fc9897a588d732afe0c293`, CI #1264 / `38699017109`, SUCCESS.

### O2 - canonical Object mutation footprint

Status: COMPLETE and CERTIFIED.

Delivered:

- full canonical `ObjectPlacement` identity retained in finalized Object footprints
- exact removed and added placement sets retained separately
- deterministic canonical ordering and exact deduplication
- patch execution preserves original mutation polarity
- semantic Undo swaps mutation polarity exactly
- semantic Redo preserves original mutation polarity
- derived `object_tiles` compatibility summary retained only for existing region routing
- O1 3x3 region fallback unchanged
- focused commit/replay footprint tests added

Final certification: `d61d9169d271c1dde9009707ff59d73cb8001716`, CI #1277 / `38701473420`, SUCCESS across Tier A/B/C.

### O3 - dependency/proof contract

Status: COMPLETE and CERTIFIED.

Delivered:

- producer-output comparison chosen as the Object proof strategy
- exact full/scoped producer output snapshots
- explicit 64-zone complete coverage for full producer state
- scoped coverage that never treats missing zones as unchanged
- exact added/removed/changed output-zone comparison
- same-producer validation
- projection-epoch and presentation-generation freshness stamp
- stale/incomplete/current lookup contract
- scoped accepted-state merge and full replacement
- no runtime narrowing enabled by O3 alone

Certification: `a555e07796a938676ddb3c4f7d0320dd480041aa`, CI #1285 / `38703366564`, SUCCESS across Tier A/B/C.

### O4 - accepted-build runtime integration and planner

Status: COMPLETE and CERTIFIED.

Delivered:

- accepted Object output store installed only from current worker results
- lifecycle clearing on source/presentation/unload/absence boundaries
- all-or-nothing proof eligibility across every resident non-empty producer in the O1 3x3 universe
- full contextual post-edit candidate builds
- exact accepted pre/post zone comparison
- exact changed-zone static installation
- exact empty-diff handling
- fail-closed full-region install/fallback for missing, stale, incomplete, mismatched, or invalid proof
- pending proof cancellation on later invalidation
- stale in-flight candidate rejection through existing projection epoch/request identity gate
- region-complete picker, animation, and dependency projections preserved
- no worker protocol or contextual scene assembly narrowing

Code certification: `52eadc6284e71d9bc00de878d89cc67a0f0c7628`, CI #1294 / `38705182639`, SUCCESS across Tier A/B/C.

### O5 - adversarial coverage

Status: NOT STARTED.

Required cases include:

- 8x8 zone boundary
- 64x64 region boundary
- fixed-slot replacement/reveal
- wall-decoration displacement host changes
- game-object five-per-tile capacity where removal reveals a later object
- overlapping multi-tile capacity chains
- clipped wall shadow changing neighboring terrain light
- type-10/11 footprint shadow affecting terrain across a zone edge
- ModelData normal merge changing a neighboring object's output
- dynamic/morphed/omitted cases remaining fail-closed unless proven
- semantic Undo/Redo exact mutation polarity
- stale/in-flight worker result rejection

O5 should prefer end-to-end producer-output assertions over duplicating scene dependency rules in planner tests. It must prove both that changed zones are not missed and that stale/incomplete proof state falls back safely.

### O6 - exact-head certification, merge, integrated-main certification

Status: NOT STARTED.

Before merge:

- exact final branch head
- architecture boundaries and architecture guard
- reference regeneration safety
- historical deob golden index
- rustfmt
- workspace compile
- strict Clippy
- WebAssembly portability
- full workspace tests
- M3-M9 semantic/reference parity
- focused Object invalidation tests
- branch remains Object-only plus handoff/plan documentation

Then:

1. open one PR from `refactor/object-zone-invalidation` to `main`
2. merge before creating any new development branch
3. validate exact integrated `main`
4. update `DEV_HANDOFF.md`
5. certify the final handoff-bearing `main` head if docs advance `main`
6. stop before any new branch

## Branch discipline

This is the only active development subsystem branch.

No second development branch may be created until the Object subsystem is complete, merged into `main`, exact integrated `main` is green, the handoff is current, and the user explicitly directs the next subsystem.
