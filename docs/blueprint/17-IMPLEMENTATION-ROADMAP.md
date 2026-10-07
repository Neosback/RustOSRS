# RustOSRS Implementation Roadmap

Status: **Checkpoint 8 normative implementation roadmap**  
Decision class: `PROJECT_PLAN`

This roadmap converts the architecture, semantic specifications, renderer/editor blueprints, and verification plan into a dependency-ordered implementation sequence.

It begins from the current repository state, where the project is primarily a source/reference/blueprint corpus rather than a Rust application.

The roadmap is intentionally correctness-first. A milestone may produce code that compiles or renders something and still be **incomplete** if its owning semantic or verification gates have not passed.

## 1. Roadmap rules

### 1.1 Completion is evidence-based

A milestone is complete only when:

1. its listed deliverables exist;
2. its owned specs/ADRs have implementation coverage;
3. required verification rows have advanced to the milestone's required state;
4. no milestone-blocking contradiction remains unresolved;
5. documentation/public API examples reflect the implemented behavior.

### 1.2 Earliest-layer correctness wins

If a failure exists at P0/P1/P2, do not compensate at a later layer.

Examples:

- wrong model orientation -> fix semantic transform code, not shader rotation;
- missing normal merge -> fix scene finalization, not enhanced lighting;
- wrong bridge plane -> fix semantic scene data, not viewport filtering;
- wrong priority emission -> fix ordered render path, not image tolerance.

### 1.3 Renderer starts after semantic foundations

The first wgpu viewport is deliberately not an early milestone.

Before the viewport milestone, RustOSRS must already have working foundations for:

- target/profile identity;
- canonical coordinates and IDs;
- object/model/floor/location decoding needed by the first scene;
- exact model transforms;
- loc placement and planes;
- scene normal reconciliation and reference lighting;
- executable differential fixtures for the highest-risk semantic contracts.

This prevents a visually plausible renderer from becoming the de facto semantic oracle.

### 1.4 `TERRAIN-004` remains a gate, not guessed work

The full terrain-color builder remains `REVISION_SENSITIVE` until its source/oracle is reproducibly closed.

Milestones may implement terrain topology, floor definitions, shaped/flat surfaces, editing, and renderer plumbing without falsely claiming complete reference terrain-color parity.

The final Reference visual parity milestone cannot claim complete terrain-color parity while this gate remains open.

### 1.5 Dependency decisions use explicit gates

Where the blueprint intentionally left a dependency choice open, implementation performs a bounded spike and records an ADR before broad adoption.

Primary example: `rs-cache` may be wrapped, extended, forked, partially reused, or replaced. The roadmap does not treat the old hybrid research recommendation as already accepted architecture.

## 2. Milestone summary

| Milestone | Name | Primary output |
|---|---|---|
| M0 | Workspace and CI foundation | compiling Rust workspace + verification skeleton |
| M1 | Target profile and cache contract | exact initial target/profile + cache dependency ADR |
| M2 | `osrs-core` semantic foundation | IDs, coordinates, definitions, model representation, exact helpers |
| M3 | Cache transport and definition decoding | cache source/archive + object/floor/var/location/map decode |
| M4 | Model decode and exact model construction | ModelData decode, selection, mirror, transforms, ownership |
| M5 | Reference fixture infrastructure | source-manifest fixtures + Rust differential runner |
| M6 | Terrain, loc placement, and plane scene construction | first exact semantic region scene |
| M7 | Normals, lighting, and scene finalization | cross-model normal reconciliation + final reference models |
| M8 | Morphs, contouring, and animation ownership | dynamic semantic model resolution |
| M9 | Semantic parity closure for first target | P0/P1 regression suite for implemented scope |
| M10 | Render extraction and headless renderer core | semantic snapshot -> renderer-owned structural artifacts |
| M11 | First deterministic wgpu Reference viewport | correct basic terrain/models with canonical raster state |
| M12 | Zones, materials, textures, priority, and transparency | production reference rendering pipeline |
| M13 | Picking, diagnostics, and GPU lifecycle | selection IDs, debug views, resource/device rebuild |
| M14 | Native editor shell and semantic document | eframe/egui/Catppuccin application + command history |
| M15 | Core editing tools and multi-region workflow | loc/terrain tools, planes, region boundaries, previews |
| M16 | Project persistence, autosave, recovery, and export | durable project workflow + validated target export |
| M17 | Reference parity closure | composed golden scenes + canonical GPU P3 suite |
| M18 | Enhanced presentation and performance hardening | enhanced profile, profiling, release-quality UX |

