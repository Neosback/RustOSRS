# M1 Cache Compatibility Spike

Status: **M1 working document, slice 2**  
Decision class: `IMPLEMENTATION_EVIDENCE`  
Branch: `impl/m1-target-cache-contract`

This document records the bounded M1 compatibility spike required by `docs/blueprint/17-IMPLEMENTATION-ROADMAP.md`.

It is evidence for the eventual cache dependency ADR. It is **not** that ADR and does not yet select a final dependency strategy.

## 1. M1 constraints

M1 must decide how `osrs-cache` obtains cache bytes and revision-aware decoded artifacts without making an external cache library the semantic authority.

The selected design must support the fields and artifacts needed by M3-M8, including:

- terrain/map archives;
- location archives and XTEA where a profile requires it;
- object definitions and morph selectors;
- 32-bit/newer model IDs where present;
- floor underlays and overlays;
- model data;
- textures/material inputs;
- varbits/varps;
- sequences/animations;
- target/revision provenance;
- deterministic cache identity and decoded-artifact invalidation.

Imported candidates/evidence are pinned by:

- `rs-cache-master/` tree `fae41f98352fc804e5d13d9bd2e836ab1e2635cd`;
- `OpenRune-FileStore-main/` tree `55f571db4b23f2d528786e1cdfbcba0061fd201a`.

OpenRune is independent decoder/tooling evidence. It is not semantic authority.

## 2. Initial target snapshot is now pinned

The initial target source is OpenRS2 cache **2727**:

- game: `oldschool`;
- environment: `live`;
- language: `en`;
- build: `241`;
- timestamp: `2026-09-30 12:30:05`;
- format: `VERSIONED`;
- archives: `25 / 25`;
- groups: `117584 / 117584`;
- source-reported XTEA keys: `0 / 0`;
- reported size: about `182 MiB`.

The exact source pin is stored in:

`profiles/osrs-live-241-2026-09-30-openrs2-2727.yaml`

The source URI is:

`https://archive.openrs2.org/caches/runescape/2727`

The large cache payload is not vendored into RustOSRS.

The repository's bundled `rs-cache-master/data/osrs_cache/` remains OSRS revision 180 and is only a legacy transport/regression fixture. It must never be inferred as the target from its presence in the repository.

The target-source portion of the M1 reproducibility gate is therefore closed. The remaining cache-identity work is to implement/verify `rustosrs-cache-v1` against downloaded logical cache bytes.

## 3. `rs-cache` decoder capability audit

### 3.1 Useful transport behavior

`rs-cache` demonstrates a viable read-only flow based on `rune-fs`:

- DAT2/index access;
- numeric archive reads;
- name-hash lookup internally;
- decompression through typed encoded/decoded buffers;
- caller-supplied XTEA keys;
- typed missing-index/archive errors.

However, its useful name-hash helper and raw archive helper are crate-private. That weakens the case for adopting the high-level `rs-cache` crate merely as transport.

### 3.2 Location/map support

Present:

- `mX_Y` / `lX_Y` lookup in its own loaders;
- smart-delta location IDs;
- location type/orientation;
- terrain height/overlay/settings/underlay inputs;
- caller-provided XTEA keys.

Problems:

1. `LocationDefinition.pos` constructs `region_x + local_x` / `region_y + local_y` while its own base coordinate helper uses `region << 6`; this must not become canonical coordinates.
2. decoded data has no target/profile provenance;
3. representation is tied to an old cache rather than a revision-aware decode contract.

### 3.3 Object definitions are not acceptable unchanged

The imported decoder has useful historical fields but hard blockers.

#### Extended model IDs

Its object model IDs are `u16` and it handles old object model opcodes `1/5` only.

OpenRune independently shows target-era extended forms:

- opcode `6`: 32-bit model ID + model type;
- opcode `7`: 32-bit model ID list without explicit type list.

RustOSRS must preserve these when present.

#### Morph fallback loss

For opcode `92`, `rs-cache` reads the explicit default transform and discards it into a temporary value. This violates `MORPH-001`.

#### Unknown opcode handling

