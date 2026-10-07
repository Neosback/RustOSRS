# osrs-scene

Deterministic semantic OSRS scene construction and mutation from canonical data.

## Allowed workspace dependencies

- `osrs-core`

## Dependency law

`osrs-scene` must not depend on `osrs-cache` or concrete cache implementation types. Asset access is provided through future semantic provider contracts or explicit build inputs.

It also must not depend on `osrs-reference`, wgpu, egui, or eframe. See `docs/blueprint/06-CRATE-ARCHITECTURE.md` and ADR-0001.
