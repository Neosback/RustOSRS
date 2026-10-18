# Building, Studio and Lab audit — 2026-10-18

Audit baseline: clean `main` at `ada7c6277fac47527e481994ef6d44798ecc66f3`.
Scope: the original building correctness → Studio → learning plan, including follow-on commits
through `ada7c627`. This is an implementation and boundary audit, not certification of every
OSRS asset or an independent RuneLite visual comparison. Product source was not changed by this audit.

The implementation is substantial, but the foundation is **not ready for acceptance**. The
native Studio startup and relocation defects below take priority over adding more generators.
AI training can be native-only. Python training, future model optimizers and GPU training runtimes
need no WASM commitment; reusable core/edit/scene semantics retain their existing portability.

## Confirmed findings

### P1 — Studio rejects the normal pinned cache

`crates/osrs-editor/src/app/prefab_gallery/stage.rs:148–158` assumes the fixed stage at
`(3232,2144)` is empty. The current binary built from the audit SHA, with the pinned build-241
cache, displays: “This cache has content in the isolated stage area. Choose a compatible source
before opening Studio.” Save, Save as new, Expand bounds and Revert remain disabled.

Reproduced using an isolated settings/library directory under `target/building-smoke`, a captured
12×10×2 prefab, and `prefab studio Studio_smoke`. The first screenshot was retained before the
smoke screenshot hook closed the window:
`target/building-audit-smoke/studio-ready-first.png`.

The guard protects map content and should remain. Provide a genuinely isolated synthetic stage
or a source-validated empty stage with an explicit fallback. Acceptance needs startup, edit,
undo/redo, save/reopen, navigation and stage-floor exclusion against both supported real caches.

### P1 — Relocation misses a bridge on source plane 1

`crates/osrs-edit/src/prefab/foundation.rs:31–46` reads the bridge bit only on the destination
base plane. The OSRS bridge projection bit belongs to plane 1. A building placed at base plane 0
therefore passes this check when plane 1 has flag 2. A pure-planner probe accepted that site and
emitted four height changes on the bridge plane.

Resolve source/collision/height planes using the existing bridge semantics before accepting any
support or upper-height edit. Test plane-0 and plane-1 sites, four rotations, blend vertices and
missing neighboring coverage; rejection must leave the whole document unchanged.

### P1 — Applying WFC edits the Lab mirror and loses the result

`app/prefab_studio/catalog.rs:189–194` commits directly to the viewer session. In an active task,
that session is the disposable Lab projection, while the authored runtime is parked.
`app/prefab_studio/lab.rs:43–64` restores the parked document on exit; Accept reads `Env`, which
was never changed by catalog application. Thus an applied floor layout can disappear on leaving
Lab, is missing from the action trace/score, and is absent from an accepted result.

Route application either through a legal Lab action or an explicit transition back to the restored
authored session before the edit gateway. Test apply → leave → save/reopen and exact undo/redo.
Also clear old pins/solutions when importing another catalog: the current importer replaces the
catalog but retains a solution whose indices belonged to the previous catalog.

### P2 — Door/portal evidence is lost between capture and learning

`app/prefab_panel/capture.rs:173–183` stores `MemberRole` strings. A door's role is `Boundary`;
its `EdgeFeature::Door` and portal approaches live separately in the scene description and are
not persisted in the prefab DTO. `scripts/lab/catalog.py:25–26` and Studio's door overlay instead
look for the string `Door`. Consequently a normal captured door becomes a wall socket and lacks
its approach overlay. Even a human object-role label is ignored by this catalog path.

A fixture using capture's actual `Boundary` label produced west socket **1 (wall)** instead of
**2 (door)**. Persist typed portal/boundary-feature evidence and consume it consistently in the
overlay, catalog and dataset. Test closed/open state, opposite anchors and a save/load round trip.

### P2 — Stepped fitting changes exact relative elevations

`crates/osrs-edit/src/prefab/foundation.rs:114–120` rounds each stepped support target to a multiple
of eight, but translates upper heights without the same rounding. On a source floor with heights
5 and 11, the copied six-unit step became eight units; a roof originally 240 units above its
support became 238 units above it. Edited semantic heights are allowed single-unit precision.

The metadata also has no explicit reviewed support groups, despite the original plan requiring
those for automatic stepped fitting. Preserve relative support heights exactly, and require/reify
support-group review rather than inferring it from one building-membership checkbox.

### P2 — New human attachment links are discarded during save

The Studio pair-link UI uses indices from live scene objects (`overlays.rs:110–147`). The save
adapter remaps them against the original prefab object list (`prefab/building/studio.rs:69–86`).
New pieces have no entries in that original list. A planner probe added a wall and decoration,
created their human attachment and saved both objects; the saved human relation count was zero.
Moves/reordering can similarly rebind or discard a live-index link.

Use a draft occurrence mapping tied to the current objects, with stable identity/remapping across
gestures. Test adding two pieces, linking them, save/reopen, moving one and undo/redo.

