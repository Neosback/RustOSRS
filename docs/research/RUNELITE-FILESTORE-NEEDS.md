# What RustOSRS's filestore and cache layers need from the RuneLite object study

Reviewed **2026-10-15**, comparing the supplied RuneLite/melxin checkout at
`1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc` with the RustOSRS source snapshot in
[runelite-object-sources.json](runelite-object-sources.json).
The reference build constants declare **235**; our selected cache is **241**.

Companion: [object-system walkthrough](RUNELITE-OBJECT-SYSTEM.md).
This is a findings and implementation-priority document. The proposed fixes below have **not**
been implemented by this documentation task. Existing specs and ADR-0012 retain authority.

## 1. Recommendation and ownership

**Finish the raw reference-table, integrity and resource-limit contract before expanding definition
packing. Keep object, sound and animation semantics above the filestore.** Most missing features
are not missing sector-reading features.

| Owner | Keep / extend here | Keep out of this layer |
| --- | --- | --- |
| `osrs-filestore` | Native/owned byte access, sectors, raw index/reference-table mechanics, bounded container codecs | Object definitions, live varps, audio playback, scene/GPU state |
| `osrs-cache` | Profiles/provenance, logical files, semantic codecs, asset references, integrity policy, pack planning and verified export | Editor history and actor simulation |
| `osrs-core` / `osrs-scene` | Canonical values and pure model/morph/placement semantics | Filesystem transport and GUI dependencies |
| `osrs-world` | Rebuild scheduling, source freshness, derived scene/map/animation projections | Authoritative map mutation |
| Future playtest runtime | Entity state, deterministic time, routes/effects/lifecycles | Editing `MapDocument` or owning source cache bytes |
| Editor / render / audio adapters | UI, projection resources, platform playback | A second cache identity or definition authority |

`osrs-filestore` is already a first-class workspace crate. The previous promotion preserved
native/WASM support, added safe positional reads with bounded read-ahead, and retained upstream
attribution. Do not replace that work with another cache library. Follow
[ADR-0012](../adr/ADR-0012-first-class-filestore.md) and the existing
[OpenRune study](OPENRUNE-CACHE.md).

## 2. What is already available

| Capability | Current implementation / qualification |
| --- | --- |
| Native and owned/WASM reading | `Dat2`, normal/expanded sector headers, bounded group length, per-group read-ahead, chain header validation |
| Compression / XTEA | Container codecs; cache wrapper validates ordinary group container lengths/limits and applies caller-owned keys |
| Logical files | `CacheRepository.read_file/read_group_files`; bounded chunk splitting with sparse file IDs and signed deltas |
| Source identity | Target profiles, canonical logical fingerprint, profile/build provenance, XTEA provenance |
| Metadata for packing | `encode::ReferenceTable` retains protocol/revision/flags, names, CRCs, versions, sizes, hashes/digests and file IDs/names |
| Safe staging workflow | Verified export and source validation; changed-only repacking verifies CRC/size/version and exact payload equality |
| Canonical object semantics | Model variants, placement, morphs, recolors/retextures, contouring, legacy animation data |
| Texture/sprite and map graphics | Revision-233+ texture codec, indexed sprite decoding, existing world-map/map-scene/3D-icon consumers |
| Sound metadata | Object IDs/ranges/fades/visibility retained; inspector available; no audio asset decoding/playback yet |

Sources: [filestore](/Users/tylercovalt/Documents/RustOSRS/crates/osrs-filestore/src/lib.rs:37),
[repository](/Users/tylercovalt/Documents/RustOSRS/crates/osrs-cache/src/transport.rs:356),
[full table representation](/Users/tylercovalt/Documents/RustOSRS/crates/osrs-cache/src/encode/reference_table.rs:56),
[current repack planning](/Users/tylercovalt/Documents/RustOSRS/crates/osrs-cache/src/encode/store/repack.rs:12).

## 3. Filestore and transport findings

Priority meanings: **P1** before broader untrusted-source/definition-packing work; **P2** when the
dependent feature is implemented or a representative measurement justifies it. These are proposed
work priorities, not claims that the pinned cache fails to load.

### F1 — P1: one complete, strict reference-table representation

The production reader's `IndexMetadata` and the packer's `ReferenceTable` are two different
representations of the same wire table. The reader drops the table revision and size columns,
does not retain per-file name hashes, flattens absent hash columns into zero-filled vectors and
discards the parser remainder after file IDs. Its counted parsers use a minimum of zero entries;
zipping partial columns can silently reduce the reported group list.

RuneLite's `IndexData.load` retains index revision, supported flags, sparse file IDs and per-file
name hashes, and rejects protocol versions outside `5..7`. The RuneLite tool supports only its
named/sized flags; do **not** copy that narrower flag set over Rust's existing hash/Whirlpool
support. Use an explicit supported-format policy and preserve every accepted column.

