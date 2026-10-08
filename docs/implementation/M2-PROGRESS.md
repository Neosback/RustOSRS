# M2 Semantic Foundation Progress

Status: **M2 implementation complete; pending PR merge**  
Milestone: **M2 - `osrs-core` semantic foundation**  
Branch: `impl/m2-core-semantic-foundation`

This record tracks the bounded implementation slices that completed M2. Final exit-gate mapping is recorded in `docs/implementation/M2-EXIT-AUDIT.md`.

## M2A - Identity, coordinates, planes, and target provenance

Implemented in:

- `crates/osrs-core/src/ids.rs`;
- `crates/osrs-core/src/coords.rs`;
- `crates/osrs-core/src/provenance.rs`.

### Typed semantic IDs

Canonical non-negative IDs are distinct Rust types rather than renderer/cache integers. The initial set now includes object/model/texture/sequence/var/floor IDs plus supporting sprite/frame/skeletal/item/map metadata IDs required by canonical definition values.

The wrappers use `u32`, preserving target-era 32-bit model IDs. Revision-specific absence sentinels are not encoded into ID wrappers; owning canonical fields use `Option<IdType>` where absence is semantic.

### Coordinate contract

`COORD-001` constants are encoded exactly:

- one tile = `128` local units;
- half tile = `64`;
- quarter tile = `32`;
- three-quarter tile = `96`;
- one region axis = `64` tiles.

Distinct value types exist for:

- `MapTile` / `RegionCoord` / `RegionTile`;
- `SceneTile`;
- `LocalCoord` / `LocalXZ` / `LocalPoint`;
- `ModelPoint`.

Map coordinates remain `(x, y)` as map/tile identity. Three-dimensional semantic coordinates use `(x, y, z)`, where semantic `y` is vertical, so map `y` becomes local horizontal `z` at the explicit conversion boundary.

Negative map/local values use Euclidean division/remainder where round-trip behavior requires it. Scene coordinates encode no universal scene dimensions.

### Plane distinctions

M2A introduces typed ordinary plane values without implementing M6 bridge behavior early:

- `SourcePlane`;
- `StoragePlane`;
- `CollisionPlane`;
- `RenderLevel`.

Ordinary validated plane indices are `0..=3`. `CollisionPlane::None` can represent the no-valid-plane state without using a negative or sentinel integer.

This establishes the `COORD-003` separation needed by later plane/bridge semantics while leaving actual bridge adjustment/relink rules to M6.

### Target provenance

`osrs-core` carries M1 identity using cache-transport-independent values:

- `ProfileDigest` for `rustosrs-target-profile-digest-v1`;
- `CacheFingerprint` for `rustosrs-cache-v1`;
- `TargetProvenance` containing profile ID, profile digest, cache fingerprint, and decoder schema version.

The digest parser requires exact 64-character lowercase hexadecimal form. No YAML, filesystem, OpenRS2, XTEA-provider, or `rune-fs` type crosses into `osrs-core`.

The tests use the accepted M1 build-241 vectors:

- profile digest `cfdefa9ef99eff799fcef4fdf0ec78d9fdcd72d8e5be78e1c154d018ab4575b7`;
- cache fingerprint `ae76dad78b4990d1b404e68e77a85ed2c96cf4a56c16f7b017cb97d1e92fdb38`.

### M2A verification

Tests cover exact tile constants, positive/negative local round trips, region borders, scene/world round trips, map-to-local conversion, model-local placement, plane range/type separation, full `u32` IDs, digest parsing, provenance, and hash/value behavior.

Result:

- Tier A architecture, formatting, workspace check, and strict Clippy: **PASS**;
- Tier B workspace tests: **PASS**.

---

## M2B - Orientation helpers and canonical definitions

Implemented in:

- `crates/osrs-core/src/orientation.rs`;
- `crates/osrs-core/src/definitions.rs`;
- extensions to `crates/osrs-core/src/ids.rs`.

### Exact orientation foundations

