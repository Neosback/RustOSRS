# M1 Cache Compatibility Spike

Status: **M1 working document, slice 1**  
Decision class: `IMPLEMENTATION_EVIDENCE`  
Branch: `impl/m1-target-cache-contract`

This document records the bounded M1 compatibility spike required by `docs/blueprint/17-IMPLEMENTATION-ROADMAP.md`.

It is evidence for the eventual cache dependency ADR. It is **not** that ADR and does not yet select a final dependency strategy.

## 1. M1 constraints

M1 must decide how `osrs-cache` obtains cache bytes and revision-aware decoded artifacts without making an external cache library the semantic authority.

The selected design must support the fields and artifacts needed by M3-M8, including:

- terrain/map archives;
- encrypted location archives;
- object definitions and morph selectors;
- 32-bit/newer model IDs where present in the target;
- floor underlays and overlays;
- model data;
- textures/material inputs;
- varbits/varps;
- sequences/animations;
- target/revision provenance;
- deterministic cache identity and decoded-artifact invalidation.

The candidate implementations under audit are pinned by the RustOSRS imported trees:

- `rs-cache-master/` tree `fae41f98352fc804e5d13d9bd2e836ab1e2635cd`;
- `OpenRune-FileStore-main/` tree `55f571db4b23f2d528786e1cdfbcba0061fd201a`.

OpenRune is independent decoder/tooling evidence. It is not semantic authority.

## 2. Target snapshot gate

The repository contains an OSRS cache under:

`rs-cache-master/data/osrs_cache/`

`rs-cache` documents that bundled integration cache as **OSRS revision 180**.

That cache is useful for transport and regression experiments, but it is not a valid target for the modern RustOSRS editor because the canonical semantic evidence and required fields include behavior introduced long after revision 180.

Therefore:

- revision 180 may be used as a transport fixture;
- revision 180 must not become `TargetProfile` merely because it is already committed;
- M1 cannot claim its final "initial target profile is explicit and reproducible" exit gate until a modern target cache/source snapshot is pinned.

This is currently the primary open gate for completion of M1.

## 3. `rs-cache` capability audit

### 3.1 Low-level cache transport

`rs-cache` provides a read-only cache abstraction backed by `rune-fs` with:

- DAT2/index access;
- archive lookup by numeric ID;
- archive lookup by name hash;
- archive decompression through `Buffer::decode()`;
- XTEA keys passed to encoded location archive buffers;
- cache checksum generation;
- typed index/archive-not-found transport errors.

This portion remains a plausible candidate for reuse behind `osrs-cache` and requires a focused transport spike in the next M1 slice.

Important architectural condition: external transport types must remain private to `osrs-cache`; they must never leak into `osrs-core` or `osrs-scene`.

### 3.2 Location/map support

Present:

- map archive lookup as `mX_Y`;
- location archive lookup as `lX_Y`;
- caller-supplied `[u32; 4]` XTEA keys;
- smart-delta location IDs;
- location type and orientation;
- four terrain planes;
- tile height byte;
- overlay ID/path/rotation;
- tile settings;
- underlay ID.

Concerns:

1. `LocationDefinition.pos` is constructed from `region_x + local_x` and `region_y + local_y`, while the same type's `region_base_coords()` uses `region << 6`. The `pos` values therefore must not be accepted as canonical world-tile coordinates without correction/differential verification.
2. Map representation is an old-revision convenience structure rather than a raw, revision-tagged decode contract.
3. No target revision/provenance is attached to the decoded result.

Assessment: **transport useful; decoders require ownership by RustOSRS or substantial replacement.**

### 3.3 Object definitions

Present in `rs-cache`:

- model IDs and optional model types;
- size X/Y;
- interact/projectile flags;
- contour flag/value;
- `nonFlatShading`-like field (`merge_normals` naming);
- animation ID;
- decoration displacement;
- ambient/contrast;
- recolor/retexture pairs;
- mirror/rotated flag;
- model resize and offset fields;
- clip/blocking fields;
- opcode 77/92 transform-related fields;
- parameters.

Blocking compatibility problems:

#### 32-bit model IDs

`ObjectModelData.models` is `Vec<u16>` and only object opcodes `1` and `5` are decoded.

The imported OpenRune OSRS codec independently shows extended object model opcodes:

- opcode `6`: model ID is a 32-bit integer plus model type;
- opcode `7`: model ID is a 32-bit integer with no explicit type array.

Therefore `rs-cache` object decoding cannot preserve newer model IDs unchanged.

