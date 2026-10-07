# M2 Semantic Foundation Progress

Status: **M2 in progress; M2A complete pending CI**  
Milestone: **M2 - `osrs-core` semantic foundation**  
Branch: `impl/m2-core-semantic-foundation`

This record tracks bounded implementation slices inside M2. It does not mark the milestone complete until all M2 deliverables and exit gates in `docs/blueprint/17-IMPLEMENTATION-ROADMAP.md` are satisfied.

## M2A - Identity, coordinates, planes, and target provenance

Implemented in:

- `crates/osrs-core/src/ids.rs`;
- `crates/osrs-core/src/coords.rs`;
- `crates/osrs-core/src/provenance.rs`.

### Typed semantic IDs

Canonical non-negative IDs are distinct Rust types rather than renderer/cache integers:

- `ObjectId`;
- `ModelId`;
- `TextureId`;
- `SequenceId`;
- `VarbitId`;
- `VarpId`;
- `FloorUnderlayId`;
- `FloorOverlayId`.

The wrappers use `u32`, preserving target-era 32-bit model IDs. Revision-specific absence sentinels are not encoded into ID wrappers; owning canonical fields will use `Option<IdType>` where absence is semantic.

### Coordinate contract

`COORD-001` constants are encoded exactly:

- one tile = `128` local units;
- half tile = `64`;
- quarter tile = `32`;
- three-quarter tile = `96`;
- one region axis = `64` tiles.

Distinct value types now exist for:

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

`osrs-core` now carries M1 identity using cache-transport-independent values:

- `ProfileDigest` for `rustosrs-target-profile-digest-v1`;
- `CacheFingerprint` for `rustosrs-cache-v1`;
- `TargetProvenance` containing profile ID, profile digest, cache fingerprint, and decoder schema version.

The digest parser requires exact 64-character lowercase hexadecimal form. No YAML, filesystem, OpenRS2, XTEA-provider, or `rune-fs` type crosses into `osrs-core`.

The tests use the accepted M1 build-241 vectors:

- profile digest `cfdefa9ef99eff799fcef4fdf0ec78d9fdcd72d8e5be78e1c154d018ab4575b7`;
- cache fingerprint `ae76dad78b4990d1b404e68e77a85ed2c96cf4a56c16f7b017cb97d1e92fdb38`.

## M2A verification coverage

Tests cover:

- exact tile constants;
- positive/negative local tile round trips;
- region split/reconstruction across zero and region borders;
- scene/world round trips with explicit scene origin;
- exact map-tile to local-XZ conversion;
- model-local placement without mutating source values;
- plane range validation and type separation;
- full `u32` model-ID preservation;
- ID ordering/hash/value behavior;
- digest lower-hex parsing/round trip;
- M1 target provenance preservation and invalid-input rejection.

## M2 work intentionally not started in this slice

Remaining M2 work includes:

1. exact angular/orientation helpers required by `COORD-002`;
2. canonical object/floor/texture/var/sequence definition structures;
3. canonical model geometry/face/material representation required by `FACE-001`;
4. normals/merged-normal working structures at the correct semantic ownership boundary;
5. final M2 immutability/default/absence tests and exit audit.

M3 must not begin until the complete M2 milestone is merged.
