# RustOSRS Blueprint Evidence Inventory

Status: **Checkpoint 1 complete**

This inventory classifies the material that existed before the RustOSRS implementation blueprint was started. It deliberately does **not** treat existing prose as specification truth.

Baseline repository snapshot:

`489b0603bc7c2c9fabd2614d204f809fb017d0cb`

Blueprint branch:

`blueprint/osrs-editor-foundation`

See also:

- `01-EVIDENCE-STATUS.md`
- `02-CONTRADICTION-REGISTER.md`
- `03-SOURCE-GROUP-INVENTORY.md`

## 1. Classification model

| Class | Meaning | May directly define a final requirement? |
|---|---|---:|
| **Executable evidence** | Behavior executed against a pinned reference and captured reproducibly | Yes, after provenance/scope verification |
| **Primary source** | Imported deob, RuneLite, FileStore, cache, or implementation source containing the behavior | Yes, after relevant revision/path is pinned |
| **Generated reference** | API/reference documentation generated from primary source | Navigation only |
| **Research synthesis** | Human/agent-authored Markdown summarizing conclusions | No, must be revalidated |
| **Port proposal** | Rust/wgpu/egui/dependency/architecture recommendation | No, requires project decision/ADR |
| **Hypothesis/open question** | Uncertain, disputed, revision-sensitive, or incomplete behavior | No |

### Core rule

A Markdown statement does not become true because another Markdown file cites it. Final specs must terminate their evidence chain in pinned primary source, executable oracle behavior, or an explicit RustOSRS project decision.

## 2. Repository snapshot inventory

The repository currently functions primarily as a reference corpus. There is no root Rust implementation workspace yet.

| Path | Baseline tree SHA | Classification | Blueprint treatment |
|---|---|---|---|
| `runelite-master/` | `5afef996a992bacc73655860681269242e90d6e8` | Primary/reference implementation | Newer RuneLite API/client/GPU reference. Pin individual files used by specs. |
| `OpenRune-FileStore-main/` | `55f571db4b23f2d528786e1cdfbcba0061fd201a` | Primary/reference implementation | Decoder/opcode/tooling reference. Not canonical without target-revision verification. |
| `rs-cache-master/` | `fae41f98352fc804e5d13d9bd2e836ab1e2635cd` | Candidate Rust dependency/reference | Existing cache implementation. Suitability remains undecided. |
| `reference-fixtures/` | `7aa46bc0c0f2611d4f999e5a310ad3243b6c0491` | Executable evidence | Preserve, index, add provenance, and expand. |
| `reference-shaders/` | `bcf531292db5b5a95c9d9298e9f3dc28e11309d7` | Renderer reference/historical evidence | Classify shader families before any WGSL port. |
| `tools/` | `a27903aba0e68bfb0728164d686f69547ddb517a` | Verification/navigation infrastructure | Promote differential testing to first-class architecture. |
| `docs/api/` | `9000331d0d84ee56462bffc07b70cef500096de9` | Generated reference | Navigation only. Underlying source remains authoritative. |

Detailed group classification is in `03-SOURCE-GROUP-INVENTORY.md`.

## 3. Existing top-level research documents

All current `RUNELITE_*.md` documents are initially **research synthesis**, even when they contain source pins or executed observations. Individual claims may later be promoted into atomic specs after verification.

| Document | Current role | Initial disposition |
|---|---|---|
| `index.md` | Existing corpus navigation | Legacy research index until superseded |
| `RUNELITE_RENDER_SOURCES.md` | Rendering source inventory and port mapping | Valuable map, but mixes semantics and renderer implementation |
| `RUNELITE_GPU_PIPELINE.md` | RuneLite GPU pipeline analysis | Renderer research, not mandatory architecture |
| `RUNELITE_SCENE_AND_MATERIALS.md` | Scene/material contract analysis | Strong research with several claims requiring re-audit |
| `RUNELITE_RUNTIME_RULES.md` | Consolidated R1-R27 runtime claims | Highest-risk file to mistake for canonical spec |
| `RUNELITE_DEOB_READING_GUIDE.md` | Deob algorithm recovery/pseudocode | Useful derivation/navigation layer |
| `RUNELITE_RUST_PORT_NOTES.md` | Proposed Rust/wgpu/egui architecture | Port proposal, not specification |
| `RUNELITE_HARDEST_PARTS.md` | Risk analysis/implementation order | Planning input only |
| `RUNELITE_CACHE_STACK.md` | Cache implementation comparison | Strong dependency research, decision not final |