#### Morph fallback loss

For opcode `92`, `rs-cache` reads the extra/default transform ID into a temporary `_var` and discards it instead of preserving it as the fallback branch.

That is incompatible with canonical `MORPH-001`, which requires the explicit fallback/null transform branch to survive decoding.

#### Revision handling

The object decoder has no revision/profile input and uses an `unreachable!()` default for unknown opcodes.

A newer valid opcode can therefore become a panic instead of a revision-tagged decode error.

Assessment: **not acceptable unchanged as the canonical object decoder.**

### 3.4 Missing decoder families

The audited `rs-cache` OSRS definition tree does not contain canonical decoder modules for:

- model data;
- floor underlay definitions;
- floor overlay definitions;
- texture definitions;
- varbits;
- varps;
- sequences/animations.

This is a structural gap, not merely a field-name mismatch.

### 3.5 Error/provenance quality

Positive:

- low-level transport can distinguish missing indices/archives and I/O failures.

Insufficient for RustOSRS:

- parse errors can collapse to `unknown parser error`;
- object unknown opcodes use `unreachable!()`;
- decoded definitions do not carry target/cache identity;
- decoder errors do not include the full required context tuple such as cache fingerprint, index/archive/file, definition ID, decoder schema, revision gate, and byte offset.

Assessment: **transport errors may be wrapped; decoder errors must be RustOSRS-owned.**

## 4. OpenRune FileStore capability audit

OpenRune provides substantially broader independent decoder coverage than `rs-cache`.

Observed OSRS/reference codecs include:

- `ObjectCodec`;
- `OverlayCodec`;
- `UnderlayCodec`;
- `SequenceCodec`;
- `TextureCodec`;
- `VarBitCodec`;
- var/varp-related codecs/types;
- `ModelCodec` and a large model representation.

### 4.1 Revision awareness

OpenRune explicitly passes revision values into several codecs and gates format changes.

Examples observed in this slice:

- object sound fields change at revision `220`;
- sequence opcode ownership changes at revision `226`;
- texture format changes after revision `232`;
- object model opcodes `6/7` preserve 32-bit IDs.

This is useful evidence that RustOSRS must keep revision/profile behavior explicit instead of treating one decoder layout as timeless.

### 4.2 Object/morph coverage

OpenRune preserves:

- object model/type lists;
- older and extended 32-bit model-ID forms;
- `nonFlatShading`;
- clipping/model clipping;
- recolors/retextures;
- transforms/morph selectors;
- revision-gated fields.

Its shared transform helper preserves:

- varbit selector;
- varp selector;
- transform list;
- explicit default/fallback transform for the extended opcode.

That makes it materially stronger reference evidence than the current `rs-cache` object decoder.

### 4.3 Model coverage

OpenRune's `ModelCodec` recognizes multiple model encodings and preserves inputs including:

- vertex and face counts;
- compressed vertex deltas;
- face types;
- priorities/default priority;
- alpha;
- face/vertex skins;
- materials/textures;
- texture triangles and multiple texture mapping types;
- versioned model footer flags;
- optional particle/billboard/skeletal data.

This does not make OpenRune the semantic oracle, but it demonstrates that the data surface required by RustOSRS cannot be supplied by the current `rs-cache` definition layer alone.

### 4.4 Floor, texture, and animation coverage

Observed:

- underlay RGB decoding;
- overlay primary RGB, texture, hide-underlay, secondary RGB, and water field;
- texture average color, transparency, source file, animation direction/speed with a format gate at revision 232/233;
- sequence frame IDs/delays, interleave data, priorities, equipment overrides, skeletal animation ID/ranges/sounds, and revision-gated opcode movement.

Concerns:

- several encode paths are explicitly `TODO`;
- unknown opcode behavior in some codecs logs rather than necessarily failing hard;
- field naming/semantics must still be checked against pinned deob/RuneLite evidence;
- revision thresholds are implementation evidence, not automatically project truth.

Assessment: **strong reference/porting source, not a dependency or authority decision yet.**

## 5. Compatibility matrix, slice 1

