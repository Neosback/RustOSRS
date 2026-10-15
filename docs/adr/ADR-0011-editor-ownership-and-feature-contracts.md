# ADR-0011: Runtime ownership, headless planners and enforceable feature navigation

Status: **Accepted**
Date: **2026-10-15**

## Context

The editor already uses an authoritative document, reversible semantic patches, a transaction gateway and disposable world/render projections. Broad `EditorApp` feature implementations and stale operational documentation still make changes difficult to navigate and review.

## Decision

Retain the existing workspace and transaction/projection architecture. Move Paint, Height and Path semantic planning to `osrs-edit`; keep UI DTO conversion at the application boundary. Domain intent coordinates use canonical `MapTile` values. Serialization derives for persisted Height/brush settings remain on those same canonical data types, preserving settings wire shapes.

Encapsulate interaction state and camera/spline lifecycle behavior. Project/source and native persistence policy belong to `ProjectRuntime`; UI-local state and modal/window transitions to `UiRuntime`; performance/smoke/trace observations to `DiagnosticsRuntime`. Dedicated settings, plugin, palette and world-inspection owners remove unrelated state from the composition root. The existing terrain preview/history-routing adapter is explicitly temporary and does not become semantic authority.

Preserve the FIFO worker protocol, epoch/presentation freshness checks, contextual scene construction, dependency-proof invalidation and conservative fallbacks. Diagnostic source/request/revision metadata is observational and never gates acceptance.

Use a finite optional trace shared only by main-thread owners. Diagnostics cannot mutate the map. Diagnostic observations are excluded from semantic-runtime equality, so failure/rollback tests compare product state rather than an audit trail.

Enforce boundaries through Rust privacy, manifest dependency checks, a checked-in ratchet of existing `EditorApp` methods/fields, and a feature map linking controllers, planners, gateway adapters and actual behavior tests. Require lower ratchet baselines when behavior is extracted. These source checks complement compiler/tests; they cannot prove behavior from filenames or replace code review.

## Consequences

Adding a built-in tool has one established flow and test contract. AI contributors start from `AGENTS.md` and the executable feature map rather than reconstructing ownership from chat history. Existing app orchestration adapters remain explicitly enumerated; the ratchet prevents their unreviewed growth and does not claim every application method has been eliminated.

No event-sourced persistence, ECS migration, plugin ABI, render-thread rewrite, or project-format change is introduced. Optimizations remain subject to profiling and semantic parity evidence.

## Related decisions

Extends ADR-0001 (crate boundaries), ADR-0003 (semantic/render/editor boundaries), ADR-0008 (transactions/snapshots) and ADR-0009 (save/export separation). `ARCHITECTURE.md` is the current repository ownership contract; specifications remain authoritative for OSRS semantics.
