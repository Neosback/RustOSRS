# RustOSRS Truth and Parity Model

Status: **Checkpoint 2 normative architecture**  
Decision class: `PROJECT_DECISION`

This document defines what RustOSRS means by "parity" and how revision-aware truth flows through the implementation.

## 1. No unversioned OSRS truth

RustOSRS must not encode revision-sensitive behavior as an unexplained global constant.

Every imported cache/source dataset is associated with a **target profile** containing enough provenance to determine which contracts apply.

Conceptually a target profile includes:

- cache/source revision identifier when known
- cache fingerprint/checksums where useful
- semantic reference snapshot(s)
- decoder schema version
- known revision gates
- deterministic presentation inputs used by fixtures

The exact Rust type is deferred until the cache contract is designed.

## 2. Canonical truth categories

RustOSRS recognizes four different kinds of truth.

### T1. Encoded-data truth

What bytes/opcodes/tables exist in the source data and how they decode.

Examples:

- object definition opcode meaning
- model encoding
- location delta encoding
- texture/floor definitions
- sentinel/id widths

Primary owner: `osrs-cache` + canonical specs.

### T2. Semantic-runtime truth

How decoded data becomes scene behavior.

Examples:

- loc type dispatch
- model selection
- transforms
- footprints
- contouring
- normal behavior
- bridge planes
- morph resolution
- animation state

Primary owner: `osrs-core` / `osrs-scene` + canonical specs.

### T3. Reference-presentation truth

How the selected reference renderer presents a semantically correct scene.

Examples may include:

- software-client face ordering
- reference fixed lighting result
- palette/color behavior
- camera/projection conventions where needed for golden views

This is used to verify appearance, but must not be confused with all possible correct renderer implementations.

### T4. RustOSRS product truth

Deliberate project choices.

Examples:

- wgpu architecture
- reverse-Z
- editor camera controls
- Catppuccin theme
- enhanced rendering options

Owner: ADRs/blueprints.

## 3. Parity levels

A feature or milestone must state which parity level it satisfies.

### P0. Decode parity

Canonical decoded values exactly match the target data semantics.

Use exact comparisons wherever the source domain is exact.

### P1. Semantic parity

Given equivalent decoded inputs and explicit state, Rust produces the same semantic result as the selected OSRS reference contract.

Examples:

- selected model IDs
- transformed vertices
- scene slot/type
- footprint
- plane
- face metadata
- resolved morph

This is the core correctness target.

### P2. Render-structural parity

The renderer receives and preserves the information required to realize reference behavior.

Examples:

- face priority remains intact
- alpha classification is correct
- texture/UV assignment is correct
- draw-order constraints are represented
- bridge/roof visibility data is preserved

This level can be exact without requiring pixel-identical rasterization.

### P3. Reference visual parity

A fixed semantic scene, camera, deterministic state, and reference presentation settings produce an image within the accepted golden-image tolerance.

This is where GPU rasterization/float/platform tolerance may be used.

### P4. Enhanced presentation compatibility

RustOSRS may intentionally improve presentation while preserving P0-P2 semantic/structural parity.

Examples could include higher-quality filtering, anti-aliasing, optional improved lighting, or editor visualization.

P4 is not "more correct" than P3. It is a deliberate alternate presentation of the same semantic scene.

## 4. Exact-versus-tolerance rule

Use exact equality for:

- decoded integer values
- fixed-point/integer transform outputs
- IDs/opcode interpretation
- terrain shape topology
- scene slot/type/orientation
- footprints/planes
- semantic face metadata
- deterministic sort/order outputs when defined exactly

Tolerance is allowed only where the contract itself is floating/rasterized or platform-sensitive, such as:

- final GPU pixels
- depth-edge coverage
- floating-point camera matrices if the implementation contract permits float math
- time-interpolated animation presentation when exact reference tick state is not the tested contract

Do not use screenshot tolerance to hide a failed exact semantic test.

## 5. Reference mode versus enhanced mode

### Reference mode

Purpose:

- golden scenes
- bug diagnosis
- semantic/render structural verification
- side-by-side comparison

Rules:

- deterministic terrain/color jitter
- fixed camera/time/state
- presentation improvements disabled where they would obscure comparison
- diagnostic metadata available

### Enhanced/editor mode

Purpose:

- everyday editing
- readability
- performance
- modern visual presentation

Rules:

- canonical semantic scene is unchanged
- enabled deviations are project decisions
- user-facing diagnostics can identify active deviations

## 6. Semantic target selection

The final blueprint must select and document a canonical initial target profile before implementation begins.

Because the current corpus mixes January 2026 deob and October 2026 RuneLite material, Checkpoint 3 must determine which behaviors are stable across those references and which require revision gates.

Until then, no statement becomes `VERIFIED` merely because both source groups are present.

## 7. Revision-gated behavior

When behavior differs across supported revisions, prefer explicit semantic branching keyed by target profile rather than heuristic detection buried inside algorithms.

Conceptually:

```text
match target_profile.semantic_revision {
    revision/range A => contract A,
    revision/range B => contract B,
}
```

The actual implementation may use capabilities/features rather than numeric revision matching when that is more accurate, but the branch must remain explicit and tested.

## 8. Cache/schema invalidation

Any persistent decoded-artifact cache must include enough identity to prevent stale semantic values from surviving:

- source/cache fingerprint or revision
- decoder schema version
- relevant semantic contract version where needed

A decoder change must not silently reuse old decoded geometry/definitions.

## 9. Bug classification

When a visual issue is reported, classify it by the earliest failing parity level:

1. P0 decode
2. P1 semantic
3. P2 render-structural
4. P3 reference visual
5. P4 enhanced-presentation only

Fix the earliest incorrect layer. Do not compensate for a P1 semantic bug with a shader or editor offset.

Example:

- wrong wall model orientation: P1, fix scene/model transform
- correct wall but wrong face submission ordering: P2/P3, fix renderer
- correct reference render but enhanced AA halos: P4, fix enhanced pipeline

## 10. Specification linkage

Atomic specs must declare which parity levels they own.

Example:

```text
SPEC: LOC-PLACEMENT-002
Domain: OSRS_SEMANTIC
Parity ownership: P1
```

Renderer ADRs/specs can then cite the semantic contract they consume rather than restating it.
