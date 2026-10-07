# M1 Cache Fingerprint v1

Status: **Accepted M1 implementation contract; verified by transport spike**  
Algorithm ID: `rustosrs-cache-v1`

The cache fingerprint identifies logical cache contents independently of physical `.dat2` sector layout or download container format.

## 1. Design goals

The fingerprint must:

- be reproducible on another machine;
- change when any logical reference table or archive/group payload changes;
- not depend on local paths, file modification times, sector placement, or archive download packaging;
- work for a cache reconstructed from OpenRS2 or copied from a client installation;
- be strong enough for decoded-artifact invalidation.

## 2. Canonical input

Use SHA-256 and the ASCII domain separator:

`rustosrs-cache-v1\0`

Enumerate all present content indices in ascending numeric order, excluding index `255` as a normal content index because it is represented through each content index's reference-table bytes.

A logical master-index slot that explicitly represents no content index is not emitted as an empty content index. This matters for the build-241 target: OpenRS2 reports 25 logical archive slots, but slots `16` and `23` are empty placeholders, so the disk cache contains 23 physical content indices plus `255`.

For each present content index:

1. append index ID as unsigned 16-bit big-endian;
2. obtain the exact encoded reference-table archive for that index from index `255`;
3. append its byte length as unsigned 64-bit big-endian;
4. append `SHA-256(encoded_reference_table_bytes)`;
5. enumerate all present archive/group IDs in that index in ascending numeric order;
6. for every group append:
   - group ID as unsigned 32-bit big-endian;
   - exact encoded logical group byte length as unsigned 64-bit big-endian;
   - `SHA-256(encoded_group_bytes)`.

The final cache fingerprint is SHA-256 over that canonical stream, rendered as lowercase hexadecimal.

## 3. Why encoded logical bytes

Encoded logical group bytes include the cache container data as retrieved through the index/archive abstraction, including compression container/version bytes where present, but exclude physical DAT2 sector headers and sector placement.

Therefore two physically repacked caches with identical logical groups produce the same fingerprint, while content or reference-table changes produce a different fingerprint.

No semantic decoding is required to calculate the fingerprint.

## 4. Completeness

Fingerprinting must fail closed if a present content index/reference table/group declared by the cache metadata cannot be read.

A partial cache must not accidentally receive the fingerprint of a complete cache.

An explicitly empty logical master-index slot is different from a missing group in a declared content index. The former contributes no content-index record; the latter is a completeness failure.

The importer may also report a separate completeness summary for diagnostics.

## 5. Verified vectors

The M1 executable transport spike established two regression vectors using `rune-fs 0.2.0` behind the private `osrs-cache` wrapper.

### Bundled revision-180 fixture

`ad37f18dedd911eba2085d06029f2edf5db3c6285e56f7cb38eddd1cdce04636`

### Initial build-241 target

OpenRS2 cache `2727`:

`ae76dad78b4990d1b404e68e77a85ed2c96cf4a56c16f7b017cb97d1e92fdb38`

The build-241 spike enumerated exactly `117584` groups and reproduced this fingerprint after representative map/model/config decompression succeeded.

## 6. Performance

The algorithm intentionally scans the logical cache once. For the initial build-241 profile, the source reports roughly 182 MiB, making a one-time strong digest acceptable.

Implementations may cache the result in a sidecar keyed by cheap filesystem/stat metadata, but the sidecar is only an optimization. When its validity is uncertain, recompute the canonical fingerprint.

## 7. Source snapshot identity

An external source ID such as `openrs2:2727` is recorded separately from the cache fingerprint.

The source ID answers "which published snapshot was requested?". The fingerprint answers "which logical cache bytes are actually open?".

Both belong in provenance.

## 8. XTEA key fingerprint

Encrypted location decode inputs use a separate algorithm ID: `rustosrs-xtea-v1`.

For a region/archive requiring keys, hash:

- ASCII `rustosrs-xtea-v1\0`;
- region ID as unsigned 32-bit big-endian;
- the four XTEA words in order as unsigned 32-bit big-endian.

Persist only the resulting SHA-256 digest in decoded-artifact invalidation/provenance. Do not persist raw keys merely to support caching.

For a target/source where no XTEA key is required, record the explicit sentinel state `none-required`, not four zero keys unless zero keys are actually the transport input.

## 9. Dependency implications

A chosen transport dependency must expose enough information to enumerate indices, reference tables, archive/group IDs, and exact encoded logical bytes. If it hides any of those behind a higher-level definition API, it is insufficient for `rustosrs-cache-v1` without an additional lower-level layer.

The M1 spike proved that `rune-fs 0.2.0` exposes enough of this surface for the two verified vectors. ADR-0010 accepts it privately behind `osrs-cache`; the fingerprint contract remains RustOSRS-owned and does not make the dependency a semantic authority.
