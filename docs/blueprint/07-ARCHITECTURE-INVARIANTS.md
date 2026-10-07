# RustOSRS Architecture Invariants

Status: **Checkpoint 2 normative architecture**  
Decision class: `PROJECT_DECISION`

These invariants are guardrails for future implementation and review. A code change that violates one requires an explicit superseding ADR rather than an informal exception.

## A. Truth and evidence

### A1. Research is not specification

No implementation requirement may terminate its evidence chain in a root-level `RUNELITE_*.md` document.

### A2. Generated docs are navigation only

`docs/api/` may locate source but cannot be sole semantic authority.

### A3. Mixed revisions stay visible

Evidence from January 2026 and October 2026 snapshots must not be silently merged into a single verified claim.

### A4. Semantic claims are test-owned

Every promoted high-risk semantic spec must name the Rust test/differential/golden ownership that will detect drift.

## B. Dependency direction

### B1. Core is inward-most

`osrs-core` cannot depend on cache, scene, renderer, editor, or reference crates.

### B2. Scene cannot depend on cache implementation

`osrs-scene` consumes canonical semantic inputs/providers, never a concrete cache reader.

### B3. Renderer cannot define scene meaning

`osrs-render` cannot own loc dispatch, morph resolution, model selection, transform order, bridge semantics, or other OSRS-owned scene rules.

### B4. UI never leaks downward

No egui, eframe, Catppuccin, docking, widget, selection, command-history, or project-file type may appear in reusable semantic/render crate public APIs.

### B5. Reference tooling is not runtime infrastructure

No production crate depends on `osrs-reference` or requires Java/deob execution at runtime.

## C. Data fidelity

### C1. Preserve semantic metadata until explicitly consumed

Do not discard or prematurely bake away:

- raw loc type/orientation
- face priority
- alpha/transparency
- texture assignment
- UV information
- transform/animation groups
- relevant object-definition flags
- source/revision provenance where ambiguity remains

### C2. Revision encoding is normalized at ingestion

Wire/cache representation quirks should not leak through the whole system unless they reflect a genuine semantic difference.

### C3. Shared source assets are immutable by default

Instance-specific recolor, retexture, contour, normal, pose, or editor-preview work must not mutate shared cached originals.

### C4. Integer semantics remain integer until the owning contract permits conversion

Do not convert OSRS fixed-point/integer calculations to floating point early merely because the renderer consumes floats.

### C5. Null/absent is not fallback

If verified OSRS behavior says a model/variant is absent, downstream layers must represent absence explicitly rather than selecting a convenient alternate asset.

## D. Scene ownership

### D1. Semantic scene is renderer-independent

Destroying/recreating the renderer cannot invalidate or change the canonical semantic scene.

### D2. Placement belongs to scene state

Definitions describe possibilities; scene instances own resolved placement, type/orientation, footprint, plane, and dynamic state.

### D3. Scene build is deterministic

Equivalent canonical inputs and explicit preview/time state produce equivalent semantic output.

### D4. Dirty tracking starts from semantic edits

GPU buffers are derivative. Editor commands mutate semantic state first, then invalidate the relevant render extraction.

## E. Renderer boundaries

### E1. Semantic metadata and renderer strategy are distinct

The renderer may choose how to realize priority, alpha, culling, depth, batching, and texture allocation, but cannot redefine the semantic inputs.

### E2. Presentation improvements are explicit

Any visual improvement that intentionally diverges from reference presentation must be documented as renderer policy.

### E3. Parity remains observable

Where practical, enhanced rendering features must be disable-able or separable so semantic/reference comparisons remain meaningful.

### E4. GPU optimizations cannot become source of truth

Zone caches, staging buffers, sort keys, compact IDs, or baked GPU vertices are disposable derivatives of semantic state.

## F. Editor boundaries

### F1. Editor history uses commands/transactions

Undo/redo must reverse semantic document mutations, not patch GPU resources as the authoritative state.

### F2. Project files do not replace cache semantics

A project may store edits, metadata, view state, and references, but it must not invent a second incompatible object/model/terrain semantic model.

### F3. Diagnostics are product features

The editor must expose semantic provenance and render diagnostics rather than treating debugging as development-only logging.

### F4. Native desktop is first-class

Native filesystem, threading, diagnostics, and GPU capabilities may be used where appropriate. wasm portability is not a mandatory architectural constraint.

## G. Future-client compatibility

### G1. Editor non-goals do not constrain reusable crates

No networking/login/CS2/player simulation is required for the editor, but lower layers must not hard-code assumptions that prevent another application from adding those systems.

### G2. Shared semantics have one home

A future `osrs-client` must reuse object/model/terrain/scene semantics rather than fork editor implementations.

## H. Failure and observability

### H1. Errors retain identity

Decode/scene/render failures keep asset IDs, region/tile/plane context, revision, and semantic stage when available.

### H2. Specs are diagnosable

Canonical specs should define failure symptoms and diagnostic hooks so visual defects can be traced to an owning stage.

### H3. Silent correction is prohibited

Do not automatically clamp, substitute, skip, or repair corrupt/unsupported semantic inputs unless the canonical contract explicitly requires it. Surface the problem with context.

## I. Change control

Changing an invariant requires:

1. an ADR that names the invariant
2. rationale and alternatives
3. migration impact on crates/specs/tests
4. contradiction-register review
5. explicit supersession of the old decision

This prevents architecture drift through isolated implementation convenience.
