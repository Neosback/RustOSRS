# Contradiction and Open-Question Register

Status: **Living register, reconciled through Checkpoint 9**

This register records claims that must not become implementation requirements until they are resolved. It is intentionally stricter than the existing research notes.

## Severity

- **P0**: can produce structurally wrong OSRS geometry/scene semantics.
- **P1**: can produce visibly wrong rendering or revision breakage.
- **P2**: architecture/product ambiguity that can create unnecessary coupling or rework.

## C-001: Cross-model normal behavior is currently overstated

**Severity:** P0  
**Status:** `RESOLVED`  
**Domain:** `OSRS_SEMANTIC`

Resolved by `docs/blueprint/09-SEMANTIC-AUDIT.md`, `docs/specs/normals-lighting.md`, and the source pins in `docs/verification/SOURCE-PINS.md`.

The audited public deob contains a cross-`ModelData` normal merge routine. Eligible static scene objects can accumulate normals at translated coincident vertices before final lighting, and matched faces can be marked through render type `2` when the caller requests face suppression.

This is **not** topological mesh welding. Model/scene-object identity remains separate while lighting normals can be reconciled across models.

Positive, negative, and matched-face differential fixtures remain required by the verification plan, but the disputed behavior itself is resolved.

## C-002: Mesh welding and visual continuity must not be conflated

**Severity:** P0  
**Status:** `RESOLVED`

Resolved by `docs/blueprint/09-SEMANTIC-AUDIT.md` and `docs/specs/normals-lighting.md`.

The canonical model separately represents:

- scene-object topology;
- model ownership/caching;
- base normal generation;
- cross-model normal accumulation;
- optional matched-face suppression;
- final lighting conversion.

Separate meshes do not imply separate lighting normals.

## C-003: Evidence corpus mixes January and October 2026 snapshots

**Severity:** P1  
**Status:** `REVISION_SENSITIVE`

The corpus uses:

- October 4, 2026 RuneLite API/client/GPU material for the newer reference set;
- January 2026 melxin/deob material for runescape-client construction internals and older compute/priority shader files.

Checkpoint 3 added exact public commit and file/blob pins where possible, but the existing local deob harness source is still an unpinned developer-machine tree.

Required resolution remains:

- source pins in every semantic spec;
- explicit cross-revision equivalence checks for behavior borrowed from the January deob;
- no spec may cite a mixed source group as if it were one snapshot;
- identify or hash the exact local harness source before treating its whole output as one pinned snapshot.

## C-004: Staged shader directory contains live, historical, semantic, and optional presentation material together

**Severity:** P1  
**Status:** `RESOLVED`

Resolved at the architecture/ownership level by `docs/blueprint/03-SOURCE-GROUP-INVENTORY.md`, `11-RENDERER-ARCHITECTURE.md`, and `12-GPU-DATA-PASSES.md`.

The staged shader tree is reference evidence only. The Rust renderer does **not** transliterate the directory wholesale.

Checkpoint 5 established the split:

- semantic inputs remain in `docs/specs/` and renderer extraction;
- live RuneLite rendering formulas may serve as reference-profile evidence;
- historical priority/compute shaders are algorithm documentation/reference evidence;
- colorblind/scaling/UI shaders are optional presentation/editor features;
- WGSL is organized by RustOSRS renderer responsibility rather than source-tree shape.

Individual formulas still require their own evidence/tests when ported, but the authority/classification contradiction is closed.

## C-005: Historical local filesystem paths are not reproducible source pins

**Severity:** P1  
**Status:** `REVISION_SENSITIVE`

Several documents identify sources primarily by `/Users/...` filesystem locations and a human-readable date.

Checkpoint 3 added imported tree/blob pins and a provisional public deob upstream commit, but the historical local deob source used by the harness is still not exactly identified.

Checkpoint 7 now requires every newly promoted fixture to carry an exact source/harness manifest, so this unresolved historical path cannot silently contaminate future fixture provenance.

Required closure is documented in `docs/verification/SOURCE-PINS.md` and `REFERENCE-FIXTURES.md`.

## C-006: Deob golden fixture coverage is strong but incomplete for the highest-risk semantics

**Severity:** P0  
**Status:** `RESEARCH`

