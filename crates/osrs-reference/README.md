# osrs-reference

Development/test infrastructure for source-oracle adapters, fixtures, differential comparison, and regeneration tooling.

## Allowed workspace dependencies

- `osrs-core`
- `osrs-cache`
- `osrs-scene`

## Critical rule

This dependency is one-way: production crates must never depend on `osrs-reference`.

Reference Java/source trees and fixture generators remain development tooling and never become production runtime dependencies. See `docs/blueprint/06-CRATE-ARCHITECTURE.md`, `docs/verification/REFERENCE-FIXTURES.md`, and ADR-0001.
