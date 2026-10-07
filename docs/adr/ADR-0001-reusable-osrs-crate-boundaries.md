# ADR-0001: Reusable OSRS Crate Boundaries

Status: **Accepted**  
Date: **2026-10-07**

## Context

The pre-blueprint port notes proposed editor-prefixed crates (`editor_core`, `editor_cache`, `editor_render`, `editor_app`). The project now explicitly wants the lower-level foundation to remain reusable by a possible future Rust OSRS client/reference implementation.

Editor-prefixed ownership would encourage semantic code to absorb editor assumptions and make future reuse harder.

## Decision

Adopt these primary logical crates:

- `osrs-core`
- `osrs-cache`
- `osrs-scene`
- `osrs-render`
- `osrs-reference`
- `osrs-editor`

Dependency direction follows `docs/blueprint/06-CRATE-ARCHITECTURE.md`.

Key rules:

- `osrs-core` is inward-most.
- `osrs-scene` does not depend on the concrete cache implementation.
- `osrs-render` consumes semantic scene state and cannot define OSRS scene semantics.
- `osrs-editor` is the first product composition root.
- `osrs-reference` is development/test infrastructure only and is never a production runtime dependency.

## Consequences

Positive:

- future applications can reuse the semantic/render stack
- cache I/O, scene semantics, GPU policy, and editor UX remain independently testable
- semantic fixtures can build scenes without filesystem/cache access
- renderer replacement does not require changing scene meaning

Costs:

- some provider/repository interfaces are required to avoid cache/scene coupling
- architectural discipline must be enforced in CI/review
- the editor may need orchestration code that would otherwise be hidden inside a monolithic engine

## Alternatives considered

### Keep `editor_*`

Rejected because it makes the first application the conceptual owner of reusable OSRS behavior.

### One monolithic `rustosrs` crate

Rejected because it would blur semantic, cache, renderer, and editor boundaries and make differential testing harder.

### Many fine-grained crates from day one

Rejected for now. Additional splits such as `osrs-types` or `osrs-assets` require demonstrated coupling/build/API value.

## Constraints/invariants affected

- B1 through B5 in `07-ARCHITECTURE-INVARIANTS.md`
- G1 and G2 in `07-ARCHITECTURE-INVARIANTS.md`

## Follow-up

- Checkpoint 3 verifies the semantic contracts these crates will implement.
- Checkpoint 5 defines renderer internals without changing this dependency direction.
- Checkpoint 8 decides implementation staging and when each crate is created.

## Supersedes

The crate-map recommendation in `RUNELITE_RUST_PORT_NOTES.md`.
