# RustOSRS Development Handoff

Last updated: 2026-10-15

`ARCHITECTURE.md` remains the authoritative architecture contract. `docs/PROJECT-STATUS.md` is the project-wide completion audit. This handoff records the exact certified development boundary and the next allowed work.

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

There is **no active feature/refactor subsystem** after the completed `InteractionRuntime` merge.

Certified integrated `main` before this documentation-only handoff update:

`d7db98cd179fe8d64b2e743f74d8aefa9a955449`

CI:

`38758868951` / #1422 - SUCCESS across Tier A/B/C.

That commit is the merge of PR #55 and includes the complete InteractionRuntime behavioral-encapsulation subsystem through Checkpoint 8.

The next recommended subsystem is **`ProjectRuntime` ownership extraction**, but no new development branch or implementation work should begin until the user explicitly directs continuation after this handoff-bearing `main` head is certified.

## Latest completed development subsystem: InteractionRuntime behavioral encapsulation

Merged PR:

#55 - `refactor(editor): encapsulate InteractionRuntime behavior`

Final feature branch:

`refactor/interaction-runtime-behavior`

Final handoff-bearing branch head:

`eacb515ccc2e2e22a63d9d42b14d135803e9c1f9`

Final branch CI:

`38758038367` / #1421 - SUCCESS across Tier A/B/C.

Merge commit:

`d7db98cd179fe8d64b2e743f74d8aefa9a955449`

Integrated-main CI:

`38758868951` / #1422 - SUCCESS across Tier A/B/C.

The user selected Checkpoint 8 as the subsystem stopping point and directed merge. No Checkpoint 9 work was started.

### Checkpoint certification ledger

#### Checkpoint 1 - selection transition ownership

Validated code head:

`bb0b538e4837e72272030eed92044916890cbf02`

CI:

`38727030087` / #1332 - SUCCESS across Tier A/B/C.

Selection clear/replace/add/toggle transitions and primary-selection bookkeeping moved behind `InteractionRuntime`; world lookup for inspector resolution remained in `EditorApp`.

#### Checkpoint 2 - marquee gesture lifecycle ownership

Validated code head:

`e37feed2be1bad377729f64d2acbe040d1c7a32c`

CI:

`38729816578` / #1350 - SUCCESS across Tier A/B/C.

Marquee begin/update/cancel/finish/read lifecycle moved behind `InteractionRuntime`, preserving viewport clamping and modifier latching while loaded-world iteration and completed-selection application remained in `EditorApp`.

#### Checkpoint 3 - hover and context-pick bookkeeping ownership

Validated code head:

`11a6ab2901c20003a6d0718da0a46a0324b37b68`

CI:

`38732036663` / #1358 - SUCCESS across Tier A/B/C.

Final hover commit, hover clear, last-hover-tile latching, and context-pick snapshot capture moved behind runtime methods. Picking, normalization, context-menu UI, and outline requests remained outside.

#### Checkpoint 4 - temporary tool-key / peek lifecycle ownership

Validated code head:

`d57965431a6f05b365aac1763dafd5b47877f0d5`

CI:

`38735438231` / #1375 - SUCCESS across Tier A/B/C.

Temporary tool-key/peek state became private behind `ToolPeek`, `press_tool_key`, `peek_key`, and `release_tool_key`. egui polling, keymap lookup, actual tool application, UI, semantic edits, world lookup, and projection remained outside.

#### Checkpoint 5 - object-drag gesture lifecycle ownership

Validated code head:

`a740843dac78880e31fba21e64652e6535f9313f`

CI:

`38740332353` / #1389 - SUCCESS across Tier A/B/C.

The transient object-drag lifecycle became runtime-owned. Pointer-driven object dragging and transform-gizmo translation preview use the same runtime-owned preview state; semantic editing, world invalidation, projection, and destination validation remain in `EditorApp`.

#### Checkpoint 6 - transform-gizmo gesture lifecycle ownership

Validated code head:

`dc533db9da1ae0f6c2f9882315cdc826f27f7c5f`

CI:

`38751272791` / #1405 - SUCCESS across Tier A/B/C.

The transient gizmo-drag lifecycle became private to `InteractionRuntime`, including start/spec latching, update, cancellation, one-shot finish, and busy-state derivation. Transform math, widget configuration, pointer collection, object-drag footprint preview, and final semantic move/rotation remain outside.

Validation history included an initial formatting-only failure on `e739c76b2e1de7ad21a19b524d7f357cab268bd1` in #1401. No guard exception or lint suppression was introduced.