Three tiny inputs were executed against both Rust representations in a scratch audit probe:

| Decoded table input, hexadecimal | `IndexMetadata` reader | `ReferenceTable::parse` | Required decision |
| --- | --- | --- | --- |
| `05000001` — protocol 5, one advertised group, no group data | Accepts, reports 0 groups | Rejects truncated group ID | Both production paths must reject truncation |
| `0800000000000000` — unknown protocol 8, zero groups | Accepts | Accepts | Both need explicit protocol validation |
| `05010001000000000000000000010000000100010000` — named table missing the file-name hash | Accepts, reports 1 group | Rejects missing file-name hash | Preserve/validate file-name hashes |

The second result matters: the existing packer parser is more complete, but it is not already a
fully strict replacement. Audit its unknown flags, counts and allocation bounds as well.

Recommended direction: promote/refactor the **existing** raw table codec into the low-level owner,
then make production reading and packing consume that one representation. Keep a compatibility
adapter for `osrs-cache`'s public encoding API if necessary; remove it when callers use the shared
codec. Profiles/provenance remain in `osrs-cache`. Do not create a third table parser.

Acceptance: protocols 5/6/7, unsupported protocol/flags, every optional column, sparse large IDs,
all truncation offsets, malformed counts, ID overflow, duplicate/nonascending IDs and trailing
bytes. A named file lookup must distinguish a logical file ID from its ordinal or sprite frame.

Sources: [reader parser](/Users/tylercovalt/Documents/RustOSRS/crates/osrs-filestore/src/index.rs:228),
[reader counted helpers](/Users/tylercovalt/Documents/RustOSRS/crates/osrs-filestore/src/index.rs:403),
[packer parser](/Users/tylercovalt/Documents/RustOSRS/crates/osrs-cache/src/encode/reference_table.rs:125),
[RuneLite table format](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/cache/src/main/java/net/runelite/cache/index/IndexData.java:45).

### F2 — P1: limits must apply during opening and metadata decoding

The regular cache group path has encoded/decoded limits. Opening indices follows a different
path: `Indices::new → Dat2::metadata → Buffer::decode`, before those ordinary group limits are
applied. A bounded `.dat2` group does not itself bound its advertised decompressed length or the
number of metadata entries subsequently allocated.

Thread explicit reference-table byte/count limits through this path. Enforce them **before**
allocation or decompression, including owned/browser sources. Catching a dependency panic does
not provide a memory-allocation budget. Consider compressed input, decoded output, group/file
counts and temporary split/decode bytes separately.

Acceptance: oversized advertised output, huge metadata counts with tiny input, arithmetic
overflow, truncated gzip/bzip/LZMA, short reads, source truncation and native/owned equivalence.
Keep existing legal-cache fingerprints and WASM behavior unchanged.

Sources: [index open](/Users/tylercovalt/Documents/RustOSRS/crates/osrs-filestore/src/index.rs:50),
[metadata decode](/Users/tylercovalt/Documents/RustOSRS/crates/osrs-filestore/src/lib.rs:110),
[ordinary group limits](/Users/tylercovalt/Documents/RustOSRS/crates/osrs-cache/src/transport.rs:586),
[container decode allocations](/Users/tylercovalt/Documents/RustOSRS/crates/osrs-filestore/src/codec.rs:299).

### F3 — P1: explicit per-group integrity and version policy

RuneLite's cache-tool `Archive.decompress` compares the compressed-container CRC to reference
metadata and throws on mismatch. Its optional stored version suffix can differ from the table;
that Java tool warns and changes its in-memory revision. That last policy must not be copied
silently into RustOSRS provenance.

Our normal `read_decoded_group` validates the container and size bounds, but does not compare
the encoded group's CRC to its metadata row on that read path. Whole-cache fingerprint matching
at opening and verified-export checks already provide other integrity guarantees; those are not
the same as per-read CRC/version validation if files can change or an import is being inspected.

Define strict import/export validation and an explicit policy for ordinary trusted immutable
reads. Validate CRC over the correct encoded container extent, excluding a version suffix and
before XTEA/decompression. Distinguish source identity mismatch, corrupt payload, unavailable key,
bad codec and unsupported format. Preserve expected/observed versions in a diagnostic; do not
quietly adopt a different version or log raw XTEA keys.

Acceptance: bit-flipped payload with a structurally valid header, stale metadata CRC, absent and
present suffix, suffix mismatch, encrypted groups and source replacement during an outstanding read.

