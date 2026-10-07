# osrs-cache

Cache/archive transport and revision-aware decoding into canonical `osrs-core` values.

## Allowed workspace dependencies

- `osrs-core`

## Forbidden ownership

This crate does not own loc placement, wall/decor scene dispatch, semantic morph resolution, contouring, rendering, or editor state.

The concrete cache dependency strategy remains deferred to M1. See `docs/blueprint/06-CRATE-ARCHITECTURE.md` and ADR-0001.
