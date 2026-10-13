# RustOSRS Editor Architecture Contract v2

Status: Adopted architecture baseline after terrain-path semantic-session integration.

This document is the authoritative repository-level architecture contract for the RustOSRS editor. `DEV_HANDOFF.md` records the current implementation checkpoint, live branch, exact SHAs, validation state, and next boundary. Repository documentation, not chat history or model memory, is the source of truth for implementation decisions.

## 1. Architectural decision

RustOSRS uses a **Transactional CQRS Workspace Architecture with Hexagonal Ports and Reactive Projections**.

The editor has four distinct flows:

```text
COMMAND FLOW
UI/input -> typed intent -> pure planner -> MapPatch -> EditorSession commit

QUERY FLOW
UI/tool/read model -> read-only semantic queries -> MapDocument

PROJECTION FLOW
EditReceipt -> ChangeImpact + touched areas -> scene/world/render invalidation
            -> async rebuild -> GPU

PLATFORM FLOW
editor/cache services -> narrow ports -> native or WebAssembly implementation
```

This is an incremental refinement of the existing workspace, not a replacement architecture built beside it.

Do not create replacement crates for responsibilities already owned by the workspace:

- no `osrs-coords`; `osrs-core` owns canonical typed coordinates and IDs
- no `osrs-edit-core`; `osrs-edit` owns the headless editing domain
- no `osrs-editor-app`; `osrs-editor` is already the composition/application crate

A new crate is justified only when a real dependency boundary cannot be enforced cleanly with modules and compiler-level isolation has a concrete benefit.

## 2. Non-negotiable invariants

These rules outrank convenience.

1. `MapDocument` is the authoritative edited map state.
2. GPU buffers, scene caches, picker data, worker mirrors, UI widgets, preview meshes, and legacy edit stores are projections only.
3. If every renderer, UI, picker, and worker cache vanished, the complete edited map must still exist correctly in the semantic edit session.
4. One logical user gesture produces at most one logical semantic transaction.
5. Empty semantic changes produce no history entry and do not destroy redo.
6. Undo/redo changes semantic authority first; projections follow the resulting state.
7. Failed projection preparation must not leave semantic history and visible state knowingly diverged.
8. Domain code never imports `wgpu`, `egui`, `eframe`, filesystem UI, browser APIs, or platform-specific runtime code.
9. Renderer code never becomes a source of semantic truth.
10. Cache decoding/encoding never depends on the editor UI.
11. Raw coordinate tuples are not allowed across domain boundaries where a typed coordinate exists.
12. There is one canonical owner for every concept. No competing `Tile`, coordinate, object-placement, history, or document type.
13. Do not create a second implementation to avoid understanding the first implementation.
14. Compatibility adapters must be visibly temporary and cannot become a second authority.

## 3. Workspace crate architecture

Keep the current workspace crates and sharpen their responsibilities.

```text
crates/
├── osrs-core/       # canonical IDs, typed coordinates, planes, tiny shared value types
├── osrs-edit/       # authoritative editable domain, patches, history, queries, validation
├── osrs-cache/      # cache codecs, archive definitions, cache-source abstractions
├── osrs-scene/      # OSRS scene semantics, terrain/object construction, collision/bridge rules
├── osrs-hd/         # reusable 117HD data/settings/material/environment semantics
├── osrs-render/     # wgpu projection only, GPU resources, pipelines, shaders, mesh upload
├── osrs-world/      # streaming/orchestration, async scene construction, projection workers
├── osrs-reference/  # reference fixtures/parity infrastructure, never a production dependency
└── osrs-editor/     # composition root, UI, input, tools/controllers, platform integration
```

Dependency direction:

```text
                           ┌──────────────┐
                           │ osrs-editor  │
                           └──────┬───────┘
                                  │
                    ┌─────────────┼─────────────┐
                    ▼             ▼             ▼
               osrs-edit      osrs-world    osrs-render
                    │             │             │
                    ▼             ├──────┐      ├──────┐
               osrs-core          ▼      ▼      ▼      ▼
                              osrs-cache osrs-scene  osrs-hd
                                  │      │
                                  └──► osrs-core
```

