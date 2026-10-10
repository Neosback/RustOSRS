# RustOSRS Implementation Roadmap

Status: **Normative roadmap, reconciled through M10 Checkpoint 5 and the M10 foundation audit**  
Decision class: `PROJECT_PLAN`

RustOSRS is built correctness-first. A milestone is complete only when its implementation, exact verification, provenance, and documentation agree.

## 1. Roadmap rules

### 1.1 Earliest-layer correctness wins

Never compensate for an earlier-layer error in a later layer.

Examples:

- wrong model transform -> fix semantic model construction, not shader transforms;
- missing normal reconciliation -> fix scene finalization, not enhanced lighting;
- wrong bridge/min-plane state -> fix semantic scene state, not roof filtering;
- wrong priority ordering -> fix ordered preparation, not screenshot tolerance;
- missing terrain-color construction -> implement the semantic builder, not a renderer approximation.

### 1.2 Source verification is not implementation completion

A pinned source method can close a provenance blocker without closing the implementation row.

Current example:

`TERRAIN-004` is now `SOURCE_VERIFIED / IMPLEMENTATION_REQUIRED` because pinned `class470.method9712(WorldView)` contains the complete terrain builder. RustOSRS still must port it and pass an exact differential fixture before claiming terrain-color implementation parity.

### 1.3 Reference targets must be named

Do not conflate:

- pinned OSRS software/client semantics;
- imported RuneLite GPU behavior;
- RustOSRS Reference renderer policy;
- RustOSRS Enhanced editor presentation.

Where RustOSRS Reference intentionally uses the software/client ordering contract rather than RuneLite GPU's path-specific sorter, documentation and tests must say so explicitly.

### 1.4 Visual regression is not external parity

RustOSRS-generated Reference screenshots are internal regression evidence (`V3R`). External visual parity (`V3`) requires an independently generated, provenance-complete client/RuneLite oracle.

### 1.5 Milestone hard stops

Each milestone has one milestone branch and normally one final PR. Checkpoints are validated and reported without opening a PR. At milestone exit, audit exact branch-vs-main scope, run the full required tiers on the frozen head, open the PR, validate PR-triggered CI, then squash merge only if the exact PR state is clean.

## 2. Milestone status

| Milestone | Name | Current state |
|---|---|---|
| M0 | Workspace and CI foundation | MERGED |
| M1 | Target profile and cache contract | MERGED |
| M2 | `osrs-core` semantic foundation | MERGED |
| M3 | Cache transport and definition decoding | MERGED |
| M4 | Model decode and exact construction | MERGED |
| M5 | Reference fixture infrastructure | MERGED |
| M6 | Terrain topology, loc placement, planes | MERGED |
| M7 | Normals, lighting, scene finalization | MERGED |
| M8 | Morphs, contouring, animation ownership | MERGED |
| M9 | Semantic parity closure for first scope | MERGED, with post-M9 terrain provenance correction |
| M10 | Render extraction and headless renderer core | IN PROGRESS |
| M11 | First deterministic wgpu Reference viewport | NOT STARTED |
| M12 | Zones/materials/textures/priority/transparency GPU realization | NOT STARTED |
| M13 | Picking, diagnostics, GPU lifecycle | NOT STARTED |
| M14 | Native editor shell/document | NOT STARTED |
| M15 | Core editing tools/multi-region workflow | NOT STARTED |
| M16 | Persistence/autosave/recovery/export | NOT STARTED |
| M17 | Reference parity closure | NOT STARTED |
| M18 | Enhanced presentation/performance hardening | NOT STARTED |

## 3. M0-M5 foundation summary

M0-M5 established:

- Rust workspace and CI tiers;
- reusable `osrs-*` crate direction;
- build-241 target profile/cache fingerprint;
- `rune-fs 0.2.0` as private read-only transport under ADR-0010;
- canonical IDs/coordinates/provenance;
- revision-aware object/floor/map/loc/texture/sequence decoding;
- exact model decode/selection/mirroring/transform ownership;
- checked-in fixture infrastructure with regeneration safety and source manifests.

These milestones remain closed.

## 4. M6-M9 semantic scene closure summary

