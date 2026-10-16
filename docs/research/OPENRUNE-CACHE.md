# OpenRune cache design review

Reviewed 2026-10-15 from the two Desktop directories supplied by the repository owner.
Exact reviewed-file SHA-256 values and source roots: [openrune-cache-sources.json](openrune-cache-sources.json).
These are local snapshots, not a claim about current upstream. No Kotlin source/assets are copied.
FileStore has no root license file in this snapshot; retain it as design evidence rather than
importing source with unclear licensing. Server's license is pinned separately in the manifest.

The Server build uses the `dev.or2` cache family (`or2 = 3.0.4` in `gradle/libs.versions.toml`).
No cache implementation explicitly named `dev2` was found in either supplied tree. The relevant
implementation is FileStore's `filesystem`, `displee`, `filestore`, `definition` and `tools`
modules plus Server's `or-cache` pack pipeline.

## Findings and application

| Source, relative to the manifest root | Useful idea | RustOSRS treatment |
| --- | --- | --- |
| FileStore `filesystem/.../FileCache.kt` | Keep index metadata resident; lazy disk reads; 12-entry access-order caches for raw sectors and split groups | Adopt lazy safe native reads in `osrs-filestore`. Metadata was already loaded once. Retained decoded-group reuse needs a measured byte budget and source/XTEA identity before adoption. |
| FileStore `filesystem/.../ReadOnlyCache.kt` | Resolve sparse file IDs separately from dense file positions; read normal/expanded sector headers; reuse decompression context | Preserve existing bounded Rust group splitting and sparse-ID tests; add native/owned sector equivalence and concurrent-reader tests. Reusable decompression needs allocation profiling. |
| FileStore `filesystem/.../util/secure/VersionTableBuilder.kt` | Master version table is built from index CRCs and revisions | Keep group CRCs/versions and reference-table rows consistent in packing. A network master table is unnecessary for offline editor export. |
| FileStore `tools/.../incremental/IncrementalSession.kt` | Scope persistent build state by cache location; reset on schema, revision or whole-cache fingerprint changes; full-rebuild escape hatch | Keep RustOSRS source fingerprints authoritative. A persistent task database is deferred until there are multiple source-driven pack tasks. No cache-path-only reuse. |
| FileStore `tools/.../incremental/IncrementalBuild.kt` | Verify output presence/CRC; propagate dirty cache-read dependencies; account for overlapping outputs and removed producers | Add `repack_changed_groups`: exact decoded equality after CRC/size/version validation; unchanged groups preserve bytes/versions. Prepare the entire batch before writes. Full task dependency/pruning machinery remains future work. |
| FileStore `tools/.../incremental/RecordingCache.kt`, `PackState.kt` | Record cache reads, emitted targets, constant lookups and auxiliary file hashes per unit; save only successful units | Reuse this model if adding custom asset/config build units. Do not create a second editing authority or silently swallow failed units. No SQLite dependency added now. |
| Server `or-cache/build.gradle.kts`, `pack/PluginPacks.kt` | Pack modules/data compile independently of game scripts; explicit ordered task composition | Preserve headless cache/storage ownership. Future pack tasks must stay independent of UI, rendering and playtest simulation. |
| Server `gamevals/GameValMaxIdManifest.kt` | Reserve official ID ranges per revision; commit a small manifest for custom-ID validation | Useful prerequisite for custom NPC/item/effect authoring; defer ID allocation until those features exist. IDs must remain stable across incremental builds. |
| Server `tools/PackServerConfigOSRS.kt` | Load live definitions/render inputs lazily for types that actually repack | Apply when adding entity codecs and multi-format packing; avoid eager decode-all on each edit/export. |
| Server `ServerCacheManager.kt` | Decode mutable builders once, then expose immutable definition tables | Fits `osrs-cache` canonical models and immutable playtest snapshots. Its server-only extensions cannot be assumed to be official client opcodes. |
| Server `net/CacheJs5GroupProvider.kt` | One immutable contiguous response arena instead of a heap object per group | Appropriate for a server serving all groups repeatedly. Do not preload every response on an 8 GB editor machine. No network service is introduced. |

## Details deliberately not copied

`FileCache`'s `index + (archive << 6)` key only separates indices below 64. A Rust cache must use
a typed `(source, index, group, key identity)` key rather than inheriting an implicit bit limit.
A fixed entry count also does not bound retained bytes when group sizes vary. Its TODO CRC methods
are not verification evidence. The server's eager response arena and full definition-table load
serve different workloads from an interactive editor.

Do not copy OpenRune's broad error swallowing into verified export. CRC equality alone is not
proof of decoded equality. Rust's incremental path checks the actual bytes and refuses inconsistent
metadata; force repacking remains available and still increments versions.

## Implemented contract

`osrs_cache::encode::repack_changed_groups(staging_directory, updates)` returns actual rewritten
group/table counts. A repeated identical batch returns zero and leaves every stored file byte
unchanged. Mixed batches rewrite only changed groups and their affected tables. Duplicate targets,
missing tables/groups, unsupported digest columns and invalid later updates fail before any write.
Extra digest columns are not rewritten until their algorithms are implemented. This is validation
preflight, **not filesystem transactionality**: an I/O failure can interrupt writes. Use a disposable
staging directory and the existing verified-export publication flow, under a single writer.

Tests live in `crates/osrs-cache/src/encode/store/incremental_tests.rs` and
`crates/osrs-filestore/tests/storage.rs`. Existing cache fixture/parity suites remain the regression
authority. No claims of a full OpenRune port, new entity support or rendering parity are made here.

## Follow-up within the authorized performance/playtest program

1. Profile repeated group decoding before selecting a byte-bounded cache or scratch-buffer pool.
2. Extend source-pinned review to NPC, item/equipment, identity-kit, spot-animation and sequence
   codecs, then verify official-cache cases independently before consuming them in playtesting.
3. Introduce recorded pack dependencies only when source-driven asset/config builders need them;
   include revision/schema/source identity, removed-output ownership and a full-rebuild fallback.
4. Add custom-ID reservation before custom entity/config creation, keeping scenario IDs distinct
   from official cache-definition IDs.
