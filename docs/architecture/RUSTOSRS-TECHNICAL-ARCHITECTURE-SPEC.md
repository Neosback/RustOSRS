# RustOSRS Technical Architecture Specification

Status: **Target architecture and engineering specification**  
Audience: engine, editor, rendering, cache, tooling, procedural-generation, verification, and AI contributors  
Scope: RustOSRS as a production-grade Old School RuneScape map editor and world-authoring platform  
Last updated: 2026-10-19

## 0. Document authority and intent

This document defines the **target technical architecture** for RustOSRS after the architecture and AI-foundation cleanup merged to `main`.

It is intentionally ambitious. The goal is to design an editor that can exceed incumbent professional map-authoring tools in responsiveness, fidelity, memory efficiency, debuggability, maintainability, automation, and developer productivity while preserving exact OSRS behavior where OSRS behavior is the contract.

This document does **not** claim knowledge of Jagex's proprietary current editor implementation, performance, or internal architecture. "Better than the in-house system" is therefore treated as a measurable product objective, not as a statement about undocumented internals.

Authority order remains:

1. `docs/specs/` for exact OSRS semantic behavior;
2. `ARCHITECTURE.md` for currently adopted repository ownership and coding boundaries;
3. accepted `docs/adr/` decisions;
4. this document for the preferred target shape and quality bar;
5. implementation plans and milestone documents for sequencing.

If this document conflicts with a pinned semantic specification or an accepted ADR, the semantic specification or ADR wins until deliberately revised.

This is also **not** permission for a clean-slate rewrite. RustOSRS already has strong architecture, parity work, rendering infrastructure, editing semantics, WFC/procedural generation, cache support, and verification. The implementation strategy is incremental replacement of debt with stronger ownership boundaries, not parallel architecture.

---

# 1. Executive architecture

RustOSRS should converge on one simple rule:

> **Authored semantic state is transactional authority. Every scene, collision map, picker, mesh, render buffer, minimap, diagnostic surface, AI feature cache, and automation view is a revisioned projection that can be destroyed and rebuilt.**

The system is a **Transactional CQRS Workspace Architecture with Reactive, Revisioned Projections**.

```text
                         AUTHORING / COMMAND FLOW

  UI / keyboard / mouse / automation / AI
                    |
                    v
             typed intent/action
                    |
                    v
              pure planner
                    |
                    v
                 patch
                    |
                    v
              EditRuntime
                    |
                    v
              EditorSession
                    |
                    v
   EditorDocument / WorkspaceDocument / MapDocument
                    |
                    v
      EditReceipt + ChangeImpact + footprint
                    |
        +-----------+------------+----------------+
        |                        |                |
        v                        v                v
 scene/world projection    query projections   diagnostics
        |                        |                |
        v                        v                |
 revisioned CPU scene      collision/picker     |
        |                                         |
        +----------------------+------------------+
                               |
                               v
                      render-facing state
                               |
                               v
                              GPU


                           SOURCE / CACHE FLOW

 osrs-filestore -> osrs-cache decode -> semantic hydration
 semantic export -> osrs-cache encode -> osrs-filestore

 Cache bytes are source data. They are not interactive edit authority.
```

The architecture should make the following impossible or structurally difficult:

- renderer state becoming authored truth;
- worker mirrors silently diverging from semantic state;
- a tool inventing its own history implementation;
- a UI widget mutating the map during painting/drawing;
- a stale async job overwriting a newer revision;
- a procedural generator bypassing undo/redo;
- a renderer optimization erasing information required by OSRS parity;
- an AI or MCP path acquiring capabilities that the normal editor does not have;
- a second implementation appearing because the first one was difficult to understand.

---

# 2. Product objectives

## 2.1 Fidelity

Reference-mode output must preserve the required OSRS semantics, including where applicable:

- terrain shape topology;
- overlay/underlay rules;
- height sampling;
- source, storage, collision, and render plane distinctions;
- bridge behavior;
- loc placement, footprints, types, and orientation;
- wall and wall-decoration placement behavior;
- model selection and transformation;
- winding and mirroring;
- contouring;
- normals and cross-model normal reconciliation;
- software/client lighting rules;
- texture and UV inputs;
- face priority;
- alpha and authored bias;
- roof grouping, suppression, and visibility policy;
- animation and morph semantics where implemented;
- profile-specific ordering behavior.

Performance work may change implementation strategy, data layout, batching, scheduling, caching, and GPU resource ownership. It may not silently change those contracts.

## 2.2 Responsiveness

The editor should feel instantaneous during ordinary work:

- camera motion must not trigger semantic work;
- tool previews must not wait for full scene rebuilds;
- a one-tile edit must not rebuild an entire map square unless a real dependency requires it;
- background loading must not stall the input/UI thread;
- stale worker results must be discarded rather than serialized behind newer edits;
- repeated drag edits should coalesce projection work;
- expensive diagnostics must be opt-in and bounded.

FPS is an outcome, not the architecture metric. The primary metrics are frame time, input-to-preview latency, semantic commit latency, projection latency, GPU submission time, stall time, allocation rate, and memory footprint.

## 2.3 Memory efficiency

Large worlds must be practical without keeping every representation of every loaded map square alive indefinitely.

The system should:

- store one authoritative semantic copy;
- keep derived data reconstructible;
- evict clean derived projections under memory pressure;
- deduplicate immutable models, textures, definitions, and shared source data;
- avoid one heap allocation per tile or per face;
- avoid per-frame whole-document clones;
- avoid snapshot-per-edit history;
- use compact indexes for hot queries;
- bound worker queues and scratch memory;
- keep dirty authored state safe even when clean source regions are evicted.

## 2.4 Maintainability

A new contributor or coding agent should be able to answer quickly:

```text
Does this feature already exist?
Who owns it?
Where is the semantic planner?
Where is the transaction gateway?
What is authoritative?
What is derived?
Which ChangeImpact does it produce?
Which projection consumes that impact?
Which OSRS specification controls behavior?
Which tests prove commit, cancel, undo, parity, and failure behavior?
Which benchmark protects its hot path?
```

The repository should make the correct path easier to discover than creating a duplicate path.

## 2.5 Determinism and reproducibility

Any difficult bug should be reducible to a reproducible package containing enough information to recreate the exact semantic and projection inputs.

The long-term debugging unit is not "a screenshot and a description." It is a deterministic reproduction bundle.

---

# 3. Non-negotiable design principles

## 3.1 One authored source of truth

The current authority chain is the correct foundation:

```text
MapDocument
  ^ WorkspaceDocument (map + scenario)
  ^ EditorDocument (workspace + appearance)
  ^ EditorSession (history + revision policy)
  ^ EditRuntime (application transaction gateway)
  ^ EditorApp (composition/orchestration)
```

`MapDocument` remains sole terrain, height, and object-placement truth.