The exact allowed dependency matrix is CI-enforced. A lower-level crate may never depend upward merely because it needs one type. Move the type to the lowest semantically correct owner instead.

`osrs-reference` is validation infrastructure. Production crates must never depend on it.

## 4. Edit domain: CQRS without replacing MapPatch

Do not replace the existing reversible `MapPatch` model with `Vec<Box<dyn Command>>`, a generic command hierarchy, or a second history mechanism.

`MapPatch` is the semantic transaction primitive because it is deterministic, typed, reversible, atomic, headless-testable, and capable of deriving `ChangeImpact`.

### Command side

User-facing tools produce typed intents, for example:

- `PaintStrokeIntent`
- `HeightStrokeIntent`
- `TerrainPathIntent`
- `ObjectPlacementIntent`
- `ClearOverlayIntent`

A pure planner converts an intent into a patch:

```text
intent + MapQuery -> Result<MapPatch, DomainError>
```

The planner must not mutate UI, renderer, workers, selection, preview state, or global application state.

### Transaction side

The semantic session executes patches only:

```text
MapPatch
  -> validate complete touched state
  -> apply atomically
  -> semantic history
  -> revision++
  -> EditReceipt
```

Recommended application-facing result:

```rust
pub struct EditReceipt {
    pub revision: u64,
    pub impact: ChangeImpact,
    pub touched_regions: Vec<RegionCoord>,
}
```

If finer projection granularity is later justified, add touched zones/chunks to the receipt without moving that granularity into semantic authority.

### Query side

Semantic reads use narrow read-only APIs such as `MapQuery` or purpose-built read models.

A UI widget or tool must never infer edited semantic state from:

- GPU meshes
- picker triangles
- worker edit maps
- selected render entities
- temporary preview state

## 5. EditorApp must stop being a distributed god class

Splitting `EditorApp` methods across many files does not solve the god-class problem if every feature module still has broad mutable access to every application field.

`EditorApp` must converge toward a thin composition root and frame orchestrator, conceptually similar to:

```rust
pub struct EditorApp {
    project: ProjectRuntime,
    edits: EditRuntime,
    world: WorldRuntime,
    render: RenderRuntime,
    interaction: InteractionRuntime,
    ui: UiRuntime,
    platform: PlatformRuntime,
    diagnostics: DiagnosticsRuntime,
}
```

Names may change when implementation evidence warrants it. The ownership principle may not.

`EditorApp` may:

- construct subsystems
- define frame phase order
- pass narrow contexts between subsystems
- coordinate startup/shutdown
- handle `eframe` lifecycle boundaries

`EditorApp` may not:

- own dozens of unrelated feature-specific maps/flags directly
- contain editing algorithms
- contain scene-building algorithms
- contain cache decoding
- contain large UI pages
- contain object, paint, height, or path business rules
- act as a mutable service locator

Feature modules must not add broad `impl EditorApp` blocks merely for convenience. Orchestration/lifecycle glue can remain on `EditorApp`; feature behavior belongs on the subsystem that owns the state.

Bad:

```text
EditorApp::apply_object_move()
  -> session + selection + renderer + worker + UI mutation
```

Target flow:

```text
ObjectToolController -> ObjectMoveIntent
EditRuntime           -> plan + commit
ProjectionRuntime     -> consume EditReceipt
SelectionRuntime      -> react to completed projection when required
```

## 6. Standard runtime subsystems

### ProjectRuntime

Owns:

- active cache/project source
- cache build/profile metadata
- project open/close lifecycle
- dirty/save state
- project-level errors

Does not own semantic tile edits.

### EditRuntime

Owns:

- authoritative semantic `EditorSession`
- application-level edit gateway
- undo/redo routing while transitional routes remain
- edit receipts/revisions

