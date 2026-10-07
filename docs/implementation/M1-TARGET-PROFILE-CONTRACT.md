# M1 Target Profile Contract

Status: **M1 contract draft, slice 2**  
Decision class: `IMPLEMENTATION_CONTRACT`

`TargetProfile` identifies the exact cache snapshot and revision-sensitive decoding rules used to produce canonical RustOSRS artifacts. It is not a renderer profile, editor preference set, or local cache path.

The first concrete profile is `profiles/osrs-live-241-2026-09-30-openrs2-2727.yaml`.

## 1. Required identity

A v1 profile must include:

- stable profile ID;
- profile schema version;
- decoder schema version;
- game family;
- exact cache source provider and provider-specific source ID;
- source game/environment/language/build/timestamp metadata;
- deterministic cache fingerprint algorithm and verified value once imported;
- semantic evidence pins;
- explicit revision gates;
- XTEA/key-provider identity without persisting raw secret keys.

A local filesystem path is never authoritative identity.

## 2. Source snapshot versus local cache instance

The profile separates two identities:

1. **source snapshot identity**: where an exact cache can be reproduced, such as OpenRS2 cache 2727;
2. **local logical cache fingerprint**: proof that the bytes currently opened by RustOSRS are the expected logical cache contents.

This permits the same target profile to be reproduced from `.dat2/.idx`, a flat-file export, or another physically different storage layout while rejecting a logically different cache.

## 3. Profile digest input

When a Rust `TargetProfile` type is introduced, its digest must be computed from canonical semantic fields rather than YAML formatting. Comments, key order, and line endings must not change identity.

The canonical digest input must include at least:

- schema/profile ID;
- decoder schema version;
- cache source identity;
- expected build/revision metadata;
- verified cache fingerprint;
- semantic source pins;
- revision-gate values;
- XTEA provider/key-set identity.

The serialization used for the digest must be specified before persistence code relies on it.

## 4. XTEA ownership

XTEA keys are decode inputs owned outside canonical semantic definitions.

Rules:

- raw keys are not serialized into a target profile;
- the profile records provider/key-set identity;
- encrypted decoded artifacts include an `rustosrs-xtea-v1` key fingerprint in invalidation identity;
- changing a region key must invalidate that region's decoded location artifact;
- profiles where the source reports no required XTEA keys still keep the boundary so older/other targets remain representable.

## 5. Revision gates

Revision gates are explicit named profile fields or decoder capabilities, not scattered `if build > N` assumptions with no provenance.

For the build-241 initial target, known required compatibility areas include:

- 32-bit/extended object model-ID forms when encountered;
- post-220 object sound fields;
- post-226 sequence opcode layout;
- post-232 texture layout;
- `TERRAIN-004` remains blocked until its exact builder provenance is closed.

OpenRune thresholds are implementation evidence. Each threshold used in production decoding must eventually be corroborated by the selected target data and/or pinned client/deob evidence.

## 6. Decoded artifact identity

A decoded artifact cache key must include enough information to make cross-target reuse impossible. At minimum:

- target profile digest;
- verified cache fingerprint;
- decoder schema version;
- index/archive/file or definition identity;
- applicable revision-gate identity;
- XTEA key fingerprint for encrypted location content.

A numeric object/model/archive ID alone is never a safe cache key.

## 7. Initial target source pin

The initial profile pins OpenRS2 cache `2727`:

- game: `oldschool`;
- environment: `live`;
- language: `en`;
- build: `241`;
- timestamp: `2026-09-30 12:30:05`;
- format: `VERSIONED`;
- archives: `25 / 25`;
- groups: `117584 / 117584`;
- reported XTEA keys: `0 / 0`.

The source URI is stored in the profile. The large cache payload is not vendored into RustOSRS.

## 8. Non-goals for M1

M1 does not yet define the complete M2 canonical ID/coordinate/definition model. It only defines enough target identity and invalidation behavior so later decoded data cannot silently lose revision provenance.
