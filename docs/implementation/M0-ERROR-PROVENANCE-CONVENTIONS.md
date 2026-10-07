# M0 Error and Provenance Conventions

Status: **M0 implementation convention**

This document establishes cross-crate error and provenance rules without inventing M1/M2 semantic APIs.

## 1. Error ownership

Errors belong to the layer that can describe the failed operation precisely.

- `osrs-core`: pure semantic validation/math failures only when those APIs exist.
- `osrs-cache`: source/archive/decompression/XTEA/decode context.
- `osrs-scene`: scene construction and semantic placement/finalization context.
- `osrs-reference`: fixture/oracle/regeneration/comparison context.
- later renderer/editor crates own renderer/editor failures.

A lower layer must not return errors typed in a higher layer.

## 2. External data is fallible

Cache bytes, fixture files, project files, source-oracle output, and user-selected paths are untrusted inputs.

Production code must not use `panic!`, `unwrap`, or `expect` as ordinary invalid-input handling. Panics are reserved for violated internal invariants where continuing would indicate a programming defect.

## 3. Preserve causes and context

When an error crosses a boundary, retain the original cause when practical and add operation-specific context instead of replacing it with a generic string.

Useful context includes:

- operation/stage;
- source identity;
- target profile/revision when known;
- archive/group/file identifier when known;
- semantic definition/model/region/tile identifier when known;
- fixture ID or oracle identity for reference tooling;
- byte offset/opcode when decoding makes that meaningful.

Do not depend on matching human-readable error strings for control flow.

## 4. Provenance is data, not logging text

Any value whose interpretation depends on source/revision must eventually be able to retain enough structured provenance to answer where it came from.

M0 does **not** define the final `TargetProfile` or semantic provenance structs. M1/M2 own those APIs. Until then:

- do not bake developer-machine paths into public data;
- do not collapse source identity to an informal label such as `latest`;
- do not silently discard revision/source information available at a boundary;
- fixture manifests carry explicit oracle and harness provenance;
- logs may display provenance, but logs are not the authoritative store.

## 5. Stable error categories before polished messages

Prefer distinguishable categories such as:

- I/O/source unavailable;
- malformed input;
- unsupported schema/revision/feature;
- integrity/checksum mismatch;
- missing required key/material/definition;
- invariant violation;
- stale generation/result where applicable later.

User-facing wording can evolve independently from programmatic categories.

## 6. No premature shared error crate

M0 deliberately does not create a universal `RustOsrsError` enum. Cross-layer mega-errors tend to couple otherwise independent crates.

Each crate should expose errors for operations it owns. Composition layers may wrap those errors while preserving the cause chain.

## 7. Dependency policy

M0 uses `std::error::Error` where sufficient. A convenience error crate such as `thiserror` may be introduced later if it materially improves owned APIs, but it is not required by the architecture.

## 8. Verification

The M0 fixture parser is the first concrete example:

- syntax failures remain distinguishable from schema-version failures;
- missing/invalid manifest invariants have typed categories;
- parsing does not panic on malformed fixture input.

Future milestones must add negative tests alongside new decoders/loaders so malformed external input proves the same behavior.
