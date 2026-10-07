# RustOSRS Blueprint

This directory is the canonical home for the new RustOSRS architecture and specification program.

Existing root-level `RUNELITE_*.md` documents remain research inputs until individual claims are revalidated and promoted.

## Current blueprint documents

| Document | Purpose | Current status |
|---|---|---|
| `00-DOCUMENT-INVENTORY.md` | Inventory and classification of the pre-blueprint corpus | Checkpoint 1 complete |
| `01-EVIDENCE-STATUS.md` | Evidence/status vocabulary and promotion rules | Foundation |
| `02-CONTRADICTION-REGISTER.md` | Conflicts, revision hazards, and open questions that block normative specs | Active register |
| `03-SOURCE-GROUP-INVENTORY.md` | Group-level source, shader, fixture, tooling, and provenance inventory | Checkpoint 1 complete |
| `04-PROJECT-CHARTER.md` | Mission, product scope, quality gates, parity ownership | Checkpoint 2 complete |
| `05-SYSTEM-ARCHITECTURE.md` | Layering, data flow, ownership, determinism, mutation boundaries | Checkpoint 2 complete |
| `06-CRATE-ARCHITECTURE.md` | Reusable `osrs-*` crates and dependency rules | Checkpoint 2 complete |
| `07-ARCHITECTURE-INVARIANTS.md` | Enforceable architecture guardrails | Checkpoint 2 complete |
| `08-PARITY-MODEL.md` | Target profiles, truth categories, parity levels, exact-vs-tolerance rules | Checkpoint 2 complete |
| `09-SEMANTIC-AUDIT.md` | Loc/model construction, normals, lighting, transforms, morphs, contouring, priority/alpha audit | Checkpoint 3A complete |
| `10-TERRAIN-MATERIAL-PLANE-AUDIT.md` | Terrain topology/materials, bridges/planes, roofs, UVs, camera/coordinates, decoder-contract audit | Checkpoint 3B complete |

Architecture decisions live in `docs/adr/`.

Accepted so far:

- `ADR-0001-reusable-osrs-crate-boundaries.md`
- `ADR-0002-native-first-editor.md`
- `ADR-0003-semantic-renderer-editor-boundaries.md`

Source provenance and revision gates live in `docs/verification/SOURCE-PINS.md`.

## Planned blueprint set

The final system is expected to cover:

1. project charter and parity boundaries
2. truth/evidence model
3. system architecture
4. crate architecture and dependency rules
5. cache architecture
6. canonical scene architecture
7. rendering architecture
8. editor architecture
9. diagnostics and observability
10. verification strategy
11. implementation roadmap
12. atomic OSRS semantic specifications
13. architecture decision records
14. source pins and golden-scene/fixture indexes

## Canonical rule

A root research document may help locate evidence, but it cannot be the terminal authority for a final implementation requirement.

Final semantic specs must end their evidence chain in pinned source and/or executable reference behavior. Project-owned renderer/editor behavior must end in an explicit architecture decision.

## Work sequence

| Checkpoint | Scope | Status |
|---|---|---|
| 0 | Repository audit | Complete |
| 1 | Documentation/evidence inventory | Complete |
| 2 | Truth model + architecture | Complete |
| 3 | Rendering semantic audit | Complete |
| 4 | Canonical OSRS specifications | Not started |
| 5 | Rust/wgpu renderer blueprint | Not started |
| 6 | Editor blueprint | Not started |
| 7 | Verification blueprint | Not started |
| 8 | Implementation roadmap | Not started |
| 9 | Documentation reconciliation | Not started |
| 10 | Merge readiness | Not started |

## Current architecture decisions

Checkpoint 2 established:

- reusable lower-level crates are OSRS-owned, not editor-owned
- `osrs-scene` does not depend on a concrete cache implementation
- `osrs-render` consumes semantic truth rather than defining it
- `osrs-editor` is the first composition root and owns product workflow only
- `osrs-reference` is development/test infrastructure, never a production runtime dependency
- shared decoded assets are immutable by default
- semantic edits precede render invalidation/GPU updates
- revision-sensitive behavior must remain explicit through a target/parity model
- exact semantic tests cannot be replaced by screenshot tolerance
- native desktop is first-class; wasm support is deferred
- editor non-client scope does not prohibit future client reuse of shared crates

## Checkpoint 3 semantic corrections

Checkpoint 3 source-level verification established several implementation-critical corrections to the pre-blueprint research:

- eligible static `ModelData` objects can merge/accumulate normals across separate models before final lighting; separate mesh topology does not imply independent lighting normals
- initial region construction and pending-spawn replacement are distinct model-construction pipelines
- model selection, mirroring, transform order, morph resolution, contouring, and priority semantics must remain exact semantic contracts
- terrain shape topology `0..12` is independently verified, while the old `class470` pin for the complete terrain-color builder is stale and the complete 11x11/color-build path remains revision-gated
- bridge behavior is not one universal plane adjustment; source/storage plane, collision plane, render level, linked-below state, and renderer roof/VIS_BELOW grouping are separate mechanisms
- the old generic `+1/+2` ground-decoration lift claim is not present in the audited placement/storage/upload path
- RuneLite roof IDs/removal ranges, the 184x184 extended scene, reverse-Z, zone upload strategy, and similar mechanisms must be classified as renderer/runtime policy rather than cache truth unless independently proven otherwise
- object/terrain UV behavior, texture-animation inputs, transparency, authored face bias, and face priority metadata must survive the semantic-to-render boundary even if Rust uses a different GPU implementation
- decoder widths/capacities must follow the selected target revision rather than being frozen to older 16-bit/fixed-array assumptions

A completed audit does not promote every claim to `VERIFIED`. `REVISION_SENSITIVE` rows remain blocked from normative specs until their source/fixture requirements are satisfied.

## Checkpoint 4 entry condition

Checkpoint 4 may now convert verified audit rows into atomic OSRS specifications.

Each specification must carry:

- evidence status
- semantic domain
- exact source pin(s)
- required behavior and integer semantics
- known scope/exceptions
- fixture/test ownership
- failure signature
- cross-links to related specs

Renderer-reference behavior must remain visibly distinct from OSRS semantic truth, and project-owned choices must remain ADR-backed.

## Current branch policy

All blueprint work is being accumulated on one branch:

`blueprint/osrs-editor-foundation`

No pull request should be opened until the complete blueprint branch is ready for merge.