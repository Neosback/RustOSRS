# ADR-0007: eframe/egui Editor Shell with Shared wgpu Viewport

Status: **Accepted**  
Date: **2026-10-07**

## Context

RustOSRS needs a native desktop editor shell that can host a high-performance wgpu scene viewport, dockable tooling, inspectors, diagnostics, shortcuts, and recovery UI without leaking UI types into reusable OSRS crates.

The existing research proposed eframe/egui, egui docking, Catppuccin, and an egui-wgpu callback. The architectural direction remains appropriate after removing obsolete wasm-first and editor-owned-core assumptions.

## Decision

RustOSRS uses `eframe` + `egui` as the primary native editor shell.

The application:

- uses Catppuccin as its baseline theme system;
- hosts `osrs-render` through an editor-owned viewport adapter/callback;
- reuses the eframe/egui wgpu device/queue/context where practical rather than creating an unrelated second GPU device;
- keeps `osrs-render` free of egui/eframe public types;
- uses a dockable panel abstraction, with `egui_dock` acceptable as the initial implementation behind stable editor panel IDs;
- renders depth-aware world editing overlays through the renderer and screen-space controls through egui;
- persists user layout/preferences separately from semantic map/project state by default.

## Consequences

### Positive

- one native UI/render event loop;
- straightforward wgpu integration;
- immediate-mode UI fits inspection/tool-heavy workflows;
- renderer can remain reusable outside the editor;
- docking and panels can evolve without changing semantic crates;
- GPU reset/recreation can be handled as presentation state.

### Costs

- viewport callback/device integration must be tested carefully across eframe/wgpu upgrades;
- third-party docking API churn must be isolated behind editor abstractions;
- complex text/table workflows may require custom widgets;
- very large inspectors/asset browsers need virtualization/caching discipline.

## Alternatives considered

### Separate native window/render framework plus egui overlay

Rejected initially because it duplicates window/device integration and increases lifecycle complexity without clear benefit.

### Fully custom UI toolkit

Rejected because the project value is OSRS editing/rendering correctness, not building a widget toolkit.

### Web-first editor

Rejected by ADR-0002. Native desktop is first-class; wasm remains deferred.

## Constraints / invariants affected

- B4: UI never leaks downward.
- D1: semantic scene is renderer-independent.
- F3: diagnostics are product features.
- F4: native desktop is first-class.

## Follow-up work

- choose tested dependency versions during implementation roadmap;
- prove shared wgpu viewport integration with a minimal renderer milestone;
- define layout preference schema;
- implement safe-mode fallback for renderer initialization failures.

## Supersedes / superseded by

None.