Pure session/history implementation should live as low as practical, preferably in `osrs-edit`. `osrs-editor` owns application integration only.

### WorldRuntime

Owns:

- region residency
- stream requests
- worker communication
- asynchronous scene-build scheduling
- loaded/in-flight projection state
- dirty projection regions/zones
- animation worker orchestration

It does not own edited semantics.

### RenderRuntime

Owns:

- `SceneRenderer`
- render targets
- GPU textures/buffers
- render-origin rebasing
- render profile/HD GPU integration
- GPU resource lifetime

It consumes projection data. It never answers semantic editing queries.

### InteractionRuntime

Owns:

- camera
- hover/picking result
- selection
- marquee
- gizmo interaction
- active tool and gesture state
- spline control state

It emits intents. It does not commit semantic map mutations directly.

### UiRuntime

Owns:

- window open/close state
- searches/filters
- menus and transient notices
- dock/panel state
- settings editing UI state

Persistent settings data remains a dedicated settings model, not a collection of widget-owned fields.

### PlatformRuntime

Owns platform-facing services and adapters such as:

- file/folder picker
- persistent browser storage
- clipboard/open URL where needed
- native/WebAssembly differences

Platform conditionals terminate here or in narrowly defined cache/storage providers.

### DiagnosticsRuntime

Owns debug-only observability:

- frame/worker counters
- recent transaction/revision summaries
- stale async-job drops
- optional trace ring buffer
- performance diagnostics

Diagnostics observe product state. They do not own it.

## 7. Frame pipeline

The main frame must converge on a stable and visible phase order, for example:

1. `poll_platform_and_stream_events()`
2. `update_interaction_and_input()`
3. `collect_tool_intents()`
4. `execute_pending_edit_intents()`
5. `dispatch_projection_invalidations()`
6. `poll_completed_projection_jobs()`
7. `update_render_runtime()`
8. `draw_ui()`
9. `render_viewport()`
10. `record_diagnostics()`

Do not allow arbitrary UI widgets to trigger hidden cache, renderer, or domain mutations halfway through drawing. An immediate UI action that changes product state returns a typed action/intention into the application flow.

## 8. Feature-slice module organization

Organize by reason to change, not by syntax category alone.

Target `osrs-editor` direction:

```text
src/
├── app/                    # thin composition/lifecycle only
├── edit/                   # application edit gateway + semantic projection adapters
├── tools/
│   ├── paint/
│   ├── height/
│   ├── path/
│   ├── object/
│   └── select/
├── interaction/            # picking, selection, camera, gizmo
├── project/                # cache/project open lifecycle
├── projection/             # invalidation and worker/render projection routing
├── ui/
│   ├── shell/
│   ├── inspector/
│   ├── settings/
│   ├── drawer/
│   ├── context_menu/
│   └── dialogs/
├── platform/
│   ├── native/
│   └── web/
└── diagnostics/
```

This is a target layout, not permission for a mass file-move PR. Migration is subsystem-by-subsystem.

A tool normally follows:

```text
controller/input -> typed intent -> pure planner -> semantic commit -> generic projection
```

Keep small tools compact. Do not manufacture five tiny files merely to match a diagram. If a tool grows, split by stable responsibility, for example:

```text
tools/path/
├── mod.rs          # public surface
├── controller.rs   # gesture/tool state
├── intent.rs       # application input intent if needed
├── preview.rs      # non-authoritative preview
└── tests.rs
```

Pure semantic planning belongs in `osrs-edit` or an already-established pure planner module when possible.

## 9. File-size and god-class prevention policy

A universal 300-line hard limit creates needless fragmentation. RustOSRS uses a ratcheted policy.

### New or fully refactored production Rust files

- preferred: 100-300 lines
- review warning: over 350 lines
- hard CI limit: 500 lines
- hard CI byte limit: 32 KiB

### Dedicated test modules

