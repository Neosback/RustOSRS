# RustOSRS Development Handoff

Last updated: 2026-10-15

`ARCHITECTURE.md` is the authoritative architecture contract. This handoff records the certified integrated baseline, completed subsystem state, remaining roadmap, and next allowed boundary.

## Project invariant

`MapDocument` / `EditorSession` is authoritative edited map state. World-worker edits, rendered geometry, GPU resources, picker data, previews, animation instances, dependency/proof snapshots, and UI state are projections only.

If every projection vanished, the complete edited map must still exist correctly in the semantic session.

## Mandatory development protocol

1. Keep exactly one active development branch/system at a time.
2. Do not start a second feature/refactor branch while another subsystem is active.
3. Break each subsystem into reviewable checkpoints.
4. Update this handoff with exact SHAs and validation as checkpoints close.
5. Keep one subsystem on the same branch until complete.
6. Do not mix unrelated architecture, renderer/117HD, cache, or UI work into an active subsystem branch.
7. Before merge, validate the exact final branch head with Tier A/B/C CI.
8. **Merge the current subsystem into `main` before opening any new development branch.**
9. After merge, validate exact integrated `main`, update this handoff, and certify the final handoff-bearing `main` head if docs advance it.
10. Exact SHA matters. A green run certifies only the SHA it tested.
11. One logical user gesture produces at most one logical semantic transaction/history entry.
12. Failed projection preparation must not knowingly leave semantic authority and visible projection divergent.
13. Do not introduce architecture-guard debt exceptions for convenience.

## Current repository state

Active development subsystem: **none**.

Open subsystem PR: **none for Object work; PR #52 is merged**.

No next development branch has been created.

The next subsystem may start only after this final handoff-bearing `main` head is green and the user explicitly directs the next work.

## Latest completed subsystem: Object dependency proof and zone invalidation

Merged PR:
#52 - `refactor(editor): prove Object zone invalidation by accepted output`

Object branch:
`refactor/object-zone-invalidation`

Object branch base:
`a7b9fb47835ab00b25db7c02e1aa3ca3157a054b`

Final Object branch head:
`86c724cd67add8ec96b011effc258cd0256f489c`

Final branch CI:
`38708254817` / #1302 - SUCCESS across Tier A/B/C.

Merge commit on `main`:
`e8f1102739345b171ee216cf47996fb2f055b114`

Integrated-main CI:
`38708782538` / #1304 - SUCCESS across Tier A/B/C.

Post-merge Object plan commit:
`334b54fffd99b6fff9078f56b87808e8f5d6ae94`

This handoff commit advances `main` once more and therefore must receive its own exact-head Tier A/B/C certification before another subsystem starts.

## Object subsystem result

Object invalidation is now exact-zone capable without relying on a guessed dependency radius.

### Conservative safety baseline

O1 proved that Object effects can propagate through:

- five-per-tile game-object capacity and insertion order
- fixed-slot replacement/reveal
- wall-decoration host/displacement behavior
- loc shadow effects on terrain lighting
- ModelData normal reconciliation
- dynamic/morphed/omitted/rejected placements
- cross-region producer context from the 104x104 scene window

Therefore every unproven Object edit retains the complete 3x3 producer-region fallback around touched Object regions.

### Canonical semantic footprint

O2 retains exact removed and added `ObjectPlacement` identity:

- object id
- source plane
- world anchor tile
- loc kind
- orientation

Ordering/deduplication is deterministic. Undo swaps removed/added polarity exactly. Redo preserves original polarity.

### Accepted producer-output proof

O3/O4 implement producer-output comparison:

- full producer snapshots explicitly cover all 64 owned 8x8 zones, including empty zones
- exact per-zone `SceneGeometry` is retained for proof comparison
- proof state is bound to producer region, projection epoch, and presentation generation
- Object edits derive the same O1 3x3 producer universe
- every required resident non-empty producer must have a current complete accepted baseline
- one untrusted producer forces the conservative fallback
- eligible producers receive full contextual candidate builds
- candidates still pass request identity, epoch, and presentation-generation freshness gates
- accepted pre/post output is compared structurally
- only exact changed static zones are installed when proof is current and complete
- invalid proof falls back to full producer installation
- later edits cancel pending proof and/or invalidate old candidates

Full contextual scene assembly remains unchanged. Region-complete picker/inspection state, animations, Height dependencies, and proof freshness continue to advance from accepted builds.

### Adversarial coverage

O5 added producer-output tests for:

- 8x8 zone boundaries
- 64x64 producer-region boundaries
- fixed-slot replacement/reveal
- wall-decoration host effects
- capacity reveal and overlapping capacity chains
- clipped-wall shadow effects on neighboring terrain
- type-10/11 shadow effects across zone edges
- ModelData normal-reconciliation changes in neighboring object output
- incomplete dynamic/morphed/omitted candidate state failing closed
- exact empty static diff for complete dynamic-only no-op output

