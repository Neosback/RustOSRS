# Contradiction and Open-Question Register

Status: **Checkpoint 1 working register**

This register records claims that must not become implementation requirements until they are resolved. It is intentionally stricter than the existing research notes.

## Severity

- **P0**: can produce structurally wrong OSRS geometry/scene semantics.
- **P1**: can produce visibly wrong rendering or revision breakage.
- **P2**: architecture/product ambiguity that can create unnecessary coupling or rework.

## C-001: Cross-model normal behavior is currently overstated

**Severity:** P0  
**Status:** `DISPUTED`  
**Domain:** `OSRS_SEMANTIC`

Current research says both that wall meshes are never welded and that normals are strictly per-model, then treats those statements as the same conclusion.

Those are separate mechanisms.

Questions that must be answered from the pinned deob/source path:

1. Are scene-object meshes ever topologically welded? Likely no, but prove the exact scope.
2. Can distinct `ModelData` instances have coincident vertex normals accumulated/reconciled before lighting?
3. Is any such behavior gated by `nonFlatShading`, object type, placement adjacency, model state, or another definition field?
4. Which wall/corner/adjacent-object paths invoke it?
5. Does it occur before `toModel`/lighting, and does it mutate copies or cached originals?
6. Does the January 2026 deob behavior match the intended target cache/client revision?

Required resolution artifact: a source-pinned `normals.md` spec plus differential fixture containing at least one positive merge case and one negative case.

## C-002: Mesh welding and visual continuity must not be conflated

**Severity:** P0  
**Status:** `RESEARCH`

The runtime notes correctly warn that adjacent walls are separate scene objects. However, "separate meshes" does not by itself prove "separate lighting normals" or "authentic hard crease".

Final wording must separately specify:

- scene-object topology
- model ownership/caching
- normal generation
- any cross-model normal accumulation
- face hiding/occlusion side effects, if present

## C-003: Evidence corpus mixes January and October 2026 snapshots

**Severity:** P1  
**Status:** `REVISION_SENSITIVE`

The corpus uses:

- October 4, 2026 RuneLite API/client/GPU material for the newer reference set
- January 2026 melxin/deob material for runescape-client construction internals and older compute/priority shader files

Existing prose often says "verified" without carrying this revision boundary into each claim.

Required resolution:

- source pins in every semantic spec
- explicit cross-revision equivalence checks for behavior borrowed from the January deob
- no spec may cite a mixed source group as if it were one snapshot

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
**Status:** `RESEARCH`

Several documents identify sources primarily by `/Users/...` filesystem locations and a human-readable date.

Those paths are useful forensic context but insufficient as final provenance.

Required resolution:

- pin imported tree/blob SHAs already present in this repository
- where an external upstream is required, record upstream repository + commit/tag
- record generator/harness version beside generated fixtures

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

Required resolution: expand the harness/fixtures as each semantic spec is promoted.

## C-007: `editor_*` crate naming conflicts with reusable OSRS foundation goals

**Severity:** P2  
**Status:** `PROJECT_DECISION` pending Checkpoint 2

Existing port notes place scene, math, cache, rendering, and application concerns under editor-prefixed crates.

The desired foundation may later support a Rust OSRS client/reference renderer, so reusable semantic layers should not be conceptually owned by the editor.

Candidate direction, not yet final:

- `osrs-cache`
- `osrs-core`
- `osrs-scene`
- `osrs-render`
- `osrs-reference`
- `osrs-editor`

Final dependency rules belong in Checkpoint 2 ADRs.

## C-008: Mandatory wasm support is not yet a product requirement

**Severity:** P2  
**Status:** `PROJECT_DECISION` pending

Existing notes require the core to compile for wasm and prescribe browser-specific fallbacks/dependencies.

The current product objective is a leading-class native Rust map editor. Web compatibility may remain desirable, but it must not distort semantic architecture, threading, file I/O, diagnostics, or rendering design without an explicit decision.

Required resolution: ADR defining native-first versus native+web support and which crates, if any, carry wasm compatibility as a hard constraint.

## C-009: Existing "map editor, not a client" non-scope is too broad for foundation architecture

**Severity:** P2  
**Status:** `PROJECT_DECISION` pending

Networking, game widgets, live entities, and official-world connectivity are correctly outside the map editor product scope.

However, reusable scene/cache/render crates should not encode assumptions that make a future Rust client/reference implementation unnecessarily difficult.

Required resolution:

- editor product remains explicitly non-client
- reusable crates remain client-capable where this does not compromise correctness or maintainability
- game-specific runtime systems stay outside editor milestones rather than being architecturally forbidden from the foundation

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

Its older-revision lineage means passing its own test suite proves implementation health, not suitability for the RustOSRS target revision.

Required resolution: define the complete cache contract first, then decide whether to extend, wrap, fork, or replace portions of `rs-cache`.

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

Some of these may be valid for the pinned renderer snapshot but are not automatically OSRS cache invariants.

Required resolution:

- separate cache texture-id domain from RuneLite GPU allocation policy
- confirm target revision limits
- make Rust renderer capacity policy explicit and test overflow/unsupported cases

## C-013: Reverse-Z is a renderer policy, not OSRS semantic parity

**Severity:** P2  
**Status:** `PROJECT_DECISION`

RuneLite GPU's reverse-Z behavior is a strong wgpu design candidate, but it is renderer policy. Object placement, priorities, face metadata, sorting, and scene semantics must remain testable independently of it.

Required resolution: renderer ADR documenting reverse-Z, coordinate mapping, culling/front-face rules, and parity tests.

## C-014: Camera defaults and pitch bands are mixed semantic/editor concerns

**Severity:** P2  
**Status:** `RESEARCH`

Existing port notes combine OSRS angular conventions with editor orbit-camera defaults.

Required separation:

- JAU/angular tables and coordinate transforms: semantic/reference math
- editor orbit target/distance/default pitch/input feel: editor policy
- projection/reverse-Z implementation: renderer policy

## C-015: RuneLite region filtering and `regions.txt` are not baseline scene semantics

**Severity:** P2  
**Status:** `RESEARCH`

`hideUnrelatedMaps`, region-list behavior, colorblind processing, UI scaling, and similar plugin settings are useful reference features, not automatically required for an OSRS editor scene model.

Required resolution: promote only features that serve editor requirements, with their own project decisions.

## C-016: Generated API docs must never become sole proof

**Severity:** P1  
**Status:** `PROJECT_DECISION`

`docs/api/` contains generated render-relevant API pages and is useful for navigation. Generated Javadocs describe API surfaces, not necessarily construction semantics or exact deob behavior.

Required rule: final specs cite the underlying source path/blob, not only generated pages.

## C-017: Existing "all verified" language hides different verification strengths

**Severity:** P1  
**Status:** `DISPUTED`

The research corpus uses "verified" for multiple meanings:

- source inspected
- output executed in the deob harness
- RuneLite API/Javadoc observed
- renderer state mapped conceptually to wgpu
- proposed Rust behavior not yet rendered

Checkpoint 1 replaces this with the status vocabulary in `01-EVIDENCE-STATUS.md`.

## C-018: Ground-decoration lift and similar visual claims need source-path verification

**Severity:** P1  
**Status:** `RESEARCH`

Some scene/material prose describes visual intent, such as small height lifts to avoid Z-fighting. Such claims may be correct but must be tied to the actual construction/upload path before they become semantic requirements.

This class of issue applies to any sentence that explains "why it looks right" without also proving the exact source mechanism.

## C-019: Static/dynamic model branch must be audited with morph and animation ownership

**Severity:** P0  
**Status:** `RESEARCH`

Existing notes describe a static cached-model branch versus `DynamicObject` based on animation/transforms. The final architecture also needs to define:

- transform resolution ownership
- varbit/varp preview state
- sequence frame application
- cached immutable source model ownership
- contouring copies
- whether transformed/morphed object definitions change size/model/type assumptions

Required resolution: object-definition, morph, animation, and model-build specs must agree on one data flow.

## C-020: Priority rendering evidence spans software semantics and RuneLite GPU strategy

**Severity:** P0  
**Status:** `RESEARCH`

Face priorities and transparency are semantic data. RuneLite's exact CPU/GPU sorting/upload machinery is one implementation strategy for reproducing their appearance.

Required separation:

- what priority values mean
- reference software ordering behavior
- transparent/opaque interaction
- authored face bias
- chosen Rust sorting/batching strategy

The Rust implementation must be proven against crafted priority fixtures rather than copied structurally from RuneLite.

## Resolution workflow

For every item resolved later:

1. add exact source pins
2. state chosen target behavior
3. add/update an atomic spec or ADR
4. attach a test/fixture requirement
5. mark the register item `RESOLVED` and link the resolving document

Nothing in this register should be "resolved" only by rewriting prose.