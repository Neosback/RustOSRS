# RustOSRS Development Handoff

Last updated: 2026-10-15

`ARCHITECTURE.md` is the authoritative architecture contract. This file is the current operational handoff: exact integrated baseline, active branch, checkpoint state, required working protocol, known hazards, and next allowed boundary.

## Project invariant

`MapDocument` / `EditorSession` is authoritative edited map state. World-worker edits, rendered geometry, GPU resources, picker data, previews, and UI state are projections only.

If every projection vanished, the complete edited map must still exist correctly in the semantic session.

## Mandatory development protocol

These rules apply to all continuing development unless the user explicitly changes them.

1. Keep exactly one active development branch/system at a time.
2. Do not start a second feature/refactor branch while the current branch is active.
3. Break each subsystem into small, reviewable checkpoints.
4. Stop after every checkpoint and report the exact progress to the user before continuing.
5. Do not begin the next checkpoint without explicit user direction.
6. Update `DEV_HANDOFF.md` at every completed checkpoint so repository state, exact SHAs, validation, hazards, and the next boundary stay current.
7. Keep a subsystem on the same branch across its checkpoints unless there is a concrete reason to close it and start a separate subsystem.
8. Do not mix unrelated architecture, renderer/117HD, cache, or UI work into the active subsystem branch.
9. Before merging a completed subsystem, validate the exact branch head with the required Tier A/B/C CI gates.
10. Merge only after the subsystem is complete and certified.
11. After merge, validate the exact integrated `main` commit, update this handoff, stop, and report progress before starting another branch.
12. One logical user gesture produces at most one logical semantic transaction/history entry.
13. Failed projection preparation must not leave semantic authority and visible projection knowingly diverged.
14. Exact SHA matters. A green run certifies only the SHA it tested.
15. New or fully refactored production Rust files remain under the architecture guard limits. Do not introduce new debt exceptions for convenience.

## Current integrated baseline

`main`:
`5938df7d723c55b66ad4522f79c08cf89bf0acbf`

Latest integrated subsystem:
PR #50 - `fix(editor): reject stale region build results`

Current repository state at this handoff refresh:

- no open PRs
- semantic editor authority is established
- central `EditRuntime` execute/replay gateway is established
- broad `Deref` / `DerefMut` compatibility escape is removed
- Paint, Height, Paste, Path, Objects, semantic Undo, and semantic Redo route through the gateway
- receipt-driven projection invalidation is established
- pure terrain edits use 8x8 zone invalidation
- world worker geometry extraction can be scoped to dirty zones while retaining full contextual scene assembly
- stale asynchronous region build results are rejected by request identity, projection epoch, and presentation generation
- loadable map-cache-copy export exists for existing map groups
- 117HD materials, environments, directional shadows, water, animated-loc HD material routing, flow maps, and parallax/displacement have substantial implementations

## Recent merged projection/edit architecture

### PR #45 - central semantic edit gateway

Integrated the gateway through Paint, Height, Paste, and Path.

### PR #46 - complete semantic edit gateway

Integrated:

- Objects through `EditRuntime::execute`
- semantic Undo/Redo through `EditRuntime::replay`
- authoritative post-transition projection preparation
- exact restore on projection failure
- removal of broad `Deref` / `DerefMut`
- closure of production commit/replay bypasses

### PR #47 - receipt-driven projection invalidation

Finalized semantic routes now derive downstream invalidation from `EditReceipt` / semantic footprint instead of transitional worker-edit adapters.

### PR #48 - terrain zone invalidation

Pure terrain edits now map their certified radius-5 terrain dependency to 8x8 world/render zones. Height and Object edits intentionally retain conservative region policies until their dependency footprints are explicit.

### PR #49 - zone-scoped worker geometry

The world worker retains full contextual scene assembly, but final Reference/HD static geometry emission can be filtered to requested world zones. Render residency is independently addressable per zone.

### PR #50 - stale build rejection

Asynchronous region build results are installed only when the corresponding request identity, projection epoch, and presentation generation are still current.

## Current active subsystem

Branch:
`refactor/height-zone-invalidation`

Base `main`:
`5938df7d723c55b66ad4522f79c08cf89bf0acbf`

Goal:
Replace Height's conservative region-ring projection invalidation with exact or conservatively bounded zone invalidation without weakening scene correctness.

This branch is Height-only. Object fine-grained invalidation is a later separate branch.

## Height zone invalidation - checkpoint plan

### H1 - dependency audit and operating protocol

Status: COMPLETE AND CERTIFIED.

Certified H1 content head before its validation-record-only handoff refresh:
`73694a9d97fd2845c8ed818400430709d6c704c5`

