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
| `11-RENDERER-ARCHITECTURE.md` | Renderer extraction, ownership, zone compilation, lifecycle, profiles, picking, diagnostics | Checkpoint 5 complete |
| `12-GPU-DATA-PASSES.md` | GPU ABI, materials, depth/culling, ordered faces, transparency, terrain/model pipelines | Checkpoint 5 complete |
| `13-EDITOR-ARCHITECTURE.md` | eframe/egui/Catppuccin shell, viewport, panels, selection, inspectors, multi-region/plane workflow | Checkpoint 6 complete |
| `14-EDITOR-DOCUMENT-TRANSACTIONS.md` | semantic document, commands, transactions, undo/redo, persistence, autosave/recovery, export boundary | Checkpoint 6 complete |
| `15-EDITOR-TOOLS-INTERACTION.md` | tool lifecycle, placement/terrain tools, gizmos, shortcuts, timeline/morph preview, interaction contracts | Checkpoint 6 complete |

Canonical atomic semantic contracts live under `docs/specs/`.

Architecture decisions live in `docs/adr/`.

Accepted so far:

- `ADR-0001-reusable-osrs-crate-boundaries.md`
- `ADR-0002-native-first-editor.md`
- `ADR-0003-semantic-renderer-editor-boundaries.md`
- `ADR-0004-reverse-z-raster-conventions.md`
- `ADR-0005-zone-compiled-hybrid-rendering.md`
- `ADR-0006-reference-and-enhanced-render-profiles.md`
- `ADR-0007-eframe-egui-editor-shell.md`
- `ADR-0008-command-transaction-history.md`
- `ADR-0009-project-save-export-separation.md`

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

## Renderer blueprint established in Checkpoint 5

Checkpoint 5 defines `osrs-render` as a reconstructible compiler/presentation layer over immutable semantic generations.

Key renderer decisions:

- semantic scene -> immutable render extraction -> render-world compiler -> GPU cache -> frame plan;
- GPU artifacts are generation-tagged and disposable;
- 8x8 tile zones are the primary static compilation/dirty unit;
- camera-dependent priority/transparency remains in an ordered side path instead of being erased by static batching;
- reverse-Z uses `Depth32Float`, clear `0.0`, `GreaterEqual`, CCW fronts, and back-face culling;
- authored face bias is retained and reference mode initially applies the audited clip-depth bias strategy;
- texture capacity is renderer-managed through paged texture arrays rather than a semantic fixed-count assumption;
- animation uses explicit deterministic tick state, not wall-clock-only time;
- reference and enhanced profiles share one semantic scene;
- reference mode preserves semantic baked lighting/color and deterministic parity settings;
- enhanced mode may add MSAA, anisotropy, smoother presentation, and non-destructive visual improvements;
- picking uses generation-scoped renderer IDs mapped back to stable semantic handles;
- diagnostics expose priorities, alpha, bias, planes, zones, normals, depth, IDs, and path classification without mutating semantics;
- GPU/device recreation rebuilds from current extraction state and never requires map re-import.

## Editor blueprint established in Checkpoint 6

Checkpoint 6 defines `osrs-editor` as the native product/composition layer over the semantic and renderer foundations.

Key editor decisions:

- `eframe`/`egui` is the primary native application shell, with Catppuccin as the baseline theme family;
- the viewport reuses the eframe/egui wgpu context through an editor-owned adapter while keeping `osrs-render` egui-independent;
- dockable panels use stable editor panel IDs so a docking library can be replaced without leaking its types through the application;
- persistent state, session/editor state, and disposable derived runtime state are explicitly separate;
- renderer pick IDs resolve back to stable semantic handles and stale generation results are ignored;
- multi-region workspaces use stable world coordinates, and unloaded neighbors are never treated as empty map data;
- source/storage/render/collision plane distinctions remain inspectable rather than being collapsed into one UI integer;
- persistent edits execute only through semantic commands/transactions;
- undo/redo restores exact semantic values and does not depend on GPU state;
- interactive gestures either use reversible live transactions or non-destructive overlay previews;
- loc movement/orientation commits only to target-representable semantic tile/orientation operations;
- terrain strokes are deterministic, frame-rate independent, exact, and grouped by user intent;
- project save, autosave/recovery, and target map/cache export are distinct workflows and dirty states;
- background scene/render/validation/save/export work operates on immutable generation-tagged snapshots;
- preview state such as morph-driving varbits/varps, animation time, deterministic tick, roof visibility, plane visibility, diagnostics, camera, and render profile is non-destructive by default;
- unsupported/revision-sensitive behavior is surfaced through Problems/provenance UI instead of silently approximated or discarded;
- menus, toolbars, context menus, command palette, and shortcuts route through one stable action registry.

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
| 5 | Rust/wgpu renderer blueprint | Complete |
| 6 | Editor blueprint | Complete |
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

Checkpoint 5 added renderer decisions without changing semantic ownership:

- reverse-Z and canonical raster conventions are renderer policy;
- zone compilation and GPU buffers are disposable derivatives;
- ordered priority/transparency handling consumes semantic metadata rather than redefining it;
- reference and enhanced profiles are presentation variants over the same semantic scene.

Checkpoint 6 added editor decisions without changing lower-level ownership:

- eframe/egui/Catppuccin own product chrome and interaction only;
- editor commands mutate semantic document state first;
- background work consumes immutable generation snapshots;
- project persistence is not a replacement cache format;
- save/autosave/export remain distinct;
- previews and diagnostics are non-destructive unless an explicit semantic command commits an edit.

## Semantic corrections retained by renderer and editor design

The product blueprint preserves the major source-audit/spec corrections:

- eligible static `ModelData` objects can accumulate normals across separate models before final lighting; renderer/editor layers consume the finalized semantic result rather than inventing their own normal behavior;
- initial region construction and pending-spawn replacement are distinct construction pipelines;
- model selection has no generic fallback-to-first-model behavior, including editor placement previews;
- mirroring includes winding semantics and is not a renderer/editor negative-scale shortcut;
- transform order, morph resolution, contouring, and face-priority semantics remain exact contracts;
- terrain shape topology `0..12` is verified while the old `class470` pin for the complete terrain-color builder remains gated;
- bridge behavior is not one universal plane adjustment;
- the old generic `+1/+2` ground-decoration lift claim is not present in the audited path;
- face priority, alpha, texture metadata, and authored face bias survive every semantic-to-render/editor boundary;
- decoder widths/capacities follow the selected target revision rather than old fixed-width assumptions.

## Checkpoint 7 entry condition

Checkpoint 7 may now define the verification system across semantics, renderer, editor transactions, persistence, and integrated golden scenes.

The verification blueprint must define, among other things:

- source-pinned reference fixture organization;
- differential Java/deob versus Rust execution;
- unit/property/fuzz testing boundaries;
- golden semantic and golden rendered scenes;
- screenshot tolerance policy;
- priority/transparency/reference-profile fixtures;
- normal-merge, terrain, bridge, morph, animation, and contouring cases;
- editor command apply/revert and randomized history tests;
- autosave/recovery/project migration tests;
- stale-generation/concurrency tests;
- GPU validation/device-recreation tests;
- diagnostics/provenance assertions;
- CI test tiers and performance/regression benchmarks.

Verification must preserve the distinction between exact semantic equality and tolerance-based final image comparison.

## Current branch policy

All blueprint work is being accumulated on one branch:

`blueprint/osrs-editor-foundation`

No pull request should be opened until the complete blueprint branch is ready for merge.
