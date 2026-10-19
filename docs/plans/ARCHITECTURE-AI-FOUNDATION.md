# Architecture and AI foundation cleanup

Status: active incremental refactor plan

Branch: `refactor/architecture-ai-foundation`

Baseline main: `ff7a1be7020f6202987beaef2c792b569c8164da`

## Goal

Make RustOSRS exceptionally easy to extend safely by humans and coding agents without changing OSRS, RuneLite-reference, or 117HD behavior merely for architectural cleanliness.

This is not a rewrite. Existing correct implementations are preserved, indexed, constrained, migrated, or simplified in place. The purpose of the branch is to turn the repository's existing architectural intent into machine-enforced ownership, one semantic editing path, deterministic derived-state synchronization, strong parity protection, measurable performance, and a future headless automation surface.

## Architectural north star

The semantic authority model is slightly broader than `MapDocument` alone because RustOSRS already has multiple legitimate authored domains:

```text
UI / scripts / future automation / future MCP
                    |
                    v
            typed feature intent
                    |
                    v
             pure planner(s)
                    |
                    v
        semantic reversible patch
                    |
                    v
               EditRuntime
                    |
                    v
              EditorSession
                    |
                    v
              EditorDocument
             /              \
    WorkspaceDocument      ThemePack
       /        \
MapDocument   ScenarioDocument
```

Authority rules:

- `MapDocument` is the sole editable terrain, height, and object-placement truth.
- `ScenarioDocument` is authored scenario truth.
- `ThemePack` is authored appearance truth.
- `EditorSession` owns semantic chronology, revision policy, dirty-region policy, and exact rollback.
- `EditRuntime` is the application transaction gateway.
- world, scene, picker, preview, diagnostics, renderer, GPU, and worker state are derived or transient.
- deleting any derived projection must never delete authored state.

The desired map-edit flow is:

```text
typed intent
  -> pure planner
  -> MapPatch
  -> EditRuntime
  -> EditorSession
  -> EditCommit / EditReceipt
  -> ChangeImpact + touched semantic coordinates
  -> deterministic projection invalidation
```

Scenario and appearance follow the same gateway principle using their own semantic patch types.

## Architectural principles

### Semantic DRY, not byte-for-byte DRY

There should be one canonical semantic definition of coordinates, IDs, patches, authored documents, object placement, terrain values, history, and mutation gateways.

Derived copies are allowed when they are explicitly projections that:

1. name their semantic authority,
2. can be regenerated from that authority,
3. cannot mutate authority themselves,
4. carry sufficient revision/generation identity to reject stale state, and
5. are invalidated deterministically.

### Preserve intentional OSRS / RuneLite / 117HD differences

Do not merge behavior merely because implementations look similar. Prefer shared primitives plus explicit policy-specific behavior over either whole-pipeline duplication or a single implementation controlled by many unrelated booleans.

OSRS quirks are part of the compatibility contract. Weird does not mean incorrect.

### The edit gateway is sacred

Interactive product edits must converge on semantic planners and the runtime gateway. UI, renderer, world workers, future automation, and future MCP must not gain independent mutation routes.

### Search before create

Before adding an important type, planner, codec, renderer pass, controller, worker, cache, query, material model, coordinate conversion, or command:

```text
python3 scripts/features.py find "<concept>"
rg "<related terms>"
```

Then inspect the owning crate, closest `AGENTS.md`, relevant spec/ADR, callers, and tests.

### One machine source for feature navigation

`scripts/editor_features.json` is the canonical machine-readable feature catalog. Human indexes are generated from it. Do not maintain a competing hand-written feature map.

### Comments and ADRs explain contracts and non-obvious behavior

Document authority, invariants, compatibility quirks, and reasons a tempting simplification is unsafe. Avoid comments that merely restate syntax.

### Optimize measured hot paths only

Keep semantic code ergonomic until profiling proves a need for specialized packed storage, arenas, bitsets, SoA layouts, or other lower-level representations.

