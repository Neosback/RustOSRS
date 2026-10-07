# Documentation Reconciliation

Status: **Checkpoint 9 normative documentation reconciliation**  
Decision class: `DOCUMENTATION_GOVERNANCE`

This document reconciles the pre-blueprint research corpus with the canonical RustOSRS architecture, semantic specifications, verification system, and implementation roadmap.

The goal is not to erase historical research. The goal is to ensure that a future implementer can tell, without guesswork, which documents are authoritative, which are evidence/navigation aids, which statements are superseded, and where every requirement is tested.

## 1. Canonical documentation layers

RustOSRS documentation is now separated by ownership.

| Layer | Purpose | Normative? |
|---|---|---|
| `docs/specs/` | OSRS semantic behavior | Yes, according to each spec's evidence status |
| `docs/adr/` | RustOSRS-owned architecture/product decisions | Yes when status is Accepted |
| `docs/blueprint/` | system, renderer, editor, verification architecture, roadmap | Yes for project architecture/planning |
| `docs/verification/` | source pins, fixture ownership, parity coverage, golden scenes, CI policy | Yes for verification governance |
| `docs/research/` | legacy/research classification | Governance only |
| root `RUNELITE_*.md` | historical research/source discovery | No |
| imported source trees / shaders / generated API docs | primary/reference evidence | Evidence only; authority depends on exact pin and owning spec |

## 2. Root index reconciliation

The old `index.md` was itself a legacy research hub. It instructed implementers to:

- treat R1-R27 as the behavior authority;
- implement `editor_core` / `editor_render`;
- use legacy port notes as the main Rust guide;
- verify claims by following the old runtime-rule numbering;
- accept `R17 no normal welding` as a one-line rule.

Checkpoint 9 replaces that reading order.

The root `index.md` is now the canonical navigation hub and routes implementation work through:

1. charter/system/crate architecture;
2. parity model;
3. canonical semantic specs;
4. accepted ADRs;
5. verification ownership;
6. implementation roadmap;
7. legacy research only when tracing evidence/history.

## 3. Legacy corpus policy

The old root `RUNELITE_*.md` files remain substantially unchanged for provenance.

They are intentionally **not cleaned up line-by-line** because doing so would blur the difference between:

- what the original research concluded at the time; and
- what later source audits proved or refuted.

Instead, `docs/research/README.md` quarantines the corpus and records the known superseded conclusions.

This is deliberate documentation hygiene: historical evidence remains inspectable while current authority remains unambiguous.

## 4. Legacy-to-canonical reconciliation matrix

### `RUNELITE_RENDER_SOURCES.md`

**Retained value**

- broad source discovery;
- file-family inventory;
- early implementation scoping.

**No longer authoritative for**

- crate ownership;
- completeness claims;
- source revision equivalence;
- required implementation order.

**Canonical owners**

- `docs/blueprint/03-SOURCE-GROUP-INVENTORY.md`;
- `docs/verification/SOURCE-PINS.md`;
- `docs/blueprint/17-IMPLEMENTATION-ROADMAP.md`.

### `RUNELITE_GPU_PIPELINE.md`

**Retained value**

- RuneLite GPU pipeline research;
- historical sort/upload behavior;
- shader/source navigation.

**No longer authoritative for**

- RustOSRS buffer ABI;
- fixed texture capacity;
- exact wgpu pass graph;
- reverse-Z ownership;
- renderer/editor boundaries.

**Canonical owners**

- `docs/blueprint/11-RENDERER-ARCHITECTURE.md`;
- `docs/blueprint/12-GPU-DATA-PASSES.md`;
- ADR-0004, ADR-0005, ADR-0006.

### `RUNELITE_SCENE_AND_MATERIALS.md`

**Retained value**

- RuneLite API field discovery;
- scene/material navigation;
- useful historical interpretation.

**Superseded concerns**

- `editor_core` ownership language;
- treating RuneLite extended-scene/roof behavior as universal semantic truth;
- historical normal-continuity conclusions;
- ambiguous plane ownership.

**Canonical owners**

- `docs/specs/`;
- `docs/blueprint/09-SEMANTIC-AUDIT.md`;
- `docs/blueprint/10-TERRAIN-MATERIAL-PLANE-AUDIT.md`;
- `docs/blueprint/05-SYSTEM-ARCHITECTURE.md`.

### `RUNELITE_RUNTIME_RULES.md`

**Retained value**

- original R1-R27 research trail;
- formulas/source-locator notes that may help investigate regressions.