| Required M1 capability | `rs-cache` imported snapshot | OpenRune imported snapshot | Current conclusion |
|---|---|---|---|
| DAT2/index transport | Present | Not audited yet in this slice | Continue transport spike |
| archive-by-name | Present | Not audited yet | `rs-cache` candidate transport strength |
| XTEA location boundary | Present, caller supplies keys | Not audited yet | Boundary shape is viable |
| location stream decode | Present | Not audited yet | `rs-cache` decode needs coordinate verification |
| terrain/map tile decode | Present | Not audited yet | old-revision representation only |
| object definitions | Partial | Broad | own/replace decoder layer |
| varbit/varp morph selectors | Partial, fallback loss | Present | `rs-cache` insufficient unchanged |
| 32-bit object model IDs | **Absent** | **Present** | blocker for `rs-cache` object decoder |
| underlay definitions | Absent | Present | RustOSRS decoder required |
| overlay definitions | Absent | Present | RustOSRS decoder required |
| model data | Absent | Broad `ModelCodec` | RustOSRS decoder required |
| texture definitions | Absent | Present, revision-gated | RustOSRS decoder required |
| sequences/animations | Absent | Present, revision-gated | RustOSRS decoder required |
| structured target provenance | Absent | Revision integer in some codecs | RustOSRS-owned |
| decode error context | Insufficient | Mixed | RustOSRS-owned |
| preserved revision distinctions | Weak/fixed old assumptions | Much stronger | must be first-class in RustOSRS |

## 6. `TargetProfile` contract requirements

M1 should define the profile before broad decoder implementation. The profile contract must contain or derive the following information.

### 6.1 Stable profile identity

- stable profile ID/name;
- profile schema version;
- decoder schema version;
- game family (`osrs` for the initial product target).

A human-readable revision number alone is not sufficient identity.

### 6.2 Cache/source identity

The profile must point to an exact cache source identity rather than a local filesystem path.

The eventual cache fingerprint format should be versioned and deterministic. It must identify the logical cache contents from stable cache metadata/reference tables and/or content hashes, and be reproducible on another machine.

The profile may record a display/source URI, but machine-local paths are non-authoritative.

### 6.3 Semantic evidence pins

The profile must reference the exact semantic evidence set used to interpret revision-sensitive fields, including pinned repository/tree/blob identities where applicable.

For the initial 2026 semantic baseline, the existing public deob pin `melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc` is a useful semantic source pin, but it does **not** identify a cache snapshot by itself.

### 6.4 Revision gates

Revision-dependent decoder behavior must be represented explicitly and tested.

Do not infer target behavior from:

- the bundled revision-180 cache;
- `TEXTURE_COUNT=256`;
- old research prose;
- a local absolute path;
- an OpenRune threshold without corroboration.

### 6.5 XTEA ownership

XTEA keys are external decoding inputs for encrypted location archives.

The target profile should identify the **key provider/key-set identity**, but should not require secret/raw XTEA keys to be serialized into a project profile.

Decoded location artifact invalidation must include the region and a non-secret fingerprint of the exact key tuple used, because changing keys can change whether/how the location archive decodes.

### 6.6 Decoded-artifact invalidation identity

A decoded artifact must never be reused merely because its numeric definition/archive ID is the same.

Its cache key must include at least:

- target profile identity/digest;
- cache fingerprint;
- decoder schema/version;
- logical index/archive/file or definition identity;
- revision-gate identity where relevant;
- XTEA-key fingerprint for encrypted locations.

The concrete Rust type/API belongs to the remaining M1/M2 work.

## 7. Preliminary direction, not yet an ADR

The current evidence makes the old "use `rs-cache` as the cache layer and fill a few gaps" recommendation too broad.

The leading direction after slice 1 is:

1. keep `osrs-cache` as the only public cache boundary;
2. consider `rs-cache`/`rune-fs` only for low-level read-only transport if the next transport spike passes;
3. own canonical revision-aware decoders inside RustOSRS;
4. use OpenRune plus pinned deob/RuneLite sources as decoder implementation evidence and differential references;
5. do not expose either external project's definition structs beyond `osrs-cache`.

This is **preliminary**. The final dependency ADR is intentionally deferred until:

- low-level transport compatibility is tested;
- dependency/safety/licensing implications are reviewed;
- a modern target cache snapshot is pinned;
- the full decoder acceptance checklist is built.

## 8. Next M1 slice

The next bounded slice should:

1. audit `rs-cache`/`rune-fs` transport against the bundled rev-180 cache without adopting its decoders;
2. audit OpenRune's FileStore transport/XTEA behavior;
3. define the versioned target-profile manifest shape;
4. define cache fingerprint v1 and XTEA-key fingerprint rules;
5. build the M3-M8 decoder acceptance checklist from canonical specs;
6. identify and pin a modern target cache snapshot before the M1 exit gate can close.
