# RustOSRS Development Handoff

Last updated: 2026-10-15

`ARCHITECTURE.md` remains the authoritative architecture contract. `docs/PROJECT-STATUS.md` is the project-wide completion audit. This handoff records the exact active subsystem boundary, certified history, development protocol, and next allowed work.

## Project invariant

`MapDocument` / `EditorSession` is authoritative edited map state. World-worker edits, rendered geometry, GPU resources, picker data, previews, animation instances, dependency/proof snapshots, export inspection state, project/UI runtime state, and diagnostics are projections or orchestration state only.

If every projection vanished, the complete edited map must still exist correctly in the semantic session.

One logical user gesture produces at most one logical semantic transaction/history entry. Failed projection preparation must not knowingly leave semantic authority and visible projection divergent.

## Mandatory development protocol

1. Keep exactly one active development branch/system at a time.
2. Do not start a second feature/refactor branch while another subsystem is active.
3. Break each subsystem into reviewable checkpoints and stop at checkpoint boundaries.
4. Update this handoff with exact SHAs and validation as checkpoints close.
5. Keep one subsystem on the same branch until complete.
6. Do not mix unrelated architecture, Reference renderer, 117HD, cache/export, or UI work into an active subsystem branch.
7. Before merge, validate the exact final branch head with Tier A/B/C CI.
8. Merge the current subsystem into `main` before opening any new development branch.
9. After merge, validate exact integrated `main`, update this handoff, and certify the final handoff-bearing `main` head if documentation advances it.
10. Exact SHA matters. A green run certifies only the SHA it tested.
11. Do not introduce architecture-guard debt exceptions or lint suppressions for convenience.

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

The subsystem moves transient interaction behavior behind the already-established `InteractionRuntime` field-ownership boundary without changing semantic edit authority, projection correctness, rendering semantics, or user-gesture transaction semantics.

### Checkpoint 1 - selection transition ownership

**CLOSED.**

Validated code head:

`bb0b538e4837e72272030eed92044916890cbf02`

CI:

`38727030087` / #1332 - SUCCESS across Tier A/B/C.

Selection clear/replace/add/toggle transitions and primary-selection bookkeeping moved behind `InteractionRuntime`. World lookup needed to resolve inspector tiles remained in `EditorApp`. Focused tests cover object primary selection, replace semantics, tile inspector behavior, and full clear. The first attempt `8bf8a6cef35765381a760eb0bcde20c472fb15f3` failed only rustfmt; the formatting-only replacement became the validated head above.

### Checkpoint 2 - marquee gesture lifecycle ownership

**CLOSED.**

Validated code head:

`e37feed2be1bad377729f64d2acbe040d1c7a32c`

CI:

`38729816578` / #1350 - SUCCESS across Tier A/B/C.

`Marquee` begin/update/cancel/finish/read lifecycle moved behind `InteractionRuntime`, with viewport clamping and modifier latching preserved. Loaded-world iteration, projection, tile/object discovery, and application of the completed marquee selection remain in `EditorApp`. Validation caught formatting, remaining direct resets, and a strict-Clippy test issue before the final green run.

### Checkpoint 3 - hover and context-pick bookkeeping ownership

**CLOSED.**

Validated code head:

`11a6ab2901c20003a6d0718da0a46a0324b37b68`

CI:

`38732036663` / #1358 - SUCCESS across Tier A/B/C.

Final hover commit, hover clear, last-hover-tile latching, and context-pick snapshot capture moved behind runtime methods. Ray construction, resident-world picking, Select/Objects hit normalization, context-menu UI, and outline requests remain outside. The first validation attempt exposed a stale warnings-denied import; removing it produced the validated head.

### Checkpoint 4 - temporary tool-key / peek lifecycle ownership

**CLOSED.**

Validated code head:

`d57965431a6f05b365aac1763dafd5b47877f0d5`

CI:

`38735438231` / #1375 - SUCCESS across Tier A/B/C.

Temporary tool-key/peek state is private behind `ToolPeek`, `press_tool_key`, `peek_key`, and `release_tool_key`. egui polling, keymap lookup, actual tool application, UI composition, spline cleanup, semantic edits, world lookup, and projection remain outside. A formatting-only correction first passed A/B/C at `1b98b78c148c6c6c0b5a8ed475484804da656a80`; a later same-branch redundant hover-tint branch was correctly rejected by strict Clippy in #1373, then removed without a lint exception. The replacement head above passed full A/B/C.

### Checkpoint 5 - object-drag gesture lifecycle ownership

**CLOSED.**

Validated code head:

`a740843dac78880e31fba21e64652e6535f9313f`

CI:

`38740332353` / #1389 - SUCCESS across Tier A/B/C.