The decoder defaults to `unreachable!()`. A valid target-era opcode can therefore become a panic instead of a provenance-rich revision error.

### 3.4 Missing decoder families

The audited `rs-cache` OSRS definition tree does not contain canonical decoder modules for:

- model data;
- floor underlay;
- floor overlay;
- textures;
- varbits;
- varps;
- sequences/animations.

### 3.5 Error/provenance quality

Low-level missing archive/index errors are useful. Higher-level parse/provenance is insufficient:

- parse errors can collapse to `unknown parser error`;
- valid new opcodes can panic;
- target/cache identity is not attached;
- decoder schema/revision/byte offset is not retained.

Conclusion: **do not adopt the `rs-cache` OSRS definition layer as canonical decoding.**

## 4. `rune-fs` direct transport audit

`rs-cache 0.9.0` depends on `rune-fs 0.2.0`. The public `rune-fs` API is materially better aligned with the boundary RustOSRS needs than the high-level `rs-cache` API.

Publicly available low-level pieces include:

- `Dat2`;
- `Indices` / `Index`;
- `ArchiveRef`;
- `IndexMetadata` / `ArchiveMetadata`;
- encoded/decoded `Buffer`;
- compression codecs;
- XTEA helpers.

This is enough in principle to build RustOSRS-owned name lookup, logical archive reads, fingerprinting, and decoders without importing third-party definition structs.

### 4.1 Strengths

- MIT license;
- Rust 2024, MSRV below RustOSRS's pinned toolchain;
- read-only cache orientation matches the first editor ingestion requirement;
- metadata exposes archive ID, name hash, CRC, version, entry count, file IDs and optional hashes/whirlpool data;
- encoded buffers can receive caller-owned XTEA keys before decode;
- public XTEA implementation is independent of higher-level `rs-cache` definitions;
- `Dat2` reads logical archive bytes through validated sector chains.

### 4.2 Risks requiring the next code spike

- project documentation calls the API experimental;
- `Dat2` uses `memmap2` and contains dependency-internal `unsafe` for memory mapping;
- some malformed conditions still use panic/assert patterns (for example index extension mismatch / buffer length assertion);
- reference metadata parsing contains TODO/skip behavior for some codec size fields;
- production error context is not RustOSRS's required provenance envelope;
- compatibility with the build-241 OpenRS2 target must be proven rather than assumed from the revision-180 fixture.

Preliminary transport direction: **test `rune-fs` directly, not `rs-cache`, as the candidate dependency behind a RustOSRS-owned transport wrapper.**

## 5. OpenRune decoder audit

OpenRune has substantially broader independent decoder coverage than `rs-cache`.

Observed codecs/types include:

- object;
- underlay/overlay;
- varbit/varp;
- sequence;
- texture;
- model.

### 5.1 Revision-aware evidence

Observed implementation gates include:

- object sound layout change at revision 220;
- sequence opcode ownership change at revision 226;
- texture format change after revision 232;
- extended object model opcodes with 32-bit IDs.

These thresholds are useful evidence, not automatic project truth. Target build 241 samples must exercise/corroborate production gates.

### 5.2 Morph/model coverage

OpenRune preserves varbit, varp, transform list, and explicit fallback transform. Its large model codec handles multiple model encodings and inputs including priorities, alpha, skins, materials and texture mapping structures.

Conclusion: **strong decoder/reference evidence; not a semantic authority or direct Rust dependency.**

## 6. OpenRune FileStore transport audit

OpenRune's read-only filesystem independently demonstrates another valid JS5 transport shape.

Its `ReadOnlyCache`:

- reads archive/reference metadata;
- handles reference-table protocol versions 5 through 7;
- preserves name/whirlpool/size/hash flags;
- reconstructs multi-file archives;
- applies XTEA only to map index (`5`) archive decompression when keys are supplied;
- supports caller-provided archive/region key maps;
- rejects writes in the read-only implementation.

Its XTEA implementation uses the expected 32 rounds and four-word key contract.

This corroborates the architectural boundary chosen for RustOSRS: **XTEA belongs at archive decode/decompression, not inside object/location semantics.**