Milestones are ordered by dependency. Small implementation PRs may subdivide a milestone, but a later milestone must not silently assume an unmet earlier exit gate.

---

# M0 - Workspace and CI foundation

## Purpose

Create the smallest real Rust workspace that can enforce the blueprint boundaries and verification discipline.

## Prerequisites

- blueprint Checkpoints 0-8;
- ADR-0001 through ADR-0009.

## Create initially

Recommended first crates:

```text
Cargo.toml
crates/
  osrs-core/
  osrs-cache/
  osrs-scene/
  osrs-reference/
```

`osrs-render` and `osrs-editor` may be created empty only if useful for workspace wiring, but meaningful implementation waits for their owning milestones.

## Deliverables

- workspace `Cargo.toml`;
- pinned Rust toolchain policy;
- formatting/lint policy;
- baseline error/provenance conventions;
- basic CI Tier A + empty/fast Tier B wiring;
- test fixture directory structure from `docs/verification/REFERENCE-FIXTURES.md`;
- crate-level README/module documentation establishing allowed dependencies;
- architecture check preventing production crates from depending on `osrs-reference`.

## Exit gates

- workspace checks cleanly;
- `cargo fmt --check`, `cargo clippy`, and workspace tests are wired;
- dependency direction matches ADR-0001;
- no egui/wgpu/cache transport types leak into `osrs-core`;
- verification fixture manifests can be parsed even before substantive fixtures exist.

## Explicit non-goal

Do not add placeholder semantic APIs merely to make future crates compile.

---

# M1 - Target profile and cache contract

## Purpose

Select the initial implementation target and decide how cache transport/decoding will be sourced without locking the semantic model to an old dependency.

## Prerequisites

M0.

## Required work

Define the initial `TargetProfile` contract including:

- target cache/source identity;
- cache fingerprint strategy;
- decoder schema version;
- semantic source/reference pins;
- revision gates;
- XTEA/key ownership boundary;
- decoded-artifact invalidation identity.

Perform a bounded compatibility spike against:

- `rs-cache-master/`;
- `OpenRune-FileStore-main/`;
- the canonical semantic field requirements in `docs/specs/`.

The spike must explicitly test at least:

- location/map loading;
- object definitions including varbit/varp transforms;
- 32-bit/newer model-id paths required by target data;
- underlay/overlay fields;
- model data availability/absence;
- texture/material availability/absence;
- sequence/animation availability/absence;
- error/provenance quality;
- ability to preserve target-revision distinctions.

## Decision output

Create an ADR selecting one of:

1. wrap and extend `rs-cache`;
2. fork/patch `rs-cache` behind `osrs-cache`;
3. use only its low-level transport while replacing decoders;
4. implement transport/decoders independently;
5. another explicitly justified combination.

OpenRune FileStore remains independent evidence/tooling, not semantic authority.

## Exit gates

- initial target profile is explicit and reproducible;
- cache dependency ADR accepted;
- C-010's **planning/dependency-selection gap** is closed by an actual decision;
- decoder acceptance checklist exists for every semantic field needed by M3-M8;
- no old `rev 180`, `TEXTURE_COUNT=256`, or local-path assumption is silently treated as target truth.

---

# M2 - `osrs-core` semantic foundation

## Purpose

Implement renderer/cache/editor-independent types and exact pure algorithms.

## Prerequisites

M1 target/profile contract.

## Deliverables

### Identity and coordinates

Implement stable types/concepts for:

- definition/model/texture/sequence/varbit IDs;
- region/world tile coordinates;
- semantic local coordinates (`128` units/tile);
- model-local coordinates;
- source/storage/collision/render-level distinctions where the lowest shared type is appropriate;
- target/profile provenance.

### Canonical definitions

Create canonical Rust representations for fields needed by:

- object definitions;
- underlays/overlays;
- textures/material inputs;
- varbits/varps;
- sequences sufficient for later milestone ownership.

### Model representation

Create canonical model structures preserving:

- vertices/faces;
- face render type;
- priority/default priority;
- alpha;
- texture id;
- texture-face selector and texture triangles;
- authored bias;
- recolor/retexture inputs;
- transform/animation groups where needed;
- normals/merged-normal working state in the appropriate semantic representation.

## Owned specs

Initial direct ownership includes:

- `COORD-001`;
- `COORD-002` helper conventions;
- `COORD-003` space separation;
- `FACE-001` representation requirements;
- shared type support required by all later semantic specs.

## Verification exit gates

- exact coordinate round trips;
- tile constants and centers exact;
- no semantic IDs represented only by untyped renderer integers;
- optional face arrays preserve absence/default semantics;
- source model structures are hashable/comparable enough for immutability tests;
- Tier B exact tests pass.

---

# M3 - Cache transport and definition decoding

## Purpose

Turn target cache bytes into canonical `osrs-core` values without scene behavior leaking into the decoder.

## Prerequisites

M1 dependency ADR; M2 canonical types.

## Implement

- cache source/archive/container reading;
- decompression and checksums needed by target;
- XTEA application for location archives;
- target/profile detection/fingerprint;
- object definition decode;
- varbit/varp decode required for transforms;
- floor underlay/overlay decode;
- texture definition/material-input decode required by renderer later;
- landscape/terrain raw decode;
- location stream decode;
- sequence metadata needed by M8;
- contextual decoder errors.

## Owned specs

- `TERRAIN-003`;
- decode prerequisites for `MORPH-001`, `TEXTURE-001`, `LOC-PLACEMENT-*`;
- P0 side of canonical fields named by `FACE-001`.

## Required tests

- checked-in synthetic/source-pinned decode fixtures;
- overlay defaults/opcodes and secondary color;
- underlay HSL/hue-multiplier boundaries;
- varbit/varp transform tables including fallback/null;
- location stream coordinates/types/orientations;
- malformed count/opcode errors;
- bounded allocation properties;
- fuzz smoke for definition/map/location decoders.

## Exit gates

- P0 exact fixtures exist for implemented formats;
- unsupported revision opcodes fail explicitly;
- no decoder silently drops fields required by later specs;
- cache/source fingerprint participates in decoded-artifact identity;
- Tier A-C relevant decode suites pass.

---

# M4 - Model decode and exact model construction

## Purpose

Implement raw model decoding and all audited construction rules before any GPU model exists.

## Prerequisites

M2 model representation; M3 cache access.

## Implement

- target ModelData binary decode;
- raw model cache keyed with revision/profile identity;
- typed/untyped model selection;
- model combination;
- mirror variant construction and winding;
- orientation transforms;
- type-4 diagonal decoration recenter;
- recolor;
- retexture;
- resize;
- translation;
- copy-on-write/structural-sharing ownership rules.

## Owned specs

- `MODEL-BUILD-001`;
- `MODEL-BUILD-002`;
- `MODEL-BUILD-003`;
- foundation for `MODEL-BUILD-004/005`;
- `COORD-002` exact transform coverage;
- P0/P1 portions of `FACE-001`.

## Exit gates

The following verification rows must be `EXISTING`:

- `MODEL-BUILD-001`;
- `MODEL-BUILD-002`;
- `MODEL-BUILD-003`;
- `COORD-002`.

Also require:

- exact integer vertex equality;
- mirror/winding differential fixture;
- combined transform-order fixture;
- two-instance source immutability control;
- model decode fuzz smoke.

No wgpu mesh is an acceptance artifact for this milestone.

---

# M5 - Reference fixture infrastructure

## Purpose

Make `osrs-reference` a real development/test tool before the most dangerous scene semantics are ported.

## Prerequisites

