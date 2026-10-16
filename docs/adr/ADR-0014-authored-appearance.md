# ADR-0014: Authored appearance belongs to the editor session

Status: **Accepted** through the owner's 2026-10-16 implementation authorization.

## Decision

Keep reusable, GPU-free theme definitions, validation and before/after patches in `osrs-hd`.
Allow its sole internal dependency on `osrs-core` for canonical coordinates and identifiers.
Do not add dependencies from `osrs-edit` to HD, rendering, cache, UI or platform crates.

At the application layer, `EditorDocument` composes the existing `osrs-edit::WorkspaceDocument`
with the authored `ThemePack`. `EditorPatch` adapts workspace and appearance patches to the existing
generic `PatchHistory` implementation. `EditRuntime` remains the only interactive commit/replay
gateway. This is application composition, not a replacement map document or another history engine.
Appearance updates advance authored revision but leave map revision and dirty regions unchanged.

GPU pipeline candidates and error-scope futures live in rendering. UI drafts remain transient.
Only validated, current candidates activate; asynchronous work is rejected after source or authored
revision changes. Authoritative shader changes are restored after undo and project reopening.

Native projects store an appearance snapshot separately from region diffs. Portable theme packs
carry checksummed assets. Immutable library entries are addressed by content, so saving a new
version never silently updates another project's pinned snapshot. Target cache export is unchanged.

## Consequences

Theme authoring can be undone alongside map and NPC edits without giving renderer state editing
authority. Asset bytes use immutable shared storage to avoid copying texture payloads per undo step.
The application must coordinate semantic history with validated rendering preparation and distinguish
native pack/library support from browser capabilities. Imported selectors/materials whose visual
projection is not implemented are rejected explicitly until those projections land.

This decision does not certify stock HD completion, 530 fidelity, a second UI shell, or a complete
game runtime. The remaining implementation and evidence are tracked in `docs/plans/THEME-STUDIO.md`.
