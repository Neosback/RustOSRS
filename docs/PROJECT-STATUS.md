# RustOSRS Current Project Status

Status: **Completion audit with completed ownership implementation**
Audit baseline: `c2f55192ea08435a6ca2df82a84a032f0493b30f`  
Audit date: 2026-10-15

This document is the current operational completion summary for RustOSRS. It does not replace semantic specifications, ADRs, or parity evidence. It answers a narrower question: **what is implemented now, what is still incomplete, and what should be worked next?**

For exact semantic authority use `docs/specs/`. For architecture decisions use `docs/adr/`. For parity proof use `docs/verification/PARITY-MATRIX.md`. For the latest exact branch/CI handoff use `DEV_HANDOFF.md`.

## 1. Current repository state at audit

At the audit baseline:

- active development branch: none;
- open pull requests: none;
- `main` was the only live branch;
- exact audited head: `c2f55192ea08435a6ca2df82a84a032f0493b30f`;
- the handoff-bearing baseline had passed Tier A, Tier B, and Tier C CI in run `38717265880` / CI #1320.

The repository is therefore at a clean subsystem boundary. No unfinished feature branch needs to be recovered before choosing the next subsystem.

## 2. What is implemented

### 2.1 Cache, semantics, and scene construction

Implemented and regression-covered:

- target cache/profile identity and guarded read-only source access;
- object, floor, map, location, texture, sequence, varbit, and varp decoding;
- exact model decode/construction, mirroring, transforms, contouring, normal generation, normal reconciliation, and target lighting;
- terrain topology and floor semantics;
- `TERRAIN-004` production terrain builder, including slope light, placement-derived shadows, radius-5 blending, overlay branches, minimum-plane output, and a pinned-client differential oracle;
- loc placement, plane/link-below behavior, semantic scene construction, morphing, and declared legacy animation scope;
- deterministic semantic scene identities and permanent M3-M9 semantic gates.

The old documentation statement that `TERRAIN-004` was only source-verified is obsolete. Commit `4e1757bded5952a18e51aef67981273dbf066c7c` implemented it with a real pinned-client oracle.

### 2.2 Reference rendering

Implemented:

- immutable scene-to-render extraction;
- signed 8x8 zone partitioning and generation-aware invalidation;
- packed Reference geometry;
- suppressed-face admission;
- opaque/alpha split;
- zone/level/min-plane grouping;
- headless reverse-Z wgpu rendering;
- real cache textures, mipmapped texture arrays, Reference sampler setup, cutout handling, brightness/lightness path, UVs, and texture animation;
- editor viewport integration;
- internal Reference pixel regression coverage.

This means M10-M12 are not "not started." The renderer exists and is used by the editor. What remains is **parity closure and stronger independent verification**, not creation of the first renderer.

### 2.3 Editor and semantic authoring

Implemented:

- native eframe/egui editor shell and wgpu viewport;
- fly/orbit-style navigation and presentation controls;
- picking, selection, object/terrain inspection, outlines and overlays;
- semantic `EditRuntime` execute/replay gateway;
- Paint, Height, Paste, Path, Object edits, Undo, and Redo routed through semantic authority;
- transaction/history semantics where one logical gesture produces at most one semantic history entry;
- multi-region streaming and contextual rebuild behavior;
- stale worker result rejection;
- exact 8x8 pure-terrain invalidation;
- dependency-proven Height invalidation with conservative fallback;
- accepted producer-output Object invalidation with conservative fallback;
- project save/open using versioned region diffs;
- byte-exact terrain/location encoders for representable map data.

### 2.4 Verified export and inspection

Implemented:

- immutable export preparation over semantic authority plus pending project state;
- strict preflight validation;
- semantic encode/decode round-trip validation before publication;
- preservation of map-group files beyond terrain/location files 0 and 1;
- staged fail-closed publication;
- JS5 existing-group repack with CRC/size/version/reference-table maintenance;
- `rustosrs-map-export-v2` manifest;
- independent output verification;
- private native-only `VerifiedExport` capability;
- isolated read-only inspection of a specifically verified edited cache;
- protection against stale jobs, tampering, file mutation after validation, occupied destinations, and version overflow.

Current export limitations are intentional and must not be hidden:

- only existing map-square groups can be repacked;
- new groups absent from the source reference table are not created;
- editor export currently rewrites map index 5 only;
- reference-table rows carrying hash/Whirlpool columns fail closed;
- an edited cache intentionally does not pass ordinary pristine profile matching;
- `VerifiedExport` read-through is native-only;
- the real build-241 end-to-end cache-copy acceptance test requires an external pinned cache and remains ignored in ordinary CI.

### 2.5 117HD port

Substantial 117HD functionality is already implemented:

- Reference/HD render-profile split and HDR frame graph;
- vendored 117HD data/resources with attribution;
- material, ground-material, tile/model override, area, environment, water, sky-preset, light, and gameval data loading;
- HD terrain/model extraction and material routing;
- diffuse/normal/specular/AO material paths;
- ground blending and terrain normals;
- environments, fog, directional lighting, and directional shadows with filtering;
- water surfaces, water types, flow/normal maps, foam, Fresnel, underwater tint, and caustics;
- animated-loc HD material routing;
- material flow maps;
- parallax/displacement with displaced shadow lookup.

The 117HD port is advanced, but it is not complete. See section 4.

## 3. Current ownership implementation

The user-authorized ownership refinement is implemented on `Neosback/editor-ownership`, based on main merge `d7db98cd179fe8d64b2e743f74d8aefa9a955449`. The user has authorized commit and merge; see `DEV_HANDOFF.md` for current integration and exact validation status. This updates the earlier audit's architecture assessment; it does not recertify its historical branch/CI metadata.

- Paint/Height/Path planners and their tests live in the headless edit crate.
- `InteractionRuntime` has private state and explicit lifecycle operations, including camera and spline preview behavior.
- `ProjectRuntime`, `UiRuntime`, `DiagnosticsRuntime`, and dedicated settings/plugin/palette owners are established.
- World-owned CPU picking/outline projections and the explicit terrain preview/history-routing adapter reduce unrelated application state.
- Compiler privacy and CI guards enforce dependency, application-surface and feature/test navigation contracts.

The existing terrain preview/history-routing adapter remains temporary. The app still coordinates cross-owner effects through enumerated orchestration methods; future extractions lower the guard baseline. These known adapters are documented in `docs/architecture/FEATURE-MAP.md`. Renderer parity, HD completeness and export limitations below are unaffected.

## 4. Functional work still incomplete

### 4.1 Reference renderer parity closure

The Reference renderer exists, but complete externally demonstrated 1:1 parity is not yet proven.

Remaining closure work includes:

- side-grid/placement-shadow edge exactness where current fixtures are not exhaustive;
- remaining `originalPlane`/visibility/roof grouping edge cases around the already implemented min-plane path;
- exact GPU depth/raster behavior fixtures, including distance-sensitive bias cases;
- exact alpha/depth-write/no-depth behavior across all relevant modes;
- exact texture/sampler behavior verification against the selected reference path rather than relying only on internal regression;
- at least one independently produced external visual oracle;
- measured external visual gates before any broad "1:1" rendering claim.

Internal RustOSRS screenshots are useful regression evidence, but they are not independent external parity evidence.

### 4.2 CI/parity hardening

The CI system is strong but its explicit milestone semantic gate matrix has not caught up with the implementation.

Current Tier C jobs explicitly name M3-M9 semantic gates. Later renderer/editor/export/117HD behavior is exercised primarily through workspace tests and specialized test suites, but is not represented as an equally explicit M10+ certification matrix.

Remaining work:

- add permanent M10+ Reference renderer parity gates;
- add dedicated editor semantic-authority/projection acceptance gates;
- add verified-export acceptance gates to the release certification story;
- add 117HD phase-level regression gates appropriate to a stylized profile;
- decide how the external build-241 cache acceptance test is supplied in release CI without making ordinary contributor CI depend on a private/local cache;
- retain exact-SHA certification discipline.

### 4.3 117HD remaining work

Highest-confidence remaining 117HD systems:

1. **dynamic/placed lights**: `lights.json` placement, tiled/clustered light culling or equivalent, and initial editor/static-light scope;
2. **sky/day-night system**: sky shader, starfield, moon, aurora, nebula/seasonal behavior, time-of-day transitions;
3. **sky/reflection integration**: reflected sky/environment behavior in water and remaining atmosphere coupling;
4. **post/settings completion**: remaining tone/color/filter/AA/settings parity and UX;
5. **wind and character displacement**;
6. remaining material/model edge behavior and visual parity cleanup.