Sources: [RuneLite integrity check](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/cache/src/main/java/net/runelite/cache/fs/Archive.java:78),
[container CRC extent](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/cache/src/main/java/net/runelite/cache/fs/Container.java:93),
[Rust ordinary decode](/Users/tylercovalt/Documents/RustOSRS/crates/osrs-cache/src/transport.rs:586),
[current repack verification](/Users/tylercovalt/Documents/RustOSRS/crates/osrs-cache/src/encode/store/repack.rs:126).

### F4 — P1 before larger packing: append capacity and publication boundaries

Our store checks the 24-bit group length, but writes sector addresses using the low three bytes
of `first/next`. There is no corresponding explicit rejection of a sector address above
`0xFFFFFF`. The on-disk field width, not available disk space, is the limit. Check the complete
prepared batch's first/last sector and all arithmetic before writing. Test the boundary with
calculated/sparse-file fixtures rather than allocating an enormous buffer.

The reader and writer also contain separate sector-chain implementations. Later consolidation
should reuse one low-level storage contract in `osrs-filestore`; semantic pack planning and
verified publication stay in `osrs-cache`. Preserve expanded headers for group IDs above 65535
and final sectors that end immediately after their payload.

Current `repack_groups` / `repack_changed_groups` preflight containers and tables, but append and
index updates are not a crash-atomic filesystem transaction. Continue using disposable staging
copies, one writer, fsync/error propagation and the existing verified-export publication path.
Unknown/new groups, deletions, definition edits and extra digest generation need explicit support;
they are not implied by the ability to replace raw bytes in an existing group.

Sources: [writer](/Users/tylercovalt/Documents/RustOSRS/crates/osrs-cache/src/encode/store.rs:207),
[RuneLite sector mechanics](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/cache/src/main/java/net/runelite/cache/fs/jagex/DataFile.java:69),
[publication validation](/Users/tylercovalt/Documents/RustOSRS/crates/osrs-cache/src/verified_export.rs:75).

### F5 — P2: measured byte budgets, lookup cost and reuse

Useful source ideas: resident index metadata, indexed/binary-search archive lookup, lazy file
reads, bounded decoded caches, and RuneLite's separate raw-model/entity/lit-model caches.
Our production metadata lookup scans a vector; the Java tool's `Index.findArchiveIndex` is a
binary search. Profile large config/model workloads before choosing an ID index.

There is also a specific allocation candidate in `parse_hashes`: an absent column creates
`archive_count * 4` zero **elements**, although consumers need one per archive. This is source
inspection, not a measured retained-memory improvement. Correct it as part of F1 rather than
building a new cache to hide allocations.

Any group/model/sprite/audio cache must have a byte budget and a key including source/profile,
index/group/file and key identity where relevant. Evict only derived data; never unsaved edits.
Deduplicate in-flight work in the existing orchestration layer and reject stale source results.
Reference/HD GPU handles and animation phases do not belong in raw-group cache keys.

The promotion's prior full-cache microbenchmark recorded safe positional read+hash about 8.1%
slower than mmap (~46 ms on the 189.7 MB encoded scan). Keep that result visible and use the
existing performance reports for actual editor improvements; do not claim this study fixed it.

Sources: [lookup](/Users/tylercovalt/Documents/RustOSRS/crates/osrs-cache/src/transport.rs:529),
[allocation candidate](/Users/tylercovalt/Documents/RustOSRS/crates/osrs-filestore/src/index.rs:319),
[Java lookup](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/cache/src/main/java/net/runelite/cache/fs/Index.java:100),
[promotion measurement](../verification/FILESTORE-PROMOTION.md).

## 4. Feature needs above the filestore

| Need | Already present / remaining work | Owning layer |
| --- | --- | --- |
| Object-definition fidelity | Modern 241 model IDs and newer opcodes already decode. Params (`249`), conditional/sub-actions and some flags are consumed rather than retained; add complete raw/semantic preservation before definition editing/repacking. | `osrs-cache`, canonical types as needed |
| Morph-dependent outputs | Pure morph semantics exist. Connect one explicit runtime variable snapshot to model, icon, sound and collision consumers; preserve null selections. World-map `object_icon` currently approximates the first transform and can fall through a null entry to fallback. | Core semantics + world/playtest projections |
| Map elements and map-scene sheets | World-map/3D-icon code already loads these. Audit variant/state refresh and move reusable byte decoders behind cache APIs when refactoring; do not start a second icon implementation. | Cache codecs + world/UI |
| Sound resources | Read the selected logical file, not hardcoded file 0. The Java file-count accessor is slot capacity, not populated-ID count; preserve sparse IDs when selecting variants. Implement both observed variants and their setup/header, synthesis/sample decoding and loop metadata. Do not infer a universal WAV/OGG payload from the outer container. | `osrs-cache` codecs, platform audio adapter |
| Sound lifecycle | Sound metadata and inspector exist. Add seeded random scheduling, Manhattan footprint attenuation, world-view/plane rules, two stream states, morph refresh and fade/removal. Fade durations use milliseconds; random countdown delta is separate. | Playtest/audio runtime |
| Sequence events | Sequence decoder currently skips the event map and animation-height offset. Retain and dispatch them before claiming complete animated sound/effect support. | Cache codec + runtime |
| Skeletal animation | IDs/ranges/masks and model skin data are retained, but the current loc runtime explicitly substitutes a static base. Decode/pose the supported skeletal formats and verify orientation, alpha and event behavior. | Cache/core animation + world |
| NPC/player/effects | Distinct NPC, item/equipment, kit and spot-animation codecs and model assembly are required for the offline runtime. A `DynamicObject` wrapper for a loc does not implement an actor or projectile. | Cache/core + playtest |
| Textures/materials | Seven-byte modern texture definitions and sprites already decode. Preserve normalization, palette brightness, alpha, animation and IDs through rendering; keep HD overrides/material files separate from official config. | Cache/core/render + `osrs-hd` |
| Definition authoring | Preserve untouched bytes/fields, track asset references and reserved IDs, validate dependent resources, then encode and reopen staged output independently. Existing map-project/history formats need not change to do this. | Asset/pack feature + `osrs-cache` |

