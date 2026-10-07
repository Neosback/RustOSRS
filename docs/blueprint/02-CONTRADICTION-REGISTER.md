# Contradiction and Open-Question Register

Status: **Checkpoint 3 active register**

This register records claims that must not become implementation requirements until they are resolved. It is intentionally stricter than the existing research notes.

## Severity

- **P0**: can produce structurally wrong OSRS geometry/scene semantics.
- **P1**: can produce visibly wrong rendering or revision breakage.
- **P2**: architecture/product ambiguity that can create unnecessary coupling or rework.

## C-001: Cross-model normal behavior is currently overstated

**Severity:** P0  
**Status:** `RESOLVED`  
**Domain:** `OSRS_SEMANTIC`

Resolved by `docs/blueprint/09-SEMANTIC-AUDIT.md` and the source pins in `docs/verification/SOURCE-PINS.md`.

The audited public deob contains a cross-`ModelData` normal merge routine. Eligible static scene objects can accumulate normals at translated coincident vertices before final lighting, and matched faces can be marked through render type `2` when the caller requests face suppression.

This is **not** topological mesh welding. Model/scene-object identity remains separate while lighting normals can be reconciled across models.

The final `normals.md` spec still requires positive, negative, and matched-face differential fixtures, but the disputed behavior itself is resolved.

## C-002: Mesh welding and visual continuity must not be conflated

**Severity:** P0  
**Status:** `RESOLVED`

Resolved by `docs/blueprint/09-SEMANTIC-AUDIT.md`.

The canonical model must separately represent:

- scene-object topology
- model ownership/caching
- base normal generation
- cross-model normal accumulation
- optional matched-face suppression
- final lighting conversion

Separate meshes do not imply separate lighting normals.

## C-003: Evidence corpus mixes January and October 2026 snapshots

**Severity:** P1  
**Status:** `REVISION_SENSITIVE`

The corpus uses:

- October 4, 2026 RuneLite API/client/GPU material for the newer reference set
- January 2026 melxin/deob material for runescape-client construction internals and older compute/priority shader files

Checkpoint 3 added exact public commit and file/blob pins where possible, but the existing local deob harness source is still an unpinned developer-machine tree.

Required resolution remains:

- source pins in every semantic spec
- explicit cross-revision equivalence checks for behavior borrowed from the January deob
- no spec may cite a mixed source group as if it were one snapshot
- identify or hash the exact local harness source before treating its whole output as one pinned snapshot

## C-004: Staged shader directory contains live, historical, semantic, and optional presentation material together

**Severity:** P1  
**Status:** `RESEARCH`

`reference-shaders/runelite-gpu/` currently combines:

- live vertex/fragment shader references
- historical compute/priority shader references retained mainly to document sorting math
- UI shaders
- colorblind processing
- scaling filters
- region-list data

These files do not all have equal authority for the Rust renderer.

Required classification before WGSL work:

- semantic-critical math
- RuneLite renderer policy
- historical algorithm documentation
- optional editor presentation feature
- unrelated/non-port-required material

Do not transliterate the whole directory to WGSL.

## C-005: Historical local filesystem paths are not reproducible source pins

**Severity:** P1  
**Status:** `REVISION_SENSITIVE`

Several documents identify sources primarily by `/Users/...` filesystem locations and a human-readable date.

Checkpoint 3 added imported tree/blob pins and a provisional public deob upstream commit, but the historical local deob source used by the harness is still not exactly identified.

Required closure is documented in `docs/verification/SOURCE-PINS.md`.

## C-006: Deob golden fixture coverage is strong but incomplete for the highest-risk semantics

**Severity:** P0  
**Status:** `RESEARCH`

The current harness proves terrain shape tables, triangulation, HSL/color functions, contouring, one lighting case, wall/decor storage, footprints, offsets, and scene capacity behavior.

It does not yet provide complete differential coverage for:

- positive/negative normal merge cases
- all transform combinations and ordering
- recolor/retexture interactions
- mirrored winding/culling
- priority/transparency behavior
- all morph/static/dynamic object paths
- animation transform semantics
- bridge/plane combinations at scene level
- the complete terrain color-builder path under an exact source pin

