# Blueprint Merge Readiness

Status: **Checkpoint 10 complete**  
Decision class: `PROJECT_GOVERNANCE`

This document records the final readiness review for the RustOSRS blueprint/specification program before implementation begins.

## 1. Review scope

Checkpoint 10 reviewed the complete `blueprint/osrs-editor-foundation` branch against `main`, including:

- canonical navigation and document reachability;
- blueprint/spec/ADR/verification ownership consistency;
- remaining contradiction and revision-gate classifications;
- source/provenance caveats;
- implementation-roadmap alignment with verification ownership;
- accidental implementation/code additions;
- changed-file scope;
- PR readiness.

## 2. Canonical entry points

The documentation system has clear entry points:

- root `index.md` — canonical navigation hub;
- `docs/blueprint/README.md` — blueprint registry and implementation entry state;
- `docs/specs/README.md` — semantic specification registry;
- `docs/adr/README.md` — accepted project-owned decisions;
- `docs/verification/README.md` — verification/provenance registry;
- `docs/research/README.md` — legacy research quarantine and supersession map.

No root `RUNELITE_*.md` document is implementation authority.

## 3. Scope validation

The blueprint branch contains documentation/index changes only.

It does not introduce the Rust workspace, production crates, wgpu implementation, editor code, cache implementation, generated binaries, or implementation fixtures beyond the pre-existing repository evidence corpus.

Implementation begins at M0 in `17-IMPLEMENTATION-ROADMAP.md` after this blueprint is merged.

## 4. Architecture consistency

The final dependency direction remains:

```text
osrs-core
  ^
  |\
  | +-- osrs-cache
  | +-- osrs-scene
  |       ^
  |       |
  +---- osrs-render
          ^
          |
      osrs-editor

osrs-reference -> development/test tooling only
```

The exact crate dependency table in `06-CRATE-ARCHITECTURE.md` is authoritative where the diagram is simplified.

The final ownership rules remain consistent across the blueprint:

- cache transport/decoding does not own loc/scene semantics;
- scene construction does not depend on concrete cache implementation types;
- renderer code consumes semantic truth and does not repair P0/P1 failures;
- editor mutations commit semantic document state before derived render state;
- source model/definition assets are immutable by default;
- project persistence, autosave/recovery, and target export are distinct workflows;
- `osrs-reference` is never a production runtime dependency.

## 5. Semantic consistency

The merge-ready specification set preserves the important source-audit corrections:

- eligible static `ModelData` objects may reconcile coincident normals across separate models before final lighting;
- normal reconciliation is not mesh welding;
- initial region construction and pending/live replacement are distinct model-construction pipelines;
- model selection has no generic fallback-to-first-model behavior;
- model mirroring includes semantic winding changes;
- integer transform order is normative;
- terrain shape topology `0..12` is verified;
- bridge/source/storage/collision/render-level concepts remain separate;
- the audited path has no generic floor-decoration `+1/+2` lift;
- face priority is not a simple `(priority, depth)` sort;
- semantic texture IDs are not constrained by a historical renderer allocation constant.

## 6. Intentional unresolved gates

The following remain open by design and do **not** block merging the blueprint.

### Cache strategy / C-010

`rs-cache` is a candidate, not an accepted decoder architecture.

M1 performs the bounded target-profile compatibility spike and records the actual dependency strategy in a new ADR.

### FileStore authority / C-011

OpenRune FileStore remains independent implementation evidence/tooling, not the semantic oracle.

Revision-sensitive opcodes/defaults must be checked against the selected target profile and pinned evidence.

### Historical local deob provenance / C-003 and C-005

The public January 2026 source pin supports the current semantic contracts, while the historical developer-machine harness tree is not claimed to be byte-identical.

New promoted fixtures must use the reproducible manifest rules in `docs/verification/REFERENCE-FIXTURES.md`.

### Verification coverage / C-006

Checkpoint 7 defines the complete required fixture/test matrix, but implementation-era coverage is intentionally incomplete.

`PARITY-MATRIX.md` must advance rows to `EXISTING` only when concrete Rust/reference tests are added.

### Terrain-color builder / C-021 and TERRAIN-004

The complete slope/11x11/overlay/jitter terrain-color builder remains `REVISION_SENSITIVE` because the legacy `class470` attribution is stale for the pinned public target.

Implementation may proceed around this capability with an explicit limitation, but complete terrain-color parity cannot be claimed until the source/oracle gate is resolved.

## 7. Verification-roadmap consistency

The roadmap and verification system agree on ordering:

- M0-M5 establish workspace/profile/core/cache/model/reference-fixture foundations;
- M6-M9 close the first semantic scene scope before rendering becomes authoritative;
- M7 normal reconciliation and lighting precede the first Reference viewport at M11;
- M10-M13 implement renderer structure, deterministic viewport, ordered/material paths, picking, diagnostics, and lifecycle;
- M14-M16 implement semantic-document editing, tools, persistence/recovery, and validated export;
- M17 closes Reference visual parity;
- M18 adds enhanced presentation and measured performance hardening.

A later milestone cannot compensate for an earlier parity failure.

## 8. Documentation authority consistency

Checkpoint 9 successfully separates historical research from current authority.

When documents disagree:

1. use `docs/specs/` for OSRS semantic requirements;
2. use accepted ADRs for RustOSRS-owned decisions;
3. use blueprint documents for architecture/product workflow;
4. use `docs/verification/` for provenance/test ownership;
5. use root research only to locate evidence/history.

`REVISION_SENSITIVE` remains a real gate, not shorthand for likely-correct behavior.

## 9. Merge-readiness result

The blueprint is ready to merge when the final PR confirms:

- base remains `main`;
- branch remains conflict-free/mergeable;
- complete changed-file scope is documentation/index only;
- no unexpected review/check failure appears;
- final PR head matches the reviewed branch head.

No implementation work should be added to this PR.

## 10. Post-merge entry point

After merge, the next work is **M0 - Workspace and CI foundation** from `17-IMPLEMENTATION-ROADMAP.md`.

M0 is a new implementation phase. It must not retroactively weaken blueprint evidence/status rules merely to accelerate scaffolding.
