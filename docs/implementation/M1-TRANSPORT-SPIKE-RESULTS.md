# M1 Executable Transport Spike Results

Status: **Accepted M1 implementation evidence; ADR-0010 adopted the tested direction**  
Decision class: `IMPLEMENTATION_EVIDENCE`  
Candidate tested: `rune-fs 0.2.0` behind private `osrs-cache` code

This file records the executable evidence that informed the M1 cache dependency decision. The architectural decision itself is `docs/adr/ADR-0010-rune-fs-private-cache-transport.md`.

## 1. Spike implementation

The branch wired `rune-fs 0.2.0` and `sha2` into `osrs-cache` and added a private `transport` module. No `runefs` type crosses the crate boundary.

The spike implements/tests:

- DAT2/index opening;
- sorted present content-index enumeration;
- sorted group enumeration from reference metadata;
- Java/Jagex-style name-hash lookup where hashes exist;
- exact encoded logical group reads;
- decompression;
- caller-owned XTEA before decompression;
- `rustosrs-cache-v1` full-cache hashing.

The transport code remains private under ADR-0010. Production M3 APIs will add RustOSRS-owned repository/decode/error boundaries around it.

## 2. Rust/toolchain result

`rune-fs 0.2.0` compiled successfully under the project-pinned Rust `1.99.0` toolchain.

Ordinary Tier A proved:

- architecture guard passes;
- architecture guard tests pass;
- workspace check passes;
- Clippy with `-D warnings` passes after formatting cleanup.

Ordinary Tier B proved the legacy fixture tests below.

## 3. Bundled revision-180 transport fixture

Source:

`rs-cache-master/data/osrs_cache/`

This fixture remains test data only and is not the target profile.

Executable results:

- content indices enumerate successfully;
- map index metadata/groups enumerate successfully;
- `m50_50` name-hash lookup succeeds;
- the Lumbridge map archive decompresses without XTEA;
- `l50_50` lookup succeeds;
- the known Lumbridge location archive decrypts/decompresses using `[3030157619, 2364842415, 3297319647, 1973582566]`;
- full `rustosrs-cache-v1` hashing is deterministic.

Verified fingerprint:

`ad37f18dedd911eba2085d06029f2edf5db3c6285e56f7cb38eddd1cdce04636`

This vector is asserted exactly in the Rust test rather than testing only self-consistency.

## 4. Build-241 target spike

Target source:

- OpenRS2 cache ID `2727`;
- OSRS live English;
- build `241`;
- capture timestamp `2026-09-30 12:30:05`;
- OpenRS2 reports `117584 / 117584` groups.

The target was downloaded from the pinned OpenRS2 disk export in a temporary M1-only GitHub Actions workflow. The large cache is not vendored into RustOSRS, and that temporary workflow was removed before the milestone PR.

### 4.1 Logical versus physical indices

OpenRS2 exposes 25 logical master-index slots. Slots `16` and `23` are explicit empty placeholders.

The disk export therefore contains 23 physical content index files:

`0..15, 17..22, 24`

plus index `255`.

This is valid and is reflected in `rustosrs-cache-v1`: explicitly empty logical slots do not become synthetic empty content-index records.

### 4.2 Reference/group enumeration

`rune-fs` parsed the build-241 reference tables and produced exactly:

`117584`

groups across the 23 present content indices.

This exactly matches the pinned target-source group count.

### 4.3 Modern map archive naming

Index `5` contained:

`0`

archive metadata records with nonzero name hashes.

Therefore the old revision-180 convention of resolving a map square by hashing `mX_Y` / `lX_Y` cannot be treated as a universal modern cache transport API.

This is not a `rune-fs` transport failure. The target reference table itself does not publish the names required for that lookup.

Consequence:

**RustOSRS must make map-square-to-group resolution target/profile-aware and must not require name-hash lookup for build 241.**

Where older profiles publish names, name-hash lookup remains a valid convenience and is covered by the revision-180 regression test.

### 4.4 Numeric modern group reads

The build-241 spike successfully read and decompressed representative groups from:

- map index `5`;
- model index `7`;
- config index `2` (group `6`).

This establishes that the selected private transport can parse build-241 index/reference metadata, follow DAT2 sector chains, and decode target compression containers for representative semantic inputs.

### 4.5 Build-241 fingerprint

Verified `rustosrs-cache-v1` fingerprint:

`ae76dad78b4990d1b404e68e77a85ed2c96cf4a56c16f7b017cb97d1e92fdb38`

The value is recorded in the target profile and asserted by the target-cache test whenever `RUSTOSRS_TARGET_CACHE_DIR` is supplied.

## 5. XTEA conclusion

The legacy executable test and the independent OpenRune audit agree on the boundary:

- XTEA belongs to encoded archive decode/decompression;
- raw keys are caller/provider inputs;
- semantic location decoding consumes decrypted bytes;
- decoded-artifact invalidation uses a non-secret key fingerprint when keys are required.

The build-241 OpenRS2 source reports no XTEA key set for this snapshot, so the modern compatibility test does not fabricate zero keys merely to exercise the code path.

## 6. Accepted dependency direction

ADR-0010 adopts the direction supported by this executable evidence:

1. depend on `rune-fs` directly rather than high-level `rs-cache`;
2. keep it private behind `osrs-cache`;
3. use it only for read-only JS5/DAT2/index/reference/group/compression/XTEA transport;
4. own all target/revision-aware semantic decoders in RustOSRS;
5. wrap transport failures in RustOSRS provenance-rich errors before production decode APIs are exposed;
6. do not make map archive names a transport invariant.

Accepted risks explicitly carried by ADR-0010:

- dependency documentation labels the API experimental;
- dependency-internal memory mapping uses `unsafe`;
- dependency code contains some assert/panic paths for malformed/inconsistent cache state;
- not every reference-table field is retained by `rune-fs` today;
- production provenance/error wrapping remains RustOSRS-owned M3 work.

The target-profile parser/validator is now implemented separately in `osrs-cache::profile`, so that previous spike follow-up is closed.

## 7. Evidence disposition

The temporary workflow that downloaded the 182 MiB target cache was removed before the milestone PR, so ordinary PR CI does not depend on a large external download.

The permanent regression evidence retained in the branch is:

- private transport implementation/tests;
- exact revision-180 fingerprint vector;
- exact build-241 fingerprint in the target profile and optional target-cache test;
- target profile/source pin;
- target-profile parser/validator and canonical profile digest;
- fingerprint contract;
- this result record.

Final dependency selection is no longer deferred: ADR-0010 accepts the private `rune-fs 0.2.0` transport strategy while keeping RustOSRS-owned decoders as the semantic boundary.
