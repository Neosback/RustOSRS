# ADR-0012: Own the JS5 filestore as a workspace crate

Status: Accepted (user requested first-class ownership, 2026-10-15).

## Decision

Promote the existing rune-fs 0.2.0 fork from `vendor/rune-fs` to `crates/osrs-filestore`.
It follows workspace MSRV, formatting, lint, test and native/WASM gates. Retain its MIT notice
and upstream attribution. Remove the old package/library alias and blanket Clippy exemption.

`osrs-filestore` owns sectors, indices and container mechanics. It has no upward dependencies.
Only `osrs-cache` imports it in production; profiles, fingerprints, revision-aware semantics,
bounded logical-file splitting and verified exports remain there. This supersedes ADR-0010's
dependency selection, while preserving its semantic and provenance rules.

Use safe lazy positional native file reads rather than inheriting the original unsafe mmap.
WASM retains shared immutable bytes. Do not load a whole native cache into process-owned RAM.
This trades mapped access for system calls; measure it rather than assuming a performance win.

## Incremental packing

Following the ideas audited in `docs/research/OPENRUNE-CACHE.md`, add
`osrs_cache::encode::repack_changed_groups` alongside the explicit force-repack operation.
Compare decoded content only after verifying stored CRC, lengths and version suffix. Identical
groups preserve exact physical bytes and versions. Prepare all changed containers and reference
tables before writing, so a later invalid target cannot leave an earlier group repacked.

Neither operation is an atomic filesystem transaction. They operate on a staging cache under one
writer. Existing verified export remains the publication boundary. Unsupported extra digest
columns fail before writing instead of retaining stale digests. No persistent build-state database
or second authoritative cache is introduced.

## Consequences

We can repair and test storage locally without treating dependency failures as untouchable.
Maintenance includes the inherited API and optional features. Native read latency may differ from
mmap and must be reported. OpenRune source is evidence, not authority for OSRS visual/semantic
parity; new entity codecs still require revision-specific independent verification.