**Known superseded/problematic areas**

- initial/pending object construction was over-generalized from the pending-spawn path;
- bridge handling was collapsed into broad adjusted-plane language;
- terrain builder attribution to `class470` is stale for the pinned public deob;
- the normal section correctly distinguishes mesh welding but historically led to the false conclusion that separate objects never reconcile normals;
- rule numbers are no longer normative IDs.

**Canonical owners**

- stable spec IDs in `docs/specs/`;
- semantic audit documents;
- verification fixtures mapped in `PARITY-MATRIX.md`.

### `RUNELITE_DEOB_READING_GUIDE.md`

**Retained value**

- deob navigation;
- historical cleaned pseudocode;
- variable/method discovery.

**No longer authoritative for**

- exact source revision identity;
- obfuscated class names across revisions;
- promoted semantic contracts.

**Canonical owners**

- `docs/verification/SOURCE-PINS.md`;
- exact source references inside each spec;
- fixture manifests in `REFERENCE-FIXTURES.md`.

### `RUNELITE_RUST_PORT_NOTES.md`

**Retained value**

- early Rust/wgpu/egui feasibility notes;
- useful historical API/layout ideas.

**Superseded decisions**

- `editor_core` / `editor_cache` / `editor_render` / `editor_app` architecture;
- mandatory wasm portability;
- fixed RuneLite texture-array capacity as a direct Rust contract;
- one monolithic port mapping from Java/OpenGL structure to Rust;
- any implication that editor or renderer types may define reusable semantics.

**Canonical owners**

- ADR-0001 through ADR-0009;
- `docs/blueprint/06-CRATE-ARCHITECTURE.md`;
- renderer/editor blueprint documents.

### `RUNELITE_HARDEST_PARTS.md`

**Retained value**

- historical risk analysis;
- useful reminders about fixed-point lighting, priorities, terrain, UVs, contouring, and GPU interop.

**No longer authoritative for**

- work ordering;
- acceptance gates;
- completeness status.

**Canonical owners**

- `docs/blueprint/17-IMPLEMENTATION-ROADMAP.md`;
- `docs/verification/PARITY-MATRIX.md`;
- `CI-FUZZ-BENCHMARKS.md`.

### `RUNELITE_CACHE_STACK.md`

**Retained value**

- rs-cache/FileStore capability investigation;
- opcode gap discovery;
- evidence that the existing `rs-cache` dependency is useful but incomplete.

**Superseded decisions**

- “FileStore is the spec”;
- “use rs-cache as the running foundation” as an already accepted architecture decision;
- renderer constants listed alongside decoder defaults as if they share one ownership domain.

**Canonical owners**

- M1 in `docs/blueprint/17-IMPLEMENTATION-ROADMAP.md`;
- implementation-era cache strategy ADR;
- semantic field requirements in `docs/specs/`;
- evidence hierarchy in `01-EVIDENCE-STATUS.md`.

## 5. High-risk claim reconciliation

### 5.1 Cross-model normals

**Legacy ambiguity:** separate meshes were interpreted as strictly independent normals.

**Canonical result:** eligible static `ModelData` instances can reconcile coincident vertex normals before final lighting without mesh welding.

Owners:

- `NORMALS-001..004`;
- `09-SEMANTIC-AUDIT.md`;
- verification rows and golden scene for normal reconciliation.

### 5.2 Static versus dynamic construction

**Legacy ambiguity:** the pending-spawn branch was described too broadly as the universal object-build path.

**Canonical result:** initial region construction and pending/live replacement are distinct semantic pipelines.

Owners:

- `LOC-PLACEMENT-004`;
- `MODEL-BUILD-004`;
- `NORMALS-003`.

### 5.3 Bridge planes

**Legacy ambiguity:** one bridge-adjusted-plane helper appeared sufficient for placement, heights, collision, storage, and visibility.

**Canonical result:** these domains are distinct.

Owners:

- `PLANES-001..004`.

### 5.4 Terrain color builder

**Legacy claim:** complete slope/11x11/overlay builder was source-pinned to `class470`.

**Canonical result:** the pin is stale for the public target source and the complete builder remains revision-sensitive.

Owners:

- `TERRAIN-004`;
- C-021;
- `SOURCE-PINS.md`.

### 5.5 Crate architecture

**Legacy claim:** `editor_*` crates own the implementation.

**Canonical result:** reusable semantics are OSRS-owned crates, with editor composition at the top.

