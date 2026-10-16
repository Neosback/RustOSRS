# RustOSRS Blueprint

This directory is the canonical architecture/specification system for RustOSRS.

`docs/RUNELITE_*.md` files are retained historical research, not implementation authority. See `docs/research/README.md` and `18-DOCUMENTATION-RECONCILIATION.md` before relying on them.

## Current implementation note

The blueprint phase is complete **and implementation is already well beyond its original entry point**.

Do not start at M0 based on historical text. Before implementation work, read:

1. `DEV_HANDOFF.md` for the latest exact SHA/CI/subsystem boundary;
2. `docs/PROJECT-STATUS.md` for the current completion audit;
3. `17-IMPLEMENTATION-ROADMAP.md` for the reconciled M0-M18 roadmap and next-work order;
4. `docs/verification/PARITY-MATRIX.md` for evidence/proof state.

As of the 2026-10-15 audit, M0-M9 are complete, the Reference renderer and native editor exist, semantic authoring and verified export exist, M16 remains partial because autosave/recovery is not complete, and the major active completion areas are runtime ownership extraction, Reference parity/certification closure, 117HD completion, export generalization, and project lifecycle/recovery.

## Blueprint registry

| Document | Purpose | Status |
|---|---|---|
| `00-DOCUMENT-INVENTORY.md` | pre-blueprint corpus inventory/classification | Complete |
| `01-EVIDENCE-STATUS.md` | evidence vocabulary and promotion rules | Foundation |
| `02-CONTRADICTION-REGISTER.md` | conflicts, revision hazards, open questions | Living register |
| `03-SOURCE-GROUP-INVENTORY.md` | source/shader/fixture/tooling inventory | Complete |
| `04-PROJECT-CHARTER.md` | mission, scope, quality gates, parity ownership | Complete |
| `05-SYSTEM-ARCHITECTURE.md` | layering, data flow, ownership, mutation boundaries | Complete |
| `06-CRATE-ARCHITECTURE.md` | reusable `osrs-*` crate boundaries | Complete |
| `07-ARCHITECTURE-INVARIANTS.md` | enforceable architecture guardrails | Complete |
| `08-PARITY-MODEL.md` | target profiles, P0-P4 parity, exact/tolerance rules | Complete/living |
| `09-SEMANTIC-AUDIT.md` | loc/model/normals/lighting/morph/priority audit | Historical audit record |
| `10-TERRAIN-MATERIAL-PLANE-AUDIT.md` | terrain/material/bridge/UV/coordinate audit | Historical audit record |
| `11-RENDERER-ARCHITECTURE.md` | render extraction, generations, zones, lifecycle | Canonical architecture |
| `12-GPU-DATA-PASSES.md` | GPU ABI, depth/culling, materials, passes | Canonical architecture |
| `13-EDITOR-ARCHITECTURE.md` | native editor shell, panels, viewport/workspace | Canonical architecture |
| `14-EDITOR-DOCUMENT-TRANSACTIONS.md` | document, commands, history, persistence/export boundaries | Canonical architecture |
| `15-EDITOR-TOOLS-INTERACTION.md` | tools, gizmos, selection, previews, shortcuts | Canonical architecture |
| `16-VERIFICATION-ARCHITECTURE.md` | differential/golden/property/GPU/editor verification | Canonical verification design |
| `17-IMPLEMENTATION-ROADMAP.md` | implementation milestones and current completion program | Current/reconciled |
| `18-DOCUMENTATION-RECONCILIATION.md` | canonical-vs-legacy authority map and requirement closure | Complete |
| `19-MERGE-READINESS.md` | original blueprint merge-readiness record | Historical handoff record |

## Canonical supporting sets

### Semantic specifications

`docs/specs/` contains atomic OSRS semantic contracts spanning loc placement, model construction, normals/lighting, morphs/animation/contouring, terrain, bridges/planes, face/material metadata, and coordinates.

Each contract owns its evidence status, source pin, exact behavior, invariants, failure signature, and required verification.

### Architecture decisions

Accepted decisions live in `docs/adr/` and cover reusable crate boundaries, native-first editor policy, semantic/renderer/editor ownership, reverse-Z/raster conventions, zone rendering, render profiles, eframe/egui, semantic command history, and separation of project save/recovery from target export.

### Verification

`docs/verification/` is the correctness/provenance registry:

- `SOURCE-PINS.md`;
- `PARITY-MATRIX.md`;
- `REFERENCE-FIXTURES.md`;
- `GOLDEN-SCENES.md`;
- `CI-FUZZ-BENCHMARKS.md`.

P0-P2 exact contracts are tested exactly. Screenshot tolerance starts only at P3 Reference visual parity and may never hide a failed earlier-layer test.

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
osrs-world cache-backed assembly/extraction
    |
    v
osrs-render Reference + HD rendering
    |
    v
osrs-editor native product/composition root

osrs-reference -> development/test tooling only
```

Key invariants:

- scene semantics do not depend on concrete cache implementation types;
- renderer code consumes semantic truth and does not define or repair OSRS semantics;
- editor mutations change semantic document state before derived renderer state;
- shared source assets are immutable by default;
- GPU/projection artifacts are generation-tagged disposable derivatives;
- project save, autosave/recovery, and target export are distinct workflows;
- enhanced/HD presentation never rewrites canonical semantic state.

## Major semantic corrections carried forward

The implementation preserves the source-audited corrections established by the blueprint, including cross-model normal reconciliation, distinct construction/runtime pipelines, no generic model fallback, semantic mirroring/transform ownership, exact terrain topology, distinct plane concepts, no generic floor-decoration lift, nontrivial priority ordering, and full-width semantic texture IDs.

One old unresolved gate is specifically superseded: `TERRAIN-004` is now implemented and differential-tested against the pinned client. Current truth lives in `docs/PROJECT-STATUS.md` and `PARITY-MATRIX.md`, not the historical pre-implementation wording.

## Implementation roadmap

`17-IMPLEMENTATION-ROADMAP.md` retains milestones M0-M18 but now reconciles them to production reality.

Current high-level disposition:

- M0-M9: complete;
- M10-M15: production baseline implemented, with Reference parity closure continuing under M17;
- M16: project save/open and verified export implemented, autosave/recovery incomplete;
- M17: active Reference parity/certification closure;
- M18: active enhanced/117HD completion and release hardening.

See `docs/PROJECT-STATUS.md` for the detailed remaining-work program.

## Documentation authority

The canonical navigation entry is the root `index.md`.

When documents disagree:

1. use `DEV_HANDOFF.md` and `docs/PROJECT-STATUS.md` for current repository/completion state;
2. use `docs/specs/` for OSRS semantic requirements;
3. use accepted ADRs for RustOSRS-owned design decisions;
4. use blueprint documents for architecture/product workflow;
5. use `docs/verification/` for provenance and proof ownership;
6. use root research only for evidence discovery/history.

`REVISION_SENSITIVE` is a real implementation gate, never shorthand for "probably correct."

## Blueprint checkpoint status

The original blueprint checkpoints 0-10 are complete. Their historical checkpoint labels describe the documentation-design phase, not the current implementation progress.

Implementation progress is now tracked through `DEV_HANDOFF.md`, `docs/PROJECT-STATUS.md`, the reconciled roadmap, and the parity matrix.