`ScenarioDocument` and `ThemePack` are separate authored domains composed into the same chronological editor history. They must not become alternate map models.

## 3.2 Semantic data should optimize correctness first

A common mistake in high-performance editors is to make the authoritative domain model look exactly like a GPU buffer. RustOSRS should not do that.

The semantic model should be:

- typed;
- explicit;
- invariant-safe;
- reversible;
- easy to inspect;
- faithful to OSRS storage/semantic distinctions.

Data-oriented layouts belong primarily in **derived hot projections**, where their lifetime and invariants are narrower.

Do not pack authoritative terrain into a `u64` merely because bit operations are fast. Do not change a signed or revision-sensitive OSRS value to a smaller type simply because it saves bytes. Packing is appropriate only when the exact representable range and semantics are proven and permanently tested.

## 3.3 Data-oriented in hot loops, ownership-oriented everywhere else

Use data-oriented design for:

- zone mesh inputs;
- GPU instance buffers;
- material batches;
- visibility sets;
- collision scan buffers;
- model vertex transforms;
- texture metadata;
- repeated spatial queries.

Use strongly owned domain structures for:

- authored state;
- patch history;
- tool state machines;
- project lifecycle;
- settings;
- errors;
- configuration;
- platform services.

The system should not pay abstraction costs in tight loops, but it also should not turn the entire codebase into packed arrays that are difficult to reason about.

## 3.4 Deterministic semantics, asynchronous projections

Semantic edits are ordered and deterministic.

Projection work may run in parallel and complete out of order.

This is safe because every projection job carries revision/generation identity and is rejected if obsolete.

## 3.5 Optimize from measurements, not folklore

No custom lock-free queue, ring buffer, SoA conversion, SIMD path, indirect-draw system, allocator, or cache exists merely because it sounds fast.

The progression is:

```text
correct -> observable -> benchmarked -> optimized -> parity reverified
```

## 3.6 Abstraction only at real variation boundaries

Do not create a trait because a noun exists.

A trait is justified when at least one of these is true:

- there are multiple real implementations;
- platform substitution is required;
- a stable test double provides material value;
- a plugin boundary is deliberately supported;
- dynamic dispatch is actually needed.

Pure edit planners can remain concrete typed functions/modules when that is clearer. A generic `Planner` trait is not a goal by itself.

## 3.7 Encapsulate invariants aggressively

Encapsulation matters most around:

- document mutation;
- history;
- revision lifecycle;
- cache export;
- GPU resource lifetime;
- source/profile identity;
- async stale-result acceptance;
- object identity/provenance;
- semantic-to-projection conversion.

---

# 4. Current workspace and target responsibility boundaries

Keep the existing workspace unless a future dependency proof requires a new crate.

```text
crates/
├── osrs-core/       canonical IDs, typed coordinates, planes, tiny shared value vocabulary
├── osrs-edit/       semantic editing domain, documents, patches, queries, planners, history
├── osrs-filestore/  low-level JS5 sectors, indices, compression, XTEA primitives
├── osrs-cache/      cache codecs, definitions, source/profile/export semantics
├── osrs-scene/      exact OSRS scene construction, terrain/loc semantics, collision/bridge logic
├── osrs-hd/         reusable 117HD data, materials, environments, appearance policy
├── osrs-render/     wgpu projection, GPU resources, pipelines, shaders, render data
├── osrs-world/      streaming, scene-build orchestration, projection scheduling, worker lifecycle
├── osrs-lab/        headless training/evaluation/tool environments and deterministic scoring
├── osrs-reference/  parity fixtures/oracles only, never a production dependency
├── osrs-playtest/   playtest/runtime-facing support
└── osrs-editor/     application composition, UI, input, controllers, platform integration
```

### Dependency rule

Lower layers do not import upward layers to obtain a convenient type.

If a type truly belongs to multiple lower-level consumers, move it to the lowest semantically correct owner. Do not create a catch-all common crate.

### New-crate rule

A proposed crate must answer all of these:

1. What compiler-enforced dependency boundary cannot be achieved cleanly with a module?
2. Who consumes the crate?
3. What does it explicitly not own?
4. Does it reduce coupling or merely relocate files?
5. Does it introduce a second concept that already exists?
6. Can it remain independently testable?

If those questions are not compelling, keep the code in the existing owner.

---

# 5. Command, query, transaction, and history architecture

## 5.1 Standard authored flow

Every user-visible authored operation should converge on:

```text
gesture / command / automation request
        -> typed intent
        -> pure semantic planning
        -> reversible patch
        -> EditRuntime transaction
        -> EditorSession history/revision
        -> EditReceipt
        -> reactive projection
```

The UI does not mutate a tile directly.

The renderer does not mutate an object directly.

The WFC generator does not apply placements directly.

The MCP adapter does not obtain a special back door.

## 5.2 `MapPatch` stays the semantic transaction primitive

RustOSRS already has the correct core concept. Do not replace it with a generic command-object framework or an event-sourcing rewrite.

A patch must remain:

- deterministic;
- completely validated before commit;
- reversible;
- composable;
- inspectable;
- able to derive semantic change impact;
- independent of UI/GPU state.

## 5.3 One gesture, one logical transaction

Examples:

- one paint stroke -> one semantic transaction;
- one object move -> one semantic transaction;
- one generated building -> one semantic transaction, even if it contains hundreds of operations;
- one prefab placement -> one semantic transaction;
- one scenario edit -> one transaction;
- one appearance edit -> one transaction.

Preview updates during a gesture are explicitly transient and non-authoritative.

## 5.4 History is patch-based, not snapshot-based

The canonical `PatchHistory<EditorDocument>` remains the only chronological history.

History must never require cloning the full map per edit.

Undo/redo should:

1. determine the exact authoritative next patch;
2. derive the replay footprint;
3. checkpoint semantic state;
4. replay semantics;
5. prepare required projections from the real post-replay document;
6. finalize if projection preparation succeeds;
7. restore exactly if preparation fails.

## 5.5 Query architecture

Read-only semantic queries should come from the authoritative document or purpose-built derived indexes with explicit rebuildability.

A query cache must identify:

- authoritative source;
- source revision;
- rebuild method;
- invalidation input;
- whether stale reads are allowed.

If none of those are clear, the cache is not architecturally safe.

---

# 6. Semantic storage versus performance storage

## 6.1 Do not turn the semantic model into a GPU struct

Authoritative storage should preserve meaning. Derived storage should preserve throughput.

This leads to a two-shape strategy:

```text
Semantic shape
  typed, readable, reversible, exact
           |
           | compile/project
           v
Hot projection shape
  contiguous, cache-friendly, packed where proven, GPU-friendly
```

## 6.2 Terrain projection layout

The renderer and worker pipeline may compile an 8x8 zone into tightly packed contiguous arrays or POD structures.