M6-M9 established:

- exact flat/shaped terrain topology;
- loc placement types/orientations/footprints/centers;
- source/collision/storage plane separation and structural link-below;
- base and cross-model normal reconciliation;
- exact target object lighting;
- morph/contour/verified legacy animation ownership;
- deterministic semantic scene hashes;
- parity matrix and target capability diagnostics.

### Post-M9 correction

M9 historically marked `TERRAIN-004` source-blocked. M10 foundation audit proved that conclusion wrong.

Current exact evidence:

- `class470.java` blob `1cd9cad5cb4be865dcae94dc633bba821644dc84`, `method9712(WorldView)` -> complete terrain builder;
- `FriendSystem.java` blob `b8cf51b6ee673181d8a115e28153a77f5978c390`, `addObjects(...)` -> placement-derived shadow-grid writes.

The historical M9 merge remains valid for its implemented scope. Current profile/parity state now says `source_verified`, not `blocked`.

## 5. Foundation correction gate before first Reference viewport

Because the terrain source blocker is gone and the renderer has not yet crossed into wgpu, the safest sequencing is to close the remaining semantic foundation gaps **before M11 claims a Reference viewport**.

Required correction work:

### F1. Implement `TERRAIN-004`

Port the pinned builder in the owning semantic layer, including:

- slope-light arithmetic;
- placement-derived shadow input;
- separable radius-5/11x11 underlay accumulation;
- weighted HSL operation order;
- overlay texture/magenta/secondary branches;
- deterministic non-jittered 3D corner colors;
- separately modeled jittered palette/minimap RGB if represented;
- exact `minPlane` output;
- builder ordering relative to normal finalization and link-below.

Required evidence:

- executable fixture generated from the exact pinned source revision;
- border/window arithmetic cases;
- clipped wall/game-object shadow cases;
- overlay branch cases;
- proof that 3D corner colors are independent of random presentation jitter.

### F2. Promote `minPlane` / `originalPlane`

Represent and exact-test the target scene fields needed by visibility/roof logic.

Required tests:

- ordinary, bit-8, and bridge min-plane cases;
- original-plane preservation through link-below movement;
- renderer grouping cannot mutate semantic hashes;
- RuneLite-style `maplevel` affects settings/roof lookup without rewriting semantic storage identity.

### F3. Preserve suppressed-face admission

The semantic lighting stage already emits `c == -2` for suppressed faces. M10 extraction exposes this state. Before a GPU draw-packet builder exists, add the exact admission rule that excludes such faces in Reference mode.

These are foundation corrections, not shader workarounds.

## 6. M10 current state: Render extraction and headless renderer core

Branch: `impl/m10-render-extraction-core`

Completed checkpoints:

### CP1: extraction/snapshot/rebase

- `osrs-render` crate boundary;
- immutable semantic generation/snapshot identity;
- integer-safe semantic placement and render-origin rebase;
- `RenderMesh` structural extraction;
- exact optional face metadata handoff.

### CP2: software priority and alpha preparation

- exact pinned software/client priority queue/threshold preparation;
- raw draw-alpha interpretation;
- no generic sort substitute.

### CP3: classification and draw plan

- explicit static/dynamic/ordered reasons;
- deterministic CPU pass grouping;
- classification based on render stability, not semantic object type.

### CP4: materials/texture preparation

- full-width semantic texture IDs;
- renderer-owned material handles;
- deterministic material table;
- canonical/explicit/projected model UV preparation;
- deterministic texture animation vector/tick preparation.

### CP5: zones/invalidation

- signed semantic 8x8 zone keys;
- negative-coordinate Euclidean partitioning;
- cross-zone footprint coverage;
- generation-aware dirty state;
- stale changed-zone work rejection;
- partial reuse of unaffected zones.

### M10 foundation audit

Corrected:

- terrain-source provenance;
- plane/min-plane/original-plane ownership;
- normal-merging research contradictions;
- software versus RuneLite GPU priority provenance;
- Reference depth compare from `GreaterEqual` to strict `Greater`;
- alpha depth-write assumptions;
- texture sampler/pixel evidence;
- visual-oracle policy;
- suppressed-face renderer boundary.

