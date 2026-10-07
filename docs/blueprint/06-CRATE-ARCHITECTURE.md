# RustOSRS Crate Architecture

Status: **Checkpoint 2 architecture foundation, reconciled through M1**  
Decision class: `PROJECT_DECISION`

## 1. Initial workspace crates

The initial architecture uses six primary crates:

```text
crates/
├── osrs-core/
├── osrs-cache/
├── osrs-scene/
├── osrs-render/
├── osrs-reference/
└── osrs-editor/
```

This is a logical architecture decision. The implementation roadmap may stage creation of these crates rather than generating empty crates immediately.

## 2. Dependency graph

```text
                         +----------------+
                         |  osrs-editor   |
                         +-------+--------+
                                 |
              +------------------+------------------+
              |                  |                  |
              v                  v                  v
        +-----------+      +------------+      +-----------+
        | osrs-cache|      |osrs-render |      | osrs-scene|
        +-----+-----+      +-----+------+      +-----+-----+
              |                  |                   |
              |                  +---------+---------+
              |                            |
              v                            v
                       +----------------+
                       |   osrs-core    |
                       +----------------+

  osrs-reference -> osrs-core / osrs-scene / osrs-cache as test/reference tooling
  production crates -> NEVER depend on osrs-reference
```

Allowed direct dependencies:

| Crate | May depend on |
|---|---|
| `osrs-core` | general-purpose pure Rust support libraries only |
| `osrs-cache` | `osrs-core`, cache/decompression/serialization support |
| `osrs-scene` | `osrs-core` |
| `osrs-render` | `osrs-core`, `osrs-scene`, wgpu/render support |
| `osrs-reference` | `osrs-core`, `osrs-cache`, `osrs-scene`, fixture/process/test support |
| `osrs-editor` | `osrs-core`, `osrs-cache`, `osrs-scene`, `osrs-render`, eframe/egui/editor support |

`osrs-editor` is the composition root for the first product.

## 3. `osrs-core`

### Responsibility

Canonical OSRS semantic value types and pure algorithms that are independent of cache transport, scene ownership, GPU rendering, and editor UX.

Likely modules include:

```text
osrs-core
├── ids
├── coords
├── math
├── color
├── definitions
│   ├── object
│   ├── floor
│   ├── texture
│   ├── sequence
│   ├── varbit
│   └── ...
├── model
│   ├── model_data
│   ├── faces
│   ├── normals
│   └── transforms
├── terrain
├── animation
├── provenance
└── error
```

Exact module placement waits for the semantic audit.

### Owns

- stable ID/value wrappers
- coordinate/unit types
- canonical decoded definitions
- canonical model geometry/face metadata
- pure integer/fixed-point math
- color/HSL algorithms proven semantic
- revision/provenance metadata needed downstream
- semantic enums instead of magic numbers where they preserve exact raw values

### Must not own

- filesystem/cache handles
- XTEA key discovery service
- GPU buffers or wgpu types
- scene tile slot ownership
- editor selection/history
- egui/eframe types

### API principle

Do not erase raw semantic information for convenience. For example, face priority, alpha, texture IDs, transform groups, and raw orientation/type information must survive until the owning specification explicitly permits normalization.

## 4. `osrs-cache`

### Responsibility

Read OSRS cache data and normalize revision-specific encodings into `osrs-core` canonical values.

Likely modules:

```text
osrs-cache
├── source
├── archive
├── compression
├── xtea
├── decode
│   ├── object
│   ├── model
│   ├── landscape
│   ├── locations
│   ├── floor
│   ├── texture
│   ├── sequence
│   └── vars
├── revision
├── repository
└── error
```

### Owns

- cache/archive transport
- decompression/container parsing
- XTEA application
- opcode decoding
- source revision identification
- raw-to-canonical sentinel normalization
- decoded asset caching
- cache-specific error context

### Does not own

- loc placement semantics
- wall/decor scene dispatch
- model transform order unless the transform is literally part of decode
- contouring
- semantic morph resolution
- GPU upload
- editor project format

### Dependency decision

ADR-0010 accepts `rune-fs 0.2.0` as a **private read-only low-level transport dependency** behind `osrs-cache`. High-level `rs-cache` definition structs are not canonical RustOSRS data contracts, and RustOSRS owns all target/revision-aware semantic decoders.

The dependency does not alter the crate law: no `rune-fs` type crosses into `osrs-core`, map archive names are not a universal transport invariant, and production M3 decode APIs must wrap transport failures in RustOSRS provenance-rich errors.

## 5. `osrs-scene`

### Responsibility

Deterministically construct and mutate semantic OSRS scenes from canonical data.

Likely modules:

```text
osrs-scene
├── scene
├── tile
├── placement
├── builder
├── object_instance
├── terrain
├── bridge
├── roof
├── morph
├── animation_state
├── model_prepare
├── dirty
└── diagnostics
```

### Owns

