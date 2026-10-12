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

### Coordinates and planes

Reuse the semantic coordinate and plane types already in `osrs-core`. Do not collapse world/source coordinates, storage planes, render levels, collision planes, or renderer-local positions into raw tuples.

`MapPosition` represents a source-map tile plus `SourcePlane`.

Terrain heights use a separate `HeightVertexCoord`. Tile attributes and terrain corners are deliberately distinct because an 8x8 tile area uses a 9x9 corner lattice.

### Terrain values

`TerrainTile` owns editor-facing tile channels such as underlay, overlay, and flags. Height is intentionally not stored on `TerrainTile`.

Overlay shape is validated as `0..=11` and overlay rotation as `0..=3` at construction instead of silently normalizing invalid values later.

### Object placements

Semantic object placements use validated object kind/orientation values and `ObjectId` plus `MapPosition`. Raw `(u8, i32, i32)` style infrastructure DTOs may remain at adapter boundaries but should not become the authoritative editor model.

### Future transaction model

Prefer an explicit patch model rather than command objects that mutate state while discovering undo information.

Planned direction:

```rust
MapPatch
  Vec<MapOp>
  ChangeImpact

MapOp
  SetTerrain { coord, before, after }
  SetHeight { coord, before, after }
  AddObject { placement }
  RemoveObject { placement }
```

A tool should calculate and validate a complete patch before authoritative state changes. Applying a patch should be atomic. Undo should apply its inverse. One user gesture should remain one logical history operation.

### Future view synchronization

Async scene/mesh rebuilds should eventually carry document/chunk revisions. Results built from stale revisions must be discarded so old background work cannot replace newer editor state.

### Future change impact

Dirty-region behavior should evolve from ad hoc invalidation into semantic `ChangeImpact` data describing affected geometry, material, normals, lighting, collision, locations, and bounds. Existing conservative rebuild behavior should be preserved until the replacement policy is proven.

## Current branch

`refactor/editor-edit-domain-foundation`

Purpose: establish the first additive, headless edit-domain boundary without changing editor behavior.

### Implemented on this branch

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
- Added focused unit tests around semantic validation and tile-vs-height-vertex separation.
- Regenerated the workspace lockfile after introducing `osrs-edit`.
- Removed the temporary lockfile-regeneration workflow after use.

### Current files in `osrs-edit`

```text
crates/osrs-edit/
  Cargo.toml
  src/
    lib.rs
    object.rs
    position.rs
    terrain.rs
```

No renderer, egui, cache-codec, scene-worker, or runtime dependencies belong in this crate.

## Validation record

Latest fully observed validation before this handoff update:

- Architecture boundary check: PASS
- Architecture guard tests: PASS
- Reference-regeneration safety: PASS
- Historical deob golden-index test: PASS
- `cargo fmt --all --check`: PASS
- `cargo check --locked --workspace --all-targets --all-features`: PASS
- strict workspace Clippy: PASS

That corresponds to CI Tier A on parent head `69346d01ed30c0b6bddad4e3763c064a2f96d300`.

Tier B workspace tests were still running when this handoff checkpoint began. Re-check CI on the new documentation head before advancing implementation work.

## Checkpoint history

### Checkpoint 1: semantic edit-domain foundation

Created the additive `osrs-edit` crate and established editor-facing semantic value types without changing runtime/editor behavior.

### Checkpoint 2: architecture/workspace integration

Added architecture boundary enforcement, corrected formatting, regenerated `Cargo.lock`, removed temporary CI scaffolding, and reached a clean Tier A result.

### Checkpoint 3: living handoff

Added this `DEV_HANDOFF.md` as the single project handoff and continuity document.

## Next checkpoint

**Do not redesign the path generator yet.**

Next implementation checkpoint on the current branch should be small:

1. Re-check the latest full CI state after this documentation commit.
2. If CI is green, add only the minimal `MapPatch` / `MapOp` / `ChangeImpact` type skeleton and focused inverse/impact unit tests.
3. Do not wire editor tools, spline commits, rendering, or existing undo/redo into the new patch model yet.
4. Stop and report before continuing.

After the branch eventually contains the complete foundation planned for it and full Tier A/B/C CI is green, open/update the PR as needed and merge it before starting the next branch.

## Later branches, tentative order

These are directional, not permission to start them automatically.

1. Edit-domain foundation and patch primitives.
2. Authoritative `MapDocument` plus atomic patch application/inversion.
3. Migrate one existing edit path through the domain boundary, likely spline/path **commit only**, while leaving its planner/preview algorithm untouched.
4. Migrate brush/terrain/height editing.
5. Migrate object editing.
6. Revision-based async view synchronization and stale-result rejection.
7. Semantic change-impact/invalidation cleanup.
8. Persistence/export hardening and round-trip fixtures.
9. Replace/redesign the spline/path/auto-tile generator on top of the completed edit architecture.

## Important deferred work

The current spline/path generator has grown too large and needs a better design, but intentionally defer that rewrite. When its replacement begins, treat it as a new project/branch sequence and build it against the stable semantic query/patch API rather than letting it mutate `EditorApp` or renderer state directly.

Do not delete existing spline/path golden, topology, geometry, seam, network, routing, or determinism coverage merely because the implementation is later replaced. Preserve useful behavioral fixtures as acceptance tests for the successor where they describe desired behavior.