# M1 Exit Audit

Status: **Complete; M1 implementation exit gates satisfied**  
Milestone: **M1 - Target profile and cache contract**  
Date: **2026-10-07**

This audit evaluates M1 against `docs/blueprint/17-IMPLEMENTATION-ROADMAP.md`. It does not advance M2 and does not claim semantic decoder implementation that belongs to M3-M8.

## 1. Roadmap purpose

M1 must select an initial implementation target and decide how cache transport/decoding will be sourced without locking the semantic model to an old dependency.

Result: **satisfied**.

RustOSRS now has:

- a pinned initial OSRS target source;
- a verified logical cache fingerprint;
- an executable target-profile parser/validator;
- a canonical profile identity digest;
- an explicit XTEA ownership/invalidation boundary;
- an accepted cache dependency ADR;
- a field-oriented decoder acceptance checklist for M3-M8;
- executable compatibility evidence on both the committed legacy cache and the selected build-241 target.

## 2. Initial target/profile gate

Roadmap gate: **initial target profile is explicit and reproducible**.

Status: **PASS**.

Profile:

`profiles/osrs-live-241-2026-09-30-openrs2-2727.yaml`

Pinned source:

- provider: OpenRS2;
- cache ID: `2727`;
- game/environment/language: `oldschool/live/en`;
- build: `241`;
- timestamp: `2026-09-30 12:30:05`;
- groups: `117584 / 117584`;
- logical archive slots: `25 / 25`;
- empty logical slots: `16`, `23`;
- physical content indices: `23`;
- large cache payload is not vendored.

Verified `rustosrs-cache-v1` fingerprint:

`ae76dad78b4990d1b404e68e77a85ed2c96cf4a56c16f7b017cb97d1e92fdb38`

The parser in `osrs-cache::profile` validates the committed profile in Tier B.

## 3. Target profile contract

Roadmap required work:

- target cache/source identity;
- cache fingerprint strategy;
- decoder schema version;
- semantic source/reference pins;
- revision gates;
- XTEA/key ownership boundary;
- decoded-artifact invalidation identity.

Status: **PASS**.

Normative contract:

`docs/implementation/M1-TARGET-PROFILE-CONTRACT.md`

Implementation:

`crates/osrs-cache/src/profile.rs`

The profile digest uses `rustosrs-target-profile-digest-v1` and hashes canonical semantic fields rather than YAML bytes. Tests prove comment/line-ending formatting does not change identity.

## 4. Cache fingerprint gate

Status: **PASS**.

Contract:

`docs/implementation/M1-CACHE-FINGERPRINT-V1.md`

Executable vectors:

- bundled revision-180 regression fixture: `ad37f18dedd911eba2085d06029f2edf5db3c6285e56f7cb38eddd1cdce04636`;
- build-241 target: `ae76dad78b4990d1b404e68e77a85ed2c96cf4a56c16f7b017cb97d1e92fdb38`.

The fingerprint is based on logical encoded reference/group bytes and does not depend on DAT2 sector placement, local path, timestamps, or download packaging.

## 5. Compatibility spike gate

Roadmap requires a bounded compatibility spike across existing Rust cache work, OpenRune evidence, and canonical semantic field requirements.

Status: **PASS for the M1 dependency decision**.

Evidence:

- `docs/implementation/M1-CACHE-COMPATIBILITY-SPIKE.md`;
- `docs/implementation/M1-TRANSPORT-SPIKE-RESULTS.md`;
- `docs/implementation/M1-DECODER-ACCEPTANCE.md`.

Key conclusions:

1. high-level `rs-cache` definition structs are not accepted as canonical decoding;
2. `rune-fs 0.2.0` is sufficient as the tested private low-level transport dependency;
3. OpenRune supplies broad independent decoder/revision evidence but is not semantic authority;
4. RustOSRS must own target/revision-aware semantic decoders;
5. build 241 publishes zero nonzero map-index archive name hashes, so `mX_Y` / `lX_Y` lookup is not a universal modern API;
6. target-revision gates for extended object model IDs, post-220 sound layout, post-226 sequence layout, and post-232 texture layout remain explicit decoder requirements;
7. `TERRAIN-004` remains blocked and was not guessed closed.

The spike intentionally established **availability/absence and preservation requirements** for M3-M8 rather than implementing those later milestone decoders inside M1.

## 6. Cache dependency ADR gate

Roadmap gate: **cache dependency ADR accepted**.

Status: **PASS**.

ADR:

`docs/adr/ADR-0010-rune-fs-private-cache-transport.md`

Accepted strategy:

- depend on `rune-fs 0.2.0` privately inside `osrs-cache` for read-only JS5/DAT2/index/reference/group/compression/XTEA transport;
- do not expose `rune-fs` types across the public cache/core boundary;
- do not adopt high-level `rs-cache` definitions as canonical data;
- implement target-aware decoders in RustOSRS;
- wrap dependency failures in RustOSRS provenance-rich errors before production M3 decode APIs are exposed;
- permit replacement/patching later only through a superseding ADR if target support or safety requirements demand it.

## 7. C-010 gate

Roadmap gate: **C-010 planning/dependency-selection gap closed by an actual decision**.

Status: **PASS by ADR-0010**.

The old question "wrap, fork, partially reuse, or replace `rs-cache`?" is no longer open:

- high-level `rs-cache` decoding is rejected as canonical;
- its underlying `rune-fs` transport is selected privately;
- RustOSRS owns revision-aware decoding.

`docs/blueprint/02-CONTRADICTION-REGISTER.md` now records C-010 as `RESOLVED` and links the accepted ADR and executable evidence.

## 8. Decoder acceptance checklist gate

Roadmap gate: **decoder acceptance checklist exists for every semantic field needed by M3-M8**.

Status: **PASS**.

Checklist:

`docs/implementation/M1-DECODER-ACCEPTANCE.md`

Coverage includes:

- transport/archive/reference metadata;
- terrain/map tile decode;
- location streams;
- object definitions and morph selectors/fallbacks;
- underlays/overlays;
- varbits/varps;
- model data and face/texture metadata;
- texture definitions/material inputs;
- sequences;
- deferred frames/skeleton resources;
- provenance-rich error requirements.

## 9. Forbidden old assumptions gate

Roadmap gate: **no old rev-180, fixed texture count, or local-path assumption is silently treated as target truth**.

Status: **PASS**.

The target profile validator explicitly requires policy assertions that keep false:

- `bundled_revision_180_cache_is_target`;
- `fixed_texture_count_is_semantic_truth`;
- `local_absolute_path_is_identity`;
- `external_definition_structs_cross_osrs_cache_boundary`.

The revision-180 cache remains a regression fixture only.

## 10. XTEA gate

Status: **PASS**.

Executable legacy evidence proves caller-owned XTEA before decompression. The profile contract forbids persisting raw keys and reserves `rustosrs-xtea-v1` for artifact invalidation fingerprints.

OpenRS2 cache 2727 reports no required XTEA key set, so the target profile records `none-required-by-source` rather than inventing four zero keys.

## 11. Architecture boundary gate

Status: **PASS**.

- `osrs-core` remains independent of cache transport/filesystem types;
- `TargetProfile` parsing currently lives in `osrs-cache`, where source/revision ingestion belongs;
- M2 may add renderer/cache-independent canonical provenance values to `osrs-core` without moving `rune-fs` or YAML transport structures there;
- no `rune-fs` type crosses the public crate boundary;
- production crates still do not depend on `osrs-reference`.

## 12. Known risks carried into M3

These are accepted implementation risks, not open M1 dependency-selection questions:

- `rune-fs` describes its API as experimental;
- dependency-internal memory mapping uses `unsafe`;
- malformed cache paths include some dependency assert/panic behavior;
- some reference-table fields are skipped;
- dependency errors lack RustOSRS's required provenance envelope.

ADR-0010 requires M3 to contain these behind `osrs-cache`, add typed provenance-rich errors, and replace/patch the transport if a required target field cannot be obtained safely.

## 13. Non-blocking semantic/research gates

M1 does **not** close unrelated pre-existing research gates.

In particular:

- `TERRAIN-004` / C-021 remains revision-sensitive and blocked on exact builder provenance/oracle;
- C-003/C-005 cross-revision/source-pin limitations remain governed by their existing fixture/spec rules;
- C-006 differential fixture coverage remains future implementation work under M5+.

These do not reopen the M1 cache dependency decision.

## 14. Final pre-PR checklist

Before opening the single M1 PR:

- [x] target source pinned;
- [x] target cache fingerprint verified;
- [x] revision-180 regression fingerprint verified;
- [x] direct `rune-fs` legacy transport/XTEA spike passed;
- [x] direct `rune-fs` build-241 enumeration/decompression/fingerprint spike passed;
- [x] target profile parser/validator implemented;
- [x] profile canonical digest specified and tested;
- [x] decoder acceptance checklist complete for M3-M8 inputs;
- [x] ADR-0010 accepted;
- [x] contradiction/documentation reconciliation complete;
- [x] final code branch-head Tier A green;
- [x] final code branch-head Tier B green;
- [x] `main...impl/m1-target-cache-contract` diff reviewed for accidental scope creep and stale temporary workflow;
- [x] branch is 0 commits behind `main` at exit review.

The implementation exit gate is complete. The remaining operational step is the single M1 PR review/merge. M2 must not begin before that PR is merged.