CI:
Run `38688385096` / #1221 - SUCCESS.

Passed on the certified H1 head:

- Tier A architecture boundaries and architecture guard
- reference regeneration safety
- historical deob golden index
- rustfmt
- workspace check
- strict Clippy
- WebAssembly portability
- Tier B full workspace tests
- Tier C M3 decode/fuzz
- M4 fixtures/model/fuzz
- M5 normalized fixtures
- M6 semantic scene
- M7 normals/lighting/finalization
- M8 morph/contour/animation/replacement
- M9 semantic closure/golden hashes

H1 scope:

- refreshed this handoff with the mandatory one-branch/checkpoint/stop protocol
- audited current Height invalidation and the scene/model height dependencies
- added `docs/plans/HEIGHT-ZONE-INVALIDATION.md`
- defined the safety contract before changing runtime invalidation behavior
- made no production projection behavior change

Audit result:

1. A height vertex directly participates in the terrain surfaces of the adjacent tiles that consume that corner.
2. Reference slope lighting reads height samples one grid point away, and tile corner color consumes those light values. The terrain-only geometric/lighting dependency is locally bounded.
3. Initial loc placement height does not depend only on the loc anchor. `sample_placement_height` samples four height-grid points around the midpoint of the rotated definition footprint, so the anchor-to-height dependency varies with `size_x`, `size_y`, and orientation.
4. Ground-contoured lit models use their actual model-space cylinder X/Z radius when deciding/sampling the height field.
5. Ground-contoured `ModelData` uses actual vertex min/max X/Z bounds rather than only the object definition footprint.
6. Dynamic locs recompute active morph/placement state, then pose and contour a private model. A safe invalidation planner therefore cannot assume only static definition dimensions.
7. A single fixed Height radius derived only from the edited vertex is not currently proven safe for all placed/contoured models.
8. The existing region-ring fallback remains the certified behavior until exact dependency metadata is available.

Safety decision:

Do not replace Height's region fallback with a guessed constant radius.

The Height zone planner must be driven by explicit scene/model dependency metadata. If metadata is missing, stale, unsupported, or cannot prove a bounded footprint, it must fall back to the existing conservative region policy.

### H2 - explicit height dependency metadata

Status: COMPLETE AND CERTIFIED.

Certified H2 code/test head before this handoff refresh:
`c59eea2cadffbc5861f6ecf8c138a9a0b6b11f99`

CI:
Run `38689828881` / #1227 - SUCCESS.

Passed on the certified H2 head:

- Tier A architecture boundaries and architecture guard
- reference regeneration safety
- historical deob golden index
- rustfmt
- workspace check
- strict Clippy
- WebAssembly portability
- Tier B full workspace tests
- Tier C M3 decode/fuzz
- M4 fixtures/model/fuzz
- M5 normalized fixtures
- M6 semantic scene
- M7 normals/lighting/finalization
- M8 morph/contour/animation/replacement
- M9 semantic closure/golden hashes

H2 scope:

- added `crates/osrs-editor/src/app/world_runtime/height_dependencies.rs`
- compiled the dependency contract into the editor without changing current Height invalidation behavior
- represented source dependencies with canonical `HeightVertexCoord`, preserving source plane plus world X/Y identity
- mapped each source vertex to a deterministic ordered set of output 8x8 world zones
- added per-producer-region validation so a dependency collection cannot silently claim an output zone owned by a different producer region
- added `HeightDependencyStamp` keyed by projection epoch and presentation generation
- added explicit `Incomplete` / `Complete` state
- made lookup return `Stale`, `Incomplete`, or current dependency data
- made merge reject producer/stamp mismatches, deduplicate deterministically, and remain fail-closed if either side is incomplete
- covered negative coordinates, zone/region boundaries, multi-zone dependencies, stale epoch/presentation, known-empty complete results, producer/stamp mismatches, and incomplete merge behavior
- current Height invalidation still uses the conservative region fallback

Important H2 boundary:

The dependency types exist, but no accepted world-worker build installs current dependency metadata yet. `EditFootprint` also still exposes the pre-H2 Height footprint shape used by existing invalidation. H3 must wire canonical Height coordinate identity and accepted-build metadata into the planner before any Height edit is permitted to narrow to zones.

### H3 - Height receipt to zone invalidation

Status: NOT STARTED.

Planned boundary:

- preserve enough canonical Height coordinate identity in finalized edit footprints to query H2 metadata correctly, including source plane
- collect and install Height dependency metadata only from accepted/current region build results
- tie trusted metadata to current projection epoch and presentation generation
- map finalized Height semantic footprints to exact affected zones only when every relevant producer dependency set is complete/current
- keep conservative region fallback for absent, incomplete, stale, unsupported, or cross-region-unproven dependency state
- preserve full contextual scene assembly
- change only final dirty projection scope
- do not move semantic authority into `WorldRuntime`