- preferred: under 400 lines
- hard CI limit: 700 lines
- hard CI byte limit: 40 KiB
- split by behavior when larger

### Legacy production files already above the normal cap

They must exist in `scripts/architecture_debt.json` with exact measured line and byte baselines.

Rules:

1. An allowlisted file may never grow beyond either recorded baseline.
2. A checkpoint touching it should shrink it when reasonably possible.
3. Once it falls within the normal cap, the exception becomes invalid and must be removed permanently.
4. Missing/deleted files may not leave orphaned allowlist entries.
5. No new allowlist entry is allowed merely because splitting is inconvenient.
6. Adding a new exception requires an explicit architecture checkpoint and user approval.

### Function size

- target: under 50 lines for ordinary logic
- warning: over 75 lines
- hard smell: over 100 lines
- long functions must be orchestration or declarative data, not mixed business logic

### Composition-root size

`EditorApp` should converge toward roughly 8-12 direct subsystem fields.

Do not replace 70 fields with one `EditorState` containing the same 70 fields. That is still a god object.

## 10. State ownership rules

Every mutable state item must have one canonical owner.

| State | Canonical owner | Projections may cache it? |
| --- | --- | --- |
| edited terrain/heights/objects | `MapDocument` / semantic session | yes |
| edit history | semantic session | routing metadata only while transitional |
| cache archive bytes | cache/source layer | yes |
| loaded semantic region snapshot | semantic session | world may temporarily hold source inputs |
| scene geometry | scene/world projection | renderer may upload/cache |
| GPU buffers/textures | render runtime | no semantic consumer |
| selection | interaction runtime | UI may display |
| tool preview | tool/interaction runtime | renderer may display |
| persisted editor settings | settings model | UI edits through defined API |
| window/search state | UI runtime | no |

If two structures can disagree and the program has no deterministic rule saying which wins, the architecture is invalid.

## 11. Reactive projection and invalidation

Semantic edits must not manually poke every downstream system.

The edit result supplies an explicit change description:

```text
revision
ChangeImpact
regions/touched area
```

A projection/invalidation layer maps that to work:

```text
TERRAIN_GEOMETRY -> terrain mesh rebuild
TERRAIN_MATERIAL -> material/texture projection
NORMALS          -> normal rebuild
LIGHTING         -> lighting rebuild
COLLISION        -> collision projection
LOCATIONS        -> object/scene projection
BOUNDS           -> bounds/spatial metadata
```

### Async stale-result rule

Every asynchronous build carries at least:

- source revision
- generation/source identity
- projection key such as region/zone

A result is installed only if still current. Otherwise it is discarded and observable as a stale completion.

### Region vs zone/chunk granularity

Semantic ownership remains region/world-coordinate based.

8x8 OSRS zones can be useful projection/invalidation granularity. They must not become a competing authoritative map model merely to optimize rendering.

Adopt finer 8x8 rebuilds only after profiling/reference tests demonstrate the benefit and neighbor-edge invalidation rules are specified.

## 12. Native and WebAssembly portability

Native/WebAssembly support is a first-class architecture constraint, but platform APIs stay at the edges.

These areas remain UI/GPU/platform agnostic unless their explicit purpose says otherwise:

- `osrs-core`
- `osrs-edit`
- cache codecs inside `osrs-cache`
- semantic scene logic inside `osrs-scene`

Use narrow ports/adapters instead of scattering `#[cfg(target_arch = "wasm32")]` through business logic.

Potential ports include:

- `CacheStorage` / `CacheSource`
- `ProjectPicker`
- `PersistentSettingsStore`
- `ExternalLinkOpener`
- `Clock` only where deterministic time abstraction is necessary

Do not force `async_trait`, Tokio, Rayon, or a browser-storage library into lower layers unless a demonstrated use case requires it.

Once native stability is sufficient, CI should include an exact gate equivalent to:

```bash
cargo check --target wasm32-unknown-unknown -p osrs-editor
```

and targeted lower-crate checks. Native-only dependencies belong behind target-specific manifests/modules.