#### Checkpoint 7 - post-edit object reselection lifecycle ownership

Validated code head:

`c79706fc45c8f2ed597052e83f943a55d4a6cf6b`

CI:

`38754787094` / #1413 - SUCCESS across Tier A/B/C.

The transient `pending_reselect` queue became private. Normal object edits and history replay schedule reselection through runtime methods; incomplete rebuilt-world matches preserve the queue and current selection, while complete matches atomically replace selection and clear the queue.

Validation history caught a formatting issue first, then two remaining history producers of `pending_reselect`; both were routed through the runtime API rather than reopening the field or weakening checks.

#### Checkpoint 8 - spline control-point gesture lifecycle ownership

Validated code head:

`b7f85d6bcfb8bf90cd8da9496f46495f0cd6f099`

CI:

`38757306394` / #1419 - SUCCESS across Tier A/B/C.

Pointer-driven spline point append, insert, remove, pop, active-point update, post-commit clear, and drag begin/update/finish/cancel now route through the `InteractionRuntime` behavioral boundary. egui event collection, modifier interpretation, nearest-point/segment calculations, spline fitting, worker coalescing, preview rendering, terrain projection, semantic path application, and project lifecycle remain outside.

The initial candidate `632f42bde206c5f04e761d94544cdfdbaf32ac63` passed architecture and formatting in #1417, then the warnings-denied workspace build rejected an unused test-only `spline_drag` accessor. The accessor was removed rather than suppressed; #1419 then passed full Tier A/B/C.

## InteractionRuntime subsystem boundary at merge

The completed subsystem intentionally does **not** mean every `InteractionRuntime` field is private or that every interaction-adjacent operation moved into the runtime.

The completed behavior boundaries include:

- selection transitions;
- marquee lifecycle;
- hover/context-pick bookkeeping;
- temporary tool-key/peek lifecycle;
- object-drag lifecycle;
- transform-gizmo drag lifecycle;
- post-edit object reselection lifecycle;
- spline control-point gesture lifecycle.

The following remain outside by design where they require broader application authority or UI/projection context:

- egui event collection and UI composition;
- semantic `EditorSession` edits and history authority;
- loaded-world lookup and rebuilt-world matching;
- renderer/GPU/projection ownership;
- transform/spline planning math that is not transient input capture;
- project lifecycle and persistence;
- diagnostics.

This preserves the project invariant that interaction state is orchestration only and never becomes authoritative map state.

## Previous completed development subsystem: verified map cache export and read-only inspection

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

Current `main` includes the central semantic `EditRuntime` execute/replay gateway; Paint, Height, Paste, Path, Objects, semantic Undo, and semantic Redo through that authority; receipt-driven projection invalidation; exact 8x8 pure-terrain invalidation; proof-gated exact-zone Height/Object behavior with conservative fallbacks; full contextual scene assembly; stale asynchronous build rejection; immutable export snapshots over semantic authority plus pending project state; verified export inspection isolated from authoring state; and the merged InteractionRuntime transient-behavior boundaries above.

## Current runtime ownership state

Completed boundaries:

- `EditRuntime`;
- `RenderRuntime`;
- `WorldRuntime`;
- `InteractionRuntime` field ownership;
- `InteractionRuntime` behavioral encapsulation through the user-selected Checkpoint 8 boundary.

Remaining ownership work:

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

1. `ProjectRuntime` ownership extraction;
2. `UiRuntime` ownership extraction;
3. `DiagnosticsRuntime` ownership extraction;
4. Reference parity closure plus external visual oracle;
5. explicit M10+ CI/certification expansion;
6. 117HD placed/dynamic lights;
7. 117HD sky/day-night/reflection/post/settings remainder;
8. cache/export generalization;
9. autosave/recovery plus project lifecycle/import/merge polish;
10. final pinned-cache release acceptance and documentation reconciliation.

Each major subsystem remains a separate branch/PR sequence.

## Next allowed boundary

**STOP AFTER THIS POST-MERGE HANDOFF-BEARING `main` HEAD IS CERTIFIED.**

Before starting `ProjectRuntime` or any other new subsystem:

1. certify the exact `main` head containing this handoff with Tier A/B/C;
2. report the final handoff-bearing `main` SHA and CI to the user;
3. wait for explicit user direction before creating a new development branch.

Do not start `ProjectRuntime`, UI, diagnostics, Reference parity, 117HD, export-generalization, or other development work as part of this merge-closeout step.