Existing focused tests continue to cover:

- semantic Undo/Redo exact Object polarity
- stale projection epoch rejection
- stale presentation-generation rejection
- old in-flight completion not consuming newer work
- missing/stale/incomplete Object proof fallback

O5 changed no production behavior.

## Object checkpoint certifications

- O1: `643f760a7f318d5519fc9897a588d732afe0c293`, CI #1264 / `38699017109`
- O2 code: `fed985ea59db7737067b6d19d53844d2d4e9429a`, CI #1275 / `38700886057`
- O2 docs: `d61d9169d271c1dde9009707ff59d73cb8001716`, CI #1277 / `38701473420`
- O3: `a555e07796a938676ddb3c4f7d0320dd480041aa`, CI #1285 / `38703366564`
- O4 code: `52eadc6284e71d9bc00de878d89cc67a0f0c7628`, CI #1294 / `38705182639`
- O4 docs: `55f42228003770c3adc5ef5500a32052ebf1ff2a`, CI #1296 / `38705932458`
- O5 code: `398545757f67b56cfb4bdb9b19864b620b1c822a`, CI #1300 / `38707609239`
- final branch: `86c724cd67add8ec96b011effc258cd0256f489c`, CI #1302 / `38708254817`
- integrated merge commit: `e8f1102739345b171ee216cf47996fb2f055b114`, CI #1304 / `38708782538`

All listed CI runs completed successfully across Tier A/B/C.

## Integrated edit/projection architecture

Current `main` includes:

- central semantic `EditRuntime` execute/replay gateway
- Paint, Height, Paste, Path, Objects, semantic Undo, and semantic Redo through the gateway
- receipt-driven projection invalidation
- pure terrain 8x8 zone invalidation
- Height exact 8x8 zone invalidation only with complete/current dependency proof
- conservative Height region fallback for unproven cases
- Object exact 8x8 static installation only with complete/current accepted producer-output proof
- conservative Object 3x3 producer-region fallback for unproven cases
- full contextual scene assembly with zone-scoped final static geometry installation
- stale asynchronous region-build rejection by request identity, projection epoch, and presentation generation
- accepted-build Height and Object proof freshness tied to the same projection identity
- exact canonical Object removed/added mutation footprints through semantic history

## Runtime ownership status

Completed boundaries:

- `EditRuntime`
- `RenderRuntime`
- `WorldRuntime`
- `InteractionRuntime` field ownership

Future ownership work:

- `InteractionRuntime` behavioral encapsulation
- `ProjectRuntime`
- `UiRuntime`
- `DiagnosticsRuntime`

## Reference renderer remaining work

Known remaining or partially stale audit areas include:

- side-grid shadow exactness
- minPlane/originalPlane behavior
- GPU depth/raster semantics
- alpha modes
- sampler behavior
- external visual oracle/gates

## 117HD current high-level state

Already substantially implemented:

- render-profile/HDR foundation
- HD data/material tables
- HD terrain/model extraction
- material replacement
- ground blending/normals
- environments/fog/directional lighting
- directional shadows/filtering
- water surfaces, flow/normal maps, foam, Fresnel, underwater tint/caustics
- animated-loc HD material routing
- material flow maps
- parallax/displacement and displaced shadow position

Large remaining systems include:

- dynamic/placed point lights
- sky/star/moon/aurora/day-night systems
- sky reflection/water integration
- remaining post-processing/settings parity
- wind/character displacement and remaining model/material edge cases

## Cache/export current state

Implemented:

- locations exact 2936/2936
- terrain 2935/2936 retained semantics
- five-file repack exact
- map stream encoding/cache-copy repack for existing map groups

Known limitations:

- only existing map-square groups are repacked
- new groups absent from the reference table are not created
- other indices are not generally rewritten by the editor export path
- edited cache copies intentionally no longer match the pristine-cache fingerprint/profile

## Remaining major roadmap

Each item is a separate future subsystem and therefore a separate branch after the final handoff-bearing `main` head is certified and the user explicitly directs continuation:

1. `InteractionRuntime` behavioral encapsulation
2. `ProjectRuntime` ownership extraction
3. `UiRuntime` ownership extraction
4. `DiagnosticsRuntime` ownership extraction
5. Reference renderer parity gates and external visual oracle
6. 117HD dynamic/placed lights
7. 117HD sky/day-night/reflection systems and remaining effects
8. edited-cache fingerprint/profile strategy, new-group support, and broader index repack when justified
9. project/browser lifecycle polish and future import/merge product work

## Next allowed boundary

**STOP AFTER FINAL HANDOFF-BEARING `main` CERTIFICATION.**

Do not create a new development branch until:

1. the exact `main` head containing this handoff is green across Tier A/B/C
2. the Object merge and final certification are reported to the user
3. the user explicitly directs the next subsystem
