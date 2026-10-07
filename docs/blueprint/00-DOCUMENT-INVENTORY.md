# RustOSRS Blueprint Evidence Inventory

Status: **Checkpoint 1 working document**

This file inventories the material that existed before the RustOSRS implementation blueprint was started. It deliberately does **not** treat existing prose as specification truth.

The inventory is pinned to repository `main` commit:

`489b0603bc7c2c9fabd2614d204f809fb017d0cb`

The blueprint work itself lives on:

`blueprint/osrs-editor-foundation`

## 1. Classification model

Every source used by the final blueprint must be classified before it can create an implementation requirement.

| Class | Meaning | May directly define a final requirement? |
|---|---|---:|
| **Executable evidence** | A behavior was executed and its output captured or can be reproduced by a harness. | Yes, after provenance and scope are verified. |
| **Primary source** | The imported OSRS/deob, RuneLite, FileStore, or other implementation source that contains the behavior. | Yes, after the relevant path and revision are pinned. |
| **Generated reference** | API/reference documentation generated from primary source. | Only as a navigation aid. The underlying source remains authoritative. |
| **Research synthesis** | Human/agent-authored Markdown describing conclusions drawn from primary sources. | No. It must be revalidated before promotion into a spec. |
| **Port proposal** | A recommendation for Rust architecture, crate layout, wgpu state, dependency choice, UI, or implementation order. | No. It requires an architecture decision record or blueprint decision. |
| **Hypothesis / open question** | A behavior that is uncertain, disputed, revision-sensitive, or incompletely tested. | No. |

### Core rule

A Markdown statement does not become true because another Markdown file cites it. Final specs must terminate their evidence chain in a pinned primary source, executable oracle, or an explicit project design decision.

## 2. Repository snapshot inventory

The repository currently functions primarily as a reference corpus. There is no root Rust workspace yet.

Important imported/reference trees at the pinned baseline:

| Path | Baseline tree SHA | Classification | Blueprint treatment |
|---|---|---|---|
| `runelite-master/` | `5afef996a992bacc73655860681269242e90d6e8` | Primary source | RuneLite API/client/GPU implementation reference. Pin individual files used by specs. |
| `OpenRune-FileStore-main/` | `55f571db4b23f2d528786e1cdfbcba0061fd201a` | Primary source / tooling reference | Decoder/opcode and cache-tooling reference. Do not assume its names or defaults are correct for every target revision. |
| `rs-cache-master/` | `fae41f98352fc804e5d13d9bd2e836ab1e2635cd` | Primary source / candidate dependency | Existing Rust cache implementation. Suitability is a design decision, not settled truth. |
| `reference-fixtures/` | `7aa46bc0c0f2611d4f999e5a310ad3243b6c0491` | Executable evidence | Preserve and expand. Fixtures need provenance metadata and direct mapping to specs/tests. |
| `reference-shaders/` | `bcf531292db5b5a95c9d9298e9f3dc28e11309d7` | Primary-source extract | Shader behavior reference. GLSL/OpenGL behavior must be translated semantically, not transliterated blindly to WGSL/wgpu. |
| `tools/` | `a27903aba0e68bfb0728164d686f69547ddb517a` | Verification/tooling | Contains the deob harness and RuneLite reference tooling. These should become first-class verification infrastructure. |
| `docs/api/` | `9000331d0d84ee56462bffc07b70cef500096de9` | Generated reference | Navigation only. Never use generated prose as the sole proof for a semantic requirement. |

## 3. Existing top-level research documents

All current `RUNELITE_*.md` documents are initially classified as **research synthesis**, even when they contain source pins or executed observations. Individual claims may later be promoted into atomic specs after verification.

| Document | Current role | Initial disposition |
|---|---|---|
| `index.md` | Existing map/index of the reference corpus | Keep as legacy research index until the new blueprint index supersedes it. |
| `RUNELITE_RENDER_SOURCES.md` | Rendering source inventory and proposed port mapping | Valuable source map. Revalidate completeness and separate OSRS semantics from RuneLite renderer implementation. |
| `RUNELITE_GPU_PIPELINE.md` | RuneLite GPU pipeline analysis | Strong renderer research. Must not be mistaken for the only valid renderer architecture. |
| `RUNELITE_SCENE_AND_MATERIALS.md` | Scene/material contract analysis | Strong semantic research, but several claims require re-audit before becoming specs. |
| `RUNELITE_RUNTIME_RULES.md` | Consolidated runtime rules R1-R27 | Highest-risk document to treat as truth. Convert verified rules into atomic specs; retire rule-number authority afterward. |
| `RUNELITE_DEOB_READING_GUIDE.md` | Deob algorithm recovery and pseudocode | Useful navigation and derivation record. Algorithms must be checked against the pinned source and/or harness. |
| `RUNELITE_RUST_PORT_NOTES.md` | Proposed Rust/wgpu/egui architecture | Port proposal, not specification. Crate boundaries, wasm requirements, dependencies, and GPU choices need explicit ADRs. |
| `RUNELITE_HARDEST_PARTS.md` | Risk analysis and implementation order | Useful planning input. Rebuild the risk list after semantic audit. |
| `RUNELITE_CACHE_STACK.md` | Cache implementation comparison and recommendation | Strong dependency research, but the hybrid `rs-cache` decision is not final until cache requirements are formalized. |