M0 fixture skeleton; enough M2-M4 code to consume model fixtures.

## Implement

- fixture manifest parser;
- normalized fixture input/output schemas;
- exact comparison runner;
- source/harness provenance validation;
- regeneration command that is separate from ordinary tests;
- adapters/scripts for the pinned public deob/reference sources where practical;
- migration/indexing of useful cases from `deob_golden.txt` without falsifying its historical provenance.

## First required fixture families

- model selection;
- mirror/winding;
- transform order;
- base normals;
- normal merge controls;
- lighting controls;
- loc dispatch/orientation;
- plane/bridge synthetic inputs;
- priority order crafted model.

## Exit gates

- ordinary Rust CI runs offline against checked-in expected outputs;
- every new fixture has an exact provenance manifest;
- fixture regeneration cannot silently rewrite expected outputs without manifest/source changes;
- relevant `PARITY-MATRIX.md` rows link to concrete fixture/test identifiers.

---

# M6 - Terrain, loc placement, and plane scene construction

## Purpose

Construct the first trustworthy semantic region scene.

## Prerequisites

M3 definitions/maps/locations; M4 model construction; M5 differential infrastructure.

## Implement in `osrs-scene`

- semantic tile grid/storage;
- flat and shaped terrain topology;
- exact `SceneTileModel` shapes `0..12` and rotations;
- loc type `0..22` and `>=12` dispatch;
- wall/decor orientation/displacement tables;
- footprint/center sampling;
- floor decoration placement with no generic lift;
- source/collision/storage/render-level concepts;
- collision-plane bridge adjustment;
- structural link-below processing;
- scene side-effect scaffolding for collision/clipping/occlusion metadata, only as supported by promoted contracts;
- semantic scene query APIs.

## Owned specs

- `TERRAIN-001`;
- `TERRAIN-002`;
- `LOC-PLACEMENT-001`;
- `LOC-PLACEMENT-002`;
- `LOC-PLACEMENT-003`;
- `LOC-PLACEMENT-005` incrementally;
- `LOC-PLACEMENT-006`;
- `PLANES-001`;
- `PLANES-002`;
- `PLANES-003`;
- `COORD-003` semantic side.

`TERRAIN-004` is explicitly not implemented by guesswork.

## Exit gates

Required matrix rows become `EXISTING` for all fully owned specs above, except narrower side-effect subcontracts not yet promoted under `LOC-PLACEMENT-005`.

Golden semantic scenes required:

- terrain shape gallery;
- wall/decor orientation scene;
- four-plane bridge column;
- region-border coordinate fixture.

Tier C semantic suite passes.

---

# M7 - Normals, lighting, and scene finalization

## Purpose

Close the highest-risk visual semantic gap before rendering.

## Prerequisites

M4 ModelData working representation; M6 scene placement; M5 fixtures.

## Implement

- exact base normal generation;
- smooth/flat distinction;
- cross-model normal merge by translated integer vertex equality;
- merged-normal storage/copy ownership;
- matched-face render type `2` behavior;
- scene neighbor traversal for boundaries/game objects/floor decorations;
- dual-arm wall merge;
- plane-above neighbor behavior;
- initial `nonFlatShading` ModelData lifecycle;
- final ModelData -> lit Model conversion;
- exact ambient/contrast/light-vector integer lighting.

## Owned specs

- `MODEL-BUILD-004`;
- `MODEL-BUILD-005` scene-normal ownership portion;
- `NORMALS-001` through `NORMALS-004`;
- `LIGHTING-001`;
- initial side of `LOC-PLACEMENT-004`.

## Hard exit gate

All normal/lighting matrix rows must be `EXISTING` before M11's reference viewport may claim model-lighting parity.

Required controls include:

- positive/negative translated merge;
- hide matched faces false/true;
- dual-arm wall;
- wall/game neighbor;
- floor decoration suppression;
- plane above;
- merged-normal final-lighting difference;
- shared source immutability.

This milestone is not complete because seams merely "look good."

---

# M8 - Morphs, contouring, and animation ownership

## Purpose

