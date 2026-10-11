# ADR-0001: Reusable OSRS Crate Boundaries

Status: **Accepted**  
Date: **2026-10-07**  
Amended: **2026-10-10** to record the implemented `osrs-world` integration layer

## Context

The pre-blueprint port notes proposed editor-prefixed crates (`editor_core`, `editor_cache`, `editor_render`, `editor_app`). The project now explicitly wants the lower-level foundation to remain reusable by a possible future Rust OSRS client/reference implementation.

Editor-prefixed ownership would encourage semantic code to absorb editor assumptions and make future reuse harder.

As the runnable world pipeline was implemented, region/window loading, definition resolution, static scene extraction, streaming, animation orchestration, and inspection data needed a reusable composition layer above the cache/scene/render primitives but below the editor product shell. That implemented layer is `osrs-world`.

## Decision

Adopt these primary logical crates:

- `osrs-core`
- `osrs-cache`
- `osrs-scene`
- `osrs-render`
- `osrs-world`
- `osrs-reference`
- `osrs-editor`

Dependency direction follows the same inward-only rules established by the blueprint, with `osrs-world` as a runtime integration/orchestration layer:

```text
osrs-core
   ^
   +-- osrs-cache
   +-- osrs-scene
           ^
           +-- osrs-render

osrs-world -> osrs-core + osrs-cache + osrs-scene + osrs-render
osrs-editor -> reusable layers + osrs-world
osrs-reference -> semantic/cache layers + osrs-world as test/integration infrastructure
```

Key rules:

- `osrs-core` is inward-most.
- `osrs-scene` does not depend on the concrete cache implementation.
- `osrs-render` consumes semantic scene state and cannot define OSRS scene semantics.
- `osrs-world` may compose cache, semantic scene, and renderer facilities for region/world runtime orchestration, but those reusable lower layers must not depend back on `osrs-world`.
- `osrs-editor` remains the product composition root and owns UI/application policy.
- `osrs-reference` is development/test infrastructure only and is never a production runtime dependency. It may consume `osrs-world` to exercise complete integration/reference paths.

## Consequences

Positive:

- future applications can reuse both the semantic/render stack and the implemented world-loading/orchestration layer
- cache I/O, scene semantics, GPU policy, world composition, and editor UX remain independently testable
- semantic fixtures can still build scenes without filesystem/cache access
- renderer replacement does not require changing scene meaning
- the editor does not need to own reusable region streaming, animation, or world extraction logic

Costs:

- `osrs-world` is intentionally broader than the lower semantic crates and must not become a dumping ground for editor-only policy
- some provider/repository interfaces are required to avoid cache/scene coupling
- architectural discipline must be enforced in CI/review
- the editor may still need orchestration code for product-specific behavior

## Alternatives considered

### Keep `editor_*`

Rejected because it makes the first application the conceptual owner of reusable OSRS behavior.

### One monolithic `rustosrs` crate

Rejected because it would blur semantic, cache, renderer, world-integration, and editor boundaries and make differential testing harder.

### Put world loading/streaming directly in `osrs-editor`

Rejected after implementation demonstrated reusable world composition needs. Doing so would make region loading, definition resolution, scene extraction, and animation orchestration editor-owned despite being useful to a future client/reference application.

### Many fine-grained crates from day one

Rejected for now. Additional splits such as `osrs-types` or `osrs-assets` require demonstrated coupling/build/API value.

## Constraints/invariants affected

- B1 through B5 in `07-ARCHITECTURE-INVARIANTS.md`
- G1 and G2 in `07-ARCHITECTURE-INVARIANTS.md`

The CI architecture guard is normative for the implemented direct dependency graph and must be updated when an accepted architectural layer is added. It must not be left rejecting the repository's actual accepted crate graph.

## Follow-up

- keep `osrs-world` free of editor/UI dependencies;
- keep semantic ownership in `osrs-core`/`osrs-scene`, not in world orchestration;
- keep renderer policy in `osrs-render`;
- update the blueprint crate diagram during the next documentation reconciliation so historical milestone prose and the implemented crate map cannot be confused.

## Supersedes

The crate-map recommendation in `RUNELITE_RUST_PORT_NOTES.md`.