However, the exact fields and integer widths must be derived from existing semantic contracts and benchmark evidence. A universal `u64 tile` is not assumed.

Useful hot-path structures may include:

- contiguous height samples needed by the zone plus dependency border;
- underlay/overlay/material IDs;
- shape/rotation values;
- render flags;
- preclassified visibility/suppression bits;
- compact per-face material descriptors.

Keep semantic IDs typed at the boundary and convert once into a projection packet.

## 6.3 Object placement storage

Avoid multiple heap-allocated `Vec`s per 8x8 zone when most zones contain few objects.

A strong default is:

```text
Region placement arena
  Vec<Placement>

Zone membership index
  offsets[65] + packed PlacementId members
```

This CSR-style index gives:

- one contiguous placement allocation;
- one contiguous zone-membership allocation;
- fast zone iteration;
- no per-zone vector allocation;
- support for objects whose footprint touches multiple zones.

The renderer may derive SoA GPU instance streams from this data when profiling shows benefit.

## 6.4 Spatial indexes

OSRS is already a regular grid. Direct coordinate indexing should beat an R-tree for most tile/zone queries.

Use:

- direct region lookup by typed region coordinate;
- direct local tile/zone index arithmetic;
- compact inverted indexes for object ID/type queries;
- optional specialized spatial structures only for genuinely irregular queries.

Do not add a general R-tree to solve a problem that already has O(1) grid addressing.

---

# 7. Region, zone, and residency model

## 7.1 Spatial hierarchy

The useful hierarchy is:

```text
world
  -> map square / region (64x64 tiles)
      -> 8x8 zones
          -> tiles / shared height vertices / placements
```

The **semantic unit of authority is not forced to equal the renderer invalidation unit**.

The 8x8 zone is the accepted primary static render compilation unit. Some changes require a dependency halo beyond one zone.

## 7.2 Dependency-aware zone invalidation

A touched tile does not always mean only one zone needs rebuilding.

Examples:

- a height vertex shared by adjacent tiles may affect neighboring zone geometry;
- normals/lighting may depend on neighboring geometry;
- a multi-tile loc footprint may cross zone boundaries;
- wall adjacency can affect both sides of a boundary;
- roof connectivity can propagate beyond the directly edited tile;
- collision edge flags can affect neighboring cells.

Therefore the system should derive:

```text
semantic footprint
    + ChangeImpact
    -> dependency expansion
    -> projection footprint
```

Dependency expansion belongs to the projection/scene owner, not in UI tools.

## 7.3 Residency policy

Large projects should maintain a bounded working set.

Suggested policy:

- camera-visible regions: highest priority;
- immediate surrounding regions: prefetched;
- dirty authored regions: pinned until safely persisted or explicitly closed;
- selected/inspected regions: optionally pinned;
- clean distant source regions: evictable;
- CPU scene artifacts: evict before authored state;
- GPU artifacts: budgeted separately from CPU semantic residency;
- immutable assets: shared and deduplicated globally.

Use hysteresis so moving back and forth near a region boundary does not constantly load/evict the same data.

---

# 8. Reactive projection architecture

## 8.1 `ChangeImpact` is the semantic vocabulary

The existing `osrs-edit::ChangeImpact` remains the single semantic impact vocabulary.

Do not create a second "renderer dirty flags" enum that gradually duplicates it.

Projection code may translate semantic impact into subsystem-specific work, for example:

```text
TERRAIN_GEOMETRY -> scene terrain build -> zone mesh upload
TERRAIN_MATERIAL -> material projection -> affected draw batches
NORMALS          -> normal dependency rebuild
LIGHTING         -> lighting projection
COLLISION        -> collision cells / playtest navigation
LOCATIONS        -> loc scene construction / picking / object render packets
BOUNDS           -> spatial metadata
SCENARIO         -> scenario-specific projections
```

If a new semantic category is required, add it only after a real consumer proves the current vocabulary is insufficient.

## 8.2 Projection scheduler

Conceptually, `osrs-world` should converge on a projection scheduler responsible for:

- accepting semantic receipts;
- expanding dependency footprints;
- coalescing redundant work;
- prioritizing visible work;
- issuing immutable jobs;
- rejecting stale completions;
- tracking projection generations;
- publishing ready artifacts to renderer/picker/collision consumers.

This does not automatically require a new `ProjectionScheduler` type or crate. Reuse current `WorldRuntime` structures when they already own the lifecycle.

## 8.3 Job identity

Every async projection job should be identifiable by data equivalent to:

```rust
struct ProjectionJobKey {
    source_revision: Revision,
    target: ProjectionKind,
    spatial_key: ZoneOrRegionKey,
    generation: u64,
    profile: ProjectionProfileKey,
}
```

The exact types may differ. The invariant may not:

> A result can be applied only if it still corresponds to the latest required source generation for that target.

## 8.4 Work coalescing

If revisions 100, 101, and 102 invalidate the same zone before its rebuild begins, normally build revision 102 and skip obsolete intermediate projection work.

Semantic history still contains all three commits. Projection scheduling does not.

## 8.5 Work priority

A useful priority order is:

1. current viewport visible zones;
2. active tool preview dependencies;
3. hover/picker dependencies;
4. near-camera prefetch;
5. playtest/collision dependencies required immediately;
6. background scene completion;
7. noninteractive diagnostics/analysis.

This prevents a huge background load from making a single brush stroke feel slow.

---

# 9. Immutable render-facing state

The renderer should not traverse the live mutable semantic document during draw submission.

The target is an immutable or effectively immutable render-facing representation produced by scene/world projection.

Before adding a new snapshot abstraction, audit current scene/world/render structures. Reuse an existing revisioned immutable boundary if one already satisfies the contract.

The required behavior is:

```text
semantic authority
  -> revisioned scene/world artifact
  -> revisioned render-facing artifact
  -> GPU resources
```

No reverse ownership exists.

A render artifact should carry enough identity to answer:

- which semantic revision generated it;
- which spatial area it represents;
- which rendering profile it targets;
- which source/cache/profile identity was used;
- which dependency generation produced it.

---

# 10. Rendering architecture

RustOSRS already accepts **8x8 zone-compiled hybrid rendering**. The target architecture should deepen that decision rather than replace it.

## 10.1 Three render paths

### Static compiled path

For geometry/material state stable enough to compile and reuse:

- ordinary opaque terrain;
- static loc geometry;
- other profile-safe static surfaces.

### Dynamic path

For geometry whose preparation changes with runtime state but does not require strict ordered face emission:

- animations;
- morph changes;
- temporary previews;
- time-varying generated geometry.

### Ordered path

For content requiring explicit camera-relative/reference ordering:

- priority-sensitive Reference-profile faces;
- transparency paths requiring deterministic ordering;
- other profile-defined ordered content.

