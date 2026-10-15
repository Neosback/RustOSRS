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

### Checkpoint 3 - hover and context-pick bookkeeping ownership

**CLOSED.**

Validated code head:

`11a6ab2901c20003a6d0718da0a46a0324b37b68`

CI:

`38732036663` / #1358 - SUCCESS across Tier A/B/C.

Checkpoint 3 changes:

- final hover commit now routes through `InteractionRuntime::set_hover`;
- `set_hover` latches `last_hover_tile` from the final normalized hover tile before storing the hover state;
- pointer-leave clearing routes through `InteractionRuntime::clear_hover`, preserving the prior `last_hover_tile` exactly as before;
- Shift-right-click context capture routes through `InteractionRuntime::capture_context_pick`, preserving a snapshot of the current hover even if hover later changes;
- ray construction, resident-world picking, and Select/Objects object-to-standing-tile normalization remain in `EditorApp`, so world/projection ownership stays outside the runtime;
- context-menu UI and outline requests remain outside the runtime;
- focused tests cover last-hover-tile latching/clear semantics and context-pick snapshot isolation.

The ordering of the hover commit is intentional: Select/Objects first normalize an object hit to the tile the object stands on, then `set_hover` latches that final tile. This prevents `last_hover_tile` from retaining the terrain hit behind an object.

The first validation attempt (`38731844878` / #1356 on `6520304fed358cf09f882b95def1c1923a974e54`) passed architecture guards and formatting but failed the warnings-denied workspace check because moving pick construction removed the final use of the root-level `Pick` import. The stale import was removed in `11a6ab2901c20003a6d0718da0a46a0324b37b68`; the full replacement run then passed Tier A/B/C.

### Checkpoint 4 - temporary tool-key / peek lifecycle ownership

**CLOSED.**

Validated code head:

`d57965431a6f05b365aac1763dafd5b47877f0d5`

CI:

`38735438231` / #1375 - SUCCESS across Tier A/B/C.

Checkpoint 4 changes:

- temporary tool-key / peek state is now private to `InteractionRuntime` through a dedicated `ToolPeek` record;
- tool-key press transitions route through `InteractionRuntime::press_tool_key`, which preserves tap-to-switch, same-tool return-to-Select, and the previous tool needed for a held peek;
- release transitions route through `InteractionRuntime::release_tool_key`, which clears the peek on release and returns the previous tool only after the existing `PEEK_AFTER` threshold;
- `peek_key()` exposes only the key needed for egui key-state collection, so egui event collection remains outside the runtime;
- `EditorApp::tool_actions` still owns keymap lookup and actual tool application through `set_tool`;
- `EditorApp::tool_release` still owns egui polling and applies any returned tool through `set_tool`;
- UI composition, drawer behavior, spline cleanup, semantic edits, world lookup, and projection ownership remain outside `InteractionRuntime`;
- focused unit coverage proves return-tool latching, same-tool peek cancellation, held-key persistence, threshold-triggered return, and tap persistence.

The live branch already contained the Checkpoint 4 implementation when this checkpoint was reconciled, so that work was audited and preserved instead of duplicated. The pre-certification head `7709f2e9ccad87ecb9fa49289757614fb75b6964` passed architecture guards but failed only `cargo fmt --all --check` in `workspace/tools.rs`. The formatting-only correction produced `1b98b78c148c6c6c0b5a8ed475484804da656a80`, and `38734432620` / #1369 passed Tier A/B/C.

After the first Checkpoint 4 handoff commit, a same-branch scope-restoration commit (`cf252c489ef67bfed523866098b97c97d27070fc`) restored a redundant hover-tint branch that was unrelated to the tool-peek refactor. Strict Clippy correctly rejected that state in `38735166959` / #1373 because the hover and non-hover branches were identical. No lint exception was added. `d57965431a6f05b365aac1763dafd5b47877f0d5` removed only that redundant branch again, with no behavior change, and its full replacement run `38735438231` / #1375 passed Tier A/B/C.

### Next checkpoint boundary

Do not begin Checkpoint 5 without explicit user direction.

The next checkpoint must stay on this same branch and remain inside transient interaction ownership. Audit the remaining `InteractionRuntime` state and choose one coherent state machine for Checkpoint 5 without moving egui event collection, UI composition, semantic edits, world lookup, renderer/projection ownership, or project lifecycle into the runtime.

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

1. `InteractionRuntime` behavioral encapsulation, Checkpoints 1, 2, 3, and 4 closed on PR #55.

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

**STOP AFTER THE CHECKPOINT-4 HANDOFF-BEARING BRANCH HEAD IS CERTIFIED.**

Do not begin Checkpoint 5 until:

1. the exact branch head containing this Checkpoint 4 handoff passes Tier A/B/C;
2. Checkpoint 4 and its certification are reported to the user;
3. the user explicitly directs continuation.

Do not merge PR #55 at this checkpoint. Do not create another development branch.