Owners:

- ADR-0001;
- `06-CRATE-ARCHITECTURE.md`.

### 5.6 Web/wasm requirement

**Legacy claim:** wasm portability is mandatory.

**Canonical result:** native desktop is first-class and wasm is deferred.

Owner: ADR-0002.

### 5.7 Texture count/capacity

**Legacy claim:** RuneLite's fixed array count is an implementation constant suitable for direct propagation.

**Canonical result:** semantic texture IDs are independent of renderer allocation capacity.

Owners:

- `TEXTURE-001`;
- `12-GPU-DATA-PASSES.md`.

### 5.8 Cache dependency

**Legacy claim:** hybrid rs-cache + FileStore port is the selected solution.

**Canonical result:** it is a candidate strategy only. M1 performs a target-profile compatibility spike and records the real decision in an ADR.

Owner: M1 roadmap.

## 6. Requirement-to-evidence-to-test closure

Every normative requirement must now terminate in both an authority source and verification ownership.

### OSRS semantic requirement

Required chain:

```text
atomic spec ID
  -> exact evidence/source pin
  -> parity level
  -> PARITY-MATRIX row
  -> fixture/test family
  -> implementation milestone exit gate
```

Example:

```text
NORMALS-002
  -> ModelData source pin
  -> P1 semantic parity
  -> positive/negative/hide-face differential fixtures
  -> M7 exit gate
```

### Renderer requirement

Required chain:

```text
accepted ADR / renderer blueprint contract
  -> semantic input specs consumed
  -> renderer-policy verification row
  -> M10-M13 / M17 exit gate
```

Example:

```text
ADR-0005 ordered side path
  -> FACE-002 priority metadata/order
  -> exact priority emission fixture
  -> M12 exit gate
```

### Editor requirement

Required chain:

```text
accepted ADR / editor blueprint contract
  -> semantic command boundary
  -> V5 editor verification requirement
  -> M14-M16 exit gate
```

Example:

```text
ADR-0008 command history
  -> exact semantic apply/revert
  -> randomized history state-machine tests
  -> M14/M15 exit gates
```

### Revision-sensitive requirement

Required chain:

```text
REVISION_SENSITIVE spec/register item
  -> explicit unresolved evidence gate
  -> BLOCKED verification row when applicable
  -> roadmap branch/limitation
```

It must **not** terminate in an implementation guess.

## 7. Documentation ownership rules going forward

### Semantic changes

When OSRS behavior is newly proven or corrected:

1. update/add the atomic spec;
2. update source pins;
3. update parity/fixture ownership;
4. update contradiction register if a dispute is resolved;
5. update roadmap only when milestone scope/ordering changes.

Do not make a root research file normative by editing it.

### Project design changes

When RustOSRS changes architecture/product behavior:

1. add an ADR that supersedes the old decision;
2. update owning blueprint docs;
3. update verification ownership;
4. update roadmap if implementation sequence changes.

### Research discoveries

Research notes may be added without immediate promotion, but they must be labeled `RESEARCH`, `HYPOTHESIS`, `DISPUTED`, or `REVISION_SENSITIVE` until promotion requirements are met.

## 8. Generated/API/reference documentation

`docs/api/`, imported source trees, reference shaders, and tool-generated pages are navigation/evidence resources.

They are not specification authority merely because they are generated from source.

A final behavioral claim must point to the exact source/revision through the owning spec or source-pin record.

## 9. No hidden implementation requirements

The following are explicitly prohibited as sources of undocumented requirements:

- comments inside legacy research docs;
- generated API prose without a canonical owner;
- screenshots;
- shader constants copied without ownership analysis;
- local filesystem paths;
- human-readable snapshot dates without hashes;
- obfuscated class names without a pinned revision;
- “looks right” visual fixes;
- implementation choices that never received an ADR when an ADR is required.

## 10. Reconciliation result

After Checkpoint 9:

- `index.md` is a canonical documentation hub;
- root research documents are explicitly quarantined;
- known contradictory/superseded claims are mapped to canonical replacements;
- OSRS semantics use stable spec IDs rather than R1-R27 as normative identifiers;
- architecture choices use ADRs;
- all normative behavior has verification ownership or an explicit blocked gate;
- the implementation roadmap consumes canonical specs/ADRs/verification rather than legacy prose.

The remaining work before merge is repository-wide merge-readiness review, link/status consistency, obvious housekeeping, and final branch/PR validation in Checkpoint 10.