### Automation is another client, not another architecture

Future headless APIs and MCP must reuse the same planners, patches, gateway, semantic queries, scene inspection, and renderer capture mechanisms used by the product.

## Working protocol for this branch

1. Keep exactly one active cleanup branch: `refactor/architecture-ai-foundation`.
2. Do not create another implementation branch while this branch remains active.
3. Break work into small checkpoints with one coherent objective.
4. At each checkpoint: inspect live state, make only that checkpoint's changes, run the cheapest sufficient validation, commit, report exact head and evidence, then stop.
5. Do not continue to the next checkpoint until the user explicitly says `continue`.
6. Never overwrite or duplicate unexpected work already present on the branch. Reconcile it first.
7. Do not use remote CI as an iterative compiler.
8. Do not widen architecture baselines merely to make guards green.
9. Architectural cleanup must not silently change OSRS, RuneLite-reference, 117HD, cache, edit-history, or projection behavior.
10. Temporary adapters require a named authority source, allowed callers, and a removal condition.
11. Merge only after the complete roadmap's merge-readiness checkpoint succeeds.
12. After merge, retire the cleanup branch before beginning another long-lived branch.

## Validation ladder

Use the cheapest level that proves the checkpoint.

### L0 - no execution

Documentation-only changes, comments, or metadata with no executable consumer change.

### L1 - static/navigation guards

Python syntax/JSON parsing and architecture/registry guards. No Rust compilation.

### L2 - focused crate validation

The smallest existing crate/module check or targeted tests for the changed code.

### L3 - affected-path integration/parity validation

Required when editing OSRS semantics, codecs, projection behavior, history/replay behavior, renderer behavior, worker acceptance, or other cross-crate contracts.

### L4 - full local workspace gate

Reserve `python3 scripts/verify.py --full` for integration-ready checkpoints, merge readiness, or a genuinely broad dependency blast radius.

### L5 - remote CI / merge evidence

Use only when the branch is ready for repository-wide integration review. Remote CI does not replace focused local proof.

## Current checkpoint status

### CP0 - cleanup contract - COMPLETE

- dedicated branch
- incremental-refactor contract
- validation ladder
- no-rewrite policy

### CP1 - discoverability and contributor guidance - COMPLETE

#### CP1A - feature query CLI - COMPLETE

Implemented dependency-free feature discovery:

```text
python3 scripts/features.py list
python3 scripts/features.py find "<concept>"
python3 scripts/features.py show "<feature>"
```

#### CP1B1 - canonical feature registry loader - COMPLETE

CLI, architecture tooling, and tests use one registry interpretation.

#### CP1B2 - versioned feature schema - COMPLETE

`scripts/editor_features.json` is schema version 2 with stable IDs, aliases, ownership, status, mutation classification, controller/planner/gateway locations, and behavioral test references where applicable.

#### CP1B3 - generated human feature index - COMPLETE

`docs/architecture/FEATURE-INDEX.md` is generated from the canonical machine registry and explicitly tells contributors not to edit it by hand.

#### CP1C - scoped `AGENTS.md` guidance - COMPLETE

Repository-wide guidance is kept at the root while subsystem ownership, invariants, extension points, traps, and focused checks live near meaningful crate/script/spec boundaries.

### CP2 - semantic authority and duplicate chronology cleanup - ACTIVE

#### CP2A - authority audit and cheap ownership gates - COMPLETE

The canonical semantic chain and legal lifecycle exceptions are documented. Source-level gates protect checkpoint lifecycle ownership and retired legacy-history patterns.

#### CP2B - separate active stroke preview from completed history routing - COMPLETE

`Stroke` owns transient Paint/Height preview changes directly. It no longer embeds `EditStep` or looks like completed history.

#### CP2C1 - remove legacy runtime history replay fallback - COMPLETE

Undo/redo no longer uses duplicate `EditStep` tile/object before/after payloads as semantic truth.