M2B introduces distinct orientation types rather than passing editor/render angles as untyped integers:

- `LocOrientation` accepts only encoded loc orientations `0..=3`;
- `ModelOrientation` preserves audited model-construction values `0..=7`, including the semantic `orientation > 3` branch before `orientation & 3` reduction;
- `QuarterTurn` performs the exact integer ModelData quarter-turn mappings;
- `JauAngle` preserves the canonical `0..2047` Jagex angular-unit domain;
- `256` JAU is named explicitly as the diagonal-decoration angle required by `MODEL-BUILD-003`.

Quarter-turn negation uses wrapping two's-complement behavior so the helper follows Java `int` semantics rather than Rust debug-overflow behavior.

The actual type-4 operation `general 256-JAU rotation -> translate (45, 0, -45) -> ordinary quarter turn` remains M4 model-construction ownership. M2B establishes the exact value/quarter-turn conventions but does not prematurely implement the M4 transform sequence.

### Canonical definition identity

`DefinitionIdentity<I>` combines a typed semantic ID with immutable `TargetProvenance`. Definitions therefore cannot silently lose the target profile/cache identity that produced them.

`Rgb24` provides an exact 24-bit RGB semantic value rather than using arbitrary renderer colors.

### Object definitions

`ObjectDefinition` now has canonical cache-independent fields for the M3-M8 consumers, including:

- typed vs untyped model-table distinction through `ObjectModels`;
- full-width `ModelId` values;
- footprint size;
- collision/placement flags without performing placement;
- wall-decoration displacement/support-item metadata;
- `is_rotated` and `non_flat_shading`;
- contour clip and animation ID;
- signed ambient/contrast values;
- exact model scale/translation inputs;
- recolor/retexture pairs;
- morph selector inputs, nullable transform entries, and an explicit fallback;
- map-scene/map-icon/category metadata;
- five action slots.

This representation intentionally prevents M3 from flattening opcode-1 typed models into opcode-5 untyped models or discarding opcode-92 fallback/null semantics.

### Floors, vars, textures, and sequences

M2B also establishes canonical containers for:

- `FloorUnderlayDefinition`;
- `FloorOverlayDefinition`, preserving absent texture and secondary color distinctly;
- `VarbitDefinition` and `VarpDefinition`;
- `TextureDefinition`, with source sprites/composition inputs and no semantic fixed-256 texture limit;
- `SequenceDefinition`, retaining both frame-based and build-241-era skeletal inputs without forcing false mutual exclusivity.

These are semantic containers, not cache decoders. Opcode interpretation, target revision validation, varbit bit-range proof, texture post-232 decoding, and sequence post-226 decoding remain M3 responsibilities.

### M2B verification

Tests cover:

- loc/model orientation validation;
- extended orientation branch preservation;
- all four exact ModelData quarter turns;
- four-turn identity;
- Java-style wrapping negation;
- JAU wrapping and the `256` diagonal value;
- typed vs untyped object-model tables;
- null morph entries and explicit fallback;
- overlay absence/default distinction;
- more than 256 semantic texture/source entries without truncation;
- simultaneous retention of frame and skeletal sequence inputs;
- definition clone/equality/hash stability.

Result on the code head:

- Tier A architecture, formatting, workspace check, and strict Clippy: **PASS**;
- Tier B workspace tests: **PASS**.

---

## M2C - Canonical source model and working model ownership

Implemented in:

- `crates/osrs-core/src/model.rs`;
- `crates/osrs-core/src/lib.rs`.

### Source model representation

M2C introduces a validated, cache-independent `SourceModel` boundary with explicit semantic types for:

- vertices and triangle topology;
- vertex, face, and texture-triangle indices;
- face colors;
- reference face priorities constrained to `0..=11`;
- optional face render-type, priority, alpha, texture, texture-selector, and authored face-bias arrays;
- texture triangles and retained mapping parameters;
- vertex and face skin metadata;
- per-vertex skeletal bone/weight source data;
- source model-format identity and optional format version.