- scene dimensions/planes/extended scene representation
- tile semantic slots
- loc type dispatch
- object placement/footprints/pivots
- bridge/plane semantics
- morph resolution against supplied var state
- scene-level model preparation required by verified OSRS behavior
- semantic dirty scopes after edits
- scene query APIs

### Does not own

- cache file access
- wgpu resources
- viewport camera preferences
- selection/undo/redo
- renderer-only batching/culling

### Asset access

`osrs-scene` must consume abstract semantic asset/definition providers or explicit build inputs rather than a concrete cache handle.

Provider contracts should live at the lowest reasonable semantic layer, probably `osrs-core`, so `osrs-cache` can implement them without `osrs-scene` becoming a dependency of the cache crate.

The exact provider trait shape is deferred until cache and scene contracts are audited. The architectural law is fixed: **scene code does not import cache implementation types**.

## 6. `osrs-render`

### Responsibility

Render semantic scenes using wgpu while preserving required OSRS metadata and providing modern diagnostics/performance architecture.

Likely modules:

```text
osrs-render
├── extract
├── renderer
├── graph
├── pipeline
├── shaders
├── textures
├── buffers
├── zones
├── sorting
├── culling
├── picking
├── camera
├── overlay
├── diagnostics
└── stats
```

### Owns

- wgpu device-facing state
- pipelines/shaders
- GPU texture/buffer lifetime
- render extraction caches
- draw ordering implementation
- render-only culling
- frame graph/pass scheduling
- reverse-Z or other depth policy
- parity/enhanced presentation mode switches
- GPU diagnostics

### Must preserve

- face priority
- alpha/transparency
- texture assignment and UV semantics
- semantic coordinates/orientations
- scene ownership/visibility rules
- any verified per-face/per-vertex attributes

### Does not own

- what loc type means
- which model an object definition selects
- semantic morph resolution
- cache opcode interpretation
- editor history/project state

## 7. `osrs-reference`

### Responsibility

Development-only reference and differential verification infrastructure.

It may wrap or index:

- `tools/deob-harness`
- `reference-fixtures`
- pinned RuneLite/deob outputs
- fixture regeneration metadata
- cross-language comparison adapters
- golden semantic scene manifests

### Hard rule

No production runtime crate may depend on `osrs-reference`.

The reference crate can depend inward on production semantics for tests, never the reverse.

## 8. `osrs-editor`

### Responsibility

Native desktop product built with eframe/egui, using Catppuccin for visual styling and `osrs-*` crates for OSRS behavior.

Likely modules:

```text
osrs-editor
├── app
├── document
├── commands
├── history
├── project
├── viewport
├── selection
├── tools
├── inspectors
├── panels
├── docking
├── theme
├── settings
├── persistence
├── recovery
└── diagnostics_ui
```

### Owns

- document lifecycle
- project files
- command transactions
- undo/redo
- autosave/recovery
- selection
- gizmos/tools
- panels/docking
- user settings
- Catppuccin integration
- filesystem dialogs and native integration

### Does not reimplement

- cache semantics
- model transforms
- terrain construction
- loc placement rules
- renderer sort rules

## 9. Provider and repository pattern

To avoid cache coupling, scene construction should request semantic data through narrow interfaces or pre-materialized inputs.

Conceptually:

```text
DefinitionProvider
  object(id) -> ObjectDefinition
  floor_underlay(id) -> FloorUnderlayDefinition
  floor_overlay(id) -> FloorOverlayDefinition
  sequence(id) -> SequenceDefinition
  varbit(id) -> VarbitDefinition

ModelProvider
  model(id) -> ModelData

TextureProvider
  texture(id) -> TextureDefinition / pixel source
```

These names are illustrative, not final APIs.

Important rules:

- providers return canonical semantic values, not decoder-private structs
- source provenance can be retained alongside canonical values
- callers must be able to distinguish missing, unsupported-revision, corrupt, and intentionally-null assets
- provider APIs must not force async into pure semantic algorithms unless actual I/O requirements justify it

## 10. Crate-boundary tests

Architecture tests/lints should eventually enforce:

- no `wgpu` in `osrs-core` or `osrs-scene`
- no `egui`/`eframe` below `osrs-editor`
- no `osrs-cache` dependency from `osrs-scene`
- no `osrs-reference` dependency from production crates
- no editor module imported by reusable crates

A simple dependency audit in CI is preferred over relying only on convention.

## 11. Why not `editor_core`, `editor_cache`, `editor_render`?

Those names encode the first application into reusable layers. The project charter explicitly allows a future Rust client/reference implementation.

`osrs-*` communicates ownership correctly:

- these crates implement reusable OSRS data/scene/render behavior
- `osrs-editor` is one consumer
- a future `osrs-client` could be another consumer

## 12. Deliberately deferred crate splits

Do not create extra crates merely for conceptual purity before profiling build times and module coupling.

Potential future extractions include:

- `osrs-types`
- `osrs-math`
- `osrs-assets`
- `osrs-project`
- `osrs-cli`

They require an ADR with a concrete coupling/build/test reason. Start with coherent crates and split when boundaries become operationally valuable.