## 13. Rendering architecture

Keep WGPU isolated in `osrs-render` plus the narrow `eframe` integration required by the application.

The existing render-to-texture viewport direction remains valid:

```text
SceneRenderer -> off-screen color/depth target -> egui texture -> docked viewport
```

Do not rebuild the viewport merely because a design proposal depicts a similar pattern.

Renderer rules:

1. Renderer consumes immutable or owned projection data.
2. Renderer does not edit `MapDocument`.
3. Picking results identify domain coordinates/objects but are not domain truth.
4. Shader/material features remain isolated from editor transaction logic.
5. 117HD work remains a separate subsystem/branch unless a narrowly required adapter change is documented.

## 14. Cache decoding, encoding, and losslessness

The cache system should support strong round-trip guarantees, but byte-for-byte preservation must be claimed precisely.

Required guarantees:

1. Untouched archive preservation: do not unnecessarily decode/re-encode an archive that was not edited during save.
2. Semantic round-trip: supported decoded structures encode/decode to the same semantic value.
3. Byte-identical round-trip where promised: codecs claiming binary preservation must prove exact bytes in tests.
4. Reference parity: pinned reference/golden fixtures remain authoritative compatibility gates.
5. Property/fuzz testing: exercise valid ranges and malformed input where practical.

Do not claim all Jagex formats are byte-identical after semantic re-encoding unless tests prove it. Equivalent semantics can have multiple valid binary representations.

## 15. Debuggability and observability

A maintainable editor must explain what happened.

Each semantic transaction should be diagnosable by:

- transaction/gesture kind
- semantic revision before/after
- `ChangeImpact`
- touched regions/areas
- whether projection was scheduled
- projection generation/revision
- projection failure or stale-drop reason

Prefer structured tracing/spans over scattered `println!` debugging.

Example shape:

```text
edit.commit tool=paint revision=142 impact=TERRAIN_MATERIAL regions=[...]
projection.schedule key=region_50_50 revision=142 generation=7
projection.drop key=region_50_50 source_revision=141 current_revision=142
```

Tracing must be optional/cheap in release builds and usable on native and web targets.

Error rules:

- domain errors use typed enums
- adapter errors add context at the boundary
- avoid stringly typed control flow
- never silently swallow a semantic/projection mismatch

## 16. UI architecture

UI code renders state and emits actions. It does not own editing semantics.

Recommended pattern:

```text
read model + UiState -> draw -> UiAction
UiAction -> controller/application intent
```

Large settings, drawer, context-menu, inspector, workspace, and similar areas become feature folders once their responsibility warrants it.

Example:

```text
ui/settings/
├── mod.rs
├── model.rs
├── navigation.rs
├── interface_page.rs
├── graphics_page.rs
├── hd_lighting_page.rs
├── controls_page.rs
└── search.rs
```

Do not split solely by line count. Split by reason to change and stable UI feature/page boundaries.

A widget may update UI-local state directly. Product/domain mutations return typed actions and follow the normal command path.

## 17. AI-safe coding protocol

Repository documentation, not conversation memory, is the source of truth for an AI developer.

Before changing code, an agent must:

1. Read this architecture contract.
2. Read `DEV_HANDOFF.md`.
3. Verify the live branch and exact head SHA.
4. Verify the intended base/main SHA when relevant.
5. Search for the existing type/function/module responsible for requested behavior.
6. Inspect current call sites before inventing an abstraction.
7. State the checkpoint boundary and explicit out-of-scope systems.
8. Identify the single canonical owner of every state being changed.

### Search-before-create rule

Before adding a new type, command/intent, coordinate structure, history mechanism, worker message, renderer cache, or generically named helper, search the repository for the concept and synonyms.

If an existing mechanism is close, extend/refactor it rather than creating a parallel one unless this contract explicitly requires replacement.

### No dumping-ground modules

Do not create unrelated-behavior dumping grounds such as:

- `utils.rs`
- `helpers.rs`
- `common.rs`
- `misc.rs`

A shared module needs a precise semantic responsibility.

### One concept, one name, one owner

Do not introduce `WorldCoord`, `MapCoord`, `GlobalTile`, and `TilePos` for the same semantic concept across subsystems. Use canonical vocabulary from `osrs-core`.

### Legacy adapter labeling

Temporary compatibility code must make its transitional status obvious in naming/docs. The handoff must state what condition removes it.

A temporary bridge cannot become permanent because its purpose was forgotten.

## 18. GitHub/API-safe editing rules

These rules make repository editing safer for AI/API workflows.

1. Never use a partial/sliced file fetch as complete replacement content.
2. Before whole-file replacement, fetch the complete current file at the exact branch head.
3. Never write an old blob after another commit changed the same path; refetch first.
4. After a whole-file update, compare/diff the branch and verify no accidental deletions.
5. Prefer focused file creation/extraction over repeated rewrites of a large central file.
6. The 500-line/32-KiB target partly exists to keep whole-file API operations reviewable and safe.
7. If a file is too large for safe editing, first extract a coherent responsibility rather than performing a giant rewrite.
8. Never claim a branch/file was deleted or merged unless repository state confirms it.
9. If the user manually merges/edits something, re-read live repository state before continuing.
10. Exact SHA matters. CI from an earlier head does not certify a later code or documentation head.

## 19. Checkpoint workflow

### One active system

Work on one focused subsystem/branch at a time.

Do not mix editor refactors, renderer work, 117HD work, cache work, and UI redesign merely because files are nearby.

### Small checkpoints

Each checkpoint closes one understandable architectural or behavioral boundary, for example:

- extract render runtime ownership
- migrate one context-menu edit through semantic authority
- split one settings responsibility
- introduce revisioned projection invalidation

Do not create a checkpoint called "rewrite the editor architecture".

### Stop after every checkpoint

After completing a checkpoint:

1. update tests
2. update `DEV_HANDOFF.md` when architecture/state changed
3. validate the exact head
4. report what changed
5. report exact branch/head
6. report what remains
7. stop

Do not automatically begin the next checkpoint. Wait for explicit user direction such as `continue` or `go ahead`.

### No silent scope expansion

If an unexpected dependency requires a much larger checkpoint, choose the narrowest coherent solution. If the boundary truly cannot remain narrow, stop at a safe state and report the architectural reason rather than silently turning it into a multi-system rewrite.

### PR count

Keep at most 1-2 active PRs unless the user explicitly changes the rule. Prefer one focused PR per completed subsystem.

### Merge rule

A subsystem is merge-ready only when:

- intended behavior is complete
- no known split-authority/history hazard remains
- exact-head Tier A/B/C is green as required
- branch scope is clean
- handoff is current

Merge only with explicit user authorization or a standing instruction to manage merges for that checkpoint.

### Post-merge rule

After merge:

1. read the actual integrated `main` SHA
2. validate that exact commit through required CI
3. confirm the old branch is retired/deleted if tooling permits
4. update handoff with the real merge SHA when required
5. only then start the next subsystem on a fresh branch

Never assume squash vs merge-commit behavior.

## 20. CI quality gates

### Tier A: static architecture and quality

Must include:

- architecture dependency guard
- file-size/debt ratchet guard
- formatting
- workspace compile
- strict Clippy policy
- reference-regeneration safety tests
- architecture-guard self-tests

Recommended additions as migration permits:

- WASM compile boundary check
- forbidden dependency/import checks for GPU/UI leakage outside intended layers
- no-growth validation for all legacy oversized modules

### Tier B: fast behavior tests

- full workspace tests
- domain planner tests
- edit/history tests
- adapter/projection tests
- relevant property tests

### Tier C: semantic/reference parity

Preserve the M3-M9 semantic/reference suite.

No editor refactor may weaken reference parity merely to make CI green.

