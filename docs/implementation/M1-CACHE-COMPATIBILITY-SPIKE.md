# M1 Cache Compatibility Spike

Status: **M1 working document, slices 1-3 complete**  
Decision class: `IMPLEMENTATION_EVIDENCE`  
Branch: `impl/m1-target-cache-contract`

This document records the bounded M1 compatibility investigation required by `docs/blueprint/17-IMPLEMENTATION-ROADMAP.md`.

It is evidence for the eventual cache dependency ADR. It is **not** that ADR and does not yet select the final dependency strategy.

## 1. M1 constraints

`osrs-cache` must obtain cache bytes and produce revision-aware decoded artifacts without making an external cache library the semantic authority.

The selected design must support the M3-M8 inputs, including:

- terrain/map and location archives;
- XTEA where a profile requires it;
- object definitions and morph selectors;
- extended/32-bit model IDs;
- underlays/overlays;
- model data;
- textures/material inputs;
- varbits/varps;
- sequences/animations;
- exact target/cache provenance;
- deterministic decoded-artifact invalidation.

Imported evidence is pinned by:

- `rs-cache-master/` tree `fae41f98352fc804e5d13d9bd2e836ab1e2635cd`;
- `OpenRune-FileStore-main/` tree `55f571db4b23f2d528786e1cdfbcba0061fd201a`.

OpenRune is independent decoder/tooling evidence, not semantic authority.

## 2. Initial target snapshot

The initial target is pinned as OpenRS2 cache **2727**:

- `oldschool` / `live` / `en`;
- build `241`;
- captured `2026-09-30 12:30:05`;
- `VERSIONED`;
- 25 logical master-index slots;
- slots `16` and `23` explicitly empty;
- 23 physical content indices in the disk export;
- `117584 / 117584` groups;
- source-reported XTEA keys `0 / 0`;
- approximately `182 MiB`.

Canonical profile:

`profiles/osrs-live-241-2026-09-30-openrs2-2727.yaml`

Verified `rustosrs-cache-v1` fingerprint:

`ae76dad78b4990d1b404e68e77a85ed2c96cf4a56c16f7b017cb97d1e92fdb38`

The repository's bundled revision-180 cache remains a legacy transport/regression fixture only.

## 3. `rs-cache` decoder assessment

The high-level `rs-cache` OSRS definition layer is **not acceptable unchanged** as canonical decoding.

Confirmed blockers include:

- object model IDs represented as `u16` with only older opcodes `1/5`;
- OpenRune independently shows target-era object opcodes `6/7` carrying 32-bit model IDs;
- opcode `92` morph fallback is read and discarded;
- unknown object opcodes can reach `unreachable!()`;
- no canonical decoder modules for model data, floor underlays, floor overlays, textures, varbits, varps, or sequences;
- decode failures/provenance are insufficient for the RustOSRS error contract;
- its useful named-archive/raw helpers are not the right public abstraction for a RustOSRS-owned decoder layer.

Conclusion:

**Do not adopt `rs-cache` definition structs or decoders as canonical RustOSRS data.**

## 4. OpenRune assessment

OpenRune has substantially broader independent decoder coverage and remains strong implementation evidence.

Observed coverage includes:

- object definitions, including 32-bit model-ID forms;
- morph varbit/varp/list/fallback preservation;
- underlay and overlay definitions;
- varbits/varps;
- textures;
- sequences;
- multiple model encodings and face/texture/skin metadata.

Observed revision gates include object changes around `220`, sequence opcode movement at `226`, and texture layout changes after `232`.

These thresholds are implementation evidence that must still be corroborated against target samples and pinned semantic sources.

OpenRune's read-only filesystem also independently confirms XTEA belongs at archive decode/decompression, not scene semantics.

Conclusion:

**Use OpenRune as decoder/reference evidence, not as production dependency or semantic authority.**

## 5. Direct `rune-fs 0.2.0` transport assessment

The public `rune-fs` layer is more closely aligned with the RustOSRS boundary than high-level `rs-cache`.

Relevant public pieces include:

- `Dat2`;
- `Indices` / `Index`;
- `ArchiveRef`;
- reference/archive metadata;
- encoded/decoded buffers;
- compression;
- XTEA.

The M1 branch now includes a **private** `osrs-cache` transport spike using these APIs. No `runefs` type crosses the public crate boundary.

### 5.1 Revision-180 executable result

Ordinary Tier B proves:

- content-index and group enumeration;
- legacy `m50_50` named lookup;
- plain map decompression;
- legacy `l50_50` named lookup;
- Lumbridge location XTEA/decompression with the known four-word key;
- deterministic full-cache fingerprinting.

Exact regression fingerprint:

`ad37f18dedd911eba2085d06029f2edf5db3c6285e56f7cb38eddd1cdce04636`

### 5.2 Build-241 executable result

A temporary M1-only workflow downloaded OpenRS2 cache 2727 and ran the same private transport against the actual target.