OpenRune transport is Kotlin and is not proposed as the Rust production dependency. Its null-return/runtime-exception behavior is also not the error model RustOSRS should copy.

## 7. Target profile and fingerprint contracts

Slice 2 adds:

- `docs/implementation/M1-TARGET-PROFILE-CONTRACT.md`;
- `docs/implementation/M1-CACHE-FINGERPRINT-V1.md`;
- `profiles/osrs-live-241-2026-09-30-openrs2-2727.yaml`.

`rustosrs-cache-v1` hashes logical encoded reference-table and group bytes in canonical index/group order, independent of physical DAT2 sector layout.

`rustosrs-xtea-v1` hashes region identity plus the four key words and stores only the digest in artifact invalidation/provenance.

## 8. M3-M8 decoder acceptance checklist

Slice 2 adds:

`docs/implementation/M1-DECODER-ACCEPTANCE.md`

It covers acceptance fields and failure/provenance requirements for:

- transport;
- terrain/map tiles;
- locations;
- object definitions;
- underlays/overlays;
- varbits/varps;
- model data;
- textures;
- sequences;
- frame/skeleton resources;
- decode error provenance.

This closes the M1 planning requirement that every semantic field needed by M3-M8 has an explicit decoder acceptance owner.

## 9. Compatibility matrix, current

| Required capability | `rs-cache` | `rune-fs` direct | OpenRune | Current conclusion |
|---|---|---|---|---|
| DAT2/index transport | Wrapped | **Present** | Present | spike `rune-fs` on target |
| encoded logical group read | Wrapped | **Present** | Present | needed for fingerprint |
| reference metadata | Partial access | **Public** | Present | `rune-fs` candidate strength |
| archive-by-name | crate-private helper | metadata enables RustOSRS lookup | Present | own lookup in `osrs-cache` |
| XTEA boundary | Present | **Present** | Present | caller-owned transport input |
| location decode | Partial/coordinate concern | N/A transport | reference available | RustOSRS-owned decoder |
| object decode | **Insufficient** | N/A | Broad | RustOSRS-owned decoder |
| 32-bit model IDs | **Absent** | N/A | Present | required target gate |
| underlay/overlay | Absent | N/A | Present | RustOSRS-owned |
| model decode | Absent | N/A | Broad | RustOSRS-owned |
| texture decode | Absent | N/A | revision-gated | RustOSRS-owned |
| varbit/varp | Absent/partial morph fields | N/A | Present | RustOSRS-owned |
| sequences | Absent | N/A | revision-gated | RustOSRS-owned |
| Rust production dependency fit | too high-level | **candidate** | Kotlin evidence only | test `rune-fs` directly |
| provenance/error envelope | insufficient | must wrap | insufficient/mixed | RustOSRS-owned |

## 10. Current preliminary direction

The evidence now points more narrowly than the original research recommendation:

1. `osrs-cache` remains the only public cache boundary;
2. do **not** use `rs-cache` definition structs/decoders as canonical data;
3. test `rune-fs 0.2.0` directly as a private low-level transport dependency;
4. implement revision-aware canonical decoders in RustOSRS;
5. use OpenRune and pinned deob/RuneLite sources as implementation evidence/oracles;
6. pin build 241 / OpenRS2 2727 as the first target source;
7. preserve `TERRAIN-004` as blocked despite the newer target selection.

This is still preliminary until the direct `rune-fs` code spike runs against both the bundled rev-180 fixture and the build-241 target.

## 11. Remaining M1 work

Before the dependency ADR can be accepted:

1. add a bounded direct `rune-fs` transport spike inside `osrs-cache` tests/tools;
2. prove numeric read, reference metadata, name-hash lookup, decompression and legacy XTEA against the bundled revision-180 fixture;
3. download/use OpenRS2 2727 in a non-vendored test/spike path and prove target build-241 enumeration/read/fingerprint compatibility;
4. implement/validate target-profile manifest parsing or equivalent contract enforcement;
5. produce the cache dependency ADR and close C-010;
6. run Tier A/B and final M1 exit audit.

M2 must not begin before those gates close.