The current harness proves terrain shape tables, triangulation, HSL/color functions, contouring, one lighting case, wall/decor storage, footprints, offsets, and scene capacity behavior.

It does not yet provide complete differential coverage for:

- positive/negative normal merge cases;
- all transform combinations and ordering;
- recolor/retexture interactions;
- mirrored winding/culling;
- priority/transparency behavior;
- all morph/static/dynamic object paths;
- animation transform semantics;
- bridge/plane combinations at scene level;
- the complete terrain color-builder path under an exact source pin.

Checkpoint 7 resolved the **planning gap** by defining the complete spec-to-fixture matrix, fixture manifest contract, golden scene catalog, and CI tiers in `docs/verification/`. The **coverage gap itself remains open** until implementation adds the required fixtures/tests and advances the corresponding `PARITY-MATRIX.md` rows to `EXISTING`.

`TERRAIN-004` remains separately blocked on source/oracle provenance rather than ordinary fixture implementation.

## C-007: `editor_*` crate naming conflicts with reusable OSRS foundation goals

**Severity:** P2  
**Status:** `RESOLVED`

Resolved by `docs/adr/ADR-0001-reusable-osrs-crate-boundaries.md` and `docs/blueprint/06-CRATE-ARCHITECTURE.md`.

The accepted primary crate direction is:

- `osrs-core`;
- `osrs-cache`;
- `osrs-scene`;
- `osrs-render`;
- `osrs-reference`;
- `osrs-editor`.

Reusable lower layers are OSRS-owned, not editor-owned. Additional crate splits require demonstrated value and an ADR.

## C-008: Mandatory wasm support is not yet a product requirement

**Severity:** P2  
**Status:** `RESOLVED`

Resolved by `docs/adr/ADR-0002-native-first-editor.md`.

RustOSRS is native-first. wasm support is deferred, not forbidden. Natural portability is welcome, but wasm compatibility is not a hard acceptance gate and must not distort native editor architecture without a future superseding ADR.

## C-009: Existing "map editor, not a client" non-scope is too broad for foundation architecture

**Severity:** P2  
**Status:** `RESOLVED`

Resolved by `docs/blueprint/04-PROJECT-CHARTER.md`, `docs/blueprint/06-CRATE-ARCHITECTURE.md`, and ADR-0001.

The editor product remains explicitly non-client for its initial milestones, while reusable cache/core/scene/render crates remain suitable for a future Rust client/reference application where doing so does not compromise correctness or maintainability.

## C-010: `rs-cache` is a candidate foundation, not an accepted decoder contract

**Severity:** P1  
**Status:** `REVISION_SENSITIVE`

Current research shows useful existing Rust I/O/loaders but also material gaps:

- varbit-driven transforms;
- newer model-id widths/opcodes;
- ModelData;
- texture/material definitions;
- overlay/underlay definitions;
- sequence/spotanim definitions;
- revision-sensitive newer fields.

Checkpoint 3 established the semantic fields that must survive decoding for object placement, model construction, terrain/materials, morphs, animation, face metadata, and texture animation.

Its older-revision lineage means passing its own test suite proves implementation health, not suitability for the RustOSRS target revision.

Checkpoint 8 places the actual decision in M1: perform a bounded target-profile compatibility spike, then accept an ADR selecting whether to wrap, extend, fork, partially reuse, or replace `rs-cache` components. Until that ADR exists, no dependency strategy is canonical.

## C-011: "FileStore is the spec" is too strong

**Severity:** P1  
**Status:** `RESEARCH`

OpenRune FileStore is valuable independent implementation evidence and likely the best existing opcode map for several gaps. It is still an implementation, not the canonical OSRS oracle.

Required resolution:

- use FileStore to accelerate decoding work;
- verify revision-sensitive opcodes/defaults against target data/deob or another primary source;
- never make Rust semantics depend on Kotlin naming or implementation quirks.

Checkpoint 9 also quarantines the legacy `RUNELITE_CACHE_STACK.md` wording through `docs/research/README.md` and `18-DOCUMENTATION-RECONCILIATION.md`.

## C-012: Fixed texture count/array assumptions may be renderer implementation details

**Severity:** P1  
**Status:** `RESOLVED`

Resolved by `docs/blueprint/12-GPU-DATA-PASSES.md` and ADR-0005/renderer architecture.