Current-code pointers:
[object fields consumed](/Users/tylercovalt/Documents/RustOSRS/crates/osrs-cache/src/decode/object.rs:187),
[canonical object data](/Users/tylercovalt/Documents/RustOSRS/crates/osrs-core/src/definitions.rs:198),
[sequence event handling](/Users/tylercovalt/Documents/RustOSRS/crates/osrs-cache/src/decode/sequence.rs:109),
[skeletal runtime limit](/Users/tylercovalt/Documents/RustOSRS/crates/osrs-world/src/animation.rs:8),
[map-icon approximation](/Users/tylercovalt/Documents/RustOSRS/crates/osrs-world/src/worldmap/mod.rs:1208),
[sound inspection](/Users/tylercovalt/Documents/RustOSRS/crates/osrs-editor/src/sound/mod.rs:122).

One evidence question to resolve before audio playback: our object decoder normalizes `0xFFFF`
sound IDs to absent, while this 235 runtime's opcode-78 branch does not explicitly do so.
Validate the selected 241 source/data rather than treating this difference as either proven
correctness or a confirmed 241 bug. Also correct the existing `SoundExtras` comment describing
300-duration fades as ticks when adopting the audited millisecond playback clock. The canonical
sound-data comment calls delay bounds inclusive, but the audited helper uses an exclusive upper
bound when `min < max`; settle that contract before implementing seeded random playback.

## 5. Recommended checkpoints and evidence

1. **Reference metadata and limits:** address F1/F2 using one codec and bounded opening. Keep the
   existing fingerprint vectors and record differences between old/new metadata views.
2. **Integrity and packing:** address F3/F4; verify independent reopening, unaffected bytes,
   optional version suffix policy and failure before partial semantic packing.
3. **Asset fidelity:** extend only the missing retained object/sequence/element/audio fields;
   use revision-specific positive and negative cases. Inspect existing world-map and animation
   consumers first. If a compatibility adapter is needed, state its removal condition.
4. **Runtime consumers:** implement sound/animation/entity behavior on immutable source snapshots,
   with deterministic clocks/seeds and existing source-freshness rules. Map edits still pass only
   through `EditRuntime`; playtest state never mutates `MapDocument`.
5. **Measured reuse:** benchmark actual repeated model/sprite/audio/group reads, travel and source
   switches. Report CPU, allocations and bounded retained/peak bytes; compare equal visual settings.

Every production checkpoint still needs the repository's architecture, native/WASM, format and
parity gates. New visual/audio behavior needs independent reference evidence. This documentation
task ran only source/link/hash checks and the tiny metadata probe; it did not rerun or certify the
whole Rust suite, new rendering behavior or audio parity.

## 6. Reproducing the metadata probe

Inputs and observed output are pinned in the source manifest. A standalone Rust probe depends on
the local `osrs-filestore` and `osrs-cache` crates; no actual cache files are required:

```rust
use osrs_filestore::{IndexMetadata, codec::{Buffer, Decoded}};
use osrs_cache::encode::ReferenceTable;

let bytes = vec![5, 0, 0, 1]; // one advertised group, no following fields
let reader = IndexMetadata::from_buffer(Buffer::<Decoded>::from(bytes.clone()));
let packer = ReferenceTable::parse(&bytes);
println!("{:?}", reader.map(|table| table.iter().count())); // observed Ok(0)
println!("{packer:?}"); // observed Truncated { what: "group id" }
```

These are **undesired acceptance observations** on the recorded baseline, not tests blessing that
behavior. When fixed, convert them into rejection regression cases and update this document's status.
