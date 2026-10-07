# M1 Decoder Acceptance Checklist

Status: **M1 acceptance contract, slice 2**  
Scope: cache/decoder inputs required by implementation milestones M3-M8

This checklist defines what a RustOSRS decoder must preserve before its output can feed canonical semantics. It is deliberately field-oriented: matching a third-party struct name is not acceptance.

Statuses used during implementation:

- `REQUIRED`: must be implemented and tested before the owning milestone exits;
- `REVISION_GATED`: required when the selected profile/data uses the form;
- `BLOCKED`: known requirement whose exact target behavior is not yet proven;
- `DEFERRED`: valid cache family intentionally owned by a later milestone.

## 1. Transport and archive contract

Required before M3:

- [ ] enumerate logical indices without assuming a fixed maximum useful index count;
- [ ] read exact encoded reference-table bytes;
- [ ] enumerate group/archive IDs and file IDs;
- [ ] read exact encoded logical group bytes independent of physical DAT2 sector layout;
- [ ] support cache compression forms used by target build 241;
- [ ] expose group/archive version and reference metadata needed for provenance;
- [ ] preserve archive name hashes where present;
- [ ] provide typed missing-index/archive/file failures;
- [ ] provide decompression failures without panicking;
- [ ] support caller-owned XTEA input for profiles that require it;
- [ ] attach target profile and cache fingerprint to decode provenance;
- [ ] support `rustosrs-cache-v1` fingerprint enumeration.

No external transport type may cross the public `osrs-cache -> osrs-core` boundary.

## 2. Terrain/map tile decode

Owning milestone: M3, consumed by M6.

Required fields/behavior:

- [ ] four encoded/source planes where present;
- [ ] explicit/default height opcode behavior as raw semantic input;
- [ ] overlay ID;
- [ ] overlay shape/path;
- [ ] overlay rotation;
- [ ] tile settings/flags;
- [ ] underlay ID;
- [ ] exact tile traversal/order;
- [ ] region identity and local tile coordinates kept separate;
- [ ] no decoder-level universal bridge plane adjustment.

Acceptance links: `TERRAIN-001..004`, `PLANES-001..004`, `COORD-001..003`.

The decoder supplies encoded tile inputs only. Scene/storage/collision/render-level interpretation belongs to later semantic layers.

## 3. Location stream decode

Owning milestone: M3, consumed by M6.

Required:

- [ ] smart/delta object ID accumulation;
- [ ] smart/delta packed position accumulation;
- [ ] local X/Y and encoded plane extraction;
- [ ] loc type;
- [ ] orientation;
- [ ] 32-bit-capable object IDs in canonical output;
- [ ] region-local coordinates preserved without accidental `region + local` pseudo-world coordinates;
- [ ] source plane preserved unchanged;
- [ ] encrypted archive key identity included when applicable.

Acceptance links: `LOC-PLACEMENT-001..006`, `PLANES-001`, `COORD-001..003`.

## 4. Object definition decode

Owning milestone: M3, consumed throughout M4-M8.

Identity/model inputs:

- [ ] object ID wider than `u16` assumptions where target format permits;
- [ ] model ID list;
- [ ] optional model-type list;
- [ ] extended/32-bit model-ID forms (target profile gate);
- [ ] no decoder fallback to a first model.

Placement/scene fields:

- [ ] size X/Y;
- [ ] interact/collision flags required by placement;
- [ ] projectile blocking;
- [ ] model clipping/occlusion-related definition flags;
- [ ] decoration displacement;
- [ ] support-items field;
- [ ] obstruct-ground/hollow/clipped semantics as encoded;
- [ ] category/map-scene fields when needed for editor metadata.

Model construction fields:

- [ ] `isRotated`/mirror source flag;
- [ ] recolor pairs;
- [ ] retexture pairs;
- [ ] model resize X/Y/Z;
- [ ] model translation X/Y/Z with signed semantics preserved;
- [ ] ambient as signed encoded value;
- [ ] contrast as signed encoded value;
- [ ] contour/clip type;
- [ ] `nonFlatShading`;
- [ ] animation/sequence ID.

Morph inputs:

- [ ] varbit selector;
- [ ] varp selector;
- [ ] transform list;
- [ ] opcode-92 explicit default/fallback transform;
- [ ] `-1`/null sentinel preservation.

Revision-sensitive fields:

- [ ] post-220 sound layout when encountered;
- [ ] extended entity/model opcodes when encountered;
- [ ] unknown valid target opcode becomes typed unsupported/decode error, never `unreachable!()`.

Acceptance links: `MODEL-BUILD-001..005`, `MORPH-001`, `ANIMATION-001`, `CONTOUR-001`, `LOC-PLACEMENT-*`, `NORMALS-*`, `LIGHTING-001`.

## 5. Floor underlay definitions

Owning milestone: M3.

Required:

- [ ] RGB primary field;
- [ ] exact absence/default behavior;
- [ ] definition ID/provenance.

Derived HSL/hue-multiplier behavior belongs to canonical semantic code, not necessarily the byte decoder.

Acceptance links: `TERRAIN-003`, `TERRAIN-004`.

## 6. Floor overlay definitions

