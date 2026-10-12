# RustOSRS Map Editor Architecture Handoff

Last updated: 2026-10-12

This is the living development handoff for the current map-editor architecture project. Update it at every meaningful checkpoint and before each branch is merged. It supersedes any older general-purpose handoff notes for this effort.

## Project goal

Refactor and harden the RustOSRS map editor so editing semantics are authoritative, testable, deterministic, and independent from UI/rendering/runtime state. The immediate goal is to build the architecture needed for reliable terrain, height, object, undo/redo, save/export, and future procedural tools.

The existing spline/path generator is **not** being redesigned during this phase. It will be replaced later after the edit-domain, transaction, history, and view-synchronization foundations are in place.

## Working rules

1. Work on one branch at a time.
2. Break work into small checkpoints and stop after each checkpoint for review.
3. Do not start the next checkpoint until directed to continue.
4. When the current branch is complete and fully validated, merge it before starting the next branch.
5. Preserve existing OSRS semantic/parity tests and current editor behavior unless a checkpoint explicitly changes behavior.
6. Keep the 117HD/rendering work isolated from editor-domain refactors unless an explicit adapter change is required.
7. Update this file as decisions, checkpoints, branches, and validation state change.

## Target architecture

```text
osrs-core
  Canonical semantic vocabulary: coordinates, IDs, planes, shared OSRS value types.

osrs-edit
  Authoritative, headless editable-map domain.
  Owns edit semantics, validation, document state, patches, inverse patches,
  change impact, history/query contracts, and later editor-facing transactions.

osrs-cache
  Cache codecs and cache-format infrastructure.

osrs-scene
  OSRS scene semantics.

osrs-world
  Cache-to-scene orchestration only. It should continue to own no new semantics.

osrs-editor
  Product/application/UI layer: tool sessions, input, previews, orchestration,
  adapters, and presentation.

renderer / 117HD
  Disposable reactive projection of authoritative semantic state.
```

The dependency direction is intentional. `osrs-edit` may currently depend only on `osrs-core`; this boundary is enforced by the repository architecture guard.

## Core architecture decisions

### Authoritative editor state

ECS, renderer state, meshes, GPU resources, and scene-worker results must never become the source of truth for editable map state. They are projections/caches that can be regenerated from authoritative semantic state.

`MapDocument` is now the first authoritative, headless editable-map state container. It owns semantic terrain, explicit height vertices, and exact object placements without depending on renderer/UI/cache/runtime layers.

### Coordinates and planes

Reuse the semantic coordinate and plane types already in `osrs-core`. Do not collapse world/source coordinates, storage planes, render levels, collision planes, or renderer-local positions into raw tuples.

`MapPosition` represents a source-map tile plus `SourcePlane`.

Terrain heights use a separate `HeightVertexCoord`. Tile attributes and terrain corners are deliberately distinct because an 8x8 tile area uses a 9x9 corner lattice.

### Terrain values

`TerrainTile` owns editor-facing tile channels such as underlay, overlay, and flags. Height is intentionally not stored on `TerrainTile`.

Overlay shape is validated as `0..=11` and overlay rotation as `0..=3` at construction instead of silently normalizing invalid values later.

`MapDocument` stores terrain sparsely. A missing terrain entry is semantically `TerrainTile::default()`. Applying a patch that returns a tile to its semantic default removes the sparse entry.

### Height values

Height vertices are explicit authoritative values. A missing height vertex is not treated as height zero. `SetHeight` therefore requires the addressed height vertex to exist and its `before` value to match current state.

This distinction prevents an absent source height from being silently conflated with a legitimate numeric height.

### Object placements

Semantic object placements use validated object kind/orientation values and `ObjectId` plus `MapPosition`. Raw `(u8, i32, i32)` style infrastructure DTOs may remain at adapter boundaries but should not become the authoritative editor model.

`MapDocument` stores exact object placements. `AddObject` requires the placement to be absent; `RemoveObject` requires it to be present.

### Patch and transaction model

The edit domain uses explicit reversible patch primitives rather than command objects that mutate state while discovering undo information.

```rust
MapPatch
  private Vec<MapOp>
  private derived ChangeImpact

MapOp
  SetTerrain { coord, before, after }
  SetHeight { coord, before, after }
  AddObject { placement }
  RemoveObject { placement }
```

