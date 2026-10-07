# ADR-0010: Use `rune-fs` Privately for Read-Only Cache Transport

Status: **Accepted**  
Date: **2026-10-07**

## Context

RustOSRS needs a reproducible OSRS cache ingestion path without allowing a third-party definition model to become the project's semantic authority.

The M1 compatibility audit compared the imported `rs-cache` implementation, its underlying `rune-fs 0.2.0` transport dependency, OpenRune FileStore/codec evidence, and the canonical semantic field requirements in `docs/specs/`.

The high-level `rs-cache` OSRS definition layer is not sufficient for the selected target because it is older-revision oriented and does not preserve all required target-era fields or decoder families. Observed gaps include extended/32-bit object model IDs, complete morph fallback preservation, model data, floor definitions, textures, vars, sequences, revision-aware provenance, and safe unknown-opcode handling.

A bounded executable spike then used `rune-fs 0.2.0` directly behind private `osrs-cache` code against:

- the committed revision-180 regression cache;
- OpenRS2 cache `2727`, OSRS live build `241`, captured `2026-09-30 12:30:05`.

The spike proved index/reference-table parsing, group enumeration, encoded group reads, decompression, legacy name-hash lookup where present, caller-owned XTEA, and deterministic full-cache fingerprinting. The build-241 target enumerated exactly `117584` groups and produced the verified `rustosrs-cache-v1` fingerprint:

`ae76dad78b4990d1b404e68e77a85ed2c96cf4a56c16f7b017cb97d1e92fdb38`

The spike also proved that build-241 map index `5` publishes no nonzero archive name hashes. Therefore legacy `mX_Y` / `lX_Y` name lookup cannot be a universal modern transport contract.

## Decision

RustOSRS will use `rune-fs 0.2.0` as a **private, read-only low-level cache transport dependency** behind `osrs-cache`.

The dependency is limited to transport concerns such as:

- DAT2/index/reference-table access;
- logical group enumeration;
- exact encoded group reads;
- compression-container decoding;
- caller-provided XTEA application;
- low-level metadata needed for cache identity/provenance.

RustOSRS owns all revision-aware semantic decoding and canonical output models.

Specifically:

1. no `rune-fs` type may cross the public `osrs-cache -> osrs-core` boundary;
2. high-level `rs-cache` definition structs/decoders are not canonical RustOSRS data contracts;
3. target/revision behavior is selected by validated `TargetProfile` input, not inferred from a bundled cache, local path, fixed texture count, or dependency defaults;
4. `rustosrs-cache-v1` remains the logical cache identity algorithm;
5. XTEA remains caller/provider-owned input at archive decode/decompression, with only key fingerprints retained in artifact provenance;
6. map-square-to-group resolution is target/profile-aware and must not assume archive names exist;
7. production decode APIs must wrap transport failures in RustOSRS provenance-rich errors before M3 exposes them broadly;
8. target-era definition/model/terrain/texture/sequence codecs are implemented and tested in RustOSRS, using OpenRune/deob/RuneLite only as evidence/oracles.

## Consequences

### Positive

- RustOSRS reuses proven low-level cache mechanics without inheriting obsolete semantic structs;
- transport can be replaced later without changing canonical semantic APIs;
- revision-specific decoding remains explicit and testable;
- target identity is reproducible and independent of local filesystem layout;
- legacy XTEA/name-hash behavior remains testable without constraining modern targets;
- cache fingerprints can be computed from exact logical bytes below semantic decoding.

### Costs and accepted risks

`rune-fs 0.2.0` is not treated as infallible infrastructure.

Accepted risks include:

- its documentation describes the API as experimental;
- its memory-mapped file implementation contains dependency-internal `unsafe`;
- malformed/inconsistent cache inputs can reach dependency assert/panic paths;
- some reference-table fields are skipped rather than retained;
- its native error context is not sufficient for RustOSRS production provenance.

Mitigations:

- keep the dependency private behind `osrs-cache`;
- pin through `Cargo.lock` and test under the project toolchain;
- validate target profiles before opening/decoding target content;
- keep exact revision-180 and build-241 cache fingerprint vectors;
- add RustOSRS-owned error/provenance envelopes in M3;
- replace or patch the transport if a required target field cannot be obtained safely;
- never promote dependency behavior to OSRS semantic truth without spec evidence.

## Alternatives considered

### Adopt high-level `rs-cache` as the canonical decoder layer

Rejected. Its older decoder model loses or omits fields required by the build-241 target and canonical specs.

### Fork `rs-cache` and evolve its definition model

Rejected for the initial implementation. This would couple RustOSRS semantic evolution to a legacy high-level API when the useful low-level dependency is already separately available.

### Implement JS5/DAT2/index/compression/XTEA transport from scratch immediately

Rejected for M1. The executable spike proved `rune-fs` can provide the required read-only transport foundation, so a ground-up rewrite would add risk and delay before semantic work begins.

### Use OpenRune FileStore directly

Rejected as a production Rust dependency because it is Kotlin. It remains valuable independent transport/decoder evidence.

### Treat archive-name lookup as the map transport API

Rejected. Build 241 contains zero nonzero map-index archive name hashes, so this would fail on the selected target.

## Constraints / invariants affected

- ADR-0001 crate dependency direction remains unchanged.
- `osrs-core` stays independent of cache/filesystem types.
- `osrs-cache` owns cache transport, decompression, XTEA application, revision decoding, and cache-specific errors.
- semantic behavior remains owned by `docs/specs/`, not by `rune-fs`, `rs-cache`, or OpenRune.
- C-010's dependency-selection gap is closed by this decision.

## Follow-up work

M3 must:

- turn the private transport spike into production `osrs-cache` source/archive/repository APIs;
- add provenance-rich typed error wrapping around transport/decompression/decode failures;
- implement target-aware numeric map-square/group resolution for build 241;
- implement RustOSRS-owned revision-aware object/floor/var/location/map decoders;
- exercise the target gates listed in `docs/implementation/M1-DECODER-ACCEPTANCE.md`;
- retain the exact cache fingerprint checks and target-profile validation contract.

If `rune-fs` later blocks required target support or safety/error guarantees, a new ADR must supersede this decision rather than silently leaking transport details across crate boundaries.

## Supersedes / superseded by

None.