Implement dynamic object state without corrupting shared semantic assets.

## Prerequisites

M3 var/sequence decode; M4 model construction; M6 scene coordinates; M7 model ownership discipline.

## Implement

- varbit/varp transform resolution;
- fallback/null morphs;
- active-definition footprint recalculation;
- exact contour-ground integer algorithm;
- dynamic working-model ownership;
- deterministic sequence/frame state sufficient for target fixtures;
- replacement preserve/restart behavior covered by the audited contract;
- pending/static replacement path distinct from initial static construction.

## Owned specs

- `MORPH-001`;
- `CONTOUR-001`;
- `ANIMATION-001` within its currently verified scope;
- runtime side of `LOC-PLACEMENT-004`;
- remaining ownership expectations in `MODEL-BUILD-005`.

## Exit gates

- morph, contour, and owned animation rows become `EXISTING`;
- initial-vs-pending replacement fixture passes;
- active morph changing footprint recomputes placement inputs correctly;
- null morph yields no model;
- contouring never mutates shared base model;
- deterministic preview tick/frame tests exist.

---

# M9 - Semantic parity closure for the first target

## Purpose

Do not cross into renderer implementation with known gaps in the semantic subset needed by the first editor.

## Prerequisites

M2-M8.

## Work

- audit `PARITY-MATRIX.md` row by row;
- close all `REQUIRED` semantic rows needed for initial editor scope;
- explicitly mark unsupported/deferred target features rather than silently omitting them;
- run full Tier C;
- establish semantic golden-scene hashes for implemented scope;
- finalize target/profile diagnostics for revision-gated behavior;
- review C-003/C-005 provenance limitations and ensure no test falsely relies on the unpinned local tree.

## Terrain-color branch

At this milestone, one of two states is acceptable:

### A. `TERRAIN-004` closed

The exact builder/oracle has been found, pinned, implemented, and tested.

### B. `TERRAIN-004` remains blocked

The editor may proceed with an explicit target capability flag/problem state, but **complete reference terrain-color parity is not claimed**.

## Exit gates

- all semantic rows required by M10/M11 are `EXISTING` or explicitly `BLOCKED/DEFERRED` with user-visible capability implications;
- no P0/P1 bug is knowingly deferred to renderer compensation;
- semantic scene can be rebuilt deterministically from canonical inputs.

---

# M10 - Render extraction and headless renderer core

## Purpose

Implement the renderer-owned structural layer before touching a visible viewport.

## Prerequisites

M9 semantic closure; ADR-0004/0005/0006.

## Implement `osrs-render`

- immutable render extraction snapshot;
- generation identity;
- semantic -> render coordinate conversion/rebase;
- renderer-owned `RenderMesh`;
- static/dynamic/ordered classification;
- material table abstraction;
- semantic texture ID -> material handle mapping;
- zone keying and dirty propagation;
- ordered face preparation using exact priority oracle;
- render graph description independent of actual device execution;
- picking ID allocator/mapping data structures;
- structural diagnostics metadata.

## Owned verification

- `FACE-001` semantic-to-render preservation;
- exact `FACE-002` ordered face stream;
- `FACE-003` alpha interpretation inputs;
- `TEXTURE-001` CPU-side UV/material handoff;
- `PLANES-004` proof that renderer grouping cannot mutate semantic planes;
- `COORD-003` render conversion boundary;
- ADR-0005 zone-generation behavior.

## Exit gates

Tier D logic-level tests pass without requiring a physical GPU.

Renderer destruction/recreation of these CPU artifacts must not affect the semantic scene.

---

# M11 - First deterministic wgpu Reference viewport

## Purpose

Create the first visible 3D viewport only after the semantic and structural paths are trustworthy.

## Prerequisites

M10 plus M7 lighting parity.

## Implement

- wgpu instance/adapter/device abstraction;
- Reference-profile pipeline set;
- `Depth32Float` reverse-Z;
- clear depth `0.0`;
- `GreaterEqual`;
- CCW/front + back-face culling;
- deterministic camera/projection inputs;
- static opaque terrain/model drawing;
- canonical HSL/reference baked color path;
- deterministic render tick plumbing;
- headless/offscreen render target for tests.