The canonical rule is now:

- semantic texture IDs are not limited by RuneLite's historical `TEXTURE_COUNT = 256` renderer constant;
- renderer capacity/allocation is adapter-aware and uses a material table plus paged texture arrays;
- texture storage capacity is renderer policy;
- unsupported adapter limits must produce diagnostics/fallback selection, not silent texture loss.

Target-revision decoder limits remain a separate cache compatibility concern under C-010.

## C-013: Reverse-Z is a renderer policy, not OSRS semantic parity

**Severity:** P2  
**Status:** `RESOLVED`

Resolved by ADR-0004 and `docs/blueprint/11-RENDERER-ARCHITECTURE.md` / `12-GPU-DATA-PASSES.md`.

RustOSRS accepts reverse-Z as renderer policy with:

- `Depth32Float`;
- clear depth `0.0`;
- larger depth nearer;
- `GreaterEqual` baseline comparison;
- canonical CCW GPU-facing front faces;
- back-face culling;
- authored face bias preserved separately and consumed by the reference rendering strategy.

Semantic placement, priority, alpha, face metadata, and model behavior remain independently testable underneath this policy.

## C-014: Camera defaults and pitch bands are mixed semantic/editor concerns

**Severity:** P2  
**Status:** `RESOLVED`

Resolved by `docs/specs/coordinates.md`, renderer ADRs, and `docs/blueprint/13-EDITOR-ARCHITECTURE.md`.

The separation is explicit:

- tile/JAU/angular tables and object transforms: semantic/reference math;
- projection/clip-space/reverse-Z: renderer policy;
- orbit/fly controls, default distance/pitch, smoothing, framing, and bookmarks: editor policy.

The editor camera cannot mutate semantic loc orientation.

## C-015: RuneLite region filtering and `regions.txt` are not baseline scene semantics

**Severity:** P2  
**Status:** `RESOLVED`

Resolved by `docs/blueprint/13-EDITOR-ARCHITECTURE.md`, `15-EDITOR-TOOLS-INTERACTION.md`, and ADR-0006/0007 ownership boundaries.

`hideUnrelatedMaps`, RuneLite `regions.txt`, colorblind processing, UI scaling, and similar plugin settings are not baseline OSRS scene semantics.

RustOSRS decisions are now:

- multi-region workspace membership is explicit project/editor state;
- unloaded neighbors are distinct from empty map data;
- region/plane visibility filters are non-destructive editor/render state;
- colorblind/scaling options are optional presentation policy;
- no RuneLite plugin region list becomes semantic map truth.

## C-016: Generated API docs must never become sole proof

**Severity:** P1  
**Status:** `RESOLVED`

Resolved by `docs/blueprint/01-EVIDENCE-STATUS.md` and invariant A2 in `docs/blueprint/07-ARCHITECTURE-INVARIANTS.md`.

`docs/api/` is navigation/reference material only. Final semantic specs must cite underlying pinned source and/or executable evidence.

## C-017: Existing "all verified" language hides different verification strengths

**Severity:** P1  
**Status:** `RESOLVED`

Resolved by the status/evidence vocabulary in `docs/blueprint/01-EVIDENCE-STATUS.md`.

The canonical blueprint distinguishes `VERIFIED`, `DERIVED`, `PROJECT_DECISION`, `RESEARCH`, `HYPOTHESIS`, `DISPUTED`, `REVISION_SENSITIVE`, `DEFERRED`, and `OBSOLETE` rather than treating all source inspection or conceptual mapping as equivalent verification.

## C-018: Ground-decoration lift and similar visual claims need source-path verification

**Severity:** P1  
**Status:** `RESOLVED`

Resolved by `docs/blueprint/10-TERRAIN-MATERIAL-PLANE-AUDIT.md` and `docs/specs/loc-placement.md` for the generic lift claim.

The audited public deob stores floor-decoration Z at the supplied height unchanged, initial placement supplies the computed ground height, and the imported RuneLite static uploader uses the stored ground-object Z directly. The old generic "+1/+2 to avoid Z-fighting" statement is therefore `OBSOLETE/REFUTED` for this audited path.

Future asset-specific offsets still require source evidence. Visual-intent prose is never sufficient by itself.

## C-019: Static/dynamic model branch must be audited with morph and animation ownership

