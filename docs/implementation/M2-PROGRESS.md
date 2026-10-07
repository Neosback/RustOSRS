# M2 Semantic Foundation Progress

Status: **M2 in progress; M2A and M2B complete**  
Milestone: **M2 - `osrs-core` semantic foundation**  
Branch: `impl/m2-core-semantic-foundation`

This record tracks bounded implementation slices inside M2. It does not mark the milestone complete until all M2 deliverables and exit gates in `docs/blueprint/17-IMPLEMENTATION-ROADMAP.md` are satisfied.

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

## Remaining M2 work

M2 is **not complete**. Remaining work includes:

1. canonical source-model geometry/topology representation;
2. `FACE-001` metadata preservation, including optional-array absence/default semantics;
3. texture-triangle/mapping source structures and authored face-bias storage;
4. vertex/face skin and animation-group source metadata;
5. base/merged-normal working structures at the correct semantic ownership boundary;
6. any remaining pure integer helper required for M2-owned semantic representation without pulling M4 construction behavior early;
7. final source-model immutability/hash/comparison controls;
8. M2 exit audit against `COORD-001..003`, `FACE-001` representation requirements, and the roadmap gates.

M3 must not begin until the complete M2 milestone is merged.
