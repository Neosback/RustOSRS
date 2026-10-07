# osrs-core

Canonical renderer-, cache-transport-, scene-, and editor-independent OSRS semantic types and pure algorithms.

## Dependency rule

`osrs-core` is the inward-most production crate. It may use only general-purpose pure Rust support libraries approved for semantic use.

It must not depend on:

- `osrs-cache`
- `osrs-scene`
- `osrs-render`
- `osrs-reference`
- `osrs-editor`
- wgpu/rendering libraries
- egui/eframe UI libraries
- concrete cache transport implementations

M0 intentionally does not add placeholder semantic APIs. See `docs/blueprint/06-CRATE-ARCHITECTURE.md` and ADR-0001.