Owning milestone: M3.

Required:

- [ ] primary RGB;
- [ ] texture ID;
- [ ] `hideUnderlay` default and opcode change;
- [ ] secondary RGB with missing sentinel;
- [ ] any build-241 fields that affect canonical terrain/material semantics;
- [ ] unknown target opcode handling with provenance.

Acceptance links: `TERRAIN-003`, `TERRAIN-004`.

`TERRAIN-004` remains blocked for the complete higher-level terrain color/build routine even after floor definitions decode correctly.

## 7. Varbit and varp definitions/state metadata

Owning milestone: M3, consumed by M8.

Varbit required fields:

- [ ] backing varp ID;
- [ ] start bit;
- [ ] end bit;
- [ ] exact bit-range convention documented/tested.

Varp required fields:

- [ ] fields actually required by the selected target/editor preview model;
- [ ] stable ID/provenance even when no additional semantic field is required.

Acceptance link: `MORPH-001`.

## 8. Model data decode

Owning milestone: M4.

Required geometry:

- [ ] vertex count and signed coordinates;
- [ ] face count and vertex indices;
- [ ] texture triangle count/data;
- [ ] format/version identity;
- [ ] all model encoding variants encountered by build 241.

Required face metadata:

- [ ] face color/HSL source value;
- [ ] face render type;
- [ ] per-face priority or default model priority;
- [ ] signed alpha byte without premature reinterpretation;
- [ ] texture/material ID;
- [ ] texture-face selector;
- [ ] authored face-bias input if present in target model source/derived import path;
- [ ] face skin/group data required by animation.

Required vertex metadata:

- [ ] vertex skin/group data;
- [ ] skeletal/bone data when required by selected sequence formats;
- [ ] data necessary to preserve source topology before normals are calculated.

Texture mapping:

- [ ] explicit texture triangle vertices;
- [ ] mapping/render types used by the target;
- [ ] scale/rotation/direction/translation values when encoded by model version;
- [ ] no destructive filtering before canonical semantics have consumed required data.

Acceptance links: `MODEL-BUILD-*`, `NORMALS-*`, `FACE-*`, `TEXTURE-001`, `ANIMATION-001`.

## 9. Texture definitions/material inputs

Owning milestone: M3/M4 data acquisition, consumed by M12 renderer work.

Required:

- [ ] texture definition ID with no fixed 256-count invariant;
- [ ] source sprite/file ID(s) as target format defines them;
- [ ] average RGB/reference color field;
- [ ] transparency flag where encoded;
- [ ] animation direction;
- [ ] animation speed;
- [ ] post-232/build-241 layout;
- [ ] any composition/color-adjustment data needed to reproduce target texture pixels.

Acceptance link: `TEXTURE-001`, `FACE-003/004` where applicable.

## 10. Sequence definitions

Owning milestone: M3 decode, consumed by M8.

Required for frame-based sequences:

- [ ] frame IDs;
- [ ] frame delays;
- [ ] frame step/loop behavior;
- [ ] interleave/mask data;
- [ ] max loops;
- [ ] precedence/priority fields affecting animation selection;
- [ ] equipment override IDs where relevant;
- [ ] reply/restart mode where relevant.

Required for skeletal forms encountered by build 241:

- [ ] skeletal animation ID;
- [ ] range begin/end;
- [ ] mask data;
- [ ] revision-correct opcode ownership for build 241.

Sound-only fields may be retained for completeness but are not allowed to block visual editor M8 unless they affect animation timing/selection.

Acceptance link: `ANIMATION-001`.

## 11. Frames/skeletons

Owning milestone: M8 unless implementation proves an earlier dependency.

Status: `DEFERRED` from basic M3 config decode, but required before animated model parity can close.

Required eventually:

- [ ] frame archive lookup;
- [ ] skeleton/base lookup;
- [ ] transform labels/types;
- [ ] transform values with exact signed/smart semantics;
- [ ] alpha-transform ownership;
- [ ] skeletal animation resources used by build-241 sequences.

Acceptance link: `ANIMATION-001`.

## 12. Decoder error/provenance envelope

Every public decode failure must be able to identify:

- [ ] target profile ID/digest;
- [ ] cache fingerprint;
- [ ] decoder schema version;
- [ ] index/archive/file identity;
- [ ] definition/model/region ID when applicable;
- [ ] opcode or model-format branch when known;
- [ ] byte offset/range when known;
- [ ] revision gate/build involved;
- [ ] XTEA provider/key fingerprint for encrypted inputs, never raw keys.

Panics, `unreachable!()`, generic "unknown parser error", or silent opcode logging are not acceptable production decode outcomes for target data.

## 13. Acceptance method

A decoder family is accepted only when:

1. required fields above have canonical Rust-owned representations;
2. target build-241 samples exercise relevant revision gates;
3. values are compared against at least one independent source/oracle where feasible;
4. malformed/unknown data fails with the provenance envelope;
5. fixture ownership is recorded under `docs/verification/PARITY-MATRIX.md` / `reference-fixtures/` as implementation advances.

Third-party decoder coverage is evidence for implementation, never a substitute for these acceptance checks.