Classification is a renderer policy decision, never a semantic object type.

## 10.2 Render profiles

At minimum keep conceptual separation between:

- **Reference**: exact RustOSRS OSRS/client fidelity target;
- **RuneLite comparison** where useful for differential validation;
- **Enhanced / HD**: 117HD-derived or RustOSRS enhanced appearance that consumes the same semantics without mutating them.

Do not bury all profiles in one giant shader/function controlled by dozens of unrelated booleans.

Use explicit stable profile keys and specialized pipeline variants where that improves clarity and compilation/runtime behavior.

## 10.3 Renderer input contract

`osrs-render` receives render-ready projections, not mutable domain objects.

A zone render artifact should contain only the data needed to:

- select pipelines/materials;
- bind immutable/shared resources;
- upload/update zone-owned buffers;
- submit static/dynamic/ordered draws;
- preserve diagnostic identity;
- support picking/render correlation.

## 10.4 GPU resource ownership

`RenderRuntime` / `osrs-render` exclusively own:

- `wgpu::Device` and `Queue` integration;
- GPU buffers;
- textures;
- samplers;
- bind groups;
- pipelines;
- transient render targets;
- GPU memory budgeting;
- resource retirement.

Domain crates never hold `wgpu` objects.

## 10.5 Buffer strategy

Avoid both extremes:

- one giant monolithic buffer that is expensive to rebuild;
- one GPU buffer per tiny zone/object that creates excessive driver/resource overhead.

Prefer paged/suballocated GPU arenas:

```text
Terrain vertex pages
Terrain index pages
Static object instance pages
Dynamic object pages
Ordered-face scratch pages
```

Each zone owns lightweight handles/ranges into those arenas.

When a zone changes:

1. rebuild its CPU render packet;
2. allocate/reuse a suitably sized range;
3. upload only the changed range;
4. atomically swap the zone handle after upload scheduling;
5. retire the old range after it is safe to reuse.

Use generation handles to prevent use-after-recycle bugs.

## 10.6 Allocation strategy

Use simple free lists/slab allocators first.

Only introduce buddy allocators, compaction, ring-buffer staging, or custom pooling after allocation telemetry demonstrates fragmentation or contention.

The workspace forbids unsafe code, which is a feature. Do not build a custom unsafe lock-free allocator merely to chase theoretical throughput.

## 10.7 Draw batching

Batch by stable render identity such as:

```text
profile
pass
pipeline
material/texture binding
model geometry identity where applicable
ordered-policy class
```

Use instancing for repeated model geometry where semantics permit.

Multi-draw indirect and GPU-driven submission are optional capability-tier optimizations, not hard dependencies. They must have fallback paths for supported wgpu targets and must prove real CPU savings.

## 10.8 Model caching

Separate:

- immutable decoded model source;
- semantic transformation identity;
- contour/lighting/output variants;
- GPU-resident geometry.

Do not duplicate raw model vertices for every placement when multiple placements share an immutable source.

At the same time, do not incorrectly share geometry across placements when OSRS contouring, lighting, orientation, mirroring, animation, or other semantics make the output distinct.

The cache key must encode every semantic input that affects the produced geometry.

## 10.9 Texture/material caching

Textures/material definitions should be deduplicated by canonical identity and profile.

Use arrays/atlases only when they preserve required sampling semantics and fit wgpu capability limits. A texture-atlas optimization must not change wrapping, filtering, animation, alpha, or UV behavior.

## 10.10 Reverse-Z and precision

Keep the accepted reverse-Z/raster conventions.

For large maps, use render-origin rebasing or equivalent camera-relative coordinates so GPU floating-point precision is independent of world coordinate magnitude.

The render origin is presentation state, not semantic state.

---

# 11. Picking and selection architecture

Picking is a projection, not truth.

A strong hybrid design is:

- GPU ID/depth picking for the visible rendered surface;
- CPU semantic/spatial queries for box selection, hidden-object inspection, diagnostics, and operations that are not identical to visible-fragment picking.

Both paths carry revision identity.

A pick result should resolve to canonical semantic identity, for example a placement ID or typed tile coordinate. UI code should not edit a triangle or GPU draw index directly.

The editor should expose diagnostic modes for:

- picked semantic tile/object;
- scene entity;
- render artifact;
- GPU batch/pipeline;
- source revision;
- world/render generation.

This makes "why did I click the wrong thing?" debuggable without reading renderer internals.

---

# 12. Threading and scheduling

## 12.1 Thread ownership model

A good default topology is:

### Main/event thread

- eframe/egui lifecycle;
- input processing;
- tool state machines;
- semantic commits;
- receipt dispatch;
- applying completed projection handles;
- GPU submission;
- lightweight diagnostics.

### Bounded worker pool

- scene construction;
- terrain/loc zone compilation;
- normals/lighting work;
- collision rebuilds;
- picker projection;
- WFC/procgen solving;
- expensive AI scoring;
- CPU render packet generation.

### I/O services

- cache reads;
- project persistence;
- export;
- recovery/replay journal writes;
- optional background asset reads.

Do not make every API async. Async/threading belongs at boundaries where concurrency exists.

## 12.2 Ownership transfer over shared mutation

Prefer:

```text
immutable job input -> worker-owned scratch -> immutable result -> main-thread publication
```

over:

```text
Arc<Mutex<EntireAppState>> shared by every subsystem
```

The latter destroys ownership reasoning and makes deterministic debugging much harder.

## 12.3 Bounded queues

All worker queues must be bounded or coalescing.

A long paint drag or rapid camera movement must not enqueue thousands of obsolete rebuilds that consume memory for seconds.

## 12.4 Scratch-buffer reuse

Workers may maintain owned reusable scratch buffers for:

- mesh vertices/indices;
- temporary visibility sets;
- WFC domains;
- normal accumulation;
- decompression buffers.

Reuse should be per-worker or pool-owned, avoiding global locks.

## 12.5 Ring buffers are optional

A custom shared ring buffer can be excellent when measurements prove allocation or channel-copy overhead is important. It should not be the default architecture assumption.

Start with safe bounded channels plus pooled owned buffers. Upgrade only after profiling.

## 12.6 Determinism under parallelism

The result for a projection must be a deterministic function of its immutable inputs.

Worker completion order may affect when a frame becomes more complete. It must not affect semantic results or the final projection for a fixed revision/profile.

---

# 13. Memory architecture for very large maps

## 13.1 Memory classes

Track memory separately for:

1. authored semantic state;
2. decoded immutable cache/source data;
3. scene/world CPU projections;
4. model/texture asset caches;
5. worker scratch;
6. render CPU packets;
7. GPU buffers/textures;
8. UI/diagnostic history;
9. AI/procgen models and working memory.

A single "process memory" number is not actionable enough.

