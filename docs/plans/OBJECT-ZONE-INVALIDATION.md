# Object Zone Invalidation Plan

Status: O1-O5 complete and certified. O6 final branch certification, PR/merge, and integrated-main certification is next.

Branch: `refactor/object-zone-invalidation`

Base `main`: `a7b9fb47835ab00b25db7c02e1aa3ca3157a054b`

## Goal

Narrow finalized Object edit static projection work from conservative region rebuilds to exact 8x8 output zones only when every affected producer can be proven from accepted output.

`MapDocument` / `EditorSession` remains authoritative semantic state. World scenes, location replay, geometry, picker state, animations, GPU resources, and Object proof snapshots remain disposable projections.

## Safety invariant

For every finalized Object transaction, exact-zone static installation is permitted only after a complete, current pre-edit producer output is compared with a complete, accepted post-edit producer output for every required resident producer.

Missing, stale, incomplete, mismatched, unsupported, or invalid proof state must fail closed to the O1 conservative producer-region policy.

Full contextual scene assembly is never narrowed by this subsystem.

## O1 - dependency audit and conservative fallback

Status: COMPLETE and CERTIFIED.

Certified head: `643f760a7f318d5519fc9897a588d732afe0c293`
CI: #1264 / `38699017109` - SUCCESS across Tier A/B/C.

Key findings:

- Object placement identity alone does not bound all downstream output.
- Game-object five-per-tile capacity is order dependent and removal can reveal later placements.
- Boundary, wall-decoration, and floor-decoration slots have replacement/reveal behavior.
- Wall-decoration displacement can depend on same-tile hosts.
- Clipped locs write terrain shadow state that affects slope lighting.
- ModelData normal reconciliation can change neighboring models.
- Dynamic, morphed, omitted, rejected, or overwritten placements cannot be assumed dependency-free.
- Each producer builds a 104x104 contextual scene around owned 64x64 output, so an Object edit can affect neighboring producers.

O1 corrected the old Object-only under-invalidation by making any unproven Object footprint rebuild the complete 3x3 producer-region ring around touched Object regions.

## O2 - canonical Object mutation footprint

Status: COMPLETE and CERTIFIED.

Code head: `fed985ea59db7737067b6d19d53844d2d4e9429a`
Code CI: #1275 / `38700886057` - SUCCESS.

Final O2 documentation-bearing head: `d61d9169d271c1dde9009707ff59d73cb8001716`
Final O2 CI: #1277 / `38701473420` - SUCCESS across Tier A/B/C.

Delivered:

- exact `ObjectPlacement` identity retained for removed and added placements
- source plane, object id, anchor tile, loc kind, and orientation preserved
- deterministic canonical ordering and exact deduplication
- Undo swaps removed/added polarity exactly
- Redo preserves original polarity
- derived anchor-tile summary retained only for compatibility/routing
- O1 fallback unchanged

## O3 - producer-output proof contract

Status: COMPLETE and CERTIFIED.

Certified head: `a555e07796a938676ddb3c4f7d0320dd480041aa`
CI: #1285 / `38703366564` - SUCCESS across Tier A/B/C.

Chosen strategy: **producer output comparison**.

`ObjectOutputSnapshot` records:

- producer region
- explicit 8x8 zone coverage
- exact per-zone `SceneGeometry`
- explicit complete/incomplete state

A full producer snapshot explicitly covers all 64 owned zones, including empty zones. Scoped state never treats uncovered zones as unchanged.

Exact comparison detects added, removed, or structurally changed output zones. Editor-side proof state is stamped with producer region, projection epoch, and presentation generation.

O3 did not enable runtime narrowing by itself.

## O4 - accepted-build runtime integration and planner

Status: COMPLETE and CERTIFIED.

Code head: `52eadc6284e71d9bc00de878d89cc67a0f0c7628`
Code CI: #1294 / `38705182639` - SUCCESS across Tier A/B/C.

Documentation-bearing O4 head: `55f42228003770c3adc5ef5500a32052ebf1ff2a`
Documentation-bearing CI: #1296 / `38705932458` - SUCCESS across Tier A/B/C.

Runtime behavior:

1. Derive the same O1 3x3 conservative producer universe.
2. Consider every resident non-empty producer in that universe.
3. Require a current, complete accepted pre-edit output snapshot for every required producer.
4. If any producer is untrusted, retain the O1 full-region fallback.
5. Otherwise queue one full contextual candidate build per required producer.
6. Pass the normal request identity, projection epoch, and presentation-generation gates.
7. Compare accepted full post-edit output with the retained pre-edit output.
8. Install only exact changed 8x8 static zones when comparison is current and complete.
9. Fall back to full producer installation for any invalid comparison state.

Later semantic invalidation cancels pending Object proof state and/or advances the projection epoch, so an older candidate cannot be trusted after the world advances.

An exact empty static diff is valid. Region-complete inspection/picker state, animations, Height dependency state, and Object proof freshness still advance from the accepted full build.

## O5 - adversarial proof coverage

Status: COMPLETE and CERTIFIED.

Certified O5 code head: `398545757f67b56cfb4bdb9b19864b620b1c822a`
CI: #1300 / `38707609239` - SUCCESS across Tier A/B/C.

O5 added `crates/osrs-world/src/object_output_adversarial_tests.rs` and registered it only for tests. No production behavior changed.

The new producer-output adversarial suite proves that exact comparison detects or safely rejects the classes of effects that made radius-based Object invalidation unsafe:

- 8x8 zone-boundary output changes
- 64x64 region-boundary producer changes
- fixed-slot replacement/reveal output
- wall-decoration host/displacement output changes
- five-per-tile capacity removal revealing later output
- overlapping capacity chains changing multiple output zones
- clipped-wall shadow changing neighboring terrain output
- type-10/11 footprint shadow changing terrain across a zone edge
- ModelData normal reconciliation changing neighboring object output
- dynamic/morphed/omitted cases failing closed when candidate coverage is incomplete
- complete dynamic-only static no-op proving an exact empty diff

Existing O2/O4 focused tests remain part of the same Tier B gate and cover:

- semantic Undo/Redo exact Object mutation polarity
- stale projection epoch rejection
- stale presentation generation rejection
- old in-flight completion not consuming a newer request
- missing/stale/incomplete Object proof fallback

This validates the chosen proof boundary without duplicating scene placement, shadow, or normal-reconciliation algorithms inside the invalidation planner.

## O6 - final certification, PR/merge, integrated-main certification

Status: READY TO START.

Before merge, the exact final documentation-bearing branch head must pass:

- architecture boundaries and architecture guard
- reference regeneration safety
- historical deob golden index
- rustfmt
- workspace compile
- strict Clippy
- WebAssembly portability
- full workspace tests
- M3-M9 semantic/reference parity
- focused Object invalidation/adversarial tests through the workspace test gate

Then:

1. confirm the branch is based on current `main` with no unexpected divergence
2. open one PR from `refactor/object-zone-invalidation` to `main`
3. merge before creating any new development branch
4. validate exact integrated `main` with Tier A/B/C
5. update `DEV_HANDOFF.md` on `main` with branch final SHA, PR, merge commit, and integrated-main CI
6. certify the final handoff-bearing `main` head if the documentation update advances `main`
7. stop before any new subsystem branch

## Branch discipline

This remains the only active development subsystem branch until O6 completes.

No second development branch may be created until the Object subsystem is merged, exact integrated `main` is green, the final handoff-bearing `main` is certified, and the user explicitly directs the next subsystem.
