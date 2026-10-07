# RustOSRS System Architecture

Status: **Checkpoint 2 architecture foundation**  
Decision class: `PROJECT_DECISION`

## 1. Architectural thesis

RustOSRS separates five concerns that must never collapse into one layer:

1. **Source ingestion**: read cache/archive/reference data.
2. **Canonical OSRS semantics**: represent definitions, models, coordinates, terrain inputs, and deterministic algorithms without UI/GPU dependencies.
3. **Scene construction**: resolve locs, morphs, placement, footprints, planes, terrain, prepared models, and scene ownership.
4. **Rendering**: transform an already-correct semantic scene into GPU work.
5. **Application/editor orchestration**: tools, project state, selection, undo/redo, persistence, and UX.

The architecture is intentionally designed so a future non-editor application can reuse layers 1-4.

## 2. Canonical data flow

```text
OSRS cache / cache-like source
        |
        v
    osrs-cache
  decode + normalize
        |
        v
    osrs-core
canonical definitions/assets/value types
        |
        v
    osrs-scene
scene-build semantics + prepared semantic scene
        |
        v
   osrs-render
render extraction + wgpu resources/passes
        |
        v
   osrs-editor
 eframe / egui / Catppuccin
```

Verification is sidecar infrastructure:

```text
pinned deob / RuneLite / fixtures
        |
        v
  osrs-reference
        |
        +---- differential comparisons ----> osrs-core / osrs-scene
```

`osrs-reference` is never a production runtime dependency of the editor.

## 3. Dependency laws

### L1. Semantic crates never depend on the renderer

`osrs-core` and `osrs-scene` may not depend on wgpu, WGSL, egui, eframe, windowing, Catppuccin, or editor types.

### L2. Scene construction never depends on cache I/O

`osrs-scene` consumes canonical `osrs-core` values and asset/definition inputs. It must be possible to construct a scene entirely from synthetic test values without opening a cache.

This is essential for deterministic tests and future sources such as project snapshots, network-fed clients, fixture loaders, or generated scenes.

### L3. Cache decoding does not construct rendered scenes

`osrs-cache` may normalize revision-specific raw records into canonical semantic values. It must not decide GPU buffers, visual culling, viewport behavior, selection state, or editor policy.

### L4. Renderer consumes semantic truth; it does not reinterpret it

`osrs-render` may derive render extraction structures for performance, but it may not invent object types, change transform order, resolve morphs differently, discard required face metadata, or mutate canonical scene objects to simplify GPU work.

### L5. Editor owns editing history, not semantic truth

Undo/redo, commands, transactions, dirty documents, selection, clipboard behavior, gizmos, and project persistence belong to `osrs-editor` or editor-specific support modules. They operate on semantic scene/document APIs rather than duplicating OSRS rules.

### L6. Reference tooling is one-way

Production crates may be tested against `osrs-reference`, but production code must never call into the Java/deob reference implementation at runtime.

## 4. Three representations, not one giant model

RustOSRS will maintain clear transitions between three representations.

### 4.1 Decoded canonical data

Examples:

- object definitions
- model data
- texture definitions
- floor underlay/overlay definitions
- sequence definitions
- varbit/varp definitions
- landscape/map records
- location records

These values represent cache meaning after revision-specific normalization.

### 4.2 Semantic scene data

Examples:

- tiles and planes
- resolved scene objects
- boundary/decor/floor/game-object slots
- object footprints and pivots
- resolved model variants
- transformed/contoured/prepared model instances where the verified semantics require them
- bridge/roof relationships
- resolved morph/animation state for a selected preview state

This representation answers: **what exists in the OSRS scene and what does it mean?**

### 4.3 Render extraction data

Examples:

- immutable frame/zone extraction packets
- GPU vertex/index staging records
- material/texture binding IDs
- draw-sort keys
- opaque/alpha partitioning
- culling bounds
- diagnostic overlays

This representation answers: **how should the current renderer submit the already-defined scene?**

The renderer may rebuild extraction data freely. It must never be the only place where semantic information exists.

## 5. Ownership and mutability