#### CP2C2 - remove retired legacy payload scaffold - COMPLETE

`EditStep` no longer contains `changes`, `removed`, or `added`, including the temporary test-only scaffold. Positive semantic routing tests replace the retired payload contract.

#### CP2D / Replay R1 - semantic map replay footprint - COMPLETE

`osrs-edit` derives immutable `MapReplayFootprint` data from the exact `MapPatch`, including terrain channels, height vertices, and object mutation polarity. This remains renderer/worker independent.

#### CP2E / Replay R2 - editor replay descriptor - COMPLETE

`EditorSession` derives Map / Scenario / Appearance replay descriptors from the exact authoritative history head without exposing mutable history or raw application patches.

#### CP2F / Replay R3A - generic map replay projector foundation - COMPLETE

The editor has a channel-driven generic replay projector with focused tests. `EditRuntime` exposes the semantic replay descriptor and derives diagnostic/touch footprints from it. The old tool-specific router is still the active caller, so migration is not finished.

#### CP2G / Replay R3B - wire semantic descriptor into active replay routing - NEXT

Objective:

- make undo/redo capture and prepare map projection from `EditRuntime::replay_descriptor()` plus the generic projector
- stop passing the compatibility `EditFootprint` argument into replay
- preserve Scenario and Appearance replay behavior
- preserve projection-failure exact rollback
- preserve current success notices/reselection behavior without making labels semantic authority
- compare behavior against the existing Paint, Height, Path, Object, and composite replay coverage

Acceptance:

- no tool-specific projection planner is required to replay a map patch
- semantic descriptor is the source of projection requirements
- projection mutation still occurs only after gateway finalization
- missing transitional data still causes exact semantic rollback
- focused replay/history tests and required architecture guards pass

Stop after commit and report.

#### CP2H / Replay R4A - remove parallel chronological marker stacks

Objective:

- route undo/redo availability and ordering exclusively through `EditorSession` / `EditRuntime`
- delete `TerrainController.history` and `redo_stack`
- delete `EditStep` and `Semantic*History` routing types when no longer referenced
- preserve user-facing undo/redo labels through non-authoritative metadata only if genuinely useful

Acceptance:

- exactly one semantic chronology remains
- `can_undo` / `can_redo` query semantic authority
- no second stack must stay aligned with `PatchHistory`
- focused mixed-domain undo/redo, failed replay, redo-preservation, and composite tests pass

Stop after commit and report.

#### CP2I / Replay R4B - remove obsolete tool-specific replay projection code

After the generic route is proven:

- remove dead replay-only Paint/Height/Path/Object projection helpers
- keep helpers still required for live preview/commit projection
- update tests so deleted adapters cannot silently return

Acceptance: no behavior change; focused editor tests and architecture guards pass.

Stop after commit and report.

#### CP2J / Replay R5 - authority ratchets and architecture dashboard baseline

- remove obsolete history allowances from authority tooling
- add a cheap guard against reintroducing a second application history stack
- guard `EditRuntime.session` privacy and direct semantic write boundaries where source-level enforcement is reliable
- generate a small architecture metrics report from canonical tooling rather than hard-coding aspirational numbers

Initial dashboard candidates:

```text
registered features
unregistered mutating features
broken registry entries
authority violations
explicit architecture debt
EditorApp field/method surface
direct semantic writes outside approved boundaries
```

Metrics must be reproducible and ratchetable. Do not add a metric that cannot be interpreted reliably.

Acceptance: CP2 authority exit conditions are machine-checkable with cheap tooling.

Stop after commit and report.

### CP3 - edit pipeline normalization

By CP3, map-product mutations should converge on:

```text
typed intent
  -> pure planner
  -> MapPatch
  -> EditRuntime
  -> EditorSession
  -> EditCommit / receipt
  -> existing ChangeImpact + touched semantic coordinates
```

Important correction: `ChangeImpact` already exists in `osrs-edit`. Do not create another invalidation enum. Extend or refine the existing semantic vocabulary only where required.