## 13.2 Budget manager

A central diagnostics/budget policy should know soft and hard budgets for reconstructible caches.

It may request eviction in this order, subject to product needs:

1. stale generations;
2. invisible GPU zone artifacts;
3. distant CPU render packets;
4. distant scene projections;
5. clean decoded source regions;
6. cold immutable assets with no current references.

Never evict dirty authored state without a persistence guarantee.

## 13.3 Deduplication

Share immutable resources by stable identity:

- object definitions;
- model definitions/data;
- textures;
- material definitions;
- animation definitions;
- environment definitions;
- names/symbol metadata.

Prefer IDs/handles into shared tables over copying large structures into each region/placement.

## 13.4 Avoid accidental retention

Long-lived diagnostics, undo metadata, selection, UI closures, async jobs, and GPU retirement queues must not accidentally retain entire old scene generations.

Memory diagnostics should expose top retainers by category and generation.

---

# 14. Cache, project, persistence, and export

## 14.1 Preserve current responsibility split

`osrs-filestore` owns low-level JS5 transport mechanics.

`osrs-cache` owns cache-format codecs, definitions, source/profile identity, and verified export semantics.

The editor UI does not decode sectors.

## 14.2 Three persistence concepts remain separate

Do not conflate:

- **project save**: editor-owned state needed to resume authoring;
- **crash recovery**: append/checkpoint state designed to survive interruption;
- **cache export**: production OSRS cache-format output.

They have different correctness and lifecycle requirements.

## 14.3 Export safety

A production export should follow:

```text
validate semantic/export state
  -> encode into temporary destination
  -> verify structural invariants/checksums
  -> optionally reopen/read back critical structures
  -> atomically publish/replace destination
```

Never destroy the only good output before a new export is known valid.

## 14.4 Untouched data

Where the cache/storage model allows it, preserve untouched archive/sector data byte-for-byte instead of decoding and re-encoding it unnecessarily.

Where byte identity cannot be guaranteed or is not meaningful, require semantic round-trip equivalence and deterministic encoding.

Do not invent a type-level witness system unless it provides clear value beyond current dirty-set/export invariants.

## 14.5 Provenance

Provenance can be valuable for auditing and procedural generation, but it must remain compact and optional where possible.

Useful provenance classes include:

- loaded from cache/source;
- user-authored revision;
- generated from prefab/WFC seed;
- imported/project-diff source.

Do not inflate every hot placement record with large strings or heap-owned metadata. Store compact IDs into a side table when provenance is required.

---

# 15. Fidelity architecture

## 15.1 Exactness is layered

Different correctness questions require different oracles.

### P0: storage/semantic exactness

Exact values, IDs, planes, flags, transforms, placements, and reversible patches.

### P1: scene structural exactness

Exact produced scene topology, model choices, face metadata, collision, roof membership, visibility/suppression decisions.

### P2: reference raster behavior

Ordering, priority, alpha, depth/bias, UV/texture rules, lighting and raster-sensitive semantics where a pinned reference exists.

### P3: GPU visual parity

Image/capture comparison with a declared tolerance appropriate to cross-GPU floating-point/raster differences.

Do not use screenshot tolerance to hide a P0-P2 semantic mismatch.

## 15.2 Reference oracle strategy

`osrs-reference` remains validation infrastructure and must never become a production dependency.

If a software rasterizer/reference harness is needed, prefer implementing it inside the validation/reference boundary or as a validation-only binary/module. Create a new `osrs-raster` crate only if a genuine independent dependency boundary is proven.

The oracle should preserve the original integer/reference rules rather than "improving" them.

## 15.3 Source identity

Every parity claim should identify:

- cache/source revision;
- pinned deob/RuneLite/117HD source revision as applicable;
- profile/settings identity;
- camera/viewport state for visual tests;
- fixture identity;
- tolerance policy.

## 15.4 Compatibility fixture policy

Every discovered OSRS quirk that caused a bug should become a permanent fixture.

A compatibility bug is not fixed until the repository contains evidence preventing its return.

---

# 16. Reference versus enhanced rendering

The Reference profile and enhanced/HD rendering consume the same semantic world but have different render policy.

```text
MapDocument / scene semantics
        |
        +--------------------------+
        |                          |
        v                          v
 Reference projection        Enhanced/HD projection
        |                          |
        v                          v
 Reference pipelines          HD pipelines/materials
```

HD must not leak into map semantics.

Examples of render-only enhanced concerns:

- material replacement;
- environment lighting;
- normal/parallax mapping;
- water shading;
- reflections/refraction;
- fog/sky;
- post-processing;
- enhanced shadowing.

The base semantic geometry, loc identity, planes, transforms, and authored map state remain shared.

---

# 17. Tool architecture

## 17.1 Controller role

A tool controller is a small input/state machine.

It owns transient interaction state such as:

- gesture start/end;
- brush settings;
- drag anchors;
- selected control points;
- preview parameters.

It emits typed intent.

It does not implement cache export, scene rebuild, or GPU mutation.

## 17.2 Planner role

The planner owns semantic meaning:

```text
intent + read-only semantic query -> patch or domain error
```

Planners should be deterministic and headless-testable.

## 17.3 Preview role

Preview state is explicitly disposable.

A preview may use cheaper geometry or a temporary render path, but it must never be queried as completed semantic state.

Cancel removes/reverts preview state without touching history.

## 17.4 Tool completion contract

Every mutating product feature should have discoverable evidence for:

- controller/input path;
- planner;
- transaction gateway;
- planning test;
- cancellation test;
- commit test;
- undo/redo test;
- relevant semantic spec;
- projection impact;
- performance benchmark if hot.

The canonical feature registry should gradually include these references rather than relying on tribal knowledge.

---

# 18. Procedural generation and AI architecture

Procedural generation is not a privileged mutation system. It is another source of semantic plans.

## 18.1 Required flow

```text
Generator request + deterministic seed + semantic query
        -> generated semantic plan/patch
        -> normal EditRuntime gateway
        -> normal history
        -> normal projection
```

Generated work must be undoable exactly like manual work.

## 18.2 WFC

WFC should separate:

- learned/local compatibility constraints;
- global architectural/topological constraints;
- deterministic solve/search policy;
- semantic patch emission.

Useful global constraints include:

- room connectivity;
- exterior entrance reachability;
- floor-to-floor connectivity;
- roof closure;
- legal object footprints;
- collision/navigation viability;
- style/theme constraints;
- bounded repair behavior.

Do not force WFC to solve problems better handled by deterministic pre/post planners.

## 18.3 Realism critics and learned models

Models/critics should be data loaded by headless-capable systems.

They should not require a special inference server unless scale proves it necessary.

A model output should be explainable enough for diagnostics, at least exposing score components/features used by the product.

## 18.4 Determinism

Every generation request records:

- generator/version identity;
- seed;
- source semantic revision/hash;
- configuration/theme identity;
- resulting patch hash;
- optional score/critic metadata.

This makes generated bugs reproducible even after complex search.

---

# 19. Replay and deterministic reproduction

## 19.1 Intent logs are not sufficient by themselves

An intent can produce a different patch after planner code, cache data, or settings change.

A robust replay bundle therefore records both user-level intent context and semantic verification data.

Conceptually:

```rust
struct ReplayBundle {
    format_version: u32,
    source_identity: SourceIdentity,
    project_identity: ProjectIdentity,
    editor_build: BuildIdentity,
    profile_identity: ProfileIdentity,
    initial_document_hash: Hash,
    steps: Vec<ReplayStep>,
}

struct ReplayStep {
    intent_kind: StableFeatureId,
    intent_payload: SerializedIntent,
    seed_or_clock: Option<DeterministicToken>,
    expected_patch_hash: Hash,
    expected_revision_hash: Hash,
}
```

Exact type design should reuse existing IDs and serialization where possible.

## 19.2 Crash recovery is a separate journal

The recovery journal may store semantic patches/checkpoints directly because its job is restoration, not long-term user-action portability.

Do not make the undo stack itself the crash-recovery file format.

## 19.3 Repro command

The long-term target is a headless command capable of:

```text
open pinned source/project
load replay bundle
execute steps
verify semantic hashes
settle requested projections
capture diagnostics/images
emit failure package
```

---

# 20. Debugging and observability

## 20.1 Structured tracing

Use `tracing`-style spans or equivalent structured events for meaningful lifecycle boundaries:

- semantic commit;
- undo/redo;
- source/region load;
- projection scheduling;
- zone scene build;
- zone render compilation;
- GPU upload;
- stale job drop;
- WFC solve;
- cache export;
- project save/recovery checkpoint.

A trace should carry stable IDs and revision/generation keys, not only prose strings.

## 20.2 Diagnostic overlay

The editor should eventually expose an internal diagnostics overlay with:

- current semantic revision;
- authored edit revision;
- dirty regions;
- pending projection jobs;
- stale job discard count;
- visible zone generations;
- scene/render profile identity;
- selected semantic object/tile;
- selected scene/render identity;
- CPU frame time;
- GPU frame time when available;
- upload bytes/frame;
- draw/batch counts;
- loaded CPU/GPU memory by category.

## 20.3 Per-zone build reason

For any rebuilt zone, diagnostics should answer:

```text
Why was this rebuilt?
Which semantic receipt caused it?
Which ChangeImpact bit mattered?
Which dependency expansion added this zone?
Which source revision generated the accepted result?
How long did CPU build/upload take?
```

This is enormously more useful than a generic "renderer dirty" flag.

## 20.4 Reproduction bundle

A bug-report bundle should be able to include:

- source/cache identity;
- project/document hash;
- recent semantic intents/patch hashes;
- semantic revision;
- render profile/settings;
- camera/viewport;
- selected tile/object;
- scene/render generation keys;
- pipeline/material identity;
- GPU adapter/capabilities;
- relevant trace slice;
- screenshot/capture if visual.

---

# 21. Performance engineering

## 21.1 Performance must have a benchmark corpus

Create representative benchmark scenes instead of optimizing one developer's current map.

The corpus should include:

- empty/light rural map squares;
- dense city geometry;
- many walls/decorations;
- dense vegetation/repeated models;
- water-heavy scene;
- roof-heavy multi-floor buildings;
- transparent/priority-sensitive geometry;
- animated/morph-heavy scene;
- 117HD-heavy material/environment scene;
- large multi-region working set;
- active paint/height/object edit workloads;
- generated building commit/undo workloads.

## 21.2 Measure distributions, not anecdotes

Track at least P50/P95/P99 for:

- main-thread frame time;
- GPU frame time;
- input-to-preview latency;
- simple semantic commit;
- composite semantic commit;
- undo/redo;
- zone scene rebuild;
- zone render compilation;
- GPU upload latency/bytes;
- region load/decode;
- model build;
- WFC solve classes.

Also track maximum stalls.

## 21.3 Provisional interactive target envelope

These are targets to calibrate on fixed benchmark hardware, not claims of current performance:

- normal interactive editing should maintain a reliable 60 Hz floor;
- 120 Hz should be the primary high-responsiveness target on representative desktop hardware;
- 240 Hz is a stretch target for light/typical scenes where GPU cost permits;
- input-to-preview should normally fit within one target refresh interval;
- simple edits should not cause full-region rebuild stalls;
- background streaming should not create >16.7 ms main-thread stalls in normal operation.

Do not market a single "500 FPS" number. A tool that renders 500 FPS while freezing for 80 ms after every stroke is not a high-performance editor.

## 21.4 Regression gates

Once a benchmark is stable enough for CI:

- record benchmark hardware/environment class;
- use repeated samples and median/percentile statistics;
- fail only on meaningful regressions beyond noise;
- preserve correctness/parity checks in the same change;
- store trend artifacts rather than one opaque pass/fail number.

## 21.5 Allocation diagnostics

Measure allocations and retained bytes for hot operations.

Targets should include:

- no whole-document clone during normal commit/undo;
- bounded worker scratch;
- no unbounded queue growth;
- no per-frame recreation of stable GPU resources;
- no per-object model vertex duplication when sharing is semantically legal;
- minimal transient allocations during steady-state camera movement.

---

# 22. GPU capability tiers

Wgpu targets differ. Design optimizations in capability tiers.

Example conceptual tiers:

### Baseline

- standard indexed/instanced draws;
- portable texture/buffer limits;
- CPU-prepared draw lists.

### Advanced

- indirect drawing where supported and beneficial;
- larger binding arrays/bindless-like strategies where portable enough;
- timestamp queries;
- more aggressive GPU culling if proven.

### Diagnostic/reference

- slower ordered/reference paths;
- instrumentation/capture buffers;
- parity overlays.

A high-end optimization must not make the project impossible to run on a supported baseline adapter.

---

# 23. Error handling and resilience

## 23.1 User/source data does not cause panics

Malformed cache/project/import data should produce typed, contextual errors rather than `unwrap`/`expect` failures in production paths.

## 23.2 Error taxonomy

Differentiate at least:

- source/cache decode errors;
- semantic validation errors;
- planner/domain errors;
- projection/build errors;
- GPU capability/resource errors;
- persistence/export errors;
- deterministic replay mismatch;
- internal invariant violations.

An internal invariant failure should include revision/source identity and a path to create a repro bundle.

## 23.3 Failure injection

Test:

- projection failure after semantic checkpoint;
- worker cancellation;
- stale results;
- I/O interruption;
- out-of-disk behavior during save/export;
- corrupted cache archives;
- invalid project version;
- GPU device loss where recoverable;
- WFC unsatisfiable constraints.

