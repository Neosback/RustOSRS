# osrs-filestore

RustOSRS owns and maintains this low-level JS5 storage crate as a normal workspace member.
It is derived from [rune-fs 0.2.0](https://github.com/jimvdl/rune-fs) by jimvdl;
the original MIT notice is retained in `LICENSE`. The former local path was `vendor/rune-fs`.

## Ownership

Inputs: immutable native `.dat2`/`.idxN` sources or shared owned bytes (including WASM).
Outputs: validated sector-chain bytes, index metadata, encoded/decoded containers and XTEA primitives.
The unused upstream `ArchiveFileGroup` splitter was removed; `osrs-cache` owns the bounded,
sparse-ID-aware splitter rather than maintaining two versions.
Only `osrs-cache` consumes this crate in production. Revision-aware semantic decoding,
source fingerprints, read limits, logical-file splitting and verified export belong there.
No editor, scene or GPU dependencies are permitted.

Native reads use safe positional I/O on Unix/Windows, with serialized file access on other native
platforms. They do not copy the complete `.dat2` into RAM. Each read has a bounded 8,320-byte read-ahead window
plus its output allocation. Sources must not be modified during normal use; truncation returns an
I/O error. Browser sources use `Arc<Vec<u8>>`. No unsafe code or blanket lint exemptions are permitted.
This replaces the original native mmap and needs workload measurement before claiming a speedup.

## Verification

```sh
cargo test --locked -p osrs-filestore --all-features
cargo clippy --locked -p osrs-filestore --all-targets --all-features -- -D warnings
cargo check --locked -p osrs-filestore --all-features --target wasm32-unknown-unknown
```

Storage tests cover normal/expanded headers, unpadded last sectors, malformed chains, truncated
index entries, native/owned byte equivalence, concurrent reads and truncation after opening.
Existing `osrs-cache` fixtures retain full logical fingerprint and semantic decoder checks.
Optional `rs3` and `serde` are retained; they do not imply that RustOSRS supports RS3 editing.