### P2 — Furniture and critic models consume held-out buildings

`scripts/lab/train.py:429–434` reads every dataset record into the placement/furniture statistics
and critic training, with no split/review/coverage filter. Role-classifier examples are split, but
these other learned artifacts are not. A record explicitly marked `split: test`, containing a
unique object ID, appeared in the exported furniture model.

Enforce one versioned group/split policy across dataset records, reviewed prefabs and task families.
Fit models only on eligible training groups; reserve validation/test for evaluation. Decide and
record whether weak/unreviewed examples are allowed for each model. Add a held-out marker test
for *every* trained artifact. Studio's prefab reconstruction task currently groups by display name,
which also needs a source/family identity shared across copies and rotations.

## Where the plan stands

| Area | Current implementation | Remaining acceptance work |
|---|---|---|
| Interaction correctness | Plane clearing, stamped outlines, roof-grid cache, boundary rendering and layered Escape exist | Native regression smoke on the supported caches; independent roof/picking visual checks |
| Building ownership | Sparse floors/support, exact occurrences, evidence and review controls exist | Persist portal/room/face semantics consistently; broader shared-wall, ambiguous-roof and region-edge corpus |
| Copy/relocate | V2 + v1 loading, sparse capture, foundation fitting and reversible patches exist | Fix bridge/step behavior; real-building four-rotation relocation and nearby-terrain preservation acceptance |
| Prefab Studio | Editing, history, bounds expansion, labels and relationship editing exist | Fix source-independent startup, live relationship identity and full save/navigation UI acceptance |
| Dataset | Neighbor context, exact roof/object occurrences, resolved heights, collision and provenance exist | Curated reviewed examples; coverage policy across floors, roofs and height samples; deterministic full export audit |
| WFC | 512 patterns, Shannon entropy, observed compatibility, rotations, pins and bounded repair exist | Fix apply authority and catalog replacement; catalog-to-wall/door/roof asset binding and macro/vertical/clearance validation |
| Lab tools | Reset/step, legal bounds/palette, history, replay, private observations, paths/heights/roof tools exist | Cross-task replay identity, scene/protocol consistency, task-family evaluation and protected-site tests |
| Local learning | Role model, symbolic catalog learner, furniture priors, critic and seeded tool search exist | Hold-out barriers, model integration and measured usefulness on reviewed real buildings; no trained neural policy is established |
| RuneLite/deob | Placement semantics, typed object parameters, roof overrides, unnamed/symbol cataloging and richer inspector exist | New evidence fixtures and independent acceptance; more client code does not replace building ownership or human corrections |

Roof-to-building links remain heuristic: roof pieces near a floor below and boundary proximity
provide evidence, not full architectural support inference. Furniture “AgainstWall” links use
anchor distance; they do not establish definition-translated face contact or clearance. Collision
snapshots in the dataset are useful inputs, but the current score is not a room/portal/reachability
or furnishing-clearance validator. These are explicit remaining semantics, not solved by a green
WFC solver or high symbolic reconstruction score.

## Evidence and limits

- Built the current `osrs-editor` and `osrs-lab` with the pinned toolchain: PASS.
- Inspected stored full gate `target/verify/gates-20261018-111757/report.json`: PASS, 1,260 tests
  passed / 0 failed / 22 ignored; native and WASM gates. It records HEAD `92477b00` with the then
  dirty search fix, not an invented full run on a later SHA.
- Inspected stored focused gate `target/verify/gates-20261018-124529/report.json`: PASS on exact
  `ada7c627`, world/editor scope, 394 passed / 0 failed / 20 ignored. It contains no WASM steps.
- Current native Studio smoke: **FAIL** on the pinned b241 source as described above. Startup
  failure prevented downstream Studio save/apply UI acceptance in that smoke.
- Ad hoc pure planner and Python boundary probes are under `target/building-audit-repros/`.
  `results.json` reproduces the door and held-out leakage; `foundation-results.txt` records bridge,
  step/roof and new human-link behavior. These demonstrate defects; they are not new green suites.
- The earlier bounded OpenRune LIVE export wrote nine buildings / 997 definitions. Its binary was
  built before the latest follow-on commits, so it is supporting pipeline evidence, not current
  whole-corpus acceptance.
- No new full suite was repeated against unchanged source, no remote CI was asserted, and ignored
  external-cache/visual prerequisites are not acceptance evidence.

## Next checkpoint order

1. Make the isolated native stage work with the normal sources and add a real startup/save smoke.
2. Fix bridge resolution, exact stepped elevation, and live human relationship identity.
3. Preserve typed door/portal evidence through capture, persistence and learned catalogs.
4. Route WFC application to authority; invalidate solutions on catalog replacement; test lifecycle.
5. Enforce held-out groups for every learned model, then curate real examples and evaluate quality.

Keep native AI training separate from the browser delivery requirement. Do not expand the model
or generator surface before these correctness and evaluation boundaries are verified.
