# Height Zone Invalidation Plan

Status: H1-H4 complete. H5 exact-head certification and merge pending.

Branch: `refactor/height-zone-invalidation`

Base main: `5938df7d723c55b66ad4522f79c08cf89bf0acbf`

## Goal

Narrow finalized Height edit projection work from the conservative region-ring policy to 8x8 world zones only when runtime dependency metadata proves every affected projection zone.

Semantic ownership does not change. `EditorSession` remains authoritative and 8x8 zones remain disposable projection/cache granularity only.

## Non-goals

This branch does not:

- narrow Object edit invalidation
- change Height semantic planning or history authority
- change terrain Paint invalidation semantics
- narrow contextual scene assembly
- change Reference or HD visual semantics
- introduce another authoritative map model
- guess a universal model radius

## Safety invariant

For every Height semantic transaction, zone-scoped invalidation is permitted only when the selected zone set is a proven superset of every projection output that can change because of the touched Height vertices.

If that proof is absent, stale, incomplete, unsupported, or otherwise untrusted, the existing conservative region fallback is mandatory.

## Audited Height dependencies

### Terrain geometry and slope lighting

Terrain tiles consume height-grid corners. Reference slope lighting additionally reads one grid point in each cardinal direction around corner samples. These terrain dependencies are locally bounded.

### Initial loc placement

`sample_placement_height` reads four height-grid values around the midpoint of the rotated definition footprint. The dependency therefore varies with anchor, `size_x`, `size_y`, orientation, and scene-edge fallback behavior.

### Static lit-model contouring

Ground contouring uses actual transformed model geometry. Static lit models are bounded from their final model-space X/Z cylinder radius rather than guessed from definition footprint alone.

### ModelData reconciliation

`ModelData` can participate in neighbouring normal reconciliation. The branch does not claim a complete fine-grained dependency closure for this case. Its presence marks Height dependency metadata incomplete and therefore forces region fallback.

### Dynamic/posed contouring

Dynamic locs can morph, pose, then contour a private model. Because the active pose can change X/Z bounds, the branch does not claim a static fine-grained read-set for contoured animated models. Supported worker input marks this case incomplete and therefore retains region fallback.

## H1 - dependency audit and protocol

Status: COMPLETE AND CERTIFIED.

Certified content head: `73694a9d97fd2845c8ed818400430709d6c704c5`

CI: `38688385096` / #1221 - SUCCESS.

H1 established that a fixed Height radius was not proven safe and required explicit projection dependency metadata with conservative fallback.

## H2 - dependency metadata contract

Status: COMPLETE AND CERTIFIED.

Certified code/test head: `c59eea2cadffbc5861f6ecf8c138a9a0b6b11f99`

CI: `38689828881` / #1227 - SUCCESS.

Delivered:

- canonical `HeightVertexCoord` keys preserving source plane and world X/Y
- deterministic ordered source-to-zone mapping
- producer-region validation
- projection epoch + presentation generation freshness stamps
- explicit `Incomplete` / `Complete` state
- stale/incomplete/current lookup outcomes
- fail-closed merge semantics
- duplicate edge elimination
- boundary, stale-stamp, producer/stamp mismatch, known-empty, and incomplete-merge tests

## H3 - runtime integration and zone planner

Status: COMPLETE. Final subsystem certification occurs in H5.

Delivered:

- finalized `EditFootprint` Height coordinates now retain canonical `HeightVertexCoord`, including `SourcePlane`
- semantic Undo/Redo reconstruct the same canonical Height coordinate identity rather than flattening to plane-less map tiles
- the world worker produces a region-complete `HeightDependencySnapshot` from the full contextual `WorldScene` before static geometry output is zone-filtered
- terrain corner and slope-light dependencies are collected per producer output zone
- static loc placement midpoint samples are collected from the rotated definition footprint
- static lit-model contour dependencies use actual model X/Z radius sampling
- `ModelData` normal-reconciliation and posed animated contour cases mark the snapshot incomplete rather than guessing a bound
- `RegionGeometry` carries Height dependency metadata alongside geometry, animation, source, and inspection projections
- dependency snapshots are installed only after the existing `BuildTracker` accepts the worker result as current
- stale request identity, projection epoch, or presentation generation prevents metadata installation
- accepted metadata is stamped with the current region projection epoch and presentation generation
- source replacement, full presentation rebuild, and region unload clear/remove trusted metadata
- Height invalidation resolves through every resident non-empty producer snapshot
- exact zones are used only when all required producer snapshots are present, current, and complete
- missing, stale, incomplete, unsupported, or unproven state falls back to the previous region policy
- mixed terrain + Height footprints union certified terrain zones with proven Height zones
- Object participation still forces conservative region invalidation
- semantic authority remains in `EditorSession`
- full contextual scene assembly remains unchanged

## H4 - adversarial boundary and replay coverage

Status: COMPLETE. Final subsystem certification occurs in H5.

Coverage now includes:

- Height dependency proofs spanning an 8x8 output-zone edge
- Height dependency proofs spanning a 64x64 region edge
- Euclidean zone ownership for negative coordinates
- rotated/multi-tile placement midpoint sampling helper behavior and scene-edge fallback
- contour sample expansion, bilinear +1 behavior, and scene clamping
- deterministic dependency deduplication
- missing metadata retaining the conservative region-ring fallback
- stale projection epoch and stale presentation generation rejecting metadata
- incomplete metadata rejecting zone narrowing
- semantic Undo/Redo replay using canonical Height footprints
- stale in-flight builds rejected by request identity / projection epoch / presentation generation
- cancelled old build results unable to consume newer requests

Dynamic/morphed contoured models remain deliberately fail-closed. When their active posed contour dependency cannot be proven, the worker marks the snapshot incomplete and the editor rebuilds conservatively by region.

## Context versus output

This subsystem narrows dirty output scope only.

The world worker continues to construct the full contextual scene required for terrain lighting, normal reconciliation, contouring, object relationships, Reference extraction, and 117HD procedural generation. Only final static projection emission/installation is zone-addressable.

## H5 - exact-head certification and merge

Status: IN PROGRESS.

Required before merge:

- identify the final branch head after this plan and `DEV_HANDOFF.md` are current
- architecture boundaries pass
- architecture guard passes without new debt exceptions
- reference regeneration safety passes
- historical deob golden index passes
- rustfmt passes
- full workspace compile passes
- strict Clippy passes
- WebAssembly portability passes
- full workspace tests pass
- M3-M9 semantic/reference parity passes
- focused Height invalidation tests pass as part of the workspace suite
- branch remains Height-only plus handoff/plan documentation

After exact branch-head success:

1. open one PR from `refactor/height-zone-invalidation` to `main`
2. merge it without opening another development branch
3. validate the exact integrated `main`
4. record the merge and post-merge validation in `DEV_HANDOFF.md`
5. certify the final handoff-only `main` head if the validation record changes `main`
6. ensure temporary/merged refs contain no unmerged work
7. stop and report before any new branch is created

## Branch discipline

A new development branch must not be created while this subsystem is unmerged.

The next subsystem may begin only after:

- the Height branch is merged into `main`
- exact integrated `main` is green
- the handoff records that integrated state
- the user explicitly directs continuation