A green run certifies only the exact SHA it tested. Documentation commits that become the reported final head must also be validated when CI is triggered for them.

## 21. Testing strategy

Tests follow ownership boundaries.

### Domain tests in `osrs-edit`

Test:

- patch validation
- atomicity
- reversibility
- history
- typed coordinate behavior
- planner determinism
- no-op behavior
- region/plane boundary behavior

These tests require no GPU/UI/cache files unless semantically necessary.

### Cache tests

Test:

- decode fixtures
- encode/decode semantic round-trip
- exact byte round-trip where guaranteed
- malformed input
- property/fuzz cases

### Scene/reference tests

Test OSRS-specific behavior:

- bridges
- collision
- shapes/rotations
- model placement
- lighting/normals
- reference golden hashes

### Adapter tests

Test that known semantic results produce expected worker/render projection inputs.

Projection tests are never the only proof that semantic editing is correct.

### UI tests/smoke tests

Focus on:

- action generation
- state routing
- no panics
- startup/platform paths

Do not encode domain semantics only through pixel/UI tests.

## 22. Performance rules

Architecture should enable high performance without unproven FPS promises.

The interactive frame thread must not perform heavy:

- cache decoding
- region mesh construction
- model decoding
- large texture processing
- save/repack work

Schedule heavy operations through bounded workers appropriate to the platform.

Engineering targets:

- common semantic command commits normally sub-millisecond to a few milliseconds
- input-to-preview within one frame
- no full-world rebuild for a one-tile edit
- bounded streaming/in-flight work
- stale async jobs discarded deterministically
- native viewport designed to scale toward high-refresh displays
- WebGPU remains functional without native-only assumptions

Profile before adding complex chunk caches or ECS layers.

## 23. Decisions about the CQRS proposal

### Adopt

- strict separation of data/model, rendering, and UI
- transactional command path
- CQRS-style read/write separation
- typed coordinates
- native/WebAssembly platform boundaries
- off-screen WGPU viewport integration
- localized projection invalidation
- lossless/round-trip cache verification
- property-based testing
- strong module-size limits

### Adapt

- use existing `osrs-core` instead of creating `osrs-coords`
- use existing `osrs-edit` instead of creating `osrs-edit-core`
- keep `MapPatch` as transaction primitive instead of a generic boxed command log
- use 8x8 zones as optimization/projection granularity, not authoritative world state
- use realistic performance budgets instead of guaranteed 144+ FPS claims
- use a 500-line hard cap plus legacy ratchet instead of a universal 300-line hard cap
- evolve current cache/platform abstractions rather than forcing one trait/library design immediately

### Reject

- parallel replacement architecture alongside the current one
- rewriting all crates before continuing feature work
- Bevy/ECS merely to distribute state
- renderer-derived semantic state
- global service locators
- giant `EditorState` wrappers that merely rename the god class
- broad `impl EditorApp` feature modules
- duplicated command/history systems
- hidden UI side effects that bypass semantic transactions

## 24. Migration roadmap

Each numbered phase is a subsystem, not one giant checkpoint. Each phase can require multiple small stop/report checkpoints.

### Phase 0: finish Path integration

- complete Path semantic commit/history integration
- merge only after exact-head validation and explicit authorization
- validate integrated main

Status is tracked in `DEV_HANDOFF.md`.

### Phase 1: architecture enforcement

- add this contract to the repository
- change module-size CI to the 500-line/32-KiB production policy
- add 700-line/40-KiB dedicated-test policy
- introduce exact measured debt allowlist for current oversized production modules
- enforce no-growth/stale/orphan debt rules
- test the guard itself

### Phase 2: decompose EditorApp ownership

Extract coherent runtime structs one at a time:

1. `RenderRuntime`
2. `WorldRuntime`
3. `InteractionRuntime`
4. `ProjectRuntime`
5. `UiRuntime`
6. `DiagnosticsRuntime`

Do not change product behavior merely to extract state.

