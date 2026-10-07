# RustOSRS Blueprint

This directory is the canonical home for the RustOSRS architecture and specification program.

Existing root-level `RUNELITE_*.md` documents remain research inputs unless an individual claim has been revalidated and promoted into `docs/specs/` or an accepted ADR.

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
| `09-SEMANTIC-AUDIT.md` | Loc/model construction, normals, lighting, transforms, morphs, contouring, priority/alpha audit | Checkpoint 3A audit record |
| `10-TERRAIN-MATERIAL-PLANE-AUDIT.md` | Terrain topology/materials, bridges/planes, roofs, UVs, camera/coordinates, decoder-contract audit | Checkpoint 3B complete; closes Checkpoint 3 |

Canonical atomic semantic contracts now live under `docs/specs/`.

Architecture decisions live in `docs/adr/`.

Accepted so far:

- `ADR-0001-reusable-osrs-crate-boundaries.md`
- `ADR-0002-native-first-editor.md`
- `ADR-0003-semantic-renderer-editor-boundaries.md`

Source provenance and revision gates live in `docs/verification/SOURCE-PINS.md`.

## Canonical semantic spec set

Checkpoint 4 promoted verified audit findings into stable atomic specification IDs.

Current registry:

- `docs/specs/README.md`
- `docs/specs/loc-placement.md`
- `docs/specs/model-build.md`
- `docs/specs/normals-lighting.md`
- `docs/specs/morph-animation-contouring.md`
- `docs/specs/terrain.md`
- `docs/specs/planes-bridges.md`
- `docs/specs/face-materials.md`
- `docs/specs/coordinates.md`

The spec set covers:

- loc type dispatch, wall/decor placement, footprints, initial-vs-runtime construction, placement side effects, and floor-decoration height behavior;
- exact model selection, mirroring/winding, transform order, static `nonFlatShading` ownership, and cache immutability;
- base normals, cross-model normal accumulation, scene ModelData finalization, merged-normal precedence, and reference object lighting;
- varbit/varp morph resolution, dynamic model ownership, animation-state model resolution, and exact integer contouring;
- terrain shapes `0..12`, flat/shaped terrain contracts, floor definition decoding, and the explicit terrain-color revision gate;
- collision/source/storage/render plane separation and structural bridge relinking;
- semantic face metadata, priority meaning, alpha, authored face bias, and texture/material inputs;
- local-unit, angular, and coordinate-space contracts.

Each atomic spec carries an evidence status, source pin, required behavior, scope, integer semantics where relevant, invariants, failure signature, required tests, and related spec links.

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

`REVISION_SENSITIVE` is a real implementation gate. It must not be treated as shorthand for "probably correct." The current full terrain-color/11x11 builder is the primary example: its component inputs are partially verified, but the stale `class470` source attribution prevents unconditional promotion.

## Work sequence

| Checkpoint | Scope | Status |
|---|---|---|
| 0 | Repository audit | Complete |
| 1 | Documentation/evidence inventory | Complete |
| 2 | Truth model + architecture | Complete |
| 3 | Rendering semantic audit | Complete |
| 4 | Canonical OSRS specifications | Complete |
| 5 | Rust/wgpu renderer blueprint | Not started |
| 6 | Editor blueprint | Not started |
| 7 | Verification blueprint | Not started |
| 8 | Implementation roadmap | Not started |
| 9 | Documentation reconciliation | Not started |
| 10 | Merge readiness | Not started |

## Current architecture decisions

Checkpoint 2 established:

- reusable lower-level crates are OSRS-owned, not editor-owned;
- `osrs-scene` does not depend on a concrete cache implementation;
- `osrs-render` consumes semantic truth rather than defining it;
- `osrs-editor` is the first composition root and owns product workflow only;
- `osrs-reference` is development/test infrastructure, never a production runtime dependency;
- shared decoded assets are immutable by default;
- semantic edits precede render invalidation/GPU updates;
- revision-sensitive behavior remains explicit through a target/parity model;
- exact semantic tests cannot be replaced by screenshot tolerance;
- native desktop is first-class; wasm support is deferred;
- editor non-client scope does not prohibit future client reuse of shared crates.

## Checkpoint 3 semantic corrections retained by Checkpoint 4

The canonical spec set preserves the major source-audit corrections:

- eligible static `ModelData` objects can accumulate normals across separate models before final lighting; separate mesh topology does not imply independent lighting normals;
- initial region construction and pending-spawn replacement are distinct construction pipelines;
- model selection has no generic fallback-to-first-model behavior;
- mirroring includes winding semantics and is not a renderer negative-scale shortcut;
- transform order, morph resolution, contouring, and face-priority semantics are exact contracts;
- terrain shape topology `0..12` is verified while the old `class470` pin for the complete terrain-color builder is stale;
- bridge behavior is not one universal plane adjustment;
- the old generic `+1/+2` ground-decoration lift claim is not present in the audited path;
- RuneLite roof IDs/removal ranges, extended-scene capacity, reverse-Z, zone upload strategy, and similar mechanisms remain renderer/runtime policy unless independently proven otherwise;
- face priority, alpha, texture metadata, and authored face bias survive the semantic-to-render boundary even when Rust uses a different GPU implementation;
- decoder widths/capacities follow the selected target revision rather than old fixed-width assumptions.

## Checkpoint 5 entry condition

Checkpoint 5 may now design the Rust/wgpu renderer against the canonical semantic interface.

It must not redefine any `OSRS_SEMANTIC` requirement to make GPU implementation easier. Where the renderer intentionally differs from reference presentation, the decision must be ADR-backed and parity-testable beneath that presentation layer.

Checkpoint 5 is expected to define, among other things:

- render-neutral scene-to-GPU contracts;
- wgpu resource ownership and lifetime;
- static/dynamic geometry paths;
- priority/transparency strategy;
- depth convention and authored bias application;
- terrain/model material representation;
- texture animation;
- scene streaming/dirty rebuild strategy;
- picking support required by the editor;
- diagnostic render modes and GPU validation boundaries.

## Current branch policy

All blueprint work is being accumulated on one branch:

`blueprint/osrs-editor-foundation`

No pull request should be opened until the complete blueprint branch is ready for merge.