---

# 24. Documentation architecture

## 24.1 Code should explain ownership and invariants

Every important module should answer near its root:

```text
Owns
Does not own
Authoritative source
Derived/rebuildable state
Key invariants
Concurrency model
Primary tests/specs
```

This is more useful than documenting every trivial getter.

## 24.2 Public API documentation quality

Document public architectural APIs thoroughly, especially:

- semantic mutations;
- patches;
- revision types;
- cache/export boundaries;
- scene/render packets;
- async job identity;
- GPU resource handles;
- profile/config identities.

A blanket `deny(missing_docs)` across every UI helper can incentivize useless comments. Prefer targeted high-quality documentation gates on architectural/public surfaces.

## 24.3 WHY comments

Comments should explain constraints and surprising decisions, especially:

- OSRS compatibility quirks;
- source-derived integer behavior;
- dependency-halo rules;
- stale-job rejection logic;
- render ordering exceptions;
- cache encoding requirements.

Do not narrate obvious syntax.

## 24.4 ADR policy

Use ADRs for hard-to-reverse choices such as:

- spatial compilation unit changes;
- persistence format;
- renderer profile policy;
- cache/export ownership;
- new crate boundaries;
- GPU resource architecture;
- automation protocol boundary.

## 24.5 AI-native navigation

Continue the current hierarchy:

- small root `AGENTS.md`;
- scoped crate/subsystem `AGENTS.md` files;
- canonical machine-readable feature registry;
- generated human feature index;
- architecture ownership ratchets;
- reproducible architecture dashboard;
- semantic specs and ADR links.

The goal is not more instructions. The goal is the **smallest local set of instructions that makes the correct implementation obvious**.

---

# 25. Machine-readable architecture and feature registry

The existing feature registry should remain the canonical product-navigation source.

A future schema version may add fields such as:

```text
semantic_specs
accepted_adrs
change_impacts
projection_consumers
performance_benchmarks
parity_fixtures
replay_feature_id
```

Only add fields that are enforceable and maintained automatically or cheaply.

Do not turn the registry into a second prose architecture document.

Architecture tooling should continue ratcheting measurable facts such as:

- forbidden dependencies;
- direct semantic mutation entry points;
- number of application history stacks;
- `EditorApp` surface growth;
- architecture-debt file sizes;
- unregistered mutating features;
- stale feature symbols;
- production dependencies on reference-only crates.

A baseline is a ceiling, not a target. When debt reaches zero, delete the allowance.

---

# 26. Testing strategy

## 26.1 Unit tests

Use for:

- typed coordinates/IDs;
- planners;
- patch validation/inversion;
- bridge/collision math;
- shape/rotation behavior;
- dependency-footprint expansion;
- render classification;
- cache codecs;
- deterministic generators.

## 26.2 Property tests

High-value properties include:

- `patch.apply(); inverse.apply()` restores exact semantic hash;
- failed patch validation cannot partially mutate state;
- undo then redo restores exact state;
- object footprints never exceed legal capacity rules;
- projection order does not change final artifact hash;
- stale projection cannot replace a newer generation;
- WFC output satisfies declared topology constraints;
- cache decode/encode round trips preserve required semantics.

## 26.3 Differential/reference tests

Use pinned external/reference evidence for OSRS behavior.

The reference fixture must state provenance and revision.

## 26.4 Golden scenes

Golden scenes should combine multiple semantic systems so regressions that only appear in composition are covered.

Keep semantic hashes/counts alongside screenshots to localize failures.

## 26.5 Fuzzing

Priorities:

- cache parsers;
- compressed/archive inputs;
- project/replay parsers;
- patch deserialization if exposed;
- geometry builders with hostile but valid dimensions;
- WFC constraints/configuration.

## 26.6 Concurrency tests

Explicitly test:

- out-of-order worker completion;
- cancellation;
- repeated invalidation of the same zone;
- revision supersession;
- shutdown while jobs are pending;
- region eviction with in-flight work.

## 26.7 Performance tests

Bench semantic and projection layers separately so a renderer regression is not confused with a planner regression.

---

# 27. Automation and MCP

## 27.1 Headless semantic automation first

Before MCP, expose a stable internal/headless automation API for:

- project/source open;
- region loading;
- feature discovery;
- semantic queries;
- selection/query operations;
- edit intent execution;
- undo/redo;
- scene/tile/object inspection;
- camera control;
- render settle;
- screenshot/capture;
- diagnostic bundle generation;
- performance capture;
- parity/reference comparison.

Automation uses the same semantic planners and transaction gateway as the UI.

## 27.2 MCP is a thin adapter

Future flow:

```text
AI client
  -> MCP protocol adapter
  -> headless automation API
  -> normal semantic/query/render boundaries
```

MCP must not own:

- OSRS semantics;
- renderer policy;
- cache logic;
- mutation rules;
- a second tool registry;
- AI-only product behavior.

## 27.3 AI visual control

A future automation API should be able to request:

- deterministic camera placement;
- render settle for a specific semantic/render revision;
- screenshot capture;
- semantic identity under a pixel/viewport point;
- selected-zone diagnostics;
- before/after captures for a patch.

This provides AI with robust inspection without brittle coordinate-only UI clicking.

---

# 28. Security and trust boundaries

Even a desktop editor benefits from clear trust boundaries.

Treat as untrusted input:

- external caches;
- imported projects;
- replay bundles;
- generated AI model files;
- user-provided WFC/config data;
- automation/MCP requests.

Validate sizes, counts, indices, coordinates, versions, and resource limits before allocation or execution.

Automation should expose explicit capabilities rather than arbitrary file/process execution.

---

# 29. Build and CI architecture

The verification ladder should remain cost-aware.

## Fast/cheap gates

- architecture dependency rules;
- authority mutation rules;
- feature registry/index drift;
- generated documentation drift;
- rustfmt;
- source pin checks;
- architecture dashboard ratchets.

## Focused crate gates

During development, compile/test only affected crates and semantic/parity suites.

## Integration gate

Before a broad merge:

- strict native Clippy;
- wasm Clippy for supported crates;
- workspace tests;
- affected parity suites;
- required golden/reference tests;
- architecture/feature gates;
- benchmark checks where stable.

CI must never be used as an iterative compiler when a focused local/faster path is available.

---

# 30. Architecture anti-patterns

The following should be rejected during review unless an ADR explicitly justifies them.

## Semantic/ownership

- second editable map representation;
- second chronological history stack;
- renderer/picker queried as authored truth;
- UI state used to decide semantic validity;
- direct interactive `MapDocument` mutation outside the transaction gateway;
- WFC/AI mutations bypassing history.

## Abstraction

