# RustOSRS Development Handoff

Last updated: 2026-10-15

`ARCHITECTURE.md` remains the authoritative architecture contract. `docs/PROJECT-STATUS.md` is the current project-wide completion audit. This handoff records the exact subsystem boundary, certified baseline history, development protocol, and next allowed work.

## Project invariant

`MapDocument` / `EditorSession` is authoritative edited map state. World-worker edits, rendered geometry, GPU resources, picker data, previews, animation instances, dependency/proof snapshots, export inspection state, project/UI runtime state, and diagnostics are projections or orchestration state only.

If every projection vanished, the complete edited map must still exist correctly in the semantic session.

## Mandatory development protocol

1. Keep exactly one active development branch/system at a time.
2. Do not start a second feature/refactor branch while another subsystem is active.
3. Break each subsystem into reviewable checkpoints.
4. Update this handoff with exact SHAs and validation as checkpoints close.
5. Keep one subsystem on the same branch until complete.
6. Do not mix unrelated architecture, Reference renderer, 117HD, cache/export, or UI work into an active subsystem branch.
7. Before merge, validate the exact final branch head with Tier A/B/C CI.
8. **Merge the current subsystem into `main` before opening any new development branch.**
9. After merge, validate exact integrated `main`, update this handoff, and certify the final handoff-bearing `main` head if documentation advances it.
10. Exact SHA matters. A green run certifies only the SHA it tested.
11. One logical user gesture produces at most one logical semantic transaction/history entry.
12. Failed projection preparation must not knowingly leave semantic authority and visible projection divergent.
13. Do not introduce architecture-guard debt exceptions for convenience.

## Current repository boundary

Active development subsystem: **none**.

Open subsystem PR: **none**.

Live development branches at the 2026-10-15 audit: **`main` only**.

No next feature/refactor branch has been created.

The previously certified handoff baseline was:

`c2f55192ea08435a6ca2df82a84a032f0493b30f`

CI:

`38717265880` / #1320 - SUCCESS across Tier A/B/C.

A project-wide documentation/completion audit then advanced `main` through the following documentation commits:

- `62de140f2d0210e40fd68cdcc1582afb6dfe9fa5` - add `docs/PROJECT-STATUS.md` current completion audit;
- `ca3befb512707d4fa409d22b57922242aa7c0434` - route the documentation index through current status;
- `be1beb72449b805198ea035e22c7083a9a3a17ed` - reconcile the implementation roadmap;
- `f725bad554b61b81d0ddec030e3b1dcc1bd09567` - reconcile the blueprint registry/status;
- `48031a9b59fc4eea4bc43365e783368838ad3bfd` - reconcile the parity matrix with implemented renderer/editor/export state;
- `7f7bc3d4cd665e3d1facfea1a416a7b68d74e861` - reconcile the 117HD plan.

This handoff update advances `main` once more. **The exact handoff-bearing head must pass Tier A/B/C before a new subsystem starts.**

## Latest completed development subsystem: verified map cache export and read-only inspection

Merged PR:

#54 - `feat(export): verified map cache export and inspection`

Final feature branch head:

`83aacecdae29926d2cbc6f85577a173093d269cd`

Final branch CI:

`38715679736` / #1317 - SUCCESS across Tier A/B/C.

Merge commit:

`ad548d524976114b8d70f695f81f4ba0ad256468`

Integrated-main CI:

`38716729696` / #1319 - SUCCESS across Tier A/B/C.

The subsystem added fail-closed staged export, semantic round-trip validation, existing-group JS5 repack, manifest-backed independent verification, preservation of extra map-group files, verified native read-through, isolated read-only inspection, pending-project export support, stale-job protection, and checked group-version overflow.

Current export limitations remain explicit:

- only existing map-square groups can be repacked;
- new groups absent from the reference table are not created;
- editor export currently rewrites map index 5 only;
- hash/Whirlpool reference-table rows fail closed;
- edited caches intentionally do not match pristine profile fingerprints through ordinary matching;
- `VerifiedExport` read-through is native-only;
- the pinned real build-241 export acceptance test requires external cache data and remains ignored in ordinary CI.

## Previous completed development subsystem: Object dependency proof and zone invalidation

Merged PR:

#52 - `refactor(editor): prove Object zone invalidation by accepted output`

Final branch head:

`86c724cd67add8ec96b011effc258cd0256f489c`

Final branch CI:

`38708254817` / #1302 - SUCCESS across Tier A/B/C.

Merge commit:

`e8f1102739345b171ee216cf47996fb2f055b114`

Integrated-main CI:

`38708782538` / #1304 - SUCCESS across Tier A/B/C.

Object edits use exact 8x8 static installation only with complete/current accepted producer-output proof and otherwise retain the conservative 3x3 producer fallback.

## Integrated edit/projection architecture

Current `main` includes:

- central semantic `EditRuntime` execute/replay gateway;
- Paint, Height, Paste, Path, Objects, semantic Undo, and semantic Redo through that authority;
- receipt-driven projection invalidation;
- exact 8x8 pure-terrain invalidation;
- Height exact-zone invalidation only with complete/current dependency proof, otherwise conservative region fallback;
- Object exact-zone static installation only with complete/current accepted producer-output proof, otherwise conservative 3x3 producer fallback;
- full contextual scene assembly;
- stale asynchronous build rejection by request identity, projection epoch, and presentation generation;
- immutable export snapshots over semantic authority plus pending project state;
- verified export inspection isolated from authoring state.

## Current runtime ownership state

Completed boundaries:

- `EditRuntime`;
- `RenderRuntime`;
- `WorldRuntime`;
- `InteractionRuntime` field ownership.

Remaining ownership work:

1. `InteractionRuntime` behavioral encapsulation;
2. `ProjectRuntime` extraction;
3. `UiRuntime` extraction;
4. `DiagnosticsRuntime` extraction.

## Project-wide completion audit result

The 2026-10-15 audit corrected several stale documentation claims.

### Closed claims that must not be reintroduced as blockers

- `TERRAIN-004` is implemented and differential-tested against the pinned client;
- a packed/headless reverse-Z Reference wgpu renderer exists;
- suppressed-face admission and min-plane grouping exist;
- real cache textures, mip arrays, sampler setup, cutout handling, UVs, and texture animation exist;
- the native editor/viewport exists;
- semantic editing, multi-region workflow, project save/open, and verified export exist;
- 117HD water and parallax/displacement are implemented.

### Still incomplete

Reference parity/certification:

- side-grid/placement-shadow edge exactness where coverage is not exhaustive;
- remaining original-plane/roof/visibility edge cases;
- exact depth/raster/bias closure;
- alpha/depth-write/no-depth closure;
- exact texture/sampler proof against the selected reference path;
- independent external visual oracle and measured external 1:1 gates;
- explicit permanent M10+ CI certification entries.

117HD:

- placed/dynamic point lights;
- sky/star/moon/aurora/day-night;
- sky/reflection and remaining atmosphere/water coupling;
- remaining post/settings work;
- wind/character displacement and visual edge cleanup.

Persistence/export/product:

- autosave/crash recovery if retained in M16 scope;
- project/browser lifecycle polish and future import/merge/conflict workflow;
- new map-group creation;
- broader index/repack support where required;
- hash/Whirlpool table-row support/policy;
- future edited-cache identity/profile strategy beyond private verified inspection if desired;
- real pinned-cache release acceptance.

## Canonical current roadmap

Use these documents together:

- `docs/PROJECT-STATUS.md` - current project-wide completion audit;
- `docs/blueprint/17-IMPLEMENTATION-ROADMAP.md` - reconciled dependency/order plan;
- `docs/verification/PARITY-MATRIX.md` - current proof/evidence state;
- `docs/plans/117HD-PORT.md` - current enhanced-profile scope/status.

Historical M10-era text that says the renderer/editor are not started is superseded.

## Remaining major roadmap

Unless the user directs a different priority, the recommended next order is:

1. `InteractionRuntime` behavioral encapsulation;
2. `ProjectRuntime` ownership extraction;
3. `UiRuntime` ownership extraction;
4. `DiagnosticsRuntime` ownership extraction;
5. Reference parity closure + external visual oracle;
6. explicit M10+ CI/certification expansion;
7. 117HD placed/dynamic lights;
8. 117HD sky/day-night/reflection/post/settings remainder;
9. cache/export generalization;
10. autosave/recovery plus project lifecycle/import/merge polish;
11. final pinned-cache release acceptance and documentation reconciliation.

Each should normally be a separate subsystem branch/PR sequence.

## Next allowed boundary

**STOP AFTER THIS HANDOFF-BEARING `main` HEAD IS CERTIFIED.**

Do not create a new development branch until:

1. the exact `main` head containing this handoff passes Tier A/B/C;
2. the documentation audit and its certification are reported to the user;
3. the user explicitly directs continuation into the next subsystem.