### M10 remaining structural work

After F1-F3 are closed or explicitly incorporated into the same milestone branch:

- picking ID mapping/generation ownership;
- structural diagnostics/provenance;
- final renderer-world/draw description integration;
- semantic-hash proof around grouping/classification;
- permanent M10 Tier D logic suite;
- final M10 branch audit and milestone PR.

M10 does **not** create a physical wgpu viewport.

## 7. M11: First deterministic wgpu Reference viewport

Purpose: first meaningful visible Reference frame.

Prerequisites:

- M10 merged;
- F1-F3 semantic foundation gates closed;
- no known P0/P1 error delegated to shader compensation.

Required work:

- wgpu device/surface/offscreen abstraction;
- Reference camera/view/projection;
- reverse-Z clear `0` with strict `Greater`;
- no-far projection equivalence fixture;
- CCW/backface convention;
- authored face-bias fixture at materially different distances;
- basic opaque terrain and model drawing;
- deterministic render tick;
- headless/offscreen test target;
- suppressed faces excluded before draw encoding.

Exit result: terrain/models can be viewed with canonical basic raster state. This is not yet full material/transparency parity.

## 8. M12: Production Reference materials and ordered rendering

Required work:

- GPU zone caches and rebuild lifecycle;
- texture-page/resource realization;
- imported-reference texture pixel construction;
- RGB-zero transparency and level-0 cutout;
- explicit Reference min/mag/wrap sampler behavior;
- brightness/textured-lightness path;
- deterministic texture animation;
- priority-sensitive ordered path;
- transparency ordering;
- explicit depth-write/no-depth behavior;
- static/dynamic consistency;
- authored bias in all relevant paths.

RuneLite-GPU comparison behavior, if implemented, must remain a separate profile from the software/client Reference priority contract.

## 9. M13: Picking, diagnostics, GPU lifecycle

Required work:

- integer picking target;
- generation-scoped pick map and stale readback rejection;
- face/tile/object provenance diagnostics;
- priority/alpha/bias/depth/material/plane/zone diagnostics;
- device/resource recreation;
- resource retirement/in-flight safety.

## 10. M14-M16: Editor and persistence

### M14

- eframe/egui native shell;
- semantic document ownership;
- command/history foundation;
- viewport integration without moving semantic ownership into UI code.

### M15

- loc/terrain/plane editing tools;
- multi-region workflow;
- previews/ghosts as disposable renderer/editor state;
- exact semantic command apply/revert.

### M16

- project save/load;
- autosave/recovery;
- target export validation;
- cache/profile mismatch detection;
- export round-trip tests for supported fields.

An unimplemented target semantic feature may block export/parity claims, but should be named by its actual implementation capability, not a stale "terrain source missing" state.

## 11. M17: Reference parity closure

Required closure:

- every release-required `PARITY-MATRIX.md` row reviewed;
- semantic golden hashes verified before image comparison;
- Reference priority/alpha/texture/bridge/morph/view gaps closed;
- deterministic internal Reference image suite (`V3R`);
- at least one independent external visual oracle (`V3`) before measured external 1:1 claims;
- diagnostics validated against intentionally injected faults.

`TERRAIN-004` must be implemented and fixture-backed before complete terrain-color parity can be claimed.

## 12. M18: Enhanced presentation and performance

Only after Reference correctness:

- optional MSAA/aniso/enhanced lighting/fog;
- profiling and batching improvements;
- release-quality UX;
- optional future GPU-driven optimization only when it preserves observable Reference contracts.

Enhanced presentation must never overwrite Reference semantic/render state.

## 13. Current next-work ordering

From the current M10 foundation-audit state, the recommended order is:

1. finish/validate the documentation/profile corrections;
2. close F1 `TERRAIN-004` with production implementation + differential fixture;
3. close F2 `minPlane/originalPlane` representation and tests;
4. close F3 suppressed-face draw admission structurally;
5. finish remaining M10 picking/diagnostics/headless integration;
6. perform frozen-head M10 milestone audit and PR;
7. only then begin M11 wgpu viewport work.

This ordering uses the new source information to strengthen the foundation before visible rendering can become an accidental oracle.