Do not start H3 without explicit user direction after this checkpoint report.

### H4 - adversarial boundary and replay coverage

Status: NOT STARTED.

Planned coverage includes:

- height vertex on an 8x8 zone edge
- height vertex on a 64x64 region edge
- multi-tile rotated loc midpoint sampling
- contoured models whose geometric bounds cross zone boundaries
- dynamic/morphed contoured objects where supported by current runtime inputs
- semantic Undo/Redo using the same finalized Height footprint
- invalidation arriving while an older scoped build is in flight
- missing dependency metadata falling back to the existing region policy

### H5 - subsystem certification and merge

Status: NOT STARTED.

Required before merge:

- exact branch head identified
- architecture guard passes
- rustfmt passes
- workspace compile passes
- strict Clippy passes
- WebAssembly portability gate passes
- full workspace tests pass
- M3-M9 semantic/reference parity passes
- focused Height invalidation tests pass
- branch contains only Height invalidation/handoff work

After successful merge:

1. validate exact integrated `main`
2. update this handoff with merge SHA and post-merge CI
3. stop and report progress
4. do not create the Object invalidation branch until explicit user direction

## Current Height invalidation behavior

`crates/osrs-editor/src/app/world_runtime/invalidation.rs` intentionally routes any footprint containing `height_vertices` to the conservative region policy.

The current fallback expands one region ring for impacts that can cross region boundaries. This is safe and remains the runtime fallback after H2.

Pure terrain-only edits remain zone-scoped using the certified radius-5 underlay blend boundary.

## Why Height is not equivalent to terrain paint

Height changes can affect more than the terrain tile containing the edited vertex:

- terrain vertex positions
- slope-derived terrain lighting
- placement height of locs whose rotated footprint midpoint samples the changed grid point
- ground-contoured static models
- ground-contoured dynamic/posed models
- final bounds/projection of affected models
- downstream normal/lighting relationships for affected scene geometry

Therefore a terrain-style fixed-radius rule is insufficient unless every additional dependency is separately bounded and represented.

## Remaining major roadmap after Height

Each item below is a separate future branch/system. Do not combine them with the active Height branch.

1. Object dependency footprint and zone invalidation.
2. `InteractionRuntime` behavioral encapsulation.
3. `ProjectRuntime` ownership extraction.
4. `UiRuntime` ownership extraction.
5. `DiagnosticsRuntime` ownership extraction.
6. Reference renderer parity gates, including remaining exact raster/material/plane visibility work and external visual oracle work.
7. 117HD dynamic/placed lights.
8. 117HD sky/day-night/reflection systems and remaining post/effect edge cases.
9. Edited-cache fingerprint/profile strategy, new-group support, and broader index repack only when product requirements justify it.
10. Project/browser lifecycle polish and any future import/merge model as separate product work.

## Cache/export current state

Map stream encoding and existing map-group cache-copy repacking are implemented. The current known limitations are:

- only existing map-square groups are repacked
- new groups absent from the reference table are not created
- other indices are not generally rewritten by the editor export path
- an edited cache copy intentionally no longer matches the bundled pristine-cache fingerprint/profile

## Runtime ownership current state

Completed ownership boundaries:

- `EditRuntime`
- `RenderRuntime`
- `WorldRuntime`
- `InteractionRuntime` field ownership

Still pending on future branches:

- `InteractionRuntime` behavior encapsulation
- `ProjectRuntime`
- `UiRuntime`
- `DiagnosticsRuntime`

## 117HD current high-level state

Already substantially implemented:

- render-profile/HDR foundation
- HD data layer/material tables
- HD terrain/model extraction
- material replacement
- ground blending and normals
- environments/fog/directional lighting
- directional shadows and filtering
- water surfaces, flow/normal maps, foam, Fresnel, underwater tint/caustics
- animated-loc HD material routing
- material flow maps
- parallax/displacement and displaced shadow position

Large remaining systems include:

- placed/dynamic point lights
- full sky/star/moon/aurora/day-night systems
- sky reflection integration with water
- remaining post-processing/settings parity
- wind/character displacement and remaining model/material edge cases

## Next allowed boundary

STOPPED AFTER HEIGHT CHECKPOINT H2.

Next checkpoint is H3: runtime integration and receipt-to-zone planning using only complete/current H2 dependency metadata, with the existing region policy as mandatory fallback.

Do not start H3 until the user explicitly instructs continuation after this checkpoint report.