## 4. Executable evidence already present

### `reference-fixtures/deob_golden.txt`

Current strengths include exact executed terrain tables/triangulation, orientation/offset tables, color functions, contouring, synthetic lighting, wall/decor/floor storage, and multi-tile placement behavior.

Current weaknesses:

- monolithic fixture format
- limited per-case provenance
- no atomic future spec IDs
- incomplete normals/priority/animation/morph coverage
- no complete fixed-camera golden visual scenes

### `tools/deob-harness/`

Classification: **verification infrastructure / executable oracle harness**.

The final architecture should use Java/deob versus Rust differential verification for integer math, transforms, terrain construction, normals, lighting, contouring, and other semantics that are difficult to diagnose visually.

### `tools/runelite-mcp/`

Classification: **reference-navigation tooling**.

It accelerates source discovery but cannot be the terminal authority for a requirement.

## 5. Claims explicitly quarantined

The contradiction register now tracks 20 concrete issues. The highest-priority examples are:

### Cross-model normals / `mergeNormals`

Existing research conflates separate meshes with separate lighting normals. Topological welding, vertex-normal calculation, cross-model normal accumulation, face hiding, cache ownership, and lighting conversion must be audited independently.

### Mixed source revisions

The corpus uses October 2026 RuneLite material together with January 2026 melxin/deob and historical compute/priority shaders. Cross-revision evidence must be carried explicitly into every promoted spec.

### Cache dependency selection

`rs-cache` is a useful candidate but has material known gaps and older-revision lineage. The cache contract must be specified before dependency selection becomes normative.

### Renderer versus semantic behavior

Reverse-Z, MSAA, anisotropy, GPU allocation strategy, colorblind processing, region filtering, thread count, and similar RuneLite GPU/plugin features are renderer/editor policy unless separately proven to encode OSRS semantic behavior.

### Editor-prefixed reusable architecture

Existing `editor_*` crate proposals are not final because the reusable foundation should remain suitable for a possible future Rust OSRS client/reference implementation.

### Mandatory wasm support

Not yet accepted as a product requirement. Native editor quality is the current priority; web constraints require an explicit ADR before influencing core architecture.

## 6. Evidence hierarchy

When sources disagree, investigate in this provisional order:

1. reproducible behavior from the exact targeted OSRS/deob snapshot
2. exact primary-source path responsible for that behavior
3. corroborating primary implementations where semantically comparable
4. executed golden fixture generated from pinned source
5. generated API documentation
6. existing research Markdown
7. assumption or visual intuition

This hierarchy does not erase revision differences. Conflicts are recorded rather than silently averaged together.

## 7. Promotion rule for future specs

A behavior may become normative only when its spec records, where applicable:

- semantic domain: OSRS semantic, renderer policy, or editor policy
- evidence status
- target/source revision
- exact source tree/blob/path/method/field/table/opcode
- inputs and outputs
- coordinate/unit conventions
- integer/rounding/overflow semantics
- invariants and exceptions
- expected failure symptom
- required unit/differential/integration/golden test
- related specs and ADRs

A final implementation requirement must never cite only another summary document.

## 8. Checkpoint 1 completion

Checkpoint 1 produced:

- a pinned baseline inventory of all major imported/reference groups
- classification of all existing top-level research documents
- an evidence/status vocabulary
- a mixed-revision handling rule
- group-level inventory of shaders, generated API docs, fixtures, and verification tooling
- a 20-item contradiction/open-question register
- a canonical `docs/blueprint/` index
- explicit quarantine of architecture/dependency assumptions that previously appeared settled

No Rust implementation was scaffolded and no pull request was opened.

The branch is now ready for **Checkpoint 2: truth model + architecture**, where project charter, parity boundaries, crate architecture, dependency direction, and ADR conventions will be defined without resolving the rendering semantic disputes prematurely.