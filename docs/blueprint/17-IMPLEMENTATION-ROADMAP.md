# RustOSRS Implementation Roadmap

Status: **Normative roadmap, reconciled to the 2026-10-15 implementation audit**  
Decision class: `PROJECT_PLAN`

RustOSRS is built correctness-first. A capability is complete only when implementation, verification, provenance, and documentation agree.

For the latest exact branch/SHA/CI state, read `DEV_HANDOFF.md`. For the current project-wide completion audit, read `docs/PROJECT-STATUS.md`.

## 1. Roadmap rules

### 1.1 Earliest-layer correctness wins

Never compensate for an earlier-layer error in a later layer.

Examples:

- wrong model transform -> fix semantic model construction, not shader transforms;
- missing normal reconciliation -> fix scene finalization, not enhanced lighting;
- wrong bridge/plane state -> fix semantic scene state, not roof filtering;
- wrong priority ordering -> fix ordered preparation, not screenshot tolerance;
- wrong terrain construction -> fix the semantic terrain builder, not a renderer approximation.

### 1.2 Feature existence is not parity proof

The current repository contains a working Reference renderer, native editor, semantic authoring, projects, and verified export. Their existence does not by itself prove complete external 1:1 parity.

Internal RustOSRS image goldens are regression evidence. Independent external visual parity requires a separately generated and provenance-complete oracle.

### 1.3 Reference targets must be named

Do not conflate:

- pinned OSRS client/software semantics;
- imported RuneLite GPU behavior;
- RustOSRS Reference renderer policy;
- RustOSRS HD/Enhanced presentation.

Where RustOSRS Reference intentionally follows one path, tests and documentation must identify that path explicitly.

### 1.4 One active subsystem

Follow `DEV_HANDOFF.md`:

- exactly one active development subsystem/branch at a time;
- finish and certify it before opening the next branch;
- do not mix architecture, Reference parity, 117HD, cache/export, and unrelated UI work;
- certify the exact SHA being merged and the exact integrated `main` SHA afterward.

## 2. Milestone reconciliation

The M0-M18 model remains useful as a dependency history, but the old progress labels became obsolete after implementation advanced rapidly beyond M10.

| Milestone | Name | Current disposition |
|---|---|---|
| M0 | Workspace and CI foundation | COMPLETE / MERGED |
| M1 | Target profile and cache contract | COMPLETE / MERGED |
| M2 | `osrs-core` semantic foundation | COMPLETE / MERGED |
| M3 | Cache transport and definition decoding | COMPLETE / MERGED |
| M4 | Model decode and exact construction | COMPLETE / MERGED |
| M5 | Reference fixture infrastructure | COMPLETE / MERGED |
| M6 | Terrain topology, loc placement, planes | COMPLETE / MERGED |
| M7 | Normals, lighting, scene finalization | COMPLETE / MERGED |
| M8 | Morphs, contouring, animation ownership | COMPLETE / MERGED within declared scope |
| M9 | Semantic parity closure for first scope | COMPLETE / MERGED, with later terrain/provenance corrections incorporated |
| M10 | Render extraction and headless renderer core | IMPLEMENTED; later parity proof rolls into M17 |
| M11 | Deterministic wgpu Reference viewport | IMPLEMENTED |
| M12 | Zones/materials/textures/priority/transparency | IMPLEMENTED baseline; parity edge closure remains |
| M13 | Picking, diagnostics, GPU lifecycle | IMPLEMENTED baseline; continued hardening remains |
| M14 | Native editor shell/document | IMPLEMENTED |
| M15 | Core semantic editing/multi-region workflow | IMPLEMENTED and subsequently hardened |
| M16 | Persistence/recovery/export | PARTIAL: project save/open + verified export implemented; autosave/recovery incomplete |
| M17 | Reference parity closure | IN PROGRESS |
| M18 | Enhanced presentation/performance | IN PROGRESS; 117HD substantially implemented |

## 3. Foundation corrections that are now closed

The prior roadmap listed several blockers before a Reference viewport. They are no longer open.

### `TERRAIN-004`

Closed by production implementation and a pinned-client differential oracle. Commit `4e1757bded5952a18e51aef67981273dbf066c7c` ported the terrain loader/builder, slope lighting, placement-shadow subtraction, radius-5 blending, overlay branches, tile emission, and minimum-plane behavior.

### Placement shadows and scene ordering

The cache-backed world path now performs loc placement, clipped shadow writes, terrain-color construction, normal reconciliation, and link-below ordering in the intended construction sequence.

### Suppressed-face admission and min-plane grouping

The Reference geometry path excludes suppressed faces and groups geometry by zone/level/min-plane before rendering. Commit `210d279e42bf94bcaa87da7982f3a90951abaf81` introduced the packed Reference path and headless reverse-Z wgpu renderer.

These items must not be reintroduced as "not implemented" gates in current documentation.

## 4. Current implemented platform

### 4.1 Semantic/cache foundation

Implemented:

- target-profile and cache fingerprint safeguards;
- target-aware cache transport;
- definition/map/location/texture/sequence/var decoding;
- exact model construction and lifecycle semantics;
- terrain construction and colors;
- loc placement and side effects;
- normals, cross-model reconciliation and lighting;
- morphing, contouring and declared legacy animation execution;
- deterministic semantic scene identities.

### 4.2 Reference renderer

Implemented:

- immutable extraction and generation identity;
- 8x8 zones and dirty-state tracking;
- packed Reference geometry;
- opaque/alpha paths;
- priority/face preparation;
- real cache textures, mip levels, sampler setup, cutout and animation;
- reverse-Z wgpu renderer and offscreen/headless rendering;
- editor viewport integration;
- internal pixel regression coverage.

### 4.3 Editor/authoring