## Exit gates

- asymmetric landmark/front-face test passes;
- near/far reverse-Z test passes;
- mirrored model culling test passes;
- simple terrain/model golden images can be captured on the canonical runner;
- wgpu validation clean for supported test configuration;
- no shader computes semantic model/terrain transforms that belong upstream.

This milestone is the first time "it renders" is a meaningful project statement.

---

# M12 - Zones, materials, textures, priority, and transparency

## Purpose

Reach the real reference-rendering architecture instead of a simple opaque prototype.

## Prerequisites

M11.

## Implement

- 8x8 static zone compilation;
- dirty-zone rebuild and generation rejection;
- GPU buffer/resource cache;
- paged 2D texture arrays;
- material page/layer mapping;
- reference sampling/brightness path;
- texture UV animation at deterministic tick;
- ordered side path;
- exact priority `0..11` consumption;
- priority 10/11 interleaving result from the proven sorter;
- translucent pass/depth policy;
- model + face alpha composition as required by reference tests;
- authored face bias reference application;
- static/dynamic consistency.

## Owned specs/decisions

- `FACE-002` renderer realization;
- `FACE-003` renderer realization;
- `FACE-004`;
- `TEXTURE-001` renderer realization;
- ADR-0004;
- ADR-0005;
- Reference side of ADR-0006.

## Exit gates

- priority crafted scene passes exact structural order and P3 visual comparison;
- alpha sentinel/model-alpha cases pass;
- coplanar bias golden is stable;
- texture-face and default UV fixtures render correctly;
- texture animation is deterministic at fixed ticks;
- static zone rebuild produces equivalent structural output;
- zone-boundary dirty propagation is proven.

---

# M13 - Picking, diagnostics, and GPU lifecycle

## Purpose

Make renderer behavior inspectable and safe enough to serve an editor.

## Prerequisites

M12.

## Implement

- integer picking attachment/readback;
- generation-scoped renderer IDs -> semantic handles;
- stale pick rejection;
- diagnostics pipelines/views for topology, normals, priority, alpha, bias, material IDs, object IDs, planes, zones, depth, culling, static/dynamic classification;
- renderer stats/tracing;
- viewport resize/reconfigure;
- texture/buffer recreation;
- simulated/recoverable device/resource loss path;
- adapter capability diagnostics/fallback selection.

## Exit gates

- pick identity survives zone rebuild;
- stale asynchronous picks are rejected;
- full renderer recreation from one extraction yields equivalent structural scene;
- semantic document hash remains unchanged through GPU/device recreation;
- validation errors include scene/renderable/material identity where available.

---

# M14 - Native editor shell and semantic document

## Purpose

Build the application around already-tested semantic and renderer foundations.

## Prerequisites

M13; ADR-0007/0008/0009.

## Implement `osrs-editor`

- eframe/egui application shell;
- Catppuccin theme baseline;
- docking/panel registry;
- shared wgpu viewport integration;
- `EditorProject`/`SemanticDocument` authority model;
- stable semantic identities;
- action/shortcut registry;
- selection/hover state;
- command dispatcher;
- transaction/history core;
- Problems/Diagnostics/History panels;
- preview state for render profile, planes, roofs, varbits/varps, tick/time.

## Exit gates

- panel layout changes cannot alter semantic document hash;
- selection remains stable through renderer rebuild;
- command apply/revert identity tests pass;
- randomized bounded undo/redo state-machine suite passes;
- background work uses generation snapshots rather than mutating shared authoritative state;
- editor shell survives viewport resize/recreate.

---

# M15 - Core editing tools and multi-region workflow

## Purpose

Reach a genuinely useful map editor while preserving target semantics.

## Prerequisites

M14.

## Implement

### Loc tools

- select/box select;
- place loc;
- move loc;
- orient/rotate loc;
- duplicate/copy/paste;
- delete;
- eyedropper;
- semantic footprint/ghost previews.

### Terrain tools