## 4. Executable evidence already present

### `reference-fixtures/deob_golden.txt`

Current strengths:

- executed terrain shape and rotation outputs
- orientation/vector tables
- contour examples
- lighting/color-related fixtures
- direct values suitable for exact integer tests

Current limitations:

- one monolithic text fixture has weak traceability to individual future specs
- fixture generation provenance is not encoded beside each case
- coverage is not yet sufficient for the project's desired parity standard
- visual scene-level parity is not represented by this file alone

Required blueprint action: split or index these fixtures by semantic contract, document how each is regenerated, and attach every promoted spec to at least one executable or source-level verification path where practical.

### `tools/deob-harness/`

Classification: **verification infrastructure**.

This is strategically important. The final architecture should favor differential Java/deob versus Rust verification for integer math, transforms, terrain construction, normals, lighting, contouring, and other semantics that are otherwise difficult to diagnose visually.

### `tools/runelite-mcp/`

Classification: **reference-navigation tooling**.

Useful for research, but not an oracle by itself. Results must still point to pinned source material.

## 5. Known claims that are not yet allowed into the final spec

The following items are explicitly quarantined until re-audited.

### Q1. Cross-model normal behavior / `mergeNormals`

Current research states that normals are per-model and that no cross-model welding/normal merging occurs. That statement is too broad to accept without a fresh audit.

The blueprint must separately answer:

1. Are meshes/topology ever welded between scene objects?
2. Are vertex normals ever accumulated or reconciled across distinct `ModelData` instances before lighting?
3. Which object-definition field(s), placement types, shading modes, or scene-build paths gate that behavior?
4. Does the behavior vary by revision or static/dynamic path?
5. What exact wall/corner fixture proves the answer?

Until those questions are settled, any existing "no normal welding" or "creases between models are authentic" statement is research only.

### Q2. Editor-prefixed crate architecture

The existing `editor_core`, `editor_cache`, `editor_render`, `editor_app` proposal couples reusable OSRS semantics to the editor product name. Because the foundation may later support a Rust OSRS client/reference renderer, the blueprint will evaluate reusable `osrs-*` crates with the editor as a consumer.

No crate map is final yet.

### Q3. WebAssembly as a foundation requirement

Existing notes require `editor_core` to compile for `wasm32-unknown-unknown` and propose browser fallbacks. The project goal currently centers on a leading-class native map editor using Rust, wgpu, eframe/egui, and Catppuccin. Web support may be valuable, but it must not distort core architecture unless explicitly accepted as a product requirement.

Status: **design decision pending**.

### Q4. `rs-cache` as the runtime cache foundation

The current hybrid recommendation is plausible and backed by useful research, but dependency selection will be made only after the final cache contract lists all required revision, model, texture, overlay, underlay, morph, animation, XTEA, and write/export capabilities.

Status: **candidate, not selected**.

### Q5. RuneLite GPU behavior versus OSRS semantic behavior

RuneLite's GPU plugin is invaluable reference material, but it contains renderer policy in addition to OSRS content semantics. The blueprint will classify every requirement into one of three domains:

- **OSRS semantic parity**: accidental differences are bugs.
- **Renderer policy**: deliberate visual/technical improvements are allowed and documented.
- **Editor policy**: UX/workflow behavior owned entirely by RustOSRS.

This boundary is mandatory before implementation.

## 6. Evidence hierarchy for the final blueprint

When sources disagree, use this provisional order while investigating:

1. Reproducible behavior from the exact targeted OSRS/deob snapshot.
2. Exact primary-source path responsible for the behavior in that snapshot.
3. Multiple corroborating primary implementations where they are semantically comparable.
4. Executed golden fixture generated from the pinned source.
5. Generated API documentation.
6. Existing research Markdown.
7. Assumption or visual intuition.

This hierarchy does not eliminate judgment. RuneLite GPU code may intentionally differ from the software client, and imported cache libraries may encode old-revision assumptions. Conflicts must be recorded rather than silently resolved.

## 7. Promotion rule for future specs

A behavior may be promoted from research to a normative spec only when the spec records:

- semantic domain: OSRS parity, renderer policy, or editor policy
- status: verified, derived, project decision, disputed, or pending
- target/source revision or pinned tree/blob
- exact source path and method/field/table where applicable
- inputs and outputs
- coordinate/unit conventions
- integer/rounding/overflow semantics where applicable
- invariants and known exceptions
- failure appearance or diagnostic symptom
- required unit/differential/integration/golden test
- related specs and ADRs

A final implementation requirement must never cite only another summary document.

## 8. Checkpoint 1 remaining work

Before Checkpoint 1 is complete:

- inventory generated API/reference groups without pretending each generated page is independent evidence
- inventory shader families and verification tools at group level
- record source pin/provenance information needed to reproduce the imported trees
- identify duplicate, superseded, contradictory, and revision-sensitive claims across the current Markdown set
- create the blueprint documentation index and evidence-status vocabulary
- produce the first contradiction/open-question register for Checkpoint 2 and the semantic audit

No implementation code should be scaffolded during this checkpoint.