Implemented:

- eframe/egui native editor;
- viewport navigation and presentation controls;
- picking, selection, outlines and inspection;
- semantic command/history authority;
- Paint, Height, Paste, Path, Object edits plus semantic Undo/Redo;
- multi-region streaming;
- stale async build rejection;
- zone-scoped terrain invalidation;
- dependency-proven Height invalidation with safe fallback;
- accepted-output Object invalidation with safe fallback;
- project save/open and diff projection.

### 4.4 Verified export

Implemented:

- strict immutable export preparation;
- representability/preflight checks;
- semantic encode/decode verification;
- staged publication;
- existing-group JS5 repack and reference-table metadata updates;
- manifest-backed independent verification;
- preservation of extra map-group files;
- isolated read-only inspection through `VerifiedExport`;
- tamper/stale-job/concurrent-destination/version-overflow protections.

## 5. Active completion program

The remaining project work is no longer "build M10 then start the editor." It is the following completion program.

### A. Runtime ownership extraction

Current boundaries already include `EditRuntime`, `RenderRuntime`, `WorldRuntime`, and `InteractionRuntime` field ownership.

Remaining:

1. `InteractionRuntime` behavioral encapsulation;
2. `ProjectRuntime` ownership extraction;
3. `UiRuntime` ownership extraction;
4. `DiagnosticsRuntime` ownership extraction.

Exit condition: `EditorApp` remains a composition/orchestration root instead of re-accumulating subsystem state and behavior.

### B. Reference parity closure

Remaining closure work includes:

- exhaustive placement/side-grid shadow edge verification;
- remaining original-plane/roof/visibility grouping edge cases around the implemented min-plane path;
- exact raster/depth/bias fixtures;
- exact alpha/depth-write/no-depth cases;
- exact texture/sampler verification against the selected reference path;
- at least one independent external visual oracle;
- measured external gates before any broad 1:1 claim.

Exit condition: every release-required Reference behavior has exact/tolerance ownership and external visual proof where required.

### C. CI/certification expansion

Current Tier C naming is strongest for M3-M9. Later renderer/editor/export/HD behavior is heavily tested, but the explicit certification matrix has not caught up.

Remaining:

- named permanent M10+ Reference gates;
- semantic-authority/projection acceptance gates for editor mutation paths;
- verified-export acceptance in the release certification story;
- suitable 117HD phase regression gates;
- a supported way to run pinned real-cache release acceptance.

Exit condition: later systems have first-class durable certification, not only broad workspace coverage.

### D. 117HD completion

Already implemented includes HD materials, terrain/model extraction, ground blending/normals, environments/fog, directional lighting/shadows, water/flow/normal/foam/Fresnel/underwater/caustics, animated-loc material routing, material flow maps, and parallax/displacement.

Remaining high-confidence scope:

1. placed/dynamic point lights;
2. sky/star/moon/aurora/day-night systems;
3. sky/reflection and remaining atmosphere/water integration;
4. remaining post-processing/settings parity and UX;
5. wind/character displacement;
6. remaining material/model edge behavior and visual cleanup.

Exit condition: the intentionally supported 117HD scope is feature-complete and separately regression-tested without weakening Reference behavior.

### E. Cache/export generalization

Current verified export intentionally supports a restricted safe scope.

Remaining:

- create new map groups absent from the source reference table;
- broader index/repack support when product requirements need it;
- policy/support for hash/Whirlpool reference-table rows;
- future edited-cache identity/profile strategy beyond the private verified capability, if desired;
- real pinned-cache release acceptance.

Exit condition: every advertised export class is independently verifiable and unsupported classes fail explicitly.

### F. Project lifecycle/recovery

Project save/open is implemented. Autosave/recovery was part of the original M16 scope but was not identified as completed in the 2026-10-15 audit.

Remaining:

- autosave and crash recovery;
- session/dirty-state restoration UX;
- project/browser lifecycle polish;
- future import/merge/conflict workflow if included in product scope.

Exit condition: the persistence requirement is either implemented/tested or formally narrowed by an explicit decision.

## 6. Recommended next-work order

From the clean `main` boundary recorded in the 2026-10-15 handoff:

1. `InteractionRuntime` behavioral encapsulation;
2. `ProjectRuntime` extraction;
3. `UiRuntime` extraction;
4. `DiagnosticsRuntime` extraction;
5. Reference parity closure + external visual oracle;
6. explicit M10+ CI/certification gates;
7. 117HD dynamic/placed lights;
8. 117HD sky/day-night/reflection/post/settings remainder;
9. cache/export generalization;
10. autosave/recovery and project lifecycle/import/merge polish;
11. final pinned-cache release acceptance and documentation reconciliation.

Each numbered subsystem should normally be its own branch/PR sequence under the handoff protocol.

## 7. Project completion gate

Do not call RustOSRS broadly complete until:

- required semantic contracts are green for the supported target profile;
- Reference renderer parity gaps are closed or explicitly dispositioned;
- an independent external visual oracle exists for required Reference scenes;
- M10+ systems have durable CI certification;
- runtime ownership extraction is complete enough that `EditorApp` is not the behavioral owner of major subsystems;
- the intended persistence/recovery story is complete or formally narrowed;
- advertised export classes are verifiable and unsupported cases fail closed;
- intended 117HD scope is complete and regression-tested;
- real pinned-cache release acceptance passes;
- `DEV_HANDOFF.md`, `docs/PROJECT-STATUS.md`, this roadmap, and `PARITY-MATRIX.md` agree.

## 8. Historical note

Earlier revisions of this roadmap accurately described the state before the October 10-15 implementation surge, including an M10 branch and pre-wgpu foundation gates. Git history preserves those plans. They are historical evidence, not the current work queue.
