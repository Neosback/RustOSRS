# Object Zone Invalidation Plan

Status: O1 certified; O2 canonical Object mutation footprint complete and code-certified. Final handoff-bearing O2 head certification pending.

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

No 8x8 Object narrowing is enabled by O1 or O2.

## O1 production change

`projection_invalidation` treats any Object footprint as requiring `expand_regions(touched_regions, 1)`.

This means:

- Object-only edits rebuild the touched region and all eight neighboring producer regions when resident
- mixed Object + Height/Paint edits also fail closed to the same producer ring
- Height-only and Paint-only behavior is unchanged
- no Object dependency metadata is trusted yet

Focused tests assert that Object receipts resolve to the 3x3 ring and continue to override Height zone proofs.

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

Focused coverage now verifies:

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

The final documentation-bearing O2 branch head must be certified separately before O3 starts.

## Required Object dependency contract

The future narrowing mechanism must prove **mutation effects**, not merely current render ownership.

At minimum the proof must be able to account for:

- exact removed placements
- exact added placements
- source plane
- kind and orientation
- storage footprint/capacity dependencies
- fixed-slot replacement dependencies
- wall-decoration host/displacement dependencies
- terrain shadow/light dependencies
- ModelData normal-reconciliation dependencies
- affected static output zones
- affected region-complete animation/inspection projections where relevant
- producer region
- projection epoch
- presentation generation
- explicit completeness

A producer snapshot that cannot represent one of these effects must mark the relevant proof incomplete and force the region fallback.

## Candidate proof strategies

Two implementation families remain valid after O1/O2:

1. **Causal dependency graph**: instrument scene construction so each input Object placement records the output zones and later placement decisions causally dependent on it, including suppressed/rejected/replaced candidates.
2. **Producer output comparison**: retain enough accepted producer state/hash information to rebuild full contextual producer output and prove which 8x8 zones changed before narrowing installation.

O3 should choose the smallest architecture that can prove additions and removals without duplicating semantic authority.

Do not implement a simple `ObjectPlacement -> current output zones` table and call it complete; O1 proves that contract is insufficient for removals and placement-order effects.

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
- retained zero Object zone narrowing

Certification: `643f760a7f318d5519fc9897a588d732afe0c293`, CI #1264 / `38699017109`, SUCCESS.

### O2 - canonical Object mutation footprint

Status: COMPLETE; final documentation-bearing exact-head certification pending.

Delivered:

- full canonical `ObjectPlacement` identity retained in finalized Object footprints
- exact removed and added placement sets retained separately
- deterministic canonical ordering and exact deduplication
- patch execution preserves original mutation polarity
- semantic Undo swaps mutation polarity exactly
- semantic Redo preserves original mutation polarity
- derived `object_tiles` compatibility summary retained only for existing region routing
- O1 3x3 region fallback unchanged
- no Object zone narrowing introduced
- focused commit/replay footprint tests added

Code certification: `fed985ea59db7737067b6d19d53844d2d4e9429a`, CI #1275 / `38700886057`, SUCCESS across Tier A/B/C.

### O3 - dependency/proof contract

Status: NOT STARTED.

Planned boundary:

- choose and implement a proof representation that is sound for additions and removals
- tie proof state to producer region, projection epoch, and presentation generation
- make completeness explicit and fail-closed
- prove or reject capacity/replacement/normal/shadow dependency coverage
- no runtime narrowing until the contract can represent the required mutation effects

### O4 - accepted-build runtime integration and planner

Status: NOT STARTED.

Planned boundary:

- install Object proof state only from accepted/current worker results
- reject stale request identity, projection epoch, and presentation generation
- query every producer in the O1 conservative universe
- use exact zones only when every required producer proof is current and complete
- otherwise rebuild the O1 region ring
- preserve full contextual scene assembly

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
