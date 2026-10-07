# M1 Target Profile Contract

Status: **Accepted M1 implementation contract**  
Decision class: `IMPLEMENTATION_CONTRACT`

`TargetProfile` identifies the exact cache snapshot and revision-sensitive decoding rules used to produce canonical RustOSRS artifacts. It is not a renderer profile, editor preference set, or local cache path.

The first concrete profile is `profiles/osrs-live-241-2026-09-30-openrs2-2727.yaml`.

The v1 parser/validator is implemented in `osrs-cache::profile` so profile/source/revision inputs remain owned by the cache boundary. M2 may introduce canonical downstream provenance value types in `osrs-core`, but cache transport/profile structures do not move there.

## 1. Required identity

A runtime-usable v1 profile must include:

- stable profile ID;
- profile schema version;
- decoder schema version;
- game family;
- exact cache source provider and provider-specific source ID;
- source game/environment/language/build/timestamp metadata;
- deterministic cache fingerprint algorithm and verified value;
- semantic evidence pins;
- explicit revision gates;
- XTEA/key-provider identity without persisting raw secret keys.

A local filesystem path is never authoritative identity.

The committed build-241 profile also records source completeness and transport evidence, including the distinction between 25 logical archive slots and 23 physical content indices because logical slots `16` and `23` are empty.

## 2. Source snapshot versus local cache instance

The profile separates two identities:

1. **source snapshot identity**: where an exact cache can be reproduced, such as OpenRS2 cache 2727;
2. **local logical cache fingerprint**: proof that the bytes currently opened by RustOSRS are the expected logical cache contents.

This permits the same target profile to be reproduced from `.dat2/.idx`, a flat-file export, or another physically different storage layout while rejecting a logically different cache.

The accepted target fingerprint is:

`ae76dad78b4990d1b404e68e77a85ed2c96cf4a56c16f7b017cb97d1e92fdb38`

using `rustosrs-cache-v1`.

## 3. Profile parsing and validation

`TargetProfile::from_yaml_str` performs strict YAML parsing followed by structural validation before a profile can drive cache decoding.

All v1 profile structures use `deny_unknown_fields`. A misspelled, stale, or future field therefore fails closed instead of being silently ignored. A future schema extension that changes the accepted field set requires an explicit schema/parser change.

The v1 validator rejects at least:

- unknown YAML fields;
- unknown profile schema/version;
- empty profile/source identity fields;
- zero decoder schema version;
- non-OSRS game family;
- impossible logical/physical archive counts;
- duplicate or out-of-range empty logical indices;
- incomplete group/XTEA count relationships;
- missing or malformed cache fingerprint;
- raw XTEA key persistence;
- unpinned semantic evidence entries;
- missing required build-241 revision gates;
- policy states that would re-promote the bundled revision-180 cache, a fixed texture count, local paths, or external definition structs to target truth.

The committed build-241 profile itself is parsed by Tier B tests.

## 4. Canonical profile digest

Algorithm ID:

`rustosrs-target-profile-digest-v1`

The digest is SHA-256 over a canonical semantic field stream, not raw YAML bytes.

The stream begins with:

`rustosrs-target-profile-digest-v1\0`

Each string field is appended as:

1. field-name byte length as unsigned 64-bit big-endian;
2. UTF-8 field-name bytes;
3. field-value byte length as unsigned 64-bit big-endian;
4. UTF-8 field-value bytes.

Unsigned numeric values are represented by their canonical base-10 string form and passed through the same field encoding.

Collection rules:

- `empty_logical_indices` are sorted numerically before hashing;
- semantic-source entries are iterated by `BTreeMap` key order;
- revision-gate entries are iterated by `BTreeMap` key order;
- policy entries are iterated by `BTreeMap` key order.

The v1 digest includes:

- profile schema/ID/game family;
- profile and decoder schema versions;
- cache source provider/source ID/URI/game/environment/language/build/timestamp/format;
- logical/physical archive-shape identity and group/XTEA completeness counts;
- verified cache fingerprint algorithm/value;
- XTEA ownership/provider/key-set/fingerprint algorithm;
- semantic source names and exact repository/commit/tree pins;
- revision gate names/states/spec links;
- target policy assertions.

Informational prose such as evidence `status`, gate `note`, transport-evidence notes, source reported size, YAML comments, mapping insertion order, formatting, and line endings do not affect the v1 identity digest.

The accepted build-241 profile digest is:

`cfdefa9ef99eff799fcef4fdf0ec78d9fdcd72d8e5be78e1c154d018ab4575b7`

Tier B asserts that exact vector and separately proves that comment/line-ending-only YAML changes preserve the digest.

## 5. XTEA ownership

XTEA keys are decode inputs owned outside canonical semantic definitions.

Rules:

- raw keys are not serialized into a target profile;
- the profile records provider/key-set identity;
- encrypted decoded artifacts include an `rustosrs-xtea-v1` key fingerprint in invalidation identity;
- changing a region key must invalidate that region's decoded location artifact;
- profiles where the source reports no required XTEA keys still keep the boundary so older/other targets remain representable.

The validator rejects `persist_raw_keys: true`.

## 6. Revision gates

Revision gates are explicit named profile fields or decoder capabilities, not scattered `if build > N` assumptions with no provenance.

For the build-241 initial target, required compatibility areas are:

- 32-bit/extended object model-ID forms when encountered;
- post-220 object sound fields;
- post-226 sequence opcode layout;
- post-232 texture layout;
- `TERRAIN-004` remains blocked until its exact builder provenance is closed.

OpenRune thresholds are implementation evidence. Each threshold used in production decoding must eventually be corroborated by selected target data and/or pinned client/deob evidence.

## 7. Decoded artifact identity

A decoded artifact cache key must include enough information to make cross-target reuse impossible. At minimum:

- target profile digest;
- verified cache fingerprint;
- decoder schema version;
- index/archive/file or definition identity;
- applicable revision-gate identity;
- XTEA key fingerprint for encrypted location content.

A numeric object/model/archive ID alone is never a safe cache key.

## 8. Initial target source pin

The initial profile pins OpenRS2 cache `2727`:

- game: `oldschool`;
- environment: `live`;
- language: `en`;
- build: `241`;
- timestamp: `2026-09-30 12:30:05`;
- format: `VERSIONED`;
- logical archive slots: `25 / 25`;
- empty logical indices: `16`, `23`;
- physical content indices: `23`;
- groups: `117584 / 117584`;
- reported XTEA keys: `0 / 0`.

The source URI is stored in the profile. The large cache payload is not vendored into RustOSRS.

## 9. Dependency relationship

ADR-0010 accepts `rune-fs 0.2.0` only as a private read-only transport dependency behind `osrs-cache`.

The target profile selects RustOSRS revision behavior. A transport dependency cannot infer, redefine, or erase that behavior.

Build 241 also proves that map archive names are not a universal transport invariant: index `5` publishes no nonzero name hashes in this target. Map-square-to-group resolution therefore belongs to target-aware cache decoding/repository logic in M3.

## 10. Non-goals for M1

M1 does not define the complete M2 canonical ID/coordinate/definition model and does not implement M3 definition decoders. It defines enough target identity, validation, cache transport policy, and invalidation behavior so later decoded data cannot silently lose revision provenance.