**Severity:** P0  
**Status:** `RESOLVED`

Resolved by `docs/blueprint/09-SEMANTIC-AUDIT.md` and canonical model/morph/contour specs.

Checkpoint 3/4 established:

- initial static scene construction can preserve `ModelData` for later normal merging;
- pending-spawn/static replacement uses an already-lit model path;
- morph resolution is varbit/varp driven and includes fallback/null targets;
- dynamic objects resolve morphs at model time;
- transformed definitions can alter footprint dimensions;
- animation is applied to a working/shared model path rather than mutating the immutable source cache;
- contouring occurs on the appropriate working representation.

The differential morph/animation fixture families are now explicitly defined in Checkpoint 7, while their implementation remains future work.

## C-020: Priority rendering evidence spans software semantics and RuneLite GPU strategy

**Severity:** P0  
**Status:** `RESOLVED`

Resolved by `docs/specs/face-materials.md`, `docs/blueprint/11-RENDERER-ARCHITECTURE.md`, `12-GPU-DATA-PASSES.md`, and ADR-0005.

The canonical split is:

- priority values/face metadata and reference ordering behavior are semantic/reference contracts;
- static zone batching is renderer implementation;
- camera-dependent ordered faces use a dedicated ordered path;
- transparency and authored bias remain preserved inputs;
- Rust renderer strategy is proven against crafted priority/transparency fixtures rather than copied structurally from RuneLite.

Checkpoint 7 defines the exact all-priority/threshold fixture and composed golden scene. Fixture implementation remains future work, but the semantic/renderer ownership contradiction is closed.

## C-021: The old `class470` terrain-builder source pin is stale

**Severity:** P0  
**Status:** `REVISION_SENSITIVE`  
**Domain:** `OSRS_SEMANTIC`

Older research cites `class470` and line ranges as the source for slope lighting, 11x11 underlay blending, random terrain-color variation, overlays, and terrain-side flags.

In public `melxin/runelite@1ad572d7...`, `class470` is unrelated text-layout code.

Checkpoint 3B separately verified:

- `SceneTileModel` terrain shape topology;
- underlay definition weighted HSL inputs;
- overlay definition fields/defaults/HSL;
- downstream terrain sentinel/UV behavior in the imported RuneLite renderer.

But the complete higher-level terrain-color builder is not yet pinned.

Required resolution before normative promotion:

- identify the actual target source method/file and pin it; or
- add an end-to-end terrain-color differential fixture that proves the complete builder behavior against an exact source snapshot.

Checkpoint 7 deliberately marks the corresponding parity row and `GS-012-terrain-color-border` golden scene as `BLOCKED` rather than assigning guessed expected values.

Until closure, `TERRAIN-004` remains revision-gated and the old 11x11/color-builder prose remains research.

## C-022: Bridge behavior was conflated into one adjusted-plane rule

**Severity:** P0  
**Status:** `RESOLVED`  
**Domain:** `OSRS_SEMANTIC`

Resolved by `docs/blueprint/10-TERRAIN-MATERIAL-PLANE-AUDIT.md`, `docs/specs/planes-bridges.md`, and the editor plane workflow.

The blueprint preserves separate mechanisms for:

- encoded/source loc plane;
- bridge-adjusted collision-plane selection during decoded object placement;
- structural `Scene.setLinkBelow` tile relinking;
- tile storage plane after relinking;
- tile render/height level;
- linked-below bridge tile;
- pending-spawn/live replacement sampling rules;
- RuneLite-specific roof/VIS_BELOW grouping as renderer/product state.

These must not be represented as one universal `plane +/- 1` rule, including in the editor UI.

## Documentation quarantine

Checkpoint 9 makes the authority boundary explicit through:

- canonical root `index.md`;
- `docs/research/README.md`;
- `docs/blueprint/18-DOCUMENTATION-RECONCILIATION.md`.

Root `RUNELITE_*.md` files remain historical research and cannot resolve an item in this register by themselves.

## Resolution workflow

For every item resolved later:

1. add exact source pins;
2. state chosen target behavior;
3. add/update an atomic spec or ADR;
4. attach a test/fixture requirement where applicable;
5. mark the register item `RESOLVED` and link the resolving document.

Nothing in this register should be "resolved" only by rewriting prose.