117HD should remain a separate enhanced presentation profile. It must not become the oracle for Reference correctness.

### 4.4 Cache/export generalization

Remaining exporter/cache-product work:

- create new map groups that do not exist in the source reference table;
- generalize repacking beyond the currently supported map-index use case where product requirements justify it;
- support or deliberately define policy for hash/Whirlpool reference-table rows;
- define a future ordinary edited-cache identity/profile strategy if edited outputs should eventually open without a private verified capability;
- preserve the current fail-closed staging/verification guarantees while generalizing.

### 4.5 Project lifecycle and recovery

Core project save/open exists. The original M16 roadmap also called for autosave/recovery.

No completed autosave/recovery subsystem was identified in this audit. Until implemented and tested, M16 should be described as **core persistence/export implemented, recovery lifecycle incomplete**.

Additional product work may include:

- autosave and crash recovery;
- clearer dirty-state/session restoration UX;
- project browser/lifecycle polish;
- future import/merge workflows and conflict policy.

## 5. Milestone reconciliation

The original M0-M18 roadmap is retained as the historical dependency model, but its old progress labels were no longer accurate.

Current interpretation:

| Milestone | Current disposition |
|---|---|
| M0-M9 | Complete and merged, with later provenance/terrain corrections incorporated |
| M10 | Core render extraction/headless renderer implemented; later parity proof belongs to M17 |
| M11 | Deterministic wgpu Reference viewport implemented |
| M12 | Production Reference material/texture/zone paths implemented; parity edge closure remains |
| M13 | Picking/diagnostics/GPU/editor lifecycle baseline implemented; continued hardening remains |
| M14 | Native editor shell/semantic document implemented |
| M15 | Core semantic editing and multi-region workflow implemented and subsequently hardened |
| M16 | Project save/open and verified export implemented; autosave/recovery and some product lifecycle work remain |
| M17 | In progress: Reference parity closure and explicit later-stage certification gates |
| M18 | In progress: 117HD/enhanced presentation, performance and release-quality completion |

A milestone being implemented does not imply every originally imagined polish item or parity oracle has been closed.

## 6. Recommended completion order

The safest next sequence is:

Ownership extraction for interaction, project, UI and diagnostics is implemented (section 3). After its merge closeout, continue with:

1. **Reference parity closure + external visual oracle**, including exact raster/alpha/plane/shadow/sampler edge gates.
2. **M10+ CI certification expansion** so later systems have permanent named gates comparable to M3-M9.
3. **117HD dynamic/placed lights**.
4. **117HD sky/day-night/reflection/post/settings completion**.
5. **Cache/export generalization** for new groups and broader reference-table/index cases.
6. **Autosave/recovery, project/browser lifecycle polish, and future import/merge workflows**.
7. **Release acceptance** on the pinned real cache plus final documentation/parity audit.

Do not mix these into one branch. Follow the one-active-subsystem rule in `DEV_HANDOFF.md`.

## 7. Definition of project completion

RustOSRS should not be called broadly complete until all of the following are true:

- semantic/cache/model/scene contracts required by the supported target profile are green;
- Reference renderer parity gaps have either exact proof or an explicitly accepted non-parity disposition;
- an independent external visual oracle exists for release-required Reference scenes;
- M10+ renderer/editor/export behavior has durable CI certification rather than relying only on broad workspace tests;
- editor runtime ownership no longer leaves major subsystem behavior accumulated in the composition root;
- project persistence includes the intended recovery story or the requirement is explicitly removed by decision;
- supported exports are independently verifiable and their unsupported cases are clearly rejected;
- the intended 117HD feature scope is finished and separately regression-tested;
- pinned real-cache release acceptance passes;
- `DEV_HANDOFF.md`, this status file, the roadmap, and parity matrix agree on the release state.

## 8. Documentation maintenance rule

When current implementation advances:

1. update the owning subsystem plan/handoff during the work;
2. update `DEV_HANDOFF.md` at subsystem closure;
3. reconcile this file and `17-IMPLEMENTATION-ROADMAP.md` when completion scope changes materially;
4. update `PARITY-MATRIX.md` only when evidence/proof state changes, not merely because a feature exists;
5. never leave a historical "NOT STARTED" label on a system that is already production code.