### 5.1 Immutable source assets by default

Decoded definitions and raw/canonical model assets are treated as immutable shared inputs after loading.

Any semantic operation that historically mutates a model must have explicit ownership semantics:

- borrow if read-only
- clone/copy-on-write if modification is instance-specific
- mutate only a uniquely owned prepared instance

This rule exists specifically to prevent recolor, contour, pose, normals, or editor preview operations from corrupting a shared cached model.

### 5.2 Scene owns placement, not source definitions

An object definition says what an object can be. The scene owns where a resolved instance is placed, which orientation/type path was selected, and which dynamic state is active.

### 5.3 Renderer owns GPU lifetime only

GPU buffers, textures, pipelines, bind groups, upload caches, frame resources, and fences belong to `osrs-render`.

Canonical scene data must remain valid even if the renderer is destroyed and recreated.

### 5.4 Editor owns document transactions

The editor may stage changes before committing them into semantic document state. Semantic crates must expose operations/invariants needed to validate those edits without knowing about UI widgets.

## 6. Error model

Errors must preserve stage and identity context.

A useful error should be able to include:

- source/cache revision
- archive/index/group/file or definition ID
- object/model/texture/region ID
- tile/plane/location
- semantic stage
- related spec ID when known
- underlying decode/validation cause

Do not reduce low-level errors to generic messages such as `failed to load model` before diagnostics can identify the responsible asset and stage.

## 7. Revision normalization boundary

Revision-specific wire/cache quirks belong at or immediately above the decode boundary.

Canonical downstream semantics should not have to ask questions such as:

- was this model ID encoded as 16-bit or 32-bit?
- did this cache use an older opcode arrangement?
- was absence represented by `65535`, `-1`, or an omitted field?

Instead, `osrs-cache` normalizes raw encoding into explicit canonical values and records source revision/provenance where differences still matter.

If a semantic behavior genuinely differs by revision, that difference remains explicit rather than being normalized away.

## 8. Determinism boundary

Given identical:

- canonical decoded inputs
- selected semantic target revision
- scene coordinates/regions
- varbit/varp preview state
- animation time/frame state
- deterministic terrain jitter/settings

`osrs-scene` must produce deterministic semantic output.

GPU scheduling, floating-point rasterization, driver behavior, and editor frame timing are outside this determinism guarantee.

## 9. Renderer parity boundary

Rendering has two responsibilities:

1. faithfully preserve semantic information required by the scene
2. choose a modern GPU implementation strategy

The first is non-negotiable. The second is project-owned.

Examples:

- Face priority values are semantic metadata.
- The exact GPU algorithm used to realize correct priority ordering is renderer policy.
- Alpha/transparency metadata is semantic.
- Buffer partitioning into opaque/alpha pools is renderer policy.
- UV meaning and texture assignment are semantic.
- Texture array packing strategy is renderer policy.

Every renderer blueprint section must make this distinction explicit.

## 10. Editor-to-scene mutation rule

The editor must never directly mutate render-extraction or GPU buffers as the source of truth.

Correct flow:

```text
Editor command
    -> semantic document/scene mutation
    -> dirty semantic scope identified
    -> render extraction invalidated/rebuilt
    -> GPU upload/update
```

This ensures undo/redo, save/export, diagnostics, and future renderer backends all observe the same scene state.

## 11. Future client compatibility

A future Rust client should be able to reuse:

- `osrs-core`
- `osrs-cache`
- `osrs-scene`
- `osrs-render`

and replace `osrs-editor` with a client application layer that owns networking, entities, game state, UI, input, and tick orchestration.

Therefore editor-only concepts are prohibited from leaking downward into reusable crates.

## 12. Deliberately unresolved here

Checkpoint 2 does **not** decide the semantic answer to:

- cross-model normal accumulation/merging
- exact priority-sort behavior
- precise model lighting parity algorithm
- all roof/bridge edge cases
- animation/morph runtime rules
- exact camera parity requirements
- texture-count limits
- which RuneLite GPU paths are historical versus semantically required

Those remain blocked by the contradiction register and belong to Checkpoint 3.
