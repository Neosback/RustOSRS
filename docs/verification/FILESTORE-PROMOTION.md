# Filestore promotion evidence — 2026-10-15

The owned crate replaces the former `vendor/rune-fs` dependency. Semantic decoding, source
fingerprinting and verified export stay in `osrs-cache`. See ADR-0012 and the OpenRune cache review.
The final commit and verification counts are recorded in `DEV_HANDOFF.md`.

## Encoded byte comparison and I/O characterization

On this Apple M2 / 8 GiB Mac, compare the old reader at
`94d3ed16f4185cbc16f8af8e5e1cc32ff0740c39:vendor/rune-fs` with the promoted reader in an optimized
standalone harness. Load metadata, sort every non-reference group by `(index, group)`, read its
encoded bytes, and hash their concatenation. Alternate reader order; exclude round 0 as warm-up.

All eight reads returned **117,584 groups, 189,745,919 bytes**, with the same audit SHA-256:

`da2258d3709754a7123b3a9e03654d21f2244ab6aec6c2e4caa8bf0c9d0ca800`

This concatenation digest is separate from the canonical `rustosrs-cache-v1` fingerprint.
The existing build-241 source-shape test also checks the canonical pinned fingerprint.

| Reader | Warm median read + hash |
| --- | ---: |
| Former mmap | 0.570321 s |
| Safe positional reads with bounded read-ahead | 0.616737 s |

The candidate took about **8.1% longer** in this full-cache microbenchmark (~46 ms).
This is an explicit safety/portability tradeoff, not an accepted editor speedup. A first pilot
using one syscall per sector was slower; it overlapped compilation and is excluded from conclusions.
The retained reader amortizes adjacent sectors with an 8,320-byte per-group window. Owned sources
borrow slices directly; neither path retains a raw-byte cache between groups.

Only three warm samples per reader were measured. No `rustc` processes were active during the final
sample run, but the workspace test runner remained active. CPU time, RSS, peak allocations and GPU
execution were not measured here. These measurements do not replace the planned stationary/travel/
editing application baselines or establish reduced application CPU/GPU/memory use.

Raw machine-readable samples, hardware and candidate source hashes:
[filestore-io-2026-10-15.json](filestore-io-2026-10-15.json).
Exact harness: [filestore-io-harness.rs](filestore-io-harness.rs).
To reproduce, create an isolated Cargo package with `[workspace]`, `sha2 = "0.10"`, path dependencies
on the old `rune-fs` snapshot and current `osrs-filestore`, and use that harness as `src/main.rs`.
Run the release binary with the pinned cache directory. This is an audit harness, not a CI timing gate.

## Incremental packing evidence

`repack_changed_groups` compares decoded payloads after validating stored CRC, sizes and version.
Repeated identical input returns zero rewritten groups/tables and preserves every file byte.
Mixed input rewrites only changed groups; a repeated second call is a no-op. Tests also reject
duplicate targets, inconsistent metadata, unsupported digest columns and invalid later targets
without any earlier writes. The explicit force operation retains version-bumping behavior.

Preflight requires temporary memory for prepared containers. It does not provide filesystem
rollback or concurrent-writer protection. Use a disposable staging cache and the existing verified
export publication flow. Persistent dependency databases, new codecs, networking and entity support
were not implemented by this checkpoint.