#### CP3A - registry-driven mutating-feature audit

For every registered mutating feature:

- confirm canonical semantic owner
- confirm planner or intentional equivalent
- confirm gateway
- confirm one logical gesture becomes one semantic transaction
- confirm cancellation does not enter history
- confirm failed projection restores authority
- confirm redo invalidation occurs only after finalized replacement history

Produce a concrete exception list instead of assuming migration is needed.

#### CP3B+ - migrate only real exceptions

Each remaining exception becomes its own checkpoint. Do not perform a broad edit-system rewrite if the feature is already compliant.

#### CP3 final - ChangeImpact/touch contract audit

Verify the existing `ChangeImpact` and coordinate footprints contain enough semantic information for downstream projection invalidation. Add missing semantic impact categories only when a real consumer requires them.

Exit condition:

- zero interactive product mutations bypass the intended semantic gateway
- no competing command/history/map authority is introduced
- each mutating feature has discoverable planner/gateway/tests in the feature registry

### CP4 - derived projection ownership, revisioning, and invalidation

#### CP4A - retire semantic-looking worker/terrain mirrors where possible

The major known target is `TerrainController.edited` and related compatibility state. Workers should consume semantic snapshots/projections rather than making a UI-side map-shaped mirror look authoritative.

#### CP4B - explicit revision/epoch identities

Inventory asynchronous derived state and ensure stale results can be rejected using source/document/scene/render identities appropriate to each layer.

#### CP4C - precise invalidation

Translate `ChangeImpact` plus touched semantic coordinates into the narrowest correct scene/world/picker/render work. Correctness comes before minimization.

#### CP4D - immutable render-facing snapshot decision

Prefer immutable render-facing snapshots and generation swaps where they improve ownership/concurrency, but do not create a new snapshot architecture merely to satisfy a diagram. First audit the current world/render handoff and reuse existing immutable/revisioned structures when they already provide the needed boundary.

Exit condition:

```text
semantic authority
  -> revisioned derived scene/world state
  -> revisioned render-facing state
  -> GPU resources
```

with no reverse semantic ownership.

### CP5 - rendering parity harness hardening

Before material renderer cleanup:

- deterministic cache/source identity
- deterministic scene inputs
- fixed camera and viewport
- fixed graphics/profile/settings identity
- semantic scene hashes and counts
- visual captures/goldens with tolerances appropriate to GPU variance
- exact OSRS profile coverage
- 117HD profile coverage where behavior is implemented
- permanent regression fixtures for every discovered compatibility quirk

Do not rely on screenshot pixels alone when semantic evidence can localize a mismatch.

### CP6 - renderer and GPU resource cleanup

Only after CP5 protection exists:

- clarify GPU resource ownership and lifetime
- reduce redundant uploads/allocations where measured
- simplify material/pass/resource APIs
- improve batching/caching only with evidence
- preserve intentional OSRS / RuneLite / 117HD policy differences
- avoid giant universal renderer functions controlled by unrelated flags

Every material renderer change requires affected parity evidence.

### CP7 - performance instrumentation, budgets, then optimization

Measure representative operations such as:

- map-square decode
- scene construction
- model construction/transform work
- roof generation
- terrain gesture commit/undo/redo
- one dirty chunk/region rebuild
- streaming surrounding regions
- GPU upload preparation and bytes
- frame CPU/GPU time
- allocations / peak memory where measurement is reliable
- draw calls / vertices / indices / pipeline changes where exposed

Establish deterministic baselines and regression reporting before optimizing.

Optimize only measured bottlenecks. Semantic code does not become packed low-level storage by default.

### CP8 - deterministic reproduction and diagnostics

Some semantic replay infrastructure now exists for undo/redo, but serialized bug reproduction is a separate goal.

Build toward:

- serializable deterministic edit/replay scripts or bundles
- source/cache identity
- document hash
- scene/render identity where stable
- camera/viewport/profile/settings identity
- edited patch history sufficient to reproduce a bug
- scene/tile/object/rebuild inspection surfaces
- screenshot capture with reproducibility metadata
- internal diagnostics overlay for revisions, selected semantic/render identities, dirty projections, frame/GPU counters, and relevant resource IDs

A diagnostic bundle must contain reproducible evidence, not merely screenshots.

### CP9 - headless automation API

Expose semantic operations for:

- project/source open
- region/map loading
- feature discovery
- semantic queries
- selection
- edit planning/execution
- undo/redo
- scene/tile/object inspection
- camera control
- render settle/capture
- diagnostics
- performance capture
- reference comparison

Automation must call the same semantic planners and gateways as the UI. Prefer semantic actions over pixel-coordinate UI driving.

Place the first implementation near the existing headless/lab/application boundary that actually owns the lifecycle. Do not create a crate merely because "automation" sounds like one.

### CP10 - MCP adapter

Only after CP9 is stable:

```text
AI client
  -> MCP protocol adapter
  -> headless automation API
  -> existing semantic/query/render boundaries
```

MCP owns protocol translation only. No OSRS semantics, renderer policy, mutation rules, or MCP-only product behavior.

Create a dedicated `osrs-mcp` crate only if the protocol/process/dependency boundary makes it genuinely useful.

### CP11 - final architecture review and merge readiness

Before merge:

- update architecture/authority/design docs to final live state
- regenerate canonical generated documentation
- run all cheap architecture/registry ratchets
- run affected parity suites
- run the full local workspace verification gate
- run remote CI if repository-wide integration evidence is required
- reconcile any difference between branch assumptions and current `main`
- verify the branch contains no known architectural allowance that was supposed to be removed
- merge `refactor/architecture-ai-foundation` into `main`
- retire/delete the cleanup branch before starting another long-lived branch

Do not merge merely because individual checkpoints were locally green. CP11 is the explicit integration proof.

## Architecture outcomes to ratchet

Over this branch, move measurably toward:

- zero direct interactive semantic writes outside canonical editing authority
- zero unregistered mutating product features
- zero broken feature-registry references
- zero production dependency on `osrs-reference`
- decreasing `EditorApp` broad surface
- decreasing explicit architecture debt
- no competing canonical coordinate, tile, placement, history, or document types
- no unowned cross-cutting helper modules
- no second application-level chronological history stack
- deterministic semantic replay and reproduction
- explicit revision/epoch identity for asynchronous derived state
- feature/ownership discovery without reading large parts of the repository
- renderer changes protected by semantic and visual parity evidence
- performance claims backed by reproducible measurements

Architecture debt may decrease but must not silently increase.

## Non-goals and traps

- no clean-slate editor rewrite
- no microcrate explosion
- no `utils.rs` / `common` dumping-ground architecture
- no trait abstraction without a real substitution, platform, or testing boundary
- no global `Arc<Mutex<AppState>>`
- no generic ECS migration without a demonstrated runtime need
- no async everywhere
- no GPU objects in semantic domain types
- no renderer-driven semantic state
- no second implementation because the existing one is difficult to understand
- no giant universal OSRS/RuneLite/117HD implementation controlled by many unrelated booleans
- no MCP-only business logic
- no broad performance rewrite without profiling and parity protection

## Repository-navigation target

A new coding agent receiving a task should be able to determine, with tooling rather than repository-wide reading:

```text
Does the feature already exist?
Who owns it?
Where is the planner?
Where is the transaction gateway?
What semantic types already exist?
Which projections are affected?
Which tests prove planning/cancel/commit/undo?
Which parity/spec/ADR evidence applies?
Which local AGENTS.md governs the change?
Which focused verification command is appropriate?
```

That is the practical definition of AI-native architecture for RustOSRS: eliminate ambiguity about authority, ownership, dependencies, behavior, navigation, and validation without creating a second architecture for AI.