Verified results:

- 23 physical content indices were parsed, correctly excluding explicit empty logical slots `16` and `23`;
- all `117584` groups were enumerated, exactly matching the pinned source;
- representative groups from map index `5`, model index `7`, and config index `2` decompressed successfully;
- a complete `rustosrs-cache-v1` scan succeeded;
- exact fingerprint matched `ae76dad78b4990d1b404e68e77a85ed2c96cf4a56c16f7b017cb97d1e92fdb38`.

Detailed evidence is in `M1-TRANSPORT-SPIKE-RESULTS.md`.

### 5.3 Modern map naming discovery

Build 241 index `5` publishes **zero nonzero archive name hashes**.

Therefore `mX_Y` / `lX_Y` name-hash resolution, although valid on the revision-180 fixture, is not a universal modern cache contract.

RustOSRS must make map-square-to-group resolution target/profile-aware. The absence of names in build 241 is not a `rune-fs` failure because the target reference table itself does not provide those names.

## 6. Transport candidate risks

Executable compatibility succeeded, but the final ADR must explicitly address these risks:

- `rune-fs` describes itself as experimental;
- its internal mmap implementation uses dependency-owned `unsafe`;
- malformed/inconsistent cache paths include some assert/panic behavior;
- some reference-table fields are parsed/skipped rather than retained;
- RustOSRS still needs its own production provenance/error envelope;
- map-square resolution is not supplied by modern archive names.

The dependency would therefore be accepted, if at all, only as a **private read-only transport implementation detail**.

## 7. Target profile and fingerprint contracts

M1 has established:

- `docs/implementation/M1-TARGET-PROFILE-CONTRACT.md`;
- `docs/implementation/M1-CACHE-FINGERPRINT-V1.md`;
- `docs/implementation/M1-DECODER-ACCEPTANCE.md`;
- `profiles/osrs-live-241-2026-09-30-openrs2-2727.yaml`.

`rustosrs-cache-v1` hashes encoded logical reference-table/group bytes in canonical order while excluding physical DAT2 sector placement.

`rustosrs-xtea-v1` separately fingerprints region plus four key words without persisting raw keys.

## 8. Compatibility matrix after slice 3

| Required capability | `rs-cache` | `rune-fs` direct | OpenRune | Current evidence |
|---|---|---|---|---|
| DAT2/index transport | wrapped | **verified** | present | `rune-fs` candidate |
| build-241 reference parsing | not tested as boundary | **verified** | present | 117584 groups exact |
| encoded logical group read | wrapped | **verified** | present | fingerprint input works |
| decompression | present | **verified** | present | rev180 + build241 |
| legacy named lookup | private helper | metadata supports owned lookup | present | rev180 verified |
| build-241 map name lookup | unavailable from target names | target publishes no map names | target-dependent | use profile-aware resolver |
| XTEA boundary | present | **verified on rev180** | present | caller-owned transport input |
| location decode | partial | transport only | reference available | RustOSRS-owned decoder |
| object decode | **insufficient** | transport only | broad | RustOSRS-owned decoder |
| 32-bit model IDs | **absent** | N/A | present | target decoder requirement |
| underlay/overlay | absent | N/A | present | RustOSRS-owned |
| model decode | absent | N/A | broad | RustOSRS-owned |
| texture decode | absent | N/A | revision-gated | RustOSRS-owned |
| varbit/varp | absent/partial | N/A | present | RustOSRS-owned |
| sequences | absent | N/A | revision-gated | RustOSRS-owned |
| cache fingerprint | no project contract | **verified** | N/A | two exact vectors |
| provenance/error envelope | insufficient | must wrap | mixed | RustOSRS-owned |

## 9. Direction entering the final M1 checkpoint

The evidence now supports, but does not yet formally decide, this architecture:

1. `osrs-cache` remains the only public cache boundary;
2. use `rune-fs 0.2.0` directly as a private read-only JS5/DAT2 transport implementation;
3. do not use high-level `rs-cache` definition structs/decoders;
4. implement target/revision-aware canonical decoders in RustOSRS;
5. use OpenRune plus pinned deob/RuneLite sources as implementation evidence/oracles;
6. use build 241 / OpenRS2 2727 as the initial target source;
7. make modern map-square group resolution profile-aware rather than name-hash-dependent;
8. preserve `TERRAIN-004` as blocked.

## 10. Remaining M1 work

The executable compatibility investigation is complete.

Before M1 itself can close:

1. validate/implement the target-profile manifest parser and profile identity rules needed at the public boundary;
2. write the cache dependency ADR, explicitly accepting/mitigating the `rune-fs` risks above and closing C-010;
3. reconcile the final target-profile/cache README/API direction;
4. run Tier A/B on the exact final M1 head;
5. perform the M1 exit-gate audit;
6. open/review/merge the single M1 PR.

M2 must not begin before those gates close.