Optional source arrays preserve **absence**. `None` means the source array did not exist; present arrays retain entry-level absence/sentinels where the target semantics require them. In particular, signed alpha bytes such as `-1` are preserved rather than normalized during admission.

The audited ModelData trailer names retain source byte order explicitly: `FF FF`, `FF FE`, and `FF FD`. The type names make no unsupported claim about chronology.

### Validation boundary

`SourceModel::from_parts` rejects malformed canonical model state before it can become a shared source asset. Current validation covers:

- face-parallel array lengths;
- vertex-parallel array lengths;
- face vertex references;
- texture-triangle vertex references;
- texture-face selector bounds;
- skeletal bone/weight count agreement.

This validation is semantic shape/topology validation only. M3/M4 still own target decoding and exact construction behavior.

### Immutable source vs mutable working state

A validated `SourceModel` exposes read access only. `to_working_copy()` produces a deep owned `WorkingModel`, so later transforms cannot mutate the cached/shared source asset.

`WorkingModel` carries explicit derived state rather than overloading source arrays:

- `ModelNormalState::Uncomputed` vs computed normal data;
- base vertex normals;
- flat face normals;
- optional merged vertex-normal slots for cross-model accumulation;
- derived animation groups separate from original vertex/face skin arrays.

Geometry/topology and render-type mutation invalidates derived normal state through the working-model API. Cross-model merged normals therefore remain separate from base normals and do not imply topology welding.

### Scope held for later milestones

M2C deliberately does **not** implement:

- model cache decoding;
- typed/untyped object model selection;
- mirror, resize, recolor, retexture, translation, or orientation transform order;
- base-normal calculation or cross-model normal-merge algorithms;
- final lighting;
- animation-group construction or animation execution;
- contouring;
- renderer/GPU packing.

Those remain owned by M3/M4/M7/M8 and the renderer milestones defined in the roadmap.

### M2C verification

Tests cover:

- face-priority domain enforcement;
- audited model-trailer byte-order vocabulary;
- optional-array absence preservation;
- entry-level texture/sentinel and signed-alpha preservation;
- parallel-array validation;
- invalid topology rejection;
- texture mapping/selector retention and bounds validation;
- source/working-copy independence;
- normal-state invalidation after geometry/render-type mutation;
- separation of base and merged normal state;
- derived animation groups without destroying source skin metadata;
- skeletal bone/weight validation.

Result on code head `d49c81e0ec5bb285d7af671d2446e9d3c7fe932a`:

- Tier A architecture, formatting, workspace check, and strict Clippy: **PASS**;
- Tier B workspace tests: **PASS**.

---

## M2 exit/cleanup audit

The final audit reconciled the milestone with every deliverable and verification gate in `docs/blueprint/17-IMPLEMENTATION-ROADMAP.md` and the directly owned `COORD-001..003` and `FACE-001` requirements.

One concrete gap was found and closed: the roadmap requires tile constants **and centers** to be exact. `crates/osrs-core/src/coordinate_math.rs` now provides checked integer `tile_center` and `footprint_center` helpers using `origin_local + footprint_tiles * 64`, with tests for 1x1, 2x3, 3x2, negative coordinates, zero-size rejection, and overflow.

The audit confirmed:

- source model values remain immutable through the public API and hash/compare deterministically;
- optional face arrays retain absence/default semantics;
- raw model face/texture/bias data and object-authored recolor/retexture transform inputs remain at their correct semantic ownership boundaries;
- no cache transport, scene ownership, renderer, GPU, or editor type leaks into `osrs-core`;
- M3/M4/M6/M7/M8 behavior has not been implemented prematurely.

The full gate-by-gate closure and carryovers are recorded in `docs/implementation/M2-EXIT-AUDIT.md`.

Code head with the center-helper closure:

- Tier A architecture, formatting, workspace check, and strict Clippy: **PASS**;
- Tier B workspace tests: **PASS**.

The documentation-finalized branch head must remain green before the PR is merged.

M3 must not begin until M2 is merged.
