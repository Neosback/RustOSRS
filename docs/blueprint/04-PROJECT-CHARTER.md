# RustOSRS Project Charter

Status: **Checkpoint 2 architecture foundation**  
Decision class: `PROJECT_DECISION`

## 1. Mission

RustOSRS will build a leading-class native OSRS map editor in Rust with a correctness-first semantic foundation, a modern wgpu renderer, and an eframe/egui editor shell using Catppuccin styling.

The editor is the first product, but the lower-level OSRS implementation must be reusable enough to support a future Rust OSRS client, reference renderer, validation tool, or other scene consumer without rewriting cache, model, terrain, or scene semantics.

## 2. Primary success criterion

The project succeeds when an incorrect scene is diagnosable as a specific violated semantic contract rather than as an unexplained visual difference.

For OSRS-owned behavior, correctness means:

- cache data is decoded without lossy reinterpretation
- loc/object definitions resolve the intended model and variant
- transformations execute in the correct order and integer domain
- terrain geometry, color, bridges, planes, and footprints match the selected semantic reference
- model metadata required by rendering is preserved
- normals, contouring, lighting inputs, priorities, alpha, textures, UVs, morphs, and animations follow verified contracts
- scene construction is deterministic for the same semantic inputs

The renderer may deliberately improve presentation where documented, but it may not silently change scene meaning.

## 3. Product goals

### G1. Semantic correctness before visual polish

A visually attractive result is not sufficient if the geometry, placement, metadata, or scene behavior is wrong.

### G2. Evidence-backed implementation

Every inherited OSRS behavior implemented in reusable crates must trace to a canonical spec whose evidence terminates in pinned source and/or executable reference behavior.

### G3. Render-independent OSRS foundation

Core OSRS data and scene semantics must not depend on wgpu, egui, eframe, Catppuccin, windowing, or editor concepts.

### G4. Editor-independent renderer

The renderer must be reusable by another application. It may use wgpu, but must not depend on editor panels, undo history, docking, project files, or egui widget state.

### G5. Deterministic verification

Reference scenes and semantic tests must be reproducible. Live-client randomness or presentation-only variance must be pinned or disabled in parity fixtures.

### G6. Strong diagnostics

The implementation must expose enough structured state to answer questions such as:

- which object definition produced this model?
- which loc type/orientation path was used?
- which transform sequence ran?
- why was a face assigned this priority/alpha/texture?
- which plane/bridge rule moved this object?
- which semantic spec owns this behavior?

### G7. Extensibility without semantic duplication

Future client/editor tooling may add application behavior, but shared OSRS semantics must have one implementation and one canonical contract.

## 4. Non-goals for the initial editor product

The first editor milestone does not require:

- official-world login or networking
- packet protocols
- CS2/game widget implementation
- player/NPC simulation as a full game client
- sound playback
- a plugin marketplace
- browser/WebAssembly delivery

These are **product non-goals**, not prohibitions on reusable lower-level architecture. The shared OSRS crates must not be designed in a way that makes a future client impossible.

## 5. Native-first policy

The first-class target is a native desktop editor.

WebAssembly support is `DEFERRED`, not forbidden. No core semantic API may be made worse merely to preserve hypothetical wasm compatibility unless a future ADR accepts wasm as a product requirement.

Portable pure-Rust code remains desirable where it is natural, but native editor quality, diagnostics, filesystem workflows, GPU capabilities, and development velocity take priority.

## 6. Correctness domains

All normative requirements belong to exactly one domain.

### 6.1 `OSRS_SEMANTIC`

Inherited behavior. Accidental divergence is a bug.

Includes cache meaning, coordinates, definitions, placement, scene construction, model transforms, terrain construction, contouring, semantic normal behavior, face metadata, morph/animation semantics, bridges, roofs, and other content behavior proven to belong to the target OSRS semantics.

### 6.2 `RENDERER_POLICY`

RustOSRS technical/presentation choices made after semantic scene construction.

Examples include wgpu resource layout, reverse-Z, MSAA, anisotropy, render graph organization, GPU culling, debug modes, and optional presentation lighting.

Renderer policy may change appearance only when the deviation is explicit, testable, and switchable or otherwise separable from semantic parity where practical.

### 6.3 `EDITOR_POLICY`

User workflow owned entirely by RustOSRS: egui layout, docking, theme, tools, selection, gizmos, undo/redo, project persistence, autosave, shortcuts, and inspection UI.

## 7. Reference rendering modes

The renderer blueprint must support the concept of at least two intent modes even if implementation details are finalized later:

1. **Parity/reference mode**: presentation choices minimize intentional divergence and are suitable for golden comparisons.
2. **Enhanced/editor mode**: documented renderer improvements may be enabled without mutating underlying semantic data.

A renderer enhancement must never require rewriting canonical scene data merely to look better.

## 8. Quality gates

Before an implementation milestone is considered complete:

- normative OSRS behaviors used by the milestone have canonical specs
- unresolved `P0` contradictions relevant to that milestone are closed
- exact integer behavior is tested exactly where applicable
- differential fixtures exist for high-risk algorithms where practical
- renderer deviations from parity are named and documented
- errors preserve context rather than collapsing into generic strings
- tracing/diagnostics can identify the responsible semantic stage

## 9. Documentation hierarchy

Canonical precedence for this project:

1. atomic specs under `docs/specs/`
2. accepted ADRs under `docs/adr/`
3. architecture/blueprint documents under `docs/blueprint/`
4. executable verification metadata under `docs/verification/`
5. root-level `RUNELITE_*.md` research notes

If an old research note conflicts with a canonical spec or ADR, the canonical document wins and the old note must eventually be marked obsolete or reconciled.

## 10. Initial implementation principle

Do not scaffold code simply because the crate name is known. Checkpoint 2 defines boundaries. Checkpoints 3 and 4 must verify the semantic contracts that those crates will implement before the implementation roadmap locks module-level work.
