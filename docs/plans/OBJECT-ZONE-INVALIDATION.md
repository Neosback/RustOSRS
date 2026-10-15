# Object Zone Invalidation Plan

Status: COMPLETE, MERGED, AND INTEGRATED-MAIN CERTIFIED.

Merged PR: #52 - `refactor(editor): prove Object zone invalidation by accepted output`

Final subsystem branch head: `86c724cd67add8ec96b011effc258cd0256f489c`

Merge commit on `main`: `e8f1102739345b171ee216cf47996fb2f055b114`

## Goal achieved

Finalized Object edits now narrow static projection installation to exact 8x8 output zones only when every required resident producer is proven by accepted pre/post producer output.

`MapDocument` / `EditorSession` remains authoritative semantic state. World scenes, location replay, geometry, picker state, animations, GPU resources, and Object proof snapshots remain disposable projections.

Full contextual scene assembly is unchanged.

## Safety invariant

Exact-zone static installation is allowed only when:

- every required resident non-empty producer has a current complete accepted pre-edit baseline
- the candidate producer build is full-context and accepted by normal request identity, projection epoch, and presentation-generation gates
- producer region and proof identity match
- both pre-edit and post-edit output coverage are complete
- exact structural comparison succeeds

Any missing, stale, incomplete, mismatched, unsupported, or invalid proof state fails closed to the conservative O1 producer-region policy.

## O1 - conservative Object fallback hardening

Certified head: `643f760a7f318d5519fc9897a588d732afe0c293`
CI: #1264 / `38699017109` - SUCCESS across Tier A/B/C.

O1 established that Object invalidation cannot safely use a guessed anchor radius because output can change through:

- five-per-tile game-object capacity and insertion order
- fixed-slot replacement/reveal
- wall-decoration host/displacement behavior
- loc shadow effects on terrain lighting
- ModelData normal reconciliation
- dynamic/morphed/omitted/rejected placements
- cross-region producer context

Any unproven Object edit therefore uses the complete 3x3 producer-region ring around touched Object regions.

## O2 - canonical Object mutation footprint

Code head: `fed985ea59db7737067b6d19d53844d2d4e9429a`
Code CI: #1275 / `38700886057` - SUCCESS.

Final O2 documentation-bearing head: `d61d9169d271c1dde9009707ff59d73cb8001716`
Final O2 CI: #1277 / `38701473420` - SUCCESS across Tier A/B/C.

Delivered exact removed/added `ObjectPlacement` identity with deterministic ordering/deduplication. Source plane, object id, anchor tile, loc kind, and orientation survive commit receipts and semantic history.

Undo swaps removed/added polarity exactly. Redo preserves original polarity.

## O3 - producer-output proof contract

Certified head: `a555e07796a938676ddb3c4f7d0320dd480041aa`
CI: #1285 / `38703366564` - SUCCESS across Tier A/B/C.

Chosen strategy: producer-output comparison.

`ObjectOutputSnapshot` records producer region, explicit 8x8 coverage, exact per-zone `SceneGeometry`, and complete/incomplete state. Full snapshots explicitly cover all 64 owned zones, including empty zones.

Editor-side proof state is stamped with producer region, projection epoch, and presentation generation.

## O4 - accepted-build runtime integration

Code head: `52eadc6284e71d9bc00de878d89cc67a0f0c7628`
Code CI: #1294 / `38705182639` - SUCCESS across Tier A/B/C.

Documentation-bearing O4 head: `55f42228003770c3adc5ef5500a32052ebf1ff2a`
Documentation-bearing O4 CI: #1296 / `38705932458` - SUCCESS across Tier A/B/C.

Runtime behavior:

1. derive the same O1 3x3 conservative producer universe
2. require current complete accepted baseline output for every required resident non-empty producer
3. fall back to O1 full-region rebuilding if any producer is untrusted
4. otherwise queue full contextual candidate builds
5. accept candidates only through existing stale-result gates
6. compare accepted post-edit output with retained pre-edit output
7. install only exact changed static 8x8 zones when comparison is complete/current
8. fall back to full producer installation for invalid comparison state

Later semantic invalidation cancels pending Object proof state and/or advances the projection epoch.

## O5 - adversarial proof coverage

Certified code head: `398545757f67b56cfb4bdb9b19864b620b1c822a`
CI: #1300 / `38707609239` - SUCCESS across Tier A/B/C.

Added test-only coverage for:

- 8x8 zone-boundary changes
- 64x64 producer-region boundary changes
- fixed-slot replacement/reveal
- wall-decoration host/displacement output changes
- five-per-tile capacity reveal
- overlapping capacity chains
- clipped-wall shadow changing neighboring terrain output
- type-10/11 footprint shadow across a zone edge
- ModelData normal-reconciliation changes in neighboring object output
- incomplete dynamic/morphed/omitted candidate state failing closed
- complete dynamic-only static no-op producing an exact empty diff

Existing workspace tests cover semantic Undo/Redo polarity, stale epoch/generation rejection, old completion not consuming newer work, and missing/stale/incomplete proof fallback.

## O6 - final certification and merge

Final branch head: `86c724cd67add8ec96b011effc258cd0256f489c`

Final branch CI: #1302 / `38708254817` - SUCCESS across Tier A/B/C.

Live-main comparison before PR:

- 41 commits ahead
- 0 commits behind
- changes limited to the Object subsystem plus plan/handoff documentation

PR #52 merged successfully.

Merge commit on `main`: `e8f1102739345b171ee216cf47996fb2f055b114`

Integrated-main CI: #1304 / `38708782538` - SUCCESS across Tier A/B/C.

The final `DEV_HANDOFF.md` update after this plan commit must be certified if it advances `main`.

## Final behavior

The integrated Object subsystem provides:

- exact canonical Object mutation footprints
- full contextual producer CPU builds
- accepted producer-output proof snapshots
- exact changed-zone static GPU installation when proven
- O1 3x3 producer-region fallback for every untrusted proof state
- stale async rejection by request identity, projection epoch, and presentation generation
- unchanged region-complete picker/inspection, animation, and Height dependency refresh behavior

The proof answers the postcondition question "which producer-owned static zones actually changed?" instead of duplicating capacity, replacement, shadow, or normal-reconciliation algorithms in the invalidation planner.

## Branch discipline after merge

The Object subsystem is complete and merged. No next development branch has been created.

A future subsystem branch may be created only after the final handoff-bearing `main` head is certified and the user explicitly directs the next subsystem.