`MapPatch::new` derives the union of semantic change impact from its operations so callers cannot construct a patch with stale or missing invalidation metadata. `MapPatch::inverse` reverses operation order and inverts each operation so dependent multi-operation transactions can be undone exactly.

`MapDocument::validate_patch` simulates only entries touched by the patch. Validation sees the effects of earlier operations in the same patch, but does not mutate authoritative state.

`MapDocument::apply_patch` validates the complete patch first. Only after every operation succeeds does it mutate authoritative state. A rejected operation at any index leaves the real document unchanged.

This preserves the intended rule that one user gesture can become one atomic logical history operation.

### Patch validation errors

`PatchApplyError` records the failing operation index and semantic reason. Current failure modes cover:

- terrain `before` mismatch
- missing height vertex
- height `before` mismatch
- duplicate object add
- missing object remove

This keeps validation failures deterministic and testable without leaking renderer/UI concerns into the domain.

### Change impact

`ChangeImpact` is semantic, not renderer-specific. Current flags cover terrain geometry, terrain material, normals, lighting, collision, locations, and bounds.

The initial impact mapping is intentionally conservative. Later view-synchronization work may translate these semantic effects into tighter chunk/region rebuild radii without changing patch data.

No-op terrain/height value changes produce no impact. Terrain flag changes include collision/bounds impact. Object add/remove operations include location/collision/bounds impact.

### Future view synchronization

Async scene/mesh rebuilds should eventually carry document/chunk revisions. Results built from stale revisions must be discarded so old background work cannot replace newer editor state.

## Current integration state

The edit-domain foundation is merged to `main`.

- Foundation branch: `refactor/editor-edit-domain-foundation`
- Pull request: `#33` (`refactor(editor): establish authoritative edit-domain foundation`)
- Final pre-merge branch head: `ded9c670dd2c3658785ea462cded34dcd010bd7c`
- Squash merge commit on `main`: `5c5eef3bde696b20ae0319867e6e84567701ebf6`

No follow-on development branch has been created yet. The next branch must start from current `main` only after explicit direction to continue.

### Foundation delivered

- Added `crates/osrs-edit` to the Cargo workspace.
- `osrs-edit` depends only on `osrs-core`.
- Added architecture-guard coverage for the new dependency boundary.
- Added `MapPosition`.
- Added `HeightVertexCoord`.
- Added validated `OverlayShape`.
- Added validated `OverlayRotation`.
- Added `TerrainOverlay` and `TerrainTile`.
- Added validated `LocKind`.
- Added validated `ObjectOrientation`.
- Added semantic `ObjectPlacement`.
- Added `ChangeImpact` semantic invalidation flags.
- Added reversible `MapOp` variants for terrain, height, and object edits.
- Added `MapPatch` with private derived impact metadata.
- Added exact patch inversion with reverse operation ordering.
- Added authoritative `MapDocument` terrain/height/object storage and query methods.
- Added `PatchApplyError` with operation-indexed validation failures.
- Added whole-patch validation that simulates prior operations without mutating authoritative state.
- Added atomic patch application after complete validation.
- Added tests proving rejected late operations leave `MapDocument` unchanged.
- Added tests proving dependent operations in one patch validate sequentially.
- Added tests proving patch + inverse restores exact document state.
- Added tests for missing heights and duplicate object adds.
- Added focused unit tests for impact union, no-op impact, flag collision impact, and exact inverse round trips.
- Added focused unit tests around semantic validation and tile-vs-height-vertex separation.
- Added and maintained this living `DEV_HANDOFF.md`.

### Current files in `osrs-edit`

```text
crates/osrs-edit/
  Cargo.toml
  src/
    document.rs
    lib.rs
    object.rs
    patch.rs
    position.rs
    terrain.rs
```

No renderer, egui, cache-codec, scene-worker, or runtime dependencies belong in this crate.

## Validation record

Checkpoint 4 patch-primitives final head:

- Commit `4394278f69c3523b7115d76ac0b4afaeace06fb8`
- CI run `38231806257`: PASS
- Tier A static quality: PASS
- Tier B workspace tests: PASS
- Tier C M3-M9 semantic/reference suites: PASS

Checkpoint 5 `MapDocument` implementation head:

- Commit `c94e9a71c02b62b84a74651827b3f02d10ddf96c`
- CI run `38233801420`: PASS
- Tier A architecture/format/workspace/strict Clippy: PASS
- Tier B workspace tests: PASS
- Tier C M3-M9 semantic/reference suites: PASS

Final foundation branch before merge:

- Head `ded9c670dd2c3658785ea462cded34dcd010bd7c`
- CI run `38238688037`: PASS
- Tier A architecture/format/workspace/strict Clippy: PASS
- Tier B workspace tests: PASS
- Tier C M3-M9 semantic/reference suites: PASS
- Compared against `main`: 0 commits behind.
- Final diff limited to workspace registration, the new `osrs-edit` crate, architecture guard/tests, `DEV_HANDOFF.md`, and the required local workspace package entry in `Cargo.lock`.
- During merge-readiness review, unrelated dependency-version churn from the earlier full lockfile regeneration was identified and removed. Final `Cargo.lock` diff is only the 7-line `osrs-edit -> osrs-core` package entry.

Merge:

- PR `#33`: MERGED
- Method: squash
- Main commit: `5c5eef3bde696b20ae0319867e6e84567701ebf6`

## Checkpoint history

### Checkpoint 1: semantic edit-domain foundation

Created the additive `osrs-edit` crate and established editor-facing semantic value types without changing runtime/editor behavior.

### Checkpoint 2: architecture/workspace integration

Added architecture boundary enforcement, corrected formatting, regenerated `Cargo.lock`, removed temporary CI scaffolding, and reached a clean Tier A result.

### Checkpoint 3: living handoff

Added this `DEV_HANDOFF.md` as the single project handoff and continuity document. Full Tier A/B/C CI subsequently passed on that checkpoint.

### Checkpoint 4: patch primitives

Added semantic `ChangeImpact`, reversible `MapOp`, and invariant-preserving `MapPatch` primitives with focused tests. No editor tools, renderer state, existing undo/redo, spline planner, or commit path was wired to them.

### Checkpoint 5: authoritative MapDocument

Added sparse semantic terrain storage, explicit height storage, exact object-placement storage, read queries, deterministic patch-state validation, operation-indexed validation errors, and atomic whole-patch application. Added regression tests proving failed patches cannot partially mutate authoritative state and that patch inversion round-trips the document exactly.

No editor tools, renderer state, current undo/redo stack, spline planner, or existing edit commit path has been wired to `MapDocument` yet.

### Checkpoint 6: branch readiness and merge

Audited the complete branch against `main`, confirmed it was 0 commits behind, and found one scope-cleanliness issue in `Cargo.lock`: the earlier full regeneration had changed unrelated dependency versions. Restored the `main` lockfile baseline, let Cargo minimally add only the new local `osrs-edit` package, removed the temporary cleanup workflow, and re-ran the complete Tier A/B/C pipeline successfully.

Opened PR `#33`, verified it was mergeable at exact head `ded9c670dd2c3658785ea462cded34dcd010bd7c`, and squash-merged it to `main` as `5c5eef3bde696b20ae0319867e6e84567701ebf6`.

## Next checkpoint

**Do not redesign the path generator yet.**

When explicitly directed to continue:

1. Start a new branch from the then-current `main`.
2. Keep the branch focused on basic edit-history/query contracts around `MapDocument` and `MapPatch`.
3. Preserve one user gesture = one logical patch/history entry.
4. Add undo/redo history semantics and tests at the headless domain level before any existing editor UI/tool migration.
5. Do not migrate spline planning/preview, brush tools, object tools, rendering, or 117HD in that first history checkpoint.
6. Stop and report after the first small history checkpoint.

## Later branches, tentative order

These are directional, not permission to start them automatically.

1. Basic history/query contracts around `MapDocument` and `MapPatch`.
2. Migrate one existing edit path through the domain boundary, likely spline/path **commit only**, while leaving its planner/preview algorithm untouched.
3. Migrate brush/terrain/height editing.
4. Migrate object editing.
5. Revision-based async view synchronization and stale-result rejection.
6. Semantic change-impact/invalidation cleanup.
7. Persistence/export hardening and round-trip fixtures.
8. Replace/redesign the spline/path/auto-tile generator on top of the completed edit architecture.

## Important deferred work

The current spline/path generator has grown too large and needs a better design, but intentionally defer that rewrite. When its replacement begins, treat it as a new project/branch sequence and build it against the stable semantic query/patch API rather than letting it mutate `EditorApp` or renderer state directly.

Do not delete existing spline/path golden, topology, geometry, seam, network, routing, or determinism coverage merely because the implementation is later replaced. Preserve useful behavioral fixtures as acceptance tests for the successor where they describe desired behavior.