- `common`, `utils`, or `helpers` dumping-ground modules;
- trait-for-everything design;
- service locator/global app context;
- `Arc<Mutex<AppState>>` shared broadly;
- crate proliferation without dependency benefit;
- an ECS rewrite without a demonstrated runtime problem.

## Performance

- packing semantic data before proving range/fidelity;
- one heap allocation per tile/face;
- one GPU buffer per tiny object/zone without evidence;
- giant full-region rebuilds for local edits;
- unbounded worker channels;
- rebuilding stable GPU resources every frame;
- lock-free/custom unsafe structures before profiling;
- optimization claims without benchmark and parity evidence.

## Rendering

- one universal shader with unrelated behavior flags;
- HD rules leaking into semantic domain state;
- discarding priority/alpha/identity metadata during static compilation;
- using screenshot similarity to excuse semantic mismatch;
- assuming RuneLite GPU behavior equals the software client in every path.

## Documentation/AI

- giant root `AGENTS.md` containing every subsystem rule;
- duplicate feature indexes maintained by hand;
- stale plan documents presented as live status;
- MCP-only business logic;
- AI-specific mutation APIs that bypass normal validation.

---

# 31. Recommended evolution from current `main`

The recently completed architecture foundation provides the right semantic base. Continue incrementally.

## Phase A: projection precision and lifecycle

1. audit all `ChangeImpact` consumers;
2. define exact dependency-footprint expansion rules;
3. centralize/coalesce projection scheduling in existing world ownership;
4. ensure every async result carries source revision/generation;
5. add stale-result tests and diagnostics;
6. remove remaining projection compatibility state only when descriptor-driven replacements are proven.

Exit criteria:

- local edits rebuild only required projection areas plus explicit dependency halo;
- no stale worker result can overwrite a newer projection;
- projection reasons are observable.

## Phase B: render-facing immutability and GPU resource lifecycle

1. audit current world-to-render handoff;
2. reuse or establish immutable revisioned render packets;
3. establish paged/suballocated zone GPU resource ownership;
4. add generation-safe resource retirement;
5. remove unnecessary GPU recreation/upload;
6. keep static/dynamic/ordered profile behavior exact.

Exit criteria:

- renderer never reads mutable semantic authority during submission;
- one-zone edits do not trigger unrelated GPU resource recreation;
- resource ownership and retirement are diagnostics-visible.

## Phase C: parity harness hardening

1. strengthen semantic scene hashes/counts;
2. expand pinned compatibility fixtures;
3. provide deterministic camera/viewport/profile captures;
4. add reference-raster validation where it closes real gaps;
5. distinguish exact semantic failures from visual tolerance failures.

Exit criteria:

- each renderer optimization has affected parity proof;
- reference claims have pinned evidence.

## Phase D: performance instrumentation and budgets

1. establish benchmark scene corpus;
2. instrument CPU/GPU frame phases;
3. instrument zone build/upload;
4. measure allocation/retention by memory class;
5. freeze representative baseline hardware/configuration;
6. set regression thresholds from measured noise.

Exit criteria:

- performance decisions are driven by reproducible measurements;
- memory regressions are attributable to a subsystem.

## Phase E: optimize measured bottlenecks

Candidates only after Phase D evidence:

- zone packet SoA packing;
- SIMD transform/normal paths where safe;
- model/texture deduplication improvements;
- pooled worker scratch;
- subrange GPU uploads;
- batch compaction;
- indirect/multi-draw capability path;
- visibility/culling improvements;
- improved source-region streaming/prefetch.

Every optimization retains semantic and visual parity gates.

## Phase F: deterministic reproduction and diagnostics

1. replay bundle format;
2. semantic/project/source identity hashes;
3. trace capture;
4. diagnostic overlay;
5. headless reproduce command;
6. render/picker identity inspection.

## Phase G: headless automation

Expose stable semantic/editor operations without UI driving.

## Phase H: MCP

Add a thin protocol adapter only after the headless automation contract is stable.

---

# 32. Architecture acceptance checklist

A significant new feature or subsystem should answer **yes** to the relevant questions below.

## Authority

- Is its authoritative state owner explicit?
- Can its projections be deleted without losing authored state?
- Does it use the canonical history/revision system?
- Does it avoid a competing coordinate/object/document model?

## Fidelity

- Which semantic spec governs it?
- What pinned source/evidence supports behavior?
- Are compatibility quirks fixtures rather than comments only?
- Does optimization preserve Reference profile output?

## Performance

- Is the hot path identified?
- Are allocations and copies understood?
- Is invalidation no broader than correctness requires?
- Is work bounded/coalesced?
- Is there a benchmark before a specialized optimization is added?

## Memory

- What data is authoritative, cached, reconstructible, or evictable?
- What is its retention key/lifetime?
- Does it duplicate an immutable asset?
- Can stale generations be reclaimed?

## Concurrency

- Are worker inputs immutable?
- Does every result carry revision/generation identity?
- Can work be cancelled/superseded?
- Can completion order change final semantics?

## Maintainability

- Is there one obvious owner?
- Can a new contributor discover it through the registry/docs?
- Is the API smaller than the implementation?
- Are invariants encoded by types/private fields where practical?
- Is a trait/crate actually justified?

## Debuggability

- Can the system explain why work happened?
- Can a failure be tied to source revision/semantic revision?
- Can the issue be reproduced headlessly?
- Are diagnostics observational rather than authoritative?

## AI/automation

- Does automation call the same semantic operation as the UI?
- Is the operation identified by a stable feature ID?
- Can an AI inspect the result semantically, not only visually?
- Is there no AI-only mutation escape hatch?

---

# 33. What "best in class" means for RustOSRS

RustOSRS should not try to win by maximizing cleverness.

It should win because the architecture makes good properties reinforce each other:

- **fidelity** is protected by semantic authority, pinned evidence, and parity fixtures;
- **speed** comes from precise invalidation, zone compilation, deduplicated assets, bounded concurrency, and GPU resource reuse;
- **memory efficiency** comes from one semantic truth plus evictable derived projections;
- **undo correctness** comes from reversible transactional patches;
- **debuggability** comes from deterministic revision identity and replay bundles;
- **maintainability** comes from narrow owners and enforced dependency direction;
- **extensibility** comes from typed intents and pure planners using the same gateway;
- **AI capability** comes from a semantic headless automation layer, not brittle UI imitation;
- **future optimization** remains safe because every derived layer can be replaced without changing authored truth.

The editor should be capable of becoming extremely fast without becoming extremely fragile.

That is the central engineering standard.

---

# 34. One-sentence architecture

> **RustOSRS is a deterministic transactional OSRS authoring core (`intent -> planner -> reversible patch -> EditorSession revision`) surrounded by disposable revisioned scene, query, and zone-compiled GPU projections, with fidelity proven by pinned reference evidence and performance improved only through measured, parity-protected optimization.**