The transient `object_drag` field became private, with begin/update/cancel/finish/read access routed through runtime methods. Pointer-driven object dragging and transform-gizmo translation preview use the same runtime-owned preview lifecycle. egui collection, selected-spec resolution, destination validation, preview projection, semantic object editing, world invalidation, and projection remain in `EditorApp`.

Validation caught three useful issues before acceptance: the first candidate exceeded the 500-line production-module cap after adding tests, so tests moved into `interaction_runtime/tests.rs` rather than weakening the guard; the warnings-denied build then exposed remaining direct gizmo mutations; finally the compiler exposed an overlapping gizmo borrow. The corrected head above passed architecture, formatting, workspace check, strict Clippy, WASM, full workspace tests, and M3-M9 semantic parity.

### Checkpoint 6 - transform-gizmo gesture lifecycle ownership

**CLOSED.**

Validated code head:

`dc533db9da1ae0f6c2f9882315cdc826f27f7c5f`

CI:

`38751272791` / #1405 - SUCCESS across Tier A/B/C.

Checkpoint 6 changes:

- the transient `gizmo_drag` field is private to `InteractionRuntime`;
- `InteractionRuntime::update_gizmo_drag` creates the drag once, latches the original start transform and selected specs, then updates only the current transform on later samples;
- `cancel_gizmo_drag` clears both the in-progress drag and capture/busy state;
- `finish_gizmo_drag` clears capture/busy state and returns the completed drag exactly once;
- `gizmo_drag()` exposes read-only drag state needed for target continuity and release detection;
- `update_gizmo_busy` derives the busy state from the external gizmo focus signal plus whether a runtime drag is active;
- `GizmoDrag` now exposes constructor/update/read methods while keeping its transform/spec fields private;
- tool/modal/context-menu/empty-selection cancellation routes through `cancel_gizmo_drag` instead of mutating drag/busy fields directly;
- transform samples route through `update_gizmo_drag`, and final release routes through `finish_gizmo_drag`;
- transform-gizmo widget configuration, matrices, pointer collection, snapped translation/rotation math, object-drag footprint preview, and final semantic move-or-rotation decision remain in `gizmo.rs` / `EditorApp`;
- semantic object edits, world invalidation, renderer/projection ownership, and project lifecycle did not move into `InteractionRuntime`;
- focused tests prove initial start/spec latching across later updates, busy-state persistence while a drag is active, one-shot finish behavior, and cancellation clearing capture state.

Validation history:

- initial certification candidate `e739c76b2e1de7ad21a19b524d7f357cab268bd1` ran as `38751004541` / #1401;
- architecture checks and guard tests passed on that candidate;
- #1401 failed only `cargo fmt --all --check`, with one mechanical wrap in `gizmo.rs` and one helper-line formatting change in `interaction_runtime/tests.rs`;
- no behavioral workaround, guard exception, or lint suppression was introduced;
- formatting-only commits advanced the candidate to `dc533db9da1ae0f6c2f9882315cdc826f27f7c5f`;
- replacement run `38751272791` / #1405 passed architecture guards, formatting, warnings-denied workspace check, strict Clippy, WASM portability, full workspace tests, and the complete M3-M9 semantic parity suite.

### Checkpoint 7 - post-edit object reselection lifecycle ownership

**CLOSED.**

Validated code head:

`c79706fc45c8f2ed597052e83f943a55d4a6cf6b`

CI:

`38754787094` / #1413 - SUCCESS across Tier A/B/C.

Checkpoint 7 changes:

- the transient `pending_reselect` queue is private to `InteractionRuntime`;
- `set_pending_reselect` is the single write path used by normal object edits, legacy object-history replay, and semantic object undo/redo projection;
- `pending_reselect()` exposes only the read-only target specs needed for rebuilt-world matching;
- `complete_reselect` is all-or-nothing: an empty queue or incomplete match leaves the queue and current selection unchanged, while a complete match atomically replaces object selection and clears the pending queue;
- completing reselection reuses the existing `replace_objects` transition, so the last resolved object remains primary and inspector focus is not changed implicitly;
- `EditorApp::resolve_reselect` still owns rebuilt-region lookup and exact placement matching against `infos`; the runtime does not absorb world lookup or projection state;
- semantic object commits, history replay, world invalidation, renderer/projection ownership, egui collection, UI composition, and project lifecycle remain outside `InteractionRuntime`;
- focused unit tests prove incomplete-match preservation, complete-match replacement/clear behavior, primary-object semantics, inspector preservation, and the empty-queue guard.

Validation history:

- initial candidate `14dbcb2fe5cba91cad8da5db553662f7083b1a5e` ran as `38753973641` / #1409; architecture and guard checks passed, and the run failed only `cargo fmt --all --check` because the three Git-data-written Rust files lacked the trailing newline required by rustfmt;
- the newline-only correction produced `dd8d88bc0c992c03df5bb01825ae6cc4957aeb59`, whose `38754392652` / #1411 run passed architecture and formatting but the warnings-denied workspace build exposed two additional direct producers of `pending_reselect` in `editing/history.rs` and `editing/history/routing.rs`;
- those two history producers were routed through `set_pending_reselect` instead of reopening the field or adding an exception;
- final code head `c79706fc45c8f2ed597052e83f943a55d4a6cf6b` passed architecture guards, formatting, warnings-denied workspace check, strict Clippy, WASM portability, full workspace tests, and the complete M3-M9 semantic parity suite in `38754787094` / #1413.

### Next checkpoint boundary

**Do not begin Checkpoint 8 without explicit user direction.**

Checkpoint 8, if authorized, must stay on this same branch and remain inside transient interaction ownership. Audit the remaining `InteractionRuntime` state and choose one coherent behavioral boundary without moving egui event collection, UI composition, semantic edits, world lookup, renderer/projection ownership, project lifecycle, or unrelated runtime extraction into this subsystem.

Do not merge PR #55 at the Checkpoint 7 boundary. Do not create another development branch.

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

This subsystem added fail-closed staged export, semantic round-trip validation, existing-group JS5 repack, manifest-backed independent verification, preservation of extra map-group files, verified native read-through, isolated read-only inspection, pending-project export support, stale-job protection, and checked group-version overflow.

Current export limitations remain explicit: only existing map-square groups can be repacked; new groups absent from the reference table are not created; editor export currently rewrites map index 5 only; hash/Whirlpool reference-table rows fail closed; edited caches intentionally do not match pristine profile fingerprints through ordinary matching; `VerifiedExport` read-through is native-only; the pinned real build-241 export acceptance test requires external cache data and remains ignored in ordinary CI.

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

Current `main` includes the central semantic `EditRuntime` execute/replay gateway; Paint, Height, Paste, Path, Objects, semantic Undo, and semantic Redo through that authority; receipt-driven projection invalidation; exact 8x8 pure-terrain invalidation; proof-gated exact-zone Height/Object behavior with conservative fallbacks; full contextual scene assembly; stale asynchronous build rejection; immutable export snapshots over semantic authority plus pending project state; and verified export inspection isolated from authoring state.

## Current runtime ownership state

Completed boundaries:

- `EditRuntime`;
- `RenderRuntime`;
- `WorldRuntime`;
- `InteractionRuntime` field ownership.

In progress:

1. `InteractionRuntime` behavioral encapsulation, Checkpoints 1 through 7 closed on PR #55.

Remaining ownership work after this subsystem:

1. `ProjectRuntime` extraction;
2. `UiRuntime` extraction;
3. `DiagnosticsRuntime` extraction.

## Project-wide completion audit result

The 2026-10-15 audit corrected stale historical documentation. Claims that must not be reintroduced as blockers include: `TERRAIN-004` is implemented and differential-tested; a packed/headless reverse-Z Reference wgpu renderer exists; suppressed-face admission and min-plane grouping exist; real cache textures/mips/samplers/cutout/UV/animation exist; the native editor/viewport exists; semantic editing, multi-region workflow, project save/open, and verified export exist; and 117HD water plus parallax/displacement are implemented.

Still incomplete work includes Reference edge/depth/alpha/texture parity closure and an independent external visual oracle; explicit permanent M10+ CI certification entries; 117HD placed/dynamic lights, sky/star/moon/aurora/day-night, remaining sky/reflection/atmosphere/water coupling, post/settings, wind/character displacement, and visual cleanup; autosave/crash recovery if retained; project/browser lifecycle and import/merge/conflict polish; new map-group creation; broader index/repack support; hash/Whirlpool row policy/support; future edited-cache identity/profile strategy if desired; and real pinned-cache release acceptance.

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
6. Reference parity closure plus external visual oracle;
7. explicit M10+ CI/certification expansion;
8. 117HD placed/dynamic lights;
9. 117HD sky/day-night/reflection/post/settings remainder;
10. cache/export generalization;
11. autosave/recovery plus project lifecycle/import/merge polish;
12. final pinned-cache release acceptance and documentation reconciliation.

Each major subsystem remains a separate branch/PR sequence.

## Next allowed boundary

**STOP AFTER THE CHECKPOINT-7 HANDOFF-BEARING BRANCH HEAD IS CERTIFIED.**

Do not begin Checkpoint 8 until:

1. the exact branch head containing this Checkpoint 7 handoff passes Tier A/B/C;
2. Checkpoint 7 and its certification are reported to the user;
3. the user explicitly directs continuation.

Do not merge PR #55 at this checkpoint. Do not create another development branch.