### Phase 3: central edit gateway

- one application gateway accepts edit intents/patches
- Paint, Height, Object, Path, and context-menu edits use one semantic commit-result path
- remove remaining split global history/projection routing when no longer needed
- move pure session/history code downward when safe

### Phase 4: context-menu semantic migration

- migrate `clear_overlay_at` and remaining direct semantic edit paths
- prove all map mutations enter through the semantic transaction boundary

### Phase 5: projection/invalidation service

- centralize `EditReceipt -> invalidation`
- remove tool-specific dirty-region poking
- revision/generation-check every async install
- optionally add zone-level invalidation after profiling/tests

### Phase 6: large UI debt

Refactor one UI feature at a time, such as:

- settings window
- drawer
- context menu
- workspace/docking
- other allowlisted modules

Move logic to feature-owned structs. Do not simply shuffle broad `impl EditorApp` blocks.

### Phase 7: platform ports and WebAssembly gate

- concentrate native/web differences
- add WASM CI compile gate
- harden browser cache/project storage abstraction

### Phase 8: cache writing and round-trip guarantees

- formalize encoder guarantees
- property tests
- untouched archive preservation
- transactional save/repack design

### Phase 9: renderer/projection optimization

- profile region rebuild cost
- add finer 8x8 zone/chunk projection only where measured
- keep semantic ownership unchanged

### Phase 10: continue 117HD parity

Resume HD work on focused branches once editor authority/projection boundaries are stable enough that HD cannot become coupled to editing semantics.

## 25. Definition of architecture success

The migration succeeds when all of the following are true:

- `EditorApp` is a small orchestrator instead of a state god object.
- No production Rust file exceeds the normal hard cap without a ratcheted legacy exception.
- No feature relies on broad access to all application state.
- All map mutations go through one semantic transaction boundary.
- Undo/redo lives in one authoritative semantic history.
- Scene, worker, picker, and renderer state can be deleted and rebuilt from semantic state.
- Native and WASM platform differences are isolated.
- A developer can identify the owner of a bug by subsystem instead of searching the whole application.
- An AI agent can read this document and `DEV_HANDOFF.md`, locate one canonical implementation, make a focused change, validate it, and stop without relying on chat memory.
- Adding a new tool follows an established pattern instead of inventing a new architecture.
- Large features can change without editing a central 1,000+ line file.
- Exact-head CI continuously proves architecture boundaries and semantic parity.

## 26. Short AI checklist

Before coding:

- [ ] Read `ARCHITECTURE.md`
- [ ] Read `DEV_HANDOFF.md`
- [ ] Verify branch and exact SHA
- [ ] Identify checkpoint and out-of-scope systems
- [ ] Search before creating anything
- [ ] Identify canonical state owner
- [ ] Identify command/query/projection boundary
- [ ] Inspect callers and tests

Before committing:

- [ ] No second source of truth introduced
- [ ] No broad new `impl EditorApp` feature logic
- [ ] No unrelated cleanup
- [ ] File/function size remains within policy or debt shrinks
- [ ] Tests cover semantic behavior at the correct layer
- [ ] Projection failures cannot silently desync semantic state

Before reporting checkpoint complete:

- [ ] Exact-head Tier A green
- [ ] Exact-head Tier B green
- [ ] Exact-head Tier C green when required
- [ ] Branch diff contains only expected files/scope
- [ ] `DEV_HANDOFF.md` updated when needed
- [ ] Exact head SHA reported
- [ ] Next checkpoint stated but not started
- [ ] Stop and wait for user

## 27. Final architecture rule

When convenience conflicts with clear ownership, choose clear ownership.

When a clever abstraction conflicts with simple deterministic data flow, choose the simple deterministic data flow.

When making one checkpoint larger conflicts with preserving single semantic authority, preserve single authority and explicitly redefine the checkpoint rather than hiding duplicate authority.

The repository must be understandable from the repository itself. Chat history and model memory are never architectural dependencies.
