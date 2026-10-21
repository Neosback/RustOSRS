# osrs-reference

Development/test infrastructure for source-oracle adapters, fixtures, differential comparison, and regeneration tooling.

## Allowed workspace dependencies

- `osrs-core`
- `osrs-cache`
- `osrs-scene`
- `osrs-world`

## Normalized fixture execution

`runner` discovers and executes the model, plane and normal semantic fixture families through their
production owners. Exact normal adapters live in `runner/normals.rs`; they contain normalization,
not copies of the normal algorithms. The plane adapter retains the existing scene executor in
`runner/planes.rs`. See [fixture contracts](../../docs/verification/REFERENCE-FIXTURES.md) and the
[pinned normal-method oracle](../../scripts/normal_oracle/README.md).

## Critical rule

This dependency is one-way: production crates must never depend on `osrs-reference`.

Reference Java/source trees and fixture generators remain development tooling and never become production runtime dependencies. See `docs/blueprint/06-CRATE-ARCHITECTURE.md`, `docs/verification/REFERENCE-FIXTURES.md`, and ADR-0001.
