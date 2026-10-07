# Architecture Decision Records

RustOSRS uses ADRs for project-owned decisions that should not be mistaken for inherited OSRS behavior.

## Status vocabulary

- `Proposed`
- `Accepted`
- `Superseded`
- `Rejected`

## When an ADR is required

Use an ADR when changing or introducing a decision that materially affects:

- crate boundaries or dependency direction
- renderer architecture
- cache dependency strategy
- native/web support policy
- persistent project format
- threading/concurrency model
- public semantic APIs
- diagnostics/observability contracts
- parity-vs-enhanced rendering policy
- implementation technology with broad migration cost

Do not use ADRs to document OSRS semantic behavior. That belongs in `docs/specs/` with evidence and fixtures.

## ADR format

Each ADR contains:

1. Title
2. Status
3. Date
4. Context
5. Decision
6. Consequences
7. Alternatives considered
8. Constraints/invariants affected
9. Follow-up work
10. Supersedes / Superseded by, when applicable

## Change rule

Accepted ADRs are immutable historical records except for typo/link corrections. A changed decision gets a new ADR that supersedes the previous one.

## Naming

`ADR-NNNN-short-title.md`

Numbers are monotonic and never reused.

## Accepted ADRs

| ADR | Decision |
|---|---|
| `ADR-0001-reusable-osrs-crate-boundaries.md` | reusable `osrs-*` crate boundaries and dependency direction |
| `ADR-0002-native-first-editor.md` | native desktop is first-class; wasm is deferred |
| `ADR-0003-semantic-renderer-editor-boundaries.md` | semantic, renderer, and editor ownership boundaries |
| `ADR-0004-reverse-z-raster-conventions.md` | reverse-Z, canonical CCW winding, back-face culling, authored-bias reference strategy |
| `ADR-0005-zone-compiled-hybrid-rendering.md` | 8x8 zone static compilation plus dynamic/ordered rendering path |
| `ADR-0006-reference-and-enhanced-render-profiles.md` | explicit deterministic reference profile and higher-quality enhanced editor profile |
| `ADR-0007-eframe-egui-editor-shell.md` | native eframe/egui shell with shared wgpu viewport and isolated docking abstraction |
| `ADR-0008-command-transaction-history.md` | semantic command transactions, exact undo/redo, and snapshot-based background work |
| `ADR-0009-project-save-export-separation.md` | editor project persistence/autosave are distinct from target map/cache export |
