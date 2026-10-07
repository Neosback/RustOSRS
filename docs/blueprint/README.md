# RustOSRS Blueprint

This directory is the canonical architecture/specification system for RustOSRS.

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
| `19-MERGE-READINESS.md` | final consistency review and implementation handoff | Checkpoint 10 complete |

## Canonical supporting sets

### Semantic specifications

`docs/specs/` contains 35 atomic OSRS semantic contracts spanning loc placement, model construction, normals/lighting, morphs/animation/contouring, terrain, bridges/planes, face/material metadata, and coordinates.

Each contract owns its evidence status, source pin, exact behavior, invariants, failure signature, and required verification.

### Architecture decisions

Accepted decisions live in `docs/adr/` and currently cover:

- reusable `osrs-*` crate boundaries;
- native-first editor policy;
- semantic/renderer/editor ownership boundaries;
- reverse-Z and raster conventions;
- 8x8 zone-compiled hybrid rendering;
- Reference and Enhanced render profiles;
- eframe/egui editor shell;
- semantic command/transaction history;
- separation of project save/autosave/recovery from target export.

### Verification

`docs/verification/` is the correctness/provenance registry:

- `SOURCE-PINS.md`
- `PARITY-MATRIX.md`
- `REFERENCE-FIXTURES.md`
- `GOLDEN-SCENES.md`
- `CI-FUZZ-BENCHMARKS.md`

P0-P2 exact contracts are tested exactly. Screenshot tolerance starts only at P3 reference visual parity and may never hide a failed earlier-layer test.

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

osrs-reference -> development/test tooling only
```

Key invariants:

- scene semantics do not depend on concrete cache implementation types;
- renderer code consumes semantic truth and does not define or repair OSRS semantics;
- editor mutations change semantic document state before derived renderer state;
- shared source assets are immutable by default;
- GPU artifacts are generation-tagged disposable derivatives;
- project save, autosave/recovery, and target export are distinct workflows;
- enhanced presentation never rewrites canonical semantic state.

## Major semantic corrections carried forward

The implementation must preserve the source-audited corrections established by the blueprint:

- eligible static `ModelData` objects can reconcile coincident normals across separate models before final lighting;
- this is normal reconciliation, not mesh welding;
- initial region construction and pending/live replacement are distinct construction pipelines;
- model selection has no generic fallback-to-first-model behavior;
- mirroring changes semantic geometry/winding rather than using a renderer-only negative scale;
- transform ordering and integer rounding are semantic contracts;
- terrain shape topology `0..12` is verified;
- bridge/source/storage/collision/render-level concepts remain distinct;
- the audited path has no generic floor-decoration `+1/+2` lift;
- face priority is not a simple `(priority, depth)` sort;
- semantic texture IDs are independent of historical GPU allocation constants.

## Intentional unresolved gates

The merged blueprint intentionally leaves several implementation-era gates open:

- **C-010:** cache strategy is decided by the M1 target-profile compatibility spike and a new ADR;
- **C-011:** OpenRune FileStore is evidence/tooling, not semantic authority;
- **C-003/C-005:** historical local deob provenance remains distinct from the pinned public source;
- **C-006:** planned fixture coverage is not claimed as already implemented;
- **C-021 / `TERRAIN-004`:** complete terrain-color builder parity remains blocked until reproducible source/oracle provenance exists.

These are explicit roadmap/verification gates, not reasons to guess behavior.

## Implementation roadmap

`17-IMPLEMENTATION-ROADMAP.md` defines milestones M0-M18:

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

The first Reference viewport deliberately comes only after model/scene/plane/normal/lighting semantics and high-risk differential fixtures are established.

## Documentation authority

The canonical navigation entry is the root `index.md`.

When documents disagree:

1. use `docs/specs/` for OSRS semantic requirements;
2. use accepted ADRs for RustOSRS-owned design decisions;
3. use blueprint documents for architecture/product workflow;
4. use `docs/verification/` for provenance and test ownership;
5. use root research only for evidence discovery/history.

`REVISION_SENSITIVE` is a real implementation gate, never shorthand for “probably correct.”

## Blueprint checkpoint status

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
| 10 | Merge readiness | Complete |

## Implementation entry point

The blueprint phase is complete.

After this documentation set is merged, implementation begins with **M0 - Workspace and CI foundation** in `17-IMPLEMENTATION-ROADMAP.md`.

M0 and later milestones must consume the canonical specs/ADRs/verification gates rather than reinterpret legacy research as authority.