- height/sculpt;
- flatten;
- underlay paint;
- overlay paint;
- shape/rotation;
- tile settings/flags;
- area operations.

### Workspace

- explicit multi-region set;
- neighbor loading state;
- writable/read-only regions;
- cross-region transactions;
- plane visibility/editing plane;
- detailed source/storage/render/collision plane inspection.

## Exit gates

- loc previews use exact requested model/type and preserve no-model results;
- persistent loc transforms commit only representable semantic values;
- terrain stroke result is independent of UI frame count;
- cross-region undo restores exact baseline;
- unloaded neighbor is never treated as empty;
- renderer dirty zones derive from semantic `ChangeSet` and include footprint crossings.

---

# M16 - Project persistence, autosave, recovery, and export

## Purpose

Make projects durable and produce target data only through explicit validation.

## Prerequisites

M15; target export-format contracts sufficiently specified.

## Implement

- versioned project schema;
- cache/source fingerprint recording;
- atomic project save;
- autosave generations;
- recovery discovery/restore;
- schema migration framework;
- project annotations/bookmarks/preferences separation;
- export prevalidation;
- target map/location serializer;
- region-specific write/export planning;
- optional cache surgery only behind explicit supported workflow;
- export report showing revision gates/problems.

## Exit gates

Failure injection passes for:

- temp file failure;
- partial/write failure;
- flush failure;
- rename failure;
- stale autosave completion;
- corrupt recovery;
- migration failure.

Also require:

- failed export does not alter project-save state;
- save/reload semantic hash equality;
- cache fingerprint mismatch is detected;
- export round-trip tests exist for supported semantic fields;
- project save is allowed even when export is blocked by `TERRAIN-004` or another target gate.

---

# M17 - Reference parity closure

## Purpose

Turn the implemented editor/renderer into a defensible reference-grade system for the selected initial target.

## Prerequisites

M16.

## Required work

- implement/check every applicable golden scene in `GOLDEN-SCENES.md`;
- canonicalize reference cameras/ticks/profile settings;
- run Tier A-E on designated canonical GPU runner;
- verify semantic golden hashes before image comparisons;
- close remaining priority/alpha/texture/bridge/morph/reference-view gaps;
- validate diagnostics against intentional injected faults;
- review every `PARITY-MATRIX.md` row required for initial release.

## `TERRAIN-004` release rule

If complete terrain-color parity remains blocked:

- product must state that limitation accurately;
- reference screenshots that depend on that unknown builder cannot be advertised as full OSRS terrain-color parity;
- export behavior must preserve source/edited semantic fields without inventing unsupported color semantics.

A release may still be useful, but its parity claim must be scoped.

## Exit gates

- all initial-target P0/P1 requirements are either `EXISTING` or explicitly excluded from the release profile;
- P2 renderer structural tests are complete;
- designated P3 goldens pass within documented tolerance;
- no known semantic bug is hidden by image thresholds;
- source/profile provenance is visible in test artifacts and application diagnostics.

---

# M18 - Enhanced presentation and performance hardening

## Purpose

Improve the editing experience only after Reference behavior is measurable and preserved.

## Prerequisites

M17 reference baseline.

## Implement/optimize

Possible enhanced features:

- MSAA;
- anisotropic filtering;
- smoother camera/input;
- improved fog/sky presentation;
- optional non-destructive enhanced lighting;
- selection outlines and editor overlays;
- larger draw distance/workspace visibility;
- background zone compilation;
- asset thumbnails/search acceleration;
- memory/resource eviction;
- startup/project-load profiling;
- rendering/scene/editor tracing dashboards.

## Performance work

Establish measured benchmark baselines before setting regression thresholds for:

- cache decode;
- model construction;
- normal reconciliation;
- scene build;
- priority sorting;
- zone compile/upload;
- frame preparation;
- GPU frame;
- picking latency;
- save/autosave/export;
- large undo/redo operations.

## Exit gates

- enhanced mode does not change semantic scene hashes;
- Reference profile still passes Tier A-E;
- every optimization preserves generation/ownership invariants;
- performance gates are based on measured canonical workloads rather than inherited speculative numbers;
- diagnostic/safe mode can disable risky enhancements when troubleshooting.

