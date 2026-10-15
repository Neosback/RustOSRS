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

Active development subsystem: **`InteractionRuntime` behavioral encapsulation**.

Open subsystem PR: **#55 - `refactor(editor): encapsulate InteractionRuntime behavior` (draft)**.

Active development branch: **`refactor/interaction-runtime-behavior`**.

Certified `main` baseline for this subsystem:

`5b1975d3be40fc4c0f417a4fd37d84284ada96c2`

CI:

`38725189001` / #1327 - SUCCESS across Tier A/B/C.

No second feature/refactor branch may be created while PR #55 is active.

## Active subsystem: InteractionRuntime behavioral encapsulation

The subsystem exists to move transient interaction behavior behind the already-established `InteractionRuntime` field-ownership boundary without changing semantic edit authority, projection correctness, rendering semantics, or user-gesture transaction semantics.

### Checkpoint 1 - selection transition ownership

**CLOSED.**

Validated code head:

`bb0b538e4837e72272030eed92044916890cbf02`

CI:

`38727030087` / #1332 - SUCCESS across Tier A/B/C.

Checkpoint 1 changes:

- object/tile selection state transitions now live behind `InteractionRuntime` methods instead of being mutated directly by `selection.rs`;
- clear object, clear tile, full clear, single-object select, replace-object selection, add/toggle object, add/select/toggle tile, primary-object detection, and selection counts are encapsulated;
- world/scene lookup needed to resolve an object's inspector tile remains in `EditorApp`, so the runtime does not absorb world ownership;
- click-selection inspector behavior is preserved;
- marquee plain tile addition still does not move inspector focus;
- toggling an object out still leaves the prior inspector tile unchanged;
- replacing object selection, including Select Similar, still updates the primary object without implicitly moving inspector focus;
- focused unit tests cover object primary-selection transitions, replace semantics, tile inspector behavior, and full selection clear.

The first validation attempt (`38726838237` / #1331 on `8bf8a6cef35765381a760eb0bcde20c472fb15f3`) failed only the rustfmt check. The formatting-only correction advanced the code head to `bb0b538e4837e72272030eed92044916890cbf02`; the full replacement run then passed Tier A/B/C.

### Checkpoint 2 - marquee gesture lifecycle ownership

**CLOSED.**

Validated code head:

`e37feed2be1bad377729f64d2acbe040d1c7a32c`

CI:

`38729816578` / #1350 - SUCCESS across Tier A/B/C.

Checkpoint 2 changes:

- `Marquee` and its transient gesture state now live in `InteractionRuntime` rather than `selection.rs`;
- begin, update, cancel, finish, and read access are encapsulated through `InteractionRuntime` methods;
- pointer updates remain clamped to the viewport bounds and Shift/Cmd modifier intent remains latched across the gesture exactly as before;
- switching tools, leaving the viewport after pointer release, object placement, object dragging, quick-open/context-menu suppression, and gizmo capture now cancel marquee state through the runtime instead of mutating the field directly;
- viewport projection, loaded-world iteration, tile/object discovery, and application of the completed marquee selection remain in `EditorApp`, so `InteractionRuntime` does not absorb world or projection ownership;
- the marquee field is private to the runtime, preventing future direct lifecycle mutation from outside the boundary;
- focused unit coverage proves clamping/modifier latching and cancellation/finish behavior.

Validation caught and closed three boundary-quality issues before this checkpoint was accepted:

- the initial implementation required rustfmt-only corrections;
- making `marquee` private exposed two remaining direct resets in `viewport.rs` and `workspace/tools.rs`, which were routed through `cancel_marquee()`;
- strict Clippy rejected an `expect()` in the new unit test, so the assertion was rewritten without `unwrap`/`expect` and then formatted exactly.

The final replacement run on `e37feed2be1bad377729f64d2acbe040d1c7a32c` passed architecture guards, formatting, workspace check, strict Clippy, WASM portability, the full workspace tests, and the M3-M9 semantic parity suite.

### Next checkpoint boundary

Do not begin Checkpoint 3 without explicit user direction.

The next checkpoint must stay on this same branch and remain inside transient interaction ownership. A suitable next cut is to encapsulate another coherent interaction state machine, such as hover/context-pick bookkeeping or temporary tool-key/peek state, without moving world lookup, UI composition, semantic edits, or projection ownership into `InteractionRuntime`.

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

In progress:

1. `InteractionRuntime` behavioral encapsulation, Checkpoints 1 and 2 closed on PR #55.

Remaining ownership work after this subsystem:

1. `ProjectRuntime` extraction;
2. `UiRuntime` extraction;
3. `DiagnosticsRuntime` extraction.

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

1. finish `InteractionRuntime` behavioral encapsulation on the current branch/PR;
2. merge it to `main` and certify exact integrated `main`;
3. `ProjectRuntime` ownership extraction;
4. `UiRuntime` ownership extraction;
5. `DiagnosticsRuntime` ownership extraction;
6. Reference parity closure + external visual oracle;
7. explicit M10+ CI/certification expansion;
8. 117HD placed/dynamic lights;
9. 117HD sky/day-night/reflection/post/settings remainder;
10. cache/export generalization;
11. autosave/recovery plus project lifecycle/import/merge polish;
12. final pinned-cache release acceptance and documentation reconciliation.

Each major subsystem should remain a separate branch/PR sequence.

## Next allowed boundary

**STOP AFTER THE CHECKPOINT-2 HANDOFF-BEARING BRANCH HEAD IS CERTIFIED.**

Do not begin Checkpoint 3 until:

1. the exact branch head containing this Checkpoint 2 handoff passes Tier A/B/C;
2. Checkpoint 2 and its certification are reported to the user;
3. the user explicitly directs continuation.

Do not merge PR #55 at this checkpoint. Do not create another development branch.
