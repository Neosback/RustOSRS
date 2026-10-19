# Architecture and AI foundation cleanup

Status: active incremental refactor plan

Branch: `refactor/architecture-ai-foundation`

Baseline main: `ff7a1be7020f6202987beaef2c792b569c8164da`

## Goal

Make RustOSRS easier to extend safely by humans and coding agents without changing OSRS behavior merely for architectural cleanliness. Preserve the existing workspace architecture and strengthen ownership, discoverability, single-source-of-truth rules, deterministic editing, rendering parity, and measurable performance.

This is not a rewrite. Existing correct implementations are migrated, indexed, constrained, or simplified in place.

## Working rules

1. One branch for this cleanup sequence. Use small reviewable checkpoint commits.
2. Do not open a pull request until a checkpoint actually benefits from remote CI review.
3. Do not run the full workspace build/test gate for documentation, registry, or source-navigation-only checkpoints.
4. Prefer Python/static guards and focused crate checks. Escalate validation only when the changed layer requires it.
5. Never widen an architecture baseline merely to make a guard green.
6. Do not replace an implementation before searching the feature registry, callers, specs, ADRs, and owning crate.
7. Architectural cleanup must not silently change OSRS, RuneLite-reference, 117HD, cache, edit-history, or projection semantics.
8. Performance changes require a behavioral/parity oracle before optimization when the behavior is externally visible.
9. Generated/derived state is never semantic authority. `MapDocument` and the edit session remain canonical.
10. Temporary adapters require an authority source, allowed callers, and a stated removal condition.

## Validation ladder

Use the cheapest level that proves the checkpoint.

### L0: no execution

Documentation-only changes, comments, or metadata with no executable consumer change.

### L1: static/navigation guards

Use Python syntax/JSON parsing and the existing architecture/registry guards. No Rust compilation.

### L2: focused crate validation

Use the smallest existing verification command or targeted test for the changed crate/module. Do not run unrelated workspace tests.

### L3: affected-path parity/integration validation

Required when editing OSRS semantics, cache codecs, projection behavior, renderer behavior, edit history, worker acceptance, or other cross-crate contracts.

### L4: full workspace gate

Reserve `python3 scripts/verify.py --full` for a checkpoint intended to be integration-ready, before merge, or when the dependency blast radius genuinely makes focused validation insufficient.

Remote CI is not used as an iterative compiler. Local/focused checks should catch ordinary mistakes before a PR is opened.

## Checkpoints

### CP0 - branch and cleanup contract

- create the dedicated branch
- record scope and validation ladder
- preserve current architecture as the starting authority

Acceptance: documentation only; no runtime behavior change.

### CP1 - repository discoverability

#### CP1A - feature query CLI

- provide a dependency-free command to list, find, and inspect registered editor/workspace features
- reuse `scripts/editor_features.json`; do not create a second feature catalog
- expose canonical controller/planner/gateway/test locations to agents and humans

#### CP1B - feature registry schema and generated documentation

- version the machine-readable registry without duplicating authority
- add stable feature IDs/aliases/ownership only where useful
- generate the human feature map from canonical machine data where practical
- validate referenced files/symbols and registry/document drift statically

#### CP1C - scoped contributor guidance

- keep root `AGENTS.md` short and repository-wide
- add focused `AGENTS.md` files only at meaningful subsystem boundaries
- document scope, non-ownership, invariants, extension points, navigation, and focused checks

Acceptance: source-navigation/static checks only unless an executable guard itself changes enough to justify focused tests.

### CP2 - authority and duplication audit

- inventory map/document mutation paths
- classify each path as canonical authority, projection, preview, compatibility adapter, or violation
- inventory duplicate domain concepts and overlapping implementations
- add ratchets for newly discovered high-risk duplication where they can be checked cheaply

Acceptance: no semantic migration without focused edit-domain tests.

### CP3 - edit pipeline normalization

Converge mutating product features on:

`typed intent -> pure planner -> MapPatch -> EditRuntime -> EditReceipt -> ChangeImpact`

Remove special mutation paths incrementally. Preserve one logical user gesture as one logical semantic transaction.

Acceptance: focused planner, cancellation, commit, undo/redo, failure-rollback, and redo-preservation tests for each migrated feature.

### CP4 - projection ownership and revisioning

- make scene/world/render state explicitly derived
- tighten revision/epoch identities and stale-output rejection
- move toward immutable render-facing snapshots where this improves ownership and concurrency
- avoid broad locks/service-locator access

Acceptance: focused world/projection tests plus affected parity suites.

### CP5 - rendering parity harness hardening

- strengthen deterministic semantic and visual fixtures
- fix camera/cache/profile/settings identity for reproducible comparisons
- retain exact OSRS quirks instead of simplifying them away
- make renderer refactors prove parity rather than assume it

Acceptance: relevant reference/parity suites.

### CP6 - renderer/resource cleanup

- clarify GPU resource ownership and lifetimes
- reduce redundant uploads/allocations only after measurement
- simplify material/pass/resource APIs without collapsing intentional OSRS/117HD policy differences

Acceptance: focused renderer checks, parity fixtures, and performance evidence for claimed optimizations.

### CP7 - performance instrumentation and budgets

Track representative map decode, scene construction, dirty rebuild, editing, upload, frame, allocation, and GPU costs. Optimize measured hot paths rather than broadly packing semantic code.

Acceptance: deterministic benchmark/report tooling and documented baselines.

### CP8 - diagnostics and reproduction

- deterministic edit replay
- source/document/scene/render identity hashes where useful
- diagnostic reproduction bundles
- scene/tile/object/rebuild inspection surfaces
- screenshot capture with reproducibility metadata

Acceptance: headless reproduction tests where possible.

### CP9 - headless automation API

Expose semantic application operations for project/map loading, queries, selection, editing, undo/redo, scene inspection, render settling/capture, diagnostics, and reference comparison. Automation must use existing planners and edit gateways, not a second mutation path.

### CP10 - MCP adapter

Add MCP only after the headless automation boundary is stable. The MCP layer owns protocol adaptation, not OSRS/edit/render semantics. Prefer semantic actions over pixel-coordinate UI driving; screenshot/control helpers may sit on top of the same stable automation surface.

## Architecture outcomes to ratchet

Over this branch, prefer measurable movement toward:

- zero direct semantic writes outside canonical editing authority
- zero unregistered mutating features
- zero production dependency on `osrs-reference`
- decreasing `EditorApp` broad surface
- decreasing explicit architecture debt
- no competing canonical coordinate, tile, placement, history, or document types
- no unowned cross-cutting helper modules
- deterministic replay for mutating feature paths
- explicit revision/epoch identities for asynchronous derived state
- feature and ownership discovery without reading large portions of the repository

## Non-goals

- no clean-slate editor rewrite
- no microcrate explosion
- no trait abstraction without a real substitution/port boundary
- no global `Arc<Mutex<AppState>>` architecture
- no generic ECS migration without a demonstrated scene/runtime need
- no renderer-driven semantic state
- no AI/MCP-only product behavior
- no broad performance rewrite without profiling and parity protection