---

# 3. Cross-cutting implementation requirements

## 3.1 Documentation is part of implementation

When a milestone implements a spec/ADR:

- update the relevant spec's implementation mapping;
- update `PARITY-MATRIX.md` coverage;
- link concrete Rust tests/fixtures;
- document public APIs with invariants and failure behavior;
- update contradiction status only when the underlying gap is actually closed.

## 3.2 No speculative compatibility layers

Do not add abstraction layers for hypothetical future revisions unless:

- a current revision difference is already known; or
- the abstraction directly enforces an architectural boundary.

The target-profile mechanism is mandatory. A large generic "version framework" without evidence is not.

## 3.3 Errors carry semantic identity

Across milestones, errors should preserve available:

- target/profile;
- archive/file/definition/model ID;
- region/tile/plane;
- semantic instance ID;
- spec/stage context where practical.

## 3.4 Threading is generation-based

As background work is introduced:

- workers consume immutable snapshots;
- outputs carry source generation;
- late stale outputs are discarded;
- mutable semantic state remains serialized through document/scene ownership.

## 3.5 Unsafe code policy

Avoid `unsafe` in semantic/cache/editor code unless a measured requirement and reviewed invariant justify it.

GPU interop/library internals may require unsafe transitively, but project-authored unsafe blocks require focused tests and documentation.

## 3.6 Third-party dependencies

Before adding a foundational dependency, verify:

- maintenance/compatibility;
- target/native platform support;
- license compatibility;
- whether it leaks foreign semantics into public APIs;
- whether replacing it later would cross a crate boundary.

Crate boundaries exist specifically so dependencies such as a cache library, docking library, or image-comparison implementation can be replaced without rewriting OSRS semantics.

# 4. Recommended implementation PR sizing

The blueprint branch itself intentionally accumulated documentation together, but implementation should use small, reviewable PRs.

Recommended rule:

- one semantic behavior family or one infrastructure slice per PR;
- include owning tests in the same PR;
- do not mix unrelated cache, renderer, and editor changes merely because they are in one milestone;
- no PR should change a semantic rule and its golden expected output without showing the independent oracle/evidence for the change.

Examples of good PR boundaries:

- target profile + provenance types;
- object transform decoder fixtures;
- ModelData mirror/winding;
- normal merge primitive;
- scene normal neighbor traversal;
- link-below scene operation;
- render extraction face metadata;
- exact priority sorter;
- wgpu reverse-Z bootstrap;
- zone compiler;
- picking ID pass;
- command history core;
- terrain sculpt transaction;
- atomic project save.

# 5. Critical path

The shortest trustworthy path to a useful editor is:

```text
M0 workspace
  -> M1 target/cache decision
  -> M2 core types
  -> M3 cache definitions/maps
  -> M4 exact models
  -> M5 reference fixtures
  -> M6 scene placement/terrain/planes
  -> M7 normals/lighting
  -> M8 morph/contour/dynamic ownership
  -> M9 semantic closure
  -> M10 render extraction
  -> M11 first reference viewport
  -> M12 full materials/priority/alpha/zones
  -> M13 picking/diagnostics
  -> M14 editor shell/history
  -> M15 editing tools
  -> M16 project/export durability
  -> M17 reference parity closure
  -> M18 enhancements/performance
```

The major deliberate choice is that **M7 happens before M11**. Cross-model normals, lighting, placement, and planes are semantic construction responsibilities; they must not be discovered by staring at the first GPU viewport.

# 6. Implementation start gate

After Checkpoints 9 and 10 reconcile and merge this blueprint, implementation may begin at M0.

Before implementation starts, the merged docs must make these facts obvious to a new developer:

- which documents are normative versus historical research;
- which semantics are verified versus revision-gated;
- which target/profile is selected only when M1 is completed;
- which tests prove each semantic contract;
- which renderer/editor behaviors are project decisions;
- which unresolved issues must not be guessed.

That is the handoff contract from blueprint design to production implementation.