Required resolution: expand the harness/fixtures as each semantic spec is promoted.

## C-007: `editor_*` crate naming conflicts with reusable OSRS foundation goals

**Severity:** P2  
**Status:** `RESOLVED`

Resolved by `docs/adr/ADR-0001-reusable-osrs-crate-boundaries.md` and `docs/blueprint/06-CRATE-ARCHITECTURE.md`.

The accepted primary crate direction is:

- `osrs-core`
- `osrs-cache`
- `osrs-scene`
- `osrs-render`
- `osrs-reference`
- `osrs-editor`

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

- varbit-driven transforms
- newer model-id widths/opcodes
- ModelData
- texture/material definitions
- overlay/underlay definitions
- sequence/spotanim definitions
- revision-sensitive newer fields

Checkpoint 3 also established the semantic fields that must survive decoding for object placement, model construction, terrain/materials, morphs, animation, face metadata, and texture animation.

Its older-revision lineage means passing its own test suite proves implementation health, not suitability for the RustOSRS target revision.

Required resolution: define the target-revision cache contract first, then decide whether to extend, wrap, fork, or replace portions of `rs-cache`.

## C-011: "FileStore is the spec" is too strong

**Severity:** P1  
**Status:** `RESEARCH`

OpenRune FileStore is valuable independent implementation evidence and likely the best existing opcode map for several gaps. It is still an implementation, not the canonical OSRS oracle.

Required resolution:

- use FileStore to accelerate decoding work
- verify revision-sensitive opcodes/defaults against target data/deob or another primary source
- never make Rust semantics depend on Kotlin naming or implementation quirks

## C-012: Fixed texture count/array assumptions may be renderer implementation details

**Severity:** P1  
**Status:** `REVISION_SENSITIVE`

Existing notes elevate `TEXTURE_COUNT = 256`, texture-array shape, mip count, and related RuneLite GPU constants into a Rust mapping.

Checkpoint 3B confirmed that renderer allocation policy must remain distinct from decoded texture-id semantics.

Required resolution:

- separate cache texture-id domain from RuneLite GPU allocation policy
- confirm target revision limits
- make Rust renderer capacity policy explicit and test overflow/unsupported cases

## C-013: Reverse-Z is a renderer policy, not OSRS semantic parity

**Severity:** P2  
**Status:** `PROJECT_DECISION`

RuneLite GPU's reverse-Z behavior is a strong wgpu design candidate, but it is renderer policy. Object placement, priorities, face metadata, sorting, and scene semantics must remain testable independently of it.

Checkpoint 2 resolved the ownership boundary through ADR-0003, but the concrete reverse-Z renderer decision remains open for Checkpoint 5.

Required resolution: renderer ADR documenting reverse-Z, coordinate mapping, culling/front-face rules, and parity tests.

## C-014: Camera defaults and pitch bands are mixed semantic/editor concerns

**Severity:** P2  
**Status:** `RESOLVED`

Resolved at the ownership level by `docs/blueprint/10-TERRAIN-MATERIAL-PLANE-AUDIT.md`.

Required separation is now explicit:

- tile/JAU/angular tables and coordinate transforms: semantic/reference math
- projection/clip-space/reverse-Z implementation: renderer policy
- orbit/fly controls, default pitch/distance, smoothing, focus behavior: editor policy

The concrete renderer/editor choices remain future checkpoint work, but the contradiction about ownership is closed.

## C-015: RuneLite region filtering and `regions.txt` are not baseline scene semantics

**Severity:** P2  
**Status:** `RESEARCH`

`hideUnrelatedMaps`, region-list behavior, colorblind processing, UI scaling, and similar plugin settings are useful reference features, not automatically required for an OSRS editor scene model.

Required resolution: promote only features that serve editor requirements, with their own project decisions.

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

Resolved by `docs/blueprint/10-TERRAIN-MATERIAL-PLANE-AUDIT.md` for the generic lift claim.

