# M1 Cache Fingerprint v1

Status: **M1 contract draft, slice 2**  
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

Enumerate all present logical indices in ascending numeric order, excluding index `255` as a normal content index because it is represented through each index's reference-table bytes.

For each index:

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

Fingerprinting must fail closed if an index/reference table/group declared present by the cache metadata cannot be read.

A partial cache must not accidentally receive the fingerprint of a complete cache.

The importer may also report a separate completeness summary for diagnostics.

## 5. Performance

The algorithm intentionally scans the logical cache once. For the initial build-241 profile, OpenRS2 reports roughly 182 MiB, making a one-time strong digest acceptable.

Implementations may cache the result in a sidecar keyed by cheap filesystem/stat metadata, but the sidecar is only an optimization. When its validity is uncertain, recompute the canonical fingerprint.

## 6. Source snapshot identity

An external source ID such as `openrs2:2727` is recorded separately from the cache fingerprint.

The source ID answers "which published snapshot was requested?". The fingerprint answers "which logical cache bytes are actually open?".

Both belong in provenance.

## 7. XTEA key fingerprint

Encrypted location decode inputs use a separate algorithm ID: `rustosrs-xtea-v1`.

For a region/archive requiring keys, hash:

- ASCII `rustosrs-xtea-v1\0`;
- region ID as unsigned 32-bit big-endian;
- the four XTEA words in order as unsigned 32-bit big-endian.

Persist only the resulting SHA-256 digest in decoded-artifact invalidation/provenance. Do not persist raw keys merely to support caching.

For a target/source where no XTEA key is required, record the explicit sentinel state `none-required`, not four zero keys unless zero keys are actually the transport input.

## 8. Dependency implications

A chosen transport dependency must expose enough information to enumerate indices, reference tables, archive/group IDs, and exact encoded logical bytes. If it hides any of those behind a higher-level definition API, it is insufficient for `rustosrs-cache-v1` without an additional lower-level layer.
