# RustOSRS Blueprint

This directory is the canonical home for the RustOSRS architecture and specification program.

Root-level `RUNELITE_*.md` files are retained historical research, not implementation authority. See `docs/research/README.md` and `18-DOCUMENTATION-RECONCILIATION.md` before relying on them.

## Blueprint registry

| Document | Purpose | Status |
|---|---|---|
| `00-DOCUMENT-INVENTORY.md` | pre-blueprint corpus inventory/classification | Checkpoint 1 complete |
| `01-EVIDENCE-STATUS.md` | evidence vocabulary and promotion rules | Foundation |
| `02-CONTRADICTION-REGISTER.md` | conflicts, revision hazards, open questions | Living register |
| `03-SOURCE-GROUP-INVENTORY.md` | source/shader/fixture/tooling inventory | Checkpoint 1 complete |
| `04-PROJECT-CHARTER.md` | mission, scope, quality gates, parity ownership | Checkpoint 2 complete |
| `05-SYSTEM-ARCHITECTURE.md` | layering, data flow, ownership, mutation boundaries | Checkpoint 2 complete |
| `06-CRATE-ARCHITECTURE.md` | reusable `osrs-*` crate boundaries | Checkpoint 2 complete |
| `07-ARCHITECTURE-INVARIANTS.md` | enforceable architecture guardrails | Checkpoint 2 complete |
| `08-PARITY-MODEL.md` | target profiles, P0-P4 parity, exact/tolerance rules | Checkpoint 2 complete |
| `09-SEMANTIC-AUDIT.md` | loc/model/normals/lighting/morph/priority audit | Checkpoint 3A record |
| `10-TERRAIN-MATERIAL-PLANE-AUDIT.md` | terrain/material/bridge/UV/coordinate audit | Checkpoint 3B record |
| `11-RENDERER-ARCHITECTURE.md` | render extraction, generations, zones, lifecycle | Checkpoint 5 complete |
| `12-GPU-DATA-PASSES.md` | GPU ABI, depth/culling, materials, passes | Checkpoint 5 complete |
| `13-EDITOR-ARCHITECTURE.md` | native editor shell, panels, viewport/workspace | Checkpoint 6 complete |
| `14-EDITOR-DOCUMENT-TRANSACTIONS.md` | document, commands, history, persistence/export boundaries | Checkpoint 6 complete |
| `15-EDITOR-TOOLS-INTERACTION.md` | tools, gizmos, selection, previews, shortcuts | Checkpoint 6 complete |
| `16-VERIFICATION-ARCHITECTURE.md` | differential/golden/property/GPU/editor verification | Checkpoint 7 complete |
| `17-IMPLEMENTATION-ROADMAP.md` | dependency-ordered implementation from workspace to full editor | Checkpoint 8 complete |
| `18-DOCUMENTATION-RECONCILIATION.md` | canonical-vs-legacy authority map and requirement closure | Checkpoint 9 complete |

## Normative supporting sets

### Semantic specifications

`docs/specs/` contains 35 atomic OSRS semantic contracts covering:

- loc placement;
- model construction;
- normals and lighting;
- morphs, animation ownership, and contouring;
- terrain;
- planes and bridges;
- face/material metadata;
- coordinates.

Each contract carries evidence status, source pin, scope, required behavior, invariants, failure signature, and required tests.

### Verification

`docs/verification/` contains:

- `SOURCE-PINS.md`;
- `PARITY-MATRIX.md`;
- `REFERENCE-FIXTURES.md`;
- `GOLDEN-SCENES.md`;
- `CI-FUZZ-BENCHMARKS.md`;
- verification registry `README.md`.

Exact semantic contracts are tested exactly. Screenshot tolerance begins only at P3 reference visual parity and may never hide a failed P0/P1/P2 test.

### Architecture decisions

Accepted ADRs currently cover:

- ADR-0001 reusable `osrs-*` crate boundaries;
- ADR-0002 native-first editor;
- ADR-0003 semantic/renderer/editor ownership boundaries;
- ADR-0004 reverse-Z and raster conventions;
- ADR-0005 8x8 zone-compiled hybrid rendering;
- ADR-0006 Reference and Enhanced render profiles;
- ADR-0007 eframe/egui editor shell;
- ADR-0008 command/transaction history;
- ADR-0009 project-save/export separation.

## Canonical architecture

```text
cache/source
    |
    v
osrs-cache
    |
    v
osrs-core canonical data
    |
    v
osrs-scene semantic construction
    |
    v
osrs-render extraction / wgpu
    |
    v
osrs-editor eframe/egui product
```

`osrs-reference` is development/test tooling and is never a production runtime dependency.

Core rules:

- scene code does not depend on concrete cache implementation types;
- renderer code does not define OSRS semantics;
- editor code mutates semantic document state before renderer state;
- shared source assets are immutable by default;
- GPU artifacts are generation-tagged disposable derivatives;
- project save, autosave/recovery, and target export are distinct workflows;
- presentation improvements never rewrite canonical semantic state.

## Major semantic corrections

The canonical system preserves the source-audited corrections established in Checkpoints 3-4:

- eligible static `ModelData` objects can accumulate normals across separate models before final lighting;
- normal reconciliation is not mesh welding;
- initial region construction and pending-spawn replacement are distinct construction pipelines;
- model selection has no generic fallback-to-first-model behavior;
- mirroring includes winding semantics and is not a renderer negative-scale shortcut;
- transform order and integer rounding are semantic contracts;
- terrain shape topology `0..12` is verified;
- the complete terrain-color builder remains revision-gated under `TERRAIN-004`;
- bridge behavior is not one universal plane adjustment;
- no generic `+1/+2` floor-decoration lift exists in the audited path;
- priority, alpha, texture metadata, authored bias, and UV inputs survive the semantic-to-render boundary;
- decoder widths/capacities follow the selected target/profile rather than historical renderer constants.

## Verification model

RustOSRS uses layered parity ownership:

```text
P0 decode parity
P1 semantic parity
P2 render-structural parity
P3 reference visual parity
P4 enhanced presentation compatibility
```

The verification system mirrors those layers and adds editor/document verification.

Consequences:

- normal merging is proven by exact normal/lighting fixtures before screenshots;
- priority behavior is proven by exact face-emission order before raster comparison;
- bridge/source/storage/collision planes are exact semantic values;
- golden scenes contain semantic/extraction expectations in addition to optional images;
- async generation behavior is tested with deterministic completion ordering;
- save/recovery/export receive failure injection;
- fuzzing proves robustness, not parity.

## Implementation roadmap

`17-IMPLEMENTATION-ROADMAP.md` defines M0-M18:

```text
M0  workspace and CI foundation
M1  target profile and cache contract/dependency ADR
M2  osrs-core semantic foundation
M3  cache transport and definition decoding
M4  model decode and exact model construction
M5  reference fixture infrastructure
M6  terrain, loc placement, and plane scene construction
M7  normals, lighting, and scene finalization
M8  morphs, contouring, and animation ownership
M9  semantic parity closure for first target
M10 render extraction and headless renderer core
M11 first deterministic wgpu Reference viewport
M12 zones, materials, textures, priority, transparency
M13 picking, diagnostics, and GPU lifecycle
M14 native editor shell and semantic document
M15 core editing tools and multi-region workflow
M16 project persistence, autosave, recovery, export
M17 Reference parity closure
M18 Enhanced presentation and performance hardening
```

The first meaningful wgpu viewport is intentionally delayed until core model/scene/normal/lighting semantics and their differential fixtures exist.

### Cache decision

The old research recommendation to use `rs-cache` as the foundation is **not accepted architecture**.

M1 performs a bounded compatibility spike against the actual target/profile requirements and produces an ADR choosing whether to wrap, extend, fork, partially reuse, or replace `rs-cache` components. OpenRune FileStore remains independent evidence/tooling rather than semantic authority.

### Terrain-color gate

`TERRAIN-004` remains blocked until the complete terrain-color builder has a reproducible exact source or executable oracle.

Implementation may proceed around the gate with an explicit capability/problem state, but RustOSRS may not guess the algorithm and bless its own output as reference evidence.

## Documentation authority after Checkpoint 9

The canonical navigation entry is the root `index.md`.

Legacy research is governed by `docs/research/README.md`.

`18-DOCUMENTATION-RECONCILIATION.md` records why old claims were demoted and maps them to current owners.

Every normative requirement must now have a closure path:

```text
semantic requirement
  -> atomic spec
  -> source/evidence pin
  -> parity/test owner
  -> roadmap exit gate

project-owned requirement
  -> accepted ADR / blueprint contract
  -> verification owner
  -> roadmap exit gate
```

Revision-sensitive behavior terminates in an explicit unresolved gate, never an implementation guess.

## Work sequence

| Checkpoint | Scope | Status |
|---|---|---|
| 0 | Repository audit | Complete |
| 1 | Documentation/evidence inventory | Complete |
| 2 | Truth model + architecture | Complete |
| 3 | Rendering semantic audit | Complete |
| 4 | Canonical OSRS specifications | Complete |
| 5 | Rust/wgpu renderer blueprint | Complete |
| 6 | Editor blueprint | Complete |
| 7 | Verification blueprint | Complete |
| 8 | Implementation roadmap | Complete |
| 9 | Documentation reconciliation | Complete |
| 10 | Merge readiness | Not started |

## Checkpoint 10 entry condition

Checkpoint 10 is the final merge-readiness review for this blueprint branch.

It should:

- perform repository-wide consistency/link/status review;
- confirm every new canonical document is reachable from the root index/registry;
- check remaining contradiction items are intentional and accurately classified;
- identify obvious housekeeping that should be fixed before merge;
- confirm no implementation code accidentally entered the blueprint branch;
- compare branch against `main` and review the complete changed-file set;
- open the single blueprint PR only when the branch is internally consistent;
- review the PR diff/status and merge when ready;
- report the merged result and stop before beginning M0 implementation.

## Current branch policy

All blueprint work remains on:

`blueprint/osrs-editor-foundation`

No pull request should be opened until Checkpoint 10 determines the complete blueprint branch is merge-ready.