The audited public deob stores floor-decoration Z at the supplied height unchanged, initial placement supplies the computed ground height, and the imported RuneLite static uploader uses the stored ground-object Z directly. The old generic "+1/+2 to avoid Z-fighting" statement is therefore `OBSOLETE/REFUTED` for this audited path.

Future asset-specific offsets still require source evidence. Visual-intent prose is never sufficient by itself.

## C-019: Static/dynamic model branch must be audited with morph and animation ownership

**Severity:** P0  
**Status:** `RESOLVED`

Resolved at the semantic-flow level by `docs/blueprint/09-SEMANTIC-AUDIT.md`.

Checkpoint 3 verified:

- initial static scene construction can preserve `ModelData` for later normal merging
- pending-spawn/static replacement uses an already-lit model path
- morph resolution is varbit/varp driven and includes fallback/null targets
- dynamic objects resolve morphs at model time
- transformed definitions can alter footprint dimensions
- animation is applied to a working/shared model path rather than mutating the immutable source cache
- contouring occurs on the appropriate working representation

Differential morph/animation fixtures remain required before individual atomic specs become implementation-ready.

## C-020: Priority rendering evidence spans software semantics and RuneLite GPU strategy

**Severity:** P0  
**Status:** `RESOLVED`

Resolved at the semantic/reference ownership level by `docs/blueprint/09-SEMANTIC-AUDIT.md` and `docs/blueprint/10-TERRAIN-MATERIAL-PLANE-AUDIT.md`.

Checkpoint 3 verified:

- priority 0..11 software ordering is structured, not a simple numeric sort
- the imported RuneLite dynamic uploader independently reproduces the 10/11 interleaving thresholds
- static RuneLite scene upload uses a different batching path
- transparency and authored face bias must remain preserved semantic inputs
- the chosen Rust sorting/batching/depth strategy is a later `RENDERER_POLICY` decision

Crafted priority/transparency fixtures remain mandatory. The contradiction about which layer owns the semantics is resolved.

## C-021: The old `class470` terrain-builder source pin is stale

**Severity:** P0  
**Status:** `REVISION_SENSITIVE`  
**Domain:** `OSRS_SEMANTIC`

Older research cites `class470` and line ranges as the source for slope lighting, 11x11 underlay blending, random terrain-color variation, overlays, and terrain-side flags.

In public `melxin/runelite@1ad572d7...`, `class470` is unrelated text-layout code.

Checkpoint 3B separately verified:

- `SceneTileModel` terrain shape topology
- underlay definition weighted HSL inputs
- overlay definition fields/defaults/HSL
- downstream terrain sentinel/UV behavior in the imported RuneLite renderer

But the complete higher-level terrain-color builder is not yet pinned.

Required resolution before normative promotion:

- identify the actual target source method/file and pin it, or
- add an end-to-end terrain-color differential fixture that proves the complete builder behavior against an exact source snapshot

Until then, the old 11x11/color-builder prose remains research, not a canonical spec.

## C-022: Bridge behavior was conflated into one adjusted-plane rule

**Severity:** P0  
**Status:** `RESOLVED`  
**Domain:** `OSRS_SEMANTIC`

Resolved by `docs/blueprint/10-TERRAIN-MATERIAL-PLANE-AUDIT.md`.

Checkpoint 3B verified separate mechanisms for:

- encoded/source loc plane
- bridge-adjusted collision-plane selection during decoded object placement
- structural `Scene.setLinkBelow` tile relinking
- tile storage plane after relinking
- tile render/height level
- linked-below bridge tile
- pending-spawn/live replacement sampling rules
- RuneLite-specific roof/VIS_BELOW map-level grouping

These must not be represented as one universal `plane +/- 1` rule.

## Resolution workflow

For every item resolved later:

1. add exact source pins
2. state chosen target behavior
3. add/update an atomic spec or ADR
4. attach a test/fixture requirement where applicable
5. mark the register item `RESOLVED` and link the resolving document

Nothing in this register should be "resolved" only by rewriting prose.