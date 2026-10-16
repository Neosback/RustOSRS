# Source-driven priorities for an OSRS world editor

Reviewed **2026-10-16** against RustOSRS `fe1b703cf06e472b26bee68f2bd26cff2684dbe0` on `main`.
Status: **product recommendations and source review; not an implementation or parity certificate**.

## Recommendation

Build an editor in which an author can **construct a place, explain every visible or gameplay-relevant
result, test its states, and export a reproducible result**. That is a stronger differentiator than
adding another collection of brushes. It also gives AI contributors concrete contracts and examples.

RustOSRS already has the difficult foundation: authoritative semantic editing, undo/redo, scene
construction, streaming, Reference/HD rendering, world-map display and verified map export. Keep it.
The largest gaps are the workflows connecting those capabilities, plus runtime and asset-authoring
systems that do not yet exist end to end.

The rankings below are my judgment about author value and dependency order, inferred from the local
sources. They are not evidence about Jagex's private tools or requirements. No internal Jagex tooling
was inspected. The supplied RuneLite mirror declares game revision **235**; our selected cache is
**241**. Its runtime structure is useful evidence, but newer codecs need target-revision fixtures.

| Rank | Capability | What an author gains | Relative scope |
|---|---|---|---|
| 1 | Explainable world inspector and validation | Answer “why does this look or behave this way?” directly from a selection | Medium, delivered in slices |
| 2 | Collision, reachability and stateful offline playtest | Test whether the designed place actually works | Large; extend current actor work |
| 3 | Complete asset discovery and a safe definition workbench | Find the right content and make controlled variations | Medium discovery; large authoring |
| 4 | Reusable, OSRS-aware construction tools | Build rooms, walls, bridges and repeated structures accurately | Medium to large |
| 5 | Reproducible content builds and complete map publication | Deliver the same authored world outside the editor | Large; foundations already exist |
| 6 | Authored world-map, environment and scenario layers | Keep navigation, atmosphere and test content synchronized with the map | Large, separate deliverables |
| 7 | Recovery, semantic review and revision migration | Trust long sessions and cache updates | Medium recovery; large migration |
| 8 | Fidelity and performance evidence integrated into authoring | Know that a change preserves appearance and remains practical at scale | Continuous quality track |

Rank is not a requirement to finish one whole row before starting the next. Recovery belongs early.
The existing performance program is explicitly paused in its current plan; this review does not
resume measurement or optimization. See [the authorized program](../plans/PERFORMANCE-UI-PLAYTEST-HD.md).

## 1. Explainable world inspector and validation

**Build on the existing inspector and edit/rebuild trace.** The current object card shows identity,
position, footprint, facing, type, plane and draw level, with raw text below. The source has enough
structure to explain much more: the selected definition, model selection by placement type, morph
selection, transform order, contouring and lighting. Rust already models many of these rules.
Evidence: S01–S04, S11, S14–S16.

Selecting a tile or object should expose a navigable explanation:

- Original placement → varbit/varp selector → effective definition, including a null morph.
- Requested loc type → chosen model IDs and mirrored state → rotation, recolor/retexture, scale,
  translation, pose and contouring. Show a legitimate no-model result without inventing geometry.
- Source plane, height/render level, collision plane, storage plane and linked-below relationship.
- Movement/projectile blocking contributions, sound footprint, map-scene sprite and world-map icon.
- Source cache/profile and bytes, current project edit, semantic revision and last rebuild outcome.

Add focused views for collision edges, render flags, height discontinuities, normals/seams, shadow
contributors and missing dependencies. A validation result should select its location and state
which rule failed. Distinguish unsupported data from malformed data and ordinary invisible content.
Do not classify every overlapping object or height step as an error; many are intentional.

This is useful beyond debugging: authors learn which wall type and definition to use, while an AI
can report a reproducible chain instead of guessing from a screenshot. Explanations should come from
existing planners/build outputs or read-only semantic queries, not a second copy of placement rules.

**First acceptance slice:** explain one static loc, one two-arm boundary, one null morph and one bridge
tile. Selecting an explanation highlights the actual producer. Undo and rebuild update the explanation;
source replacement clears it. Diagnostic capture is optional and bounded.

## 2. Collision, reachability and stateful offline playtest

**The biggest functional step is to let authors test the space they build.** RuneLite's SceneOverlay
already distinguishes valid movement and line of sight. OpenRune's routefinder separates step
validation, line-of-walk/line-of-sight and interaction reach, including actor size, loc shape, orientation
and blocked approach sides. These are different questions, not one “walkable” boolean. Evidence:
S05–S09. OpenRune is an implementation/design reference, not independent proof of vanilla client parity.

RustOSRS has placement collision-side-effect plans and bridge-plane rules. Its world loc stage
explicitly does not yet materialize collision maps or sound registration. The Player panel is a mock
state holder; loc morph selection currently uses default-zero vars. New core actor sequence-slot
and spot/projectile transition code is present, so “actors have not started” would be incorrect.
The playable scene, target entity decoding/model assembly and editor integration are still separate
work. Evidence: S14–S20 and the [actor runtime contract](../specs/actor-runtime-rendering.md).

Deliver the feature in useful stages:

1. An immutable collision snapshot derived from the same semantic placement and terrain rules, with
   per-edge provenance. Click a destination to explain a blocked step, diagonal corner or approach side.
2. A controllable player and size-aware NPC route tests across walls, bridges and region boundaries.
   “Can stand here,” “can walk there,” “can see it” and “can interact from here” remain separate tests.
3. A variable/state panel: named varbits/varps, before/after values and a morph dependency view. Preview
   an open/closed door, depleted resource or quest-state object by explicit state changes. A varbit
   edits its backing varp bits; overlapping aliases cannot become independent contradictory values.
4. A deterministic scenario timeline: pause, step, restart, seeded replay; actor appearance and animation,
   graphics objects, projectiles, attached effects and light/sound lifecycle.

RuneLite's VarInspector provides a concrete example of tracing named before/after variable changes
and accounting for shared backing bits (S12). Retain that clarity in the editor's state controls.

Do not infer server actions from an object name or “Open” menu option. Cache morph definitions do not
contain complete gameplay scripts. Scenarios explicitly supply replacement/movement/variable commands.
Likewise, do not assume every visual morph changes collision: preserve the audited initial-placement
and replacement rules separately. Runtime overlays never silently commit map edits.

Keep the planned `osrs-playtest` owner headless. It consumes immutable snapshots plus typed commands;
it produces entity/event snapshots for extraction. Existing pure core transitions remain reusable.
Map edits pause and revalidate; source generation/revision rejects stale output. Client animation
cycles, scenario simulation steps and millisecond audio fades need explicit clock domains.

**Acceptance:** deterministic routes and effect lifetimes; size-2 and diagonal blockers; oriented
interaction tests; bridge-plane fixtures; null-morph disappearance and restoration; no orphan sounds
or lights after despawn/restart; editing never loses document state. Replay identifies its cache,
scenario version and seed. Legacy animation tests do not certify skeletal posing.

## 3. Complete asset discovery and a safe definition workbench

The current catalog searches names/IDs and only includes named definitions with a model. That is a
convenient scenery palette, but it is not a complete content browser: unnamed definitions, morph
shells and model-less content need an expert inspection path. A definition that does not draw in the
current state can still matter. Evidence: S01–S03, S21–S22.

Start with discovery: thumbnail and isolated turntable, placement-type variants, footprint, animation,
sound, category, map icon, texture/material, official symbol and current numeric ID. Add favorites,
recent choices and themed author palettes. Allow “find usages” from a model, definition, sequence,
sound or sprite back to locations and dependent definitions. World-wide usage results must declare
their indexed cache/scope; searching only loaded regions cannot claim completeness.

Then add controlled definition variations: duplicate under a project symbol; edit recolors/retextures,
model references, footprint and flags; compare base/effective output side by side. Preview a new torch
in daylight, darkness and its animated states before replacing placements. Keep Reference recolors
and retextures distinct from 117HD material overrides.

OpenRune contributes useful named-reference ideas: revision-specific official ID reservations,
custom ID allocation and indexes that relink typed references after names resolve to different IDs.
Do not copy its hardcoded ID ceilings or assume every cache integer is a reference. Names help, but
they are not guaranteed stable across revisions. Evidence: S23–S24.

**Safety prerequisite:** our decoder retains useful fields but consumes/skips others; it is not a
lossless general-purpose definition writer. Preserve original bytes for untouched definitions. Reject
editing unsupported encodings unless a validated representation can round-trip the entire record.
Unknown opcodes cannot generally be preserved by guessing their length. General asset editing needs
an explicit ownership/transaction ADR; do not make widgets or renderer overrides its authority, or
sneak asset definitions into the map document without a contract.

**Acceptance:** discover a model-less morph shell; show each valid loc-type model choice; follow one
model to its usages; clone an asset without altering its original; detect ID collisions; round-trip
supported definitions; preserve unchanged bytes; preview/commit/cancel/undo through the owning gateway.

## 4. Reusable, OSRS-aware construction tools

Move from individual placements to **multi-layer selection and reusable assemblies**: a shop front,
wall with corners and decorations, bridge, room, cave entrance or ruin. Existing terrain clipboard,
object multi-selection and pure edit planners are starting points. RuneLite's instance-coordinate
mapping shows why source chunk, destination chunk, plane and rotation must remain explicit; a source
chunk can appear more than once. Evidence: S10, S25 and [the feature map](../architecture/FEATURE-MAP.md).

Useful author tools include wall/fence chains with correct corners, door/window sockets, room outlines,
stair/bridge templates, themed terrain palettes and seeded scatter with masks and exclusion rules.
Extend the existing path tool for shared authoring workflows; do not replace its audited fitter casually.

A prefab should capture terrain channels, vertex heights/relative anchor, locs, planes and optional
project-layer annotations. Preview rotation/translation with an explicit absolute-height or
terrain-relative policy. Show conflicts before committing. Overlay shape rotation, vertex-grid
transforms, object footprints and wall-decoration offsets need real semantic transforms; rotating
the visible mesh alone is insufficient. Mirror only when its semantic mapping is supported.

Initially instantiate a prefab as ordinary edits, with provenance metadata. Linked instances and
propagating overrides can follow once conflict/update behavior is defined. An instance-layout authoring
mode is a distinct later feature; an 8×8 rebuild zone is not itself an authored prefab format.

**Acceptance:** rotate a multi-plane fixture four times to recover its exact semantics; place it across
a region boundary; preview/cancel without mutation; commit the assembly once and undo exactly. Seeded
scatter regenerates the same plan and reports every excluded/conflicting placement.

## 5. Reproducible content builds and complete map publication

Treat authored sources as a build with named inputs, dependencies and outputs. OpenRune records cache
reads/writes, source/shared-input hashes and output CRCs; it propagates dirty dependencies and explains
why a unit rebuilds. Its pack modules collect config/model/sprite and other content contributions.
Evidence: S26–S29. Adopt those ideas behind our existing export boundary rather than transplanting its
mutable cache/task framework or immediately adding a database.

RustOSRS already stages, validates, repacks changed existing map groups and independently verifies
output. The current preparation path explicitly rejects missing regions; the status audit also records
map-index-only publication. Expanding this is how an editor grows beyond modifying an existing town.
Evidence: S30 and [the export audit](../PROJECT-STATUS.md#24-verified-export-and-inspection).

Add a build preview: authored changes → dependent groups → conversions/unsupported cases → exact
output manifest. Group ownership conflicts are errors, rather than accidental “last writer wins.”
Keys include cache identity, profile, tool/codec version, settings and source hashes. A clean build
and an incremental build must agree logically; untouched encoded groups remain byte-identical.
Deletion may prune only outputs demonstrably owned by that source and unclaimed elsewhere.

Expand in stages: strict raw metadata and resource bounds; new map groups/regions and their names;
supported asset-definition groups; derived world-map products. Retain staging and fail-closed
publication. SQLite build-state transactions do not make physical cache writes crash-atomic.
Existing [filestore findings](RUNELITE-FILESTORE-NEEDS.md) remain prerequisite work, not feature semantics
to put inside `osrs-filestore`.

**Acceptance:** an unchanged build writes no changed groups; a changed shared model rebuilds exactly
its declared dependents or conservatively falls back; tampering invalidates reuse; a failed task
publishes nothing; a new region reopens in an independent reader. Test sparse IDs, additional files,
reference-table variants and address/version limits. Preserve explicit unsupported cases.

## 6. Authored world-map, environment and scenario layers

**Make all views tell the same story.** The existing world map reads cached area/geography/ground
products, while map export edits terrain/loc data. That is not yet a promise that an authored village
will update every navigation product. OpenRune's WorldMapPacker explicitly builds/updates areas,
geography and image output, including dirty source map squares. Evidence: S31–S32.

Add live map/minimap previews from the authored snapshot, then supported rebuild/export of geography,
ground colours, area sections, labels, map-scene decorations and icon references. Clearly label
source-derived versus current authored previews. Morph-sensitive icons share the effective state used
by the scene, with target-client rules preserved rather than “first non-null transform” guessing.

Provide separate authorable layers for scenario NPC spawns/routes, ground items, area polygons and
test triggers; later atmosphere/lighting/audio regions. OpenRune supplies concrete NPC/ground-item
spawn files and polygons with levels/includes/excludes. Its `loc` means scenery; its `obj` means a
ground item, not an OSRS scenery definition. These server-specific formats are optional adapters,
not the vanilla map format. Evidence: S33–S35.

For audio, start with listening to selected object sounds and visualizing the actual attenuation/fade
state, then support moving-listener scenarios. Current sound metadata/range inspection exists;
playback does not. Music regions and gameplay triggers require separate evidence/adapters, rather
than assuming an object-definition opcode provides them. Evidence: S04, S20.

**Acceptance:** edit a village across a map-square boundary and see matching 3D/map/icon results;
export and independently reopen the supported products. Scenario layers remain versioned separately
from map data. Source or state changes update icons, sound and environment dependencies coherently.

## 7. Recovery, semantic review and revision migration

Project save/open exists. Settings autosave is not map recovery. A professional authoring tool needs
recoverable project checkpoints, clear dirty state and a reviewable semantic change set before risky
operations. This is a product recommendation; it is not a feature inferred from the OpenRune server.

Add an autosave/recovery journal with source identity, atomic replacement, bounded retention and a
recovery browser. Summarize changes by region and type, with selectable before/after views and export
conversion warnings. Never present an older asynchronous save as containing edits made after its snapshot.

Later add cache-update assistance: compare definitions/assets across explicit profiles, flag changed
models/opcodes/missing references and present relocation decisions. Use symbols, typed dependency
indexes and provenance to assist migration; never silently substitute the same numeric ID from a
different revision or guess a rename. OpenRune's gameval index/reservations are useful foundations
for this workflow, not a complete revision migration solution. Evidence: S23–S24.

**Acceptance:** interrupt a save and recover the last verified checkpoint; reject mismatched source
identity; preserve unloaded-region edits; show a semantic diff whose replay reproduces the document.
A migration report separates proven matches, ambiguous candidates and unsupported records.

## 8. Fidelity and performance evidence integrated into authoring

Keep Reference independent and complete 117HD against its pinned feature ledger, including entities,
light lifecycle/overflow, sky/time/environment/reflections and displacement/material special cases.
Use the playtest scenarios above to exercise moving actors/effects; static scenery screenshots cannot
verify those systems. Existing manifests and rendering foundations do not mean HD is complete.

Saved camera bookmarks, variable states, clocks/seeds and scenarios should become reproducible scene
tests. Include walls/corners, bridges, interiors, transparent priorities, water, sound transitions,
large actors and boundary travel. Compare incremental/full rebuild semantics, independent pinned
client captures and exported-cache behavior. A self-generated golden can detect regressions but cannot
establish independent OSRS/117HD parity. Restore missing oracle prerequisites before claiming their gates.

When performance work resumes, reuse current capture/resource accounting rather than adding another
dashboard. Measure unchanged quality and view distance, cold/warm travel and edit/playtest workloads.
Report CPU, GPU execution, worker latency, allocations, peak/retained memory and resource lifetimes;
unavailable counters remain unavailable. Do not add process RSS and GPU allocations on unified memory.
No optimization or performance result is claimed by this review.

**Acceptance:** release evidence identifies commit/cache/backend/settings/viewport and raw samples;
optimization-only captures preserve the fixed-camera output; repeated travel/source replacement
shows bounded retained resources. Any unsupported visual behavior or missing external capture remains
an open item. UI checks include docked/floating panels at 240/320/480 logical points and display scales.

## First useful delivery and implementation order

Continue the current runtime path with a **small playable validation scene**: a bridge, wall corner,
door and torch, with one player and an NPC route. Explain collision and effective definitions; change
vars or explicit scenario replacements; pause/step/restart; inspect animation/sound/light state as each
system becomes available. Record which behaviors are implemented and which still need target evidence.
Do not make completion of all audio, HD and skeletal systems a prerequisite for the first movement tool.

Then develop asset discovery and build hardening in narrow checkpoints, followed by supported definition
authoring/new-region publication, prefab construction and synchronized map/world layers. Add recovery
early. Keep the current runtime roadmap authoritative; these are proposed follow-on priorities, not a
new authorization to implement every row.

A compelling eventual demonstration is: build a bridge-side shop from a saved assembly, select a door
and explain its definition/collision, walk an actor through both states, listen to its torch, verify
the map icon, review the changed assets and export a reproducible cache. Every layer has one owner,
and every committed map edit still uses the shared planner/transaction path.

Defer full server simulation, networking/accounts, full combat/economy, a general CS2/widget IDE,
multiplayer editing and a new ECS/rendering rewrite. None is needed to prove this authoring loop.

## Evidence and limits

The companion [source manifest](editor-opportunity-sources.json) pins the files and anchors below.
Earlier detailed studies remain useful: [objects](RUNELITE-OBJECT-SYSTEM.md),
[actors/effects](RUNELITE-RUNTIME-RENDERING.md), [filestore needs](RUNELITE-FILESTORE-NEEDS.md) and
[OpenRune packing](OPENRUNE-CACHE.md). This review inspected selected code paths, not every plugin,
server script or opcode. It ran no client, audio, rendering or end-to-end playtest parity test.
No third-party implementation or assets were copied into production.

| Evidence | Source anchor |
|---|---|
| S01 | [rust: inspector.rs](</Users/tylercovalt/Documents/RustOSRS/crates/osrs-editor/src/app/inspector.rs:234>) |
| S02 | [rl: ObjectComposition.java](</Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java/ObjectComposition.java:917>) |
| S03 | [rl: DynamicObject.java](</Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java/DynamicObject.java:9>) |
| S04 | [rl: ObjectSound.java](</Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java/ObjectSound.java:12>) |
| S05 | [rl: SceneOverlay.java](</Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runelite-client/src/main/java/net/runelite/client/plugins/devtools/SceneOverlay.java:89>) |
| S06 | [rl: CollisionMap.java](</Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java/CollisionMap.java:9>) |
| S07 | [server: StepValidator.kt](</Users/tylercovalt/Desktop/OpenRune/OpenRune-Server-241/engine/routefinder/src/main/kotlin/org/rsmod/routefinder/StepValidator.kt:20>) |
| S08 | [server: LineValidator.kt](</Users/tylercovalt/Desktop/OpenRune/OpenRune-Server-241/engine/routefinder/src/main/kotlin/org/rsmod/routefinder/LineValidator.kt:42>) |
| S09 | [server: ReachStrategy.kt](</Users/tylercovalt/Desktop/OpenRune/OpenRune-Server-241/engine/routefinder/src/main/kotlin/org/rsmod/routefinder/reach/ReachStrategy.kt:34>) |
| S10 | [rl: WorldPoint.java](</Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runelite-api/src/main/java/net/runelite/api/coords/WorldPoint.java:293>) |
| S11 | [rl: FriendSystem.java](</Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java/FriendSystem.java:358>) |
| S12 | [rl: VarInspector.java](</Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runelite-client/src/main/java/net/runelite/client/plugins/devtools/VarInspector.java:206>) |
| S13 | [rust: definitions.rs](</Users/tylercovalt/Documents/RustOSRS/crates/osrs-core/src/definitions.rs:198>) |
| S14 | [rust: side_effects.rs](</Users/tylercovalt/Documents/RustOSRS/crates/osrs-scene/src/side_effects.rs:47>) |
| S15 | [rust: planes.rs](</Users/tylercovalt/Documents/RustOSRS/crates/osrs-scene/src/planes.rs:32>) |
| S16 | [rust: loc_stage.rs](</Users/tylercovalt/Documents/RustOSRS/crates/osrs-world/src/loc_stage.rs:327>) |
| S17 | [rust: entity.rs](</Users/tylercovalt/Documents/RustOSRS/crates/osrs-core/src/entity.rs:40>) |
| S18 | [rust: mod.rs](</Users/tylercovalt/Documents/RustOSRS/crates/osrs-core/src/entity_effect/mod.rs:65>) |
| S19 | [rust: mod.rs](</Users/tylercovalt/Documents/RustOSRS/crates/osrs-editor/src/player/mod.rs:76>) |
| S20 | [rust: mod.rs](</Users/tylercovalt/Documents/RustOSRS/crates/osrs-editor/src/sound/mod.rs:122>) |
| S21 | [rust: object_catalog.rs](</Users/tylercovalt/Documents/RustOSRS/crates/osrs-world/src/object_catalog.rs:3>) |
| S22 | [rust: object.rs](</Users/tylercovalt/Documents/RustOSRS/crates/osrs-cache/src/decode/object.rs:19>) |
| S23 | [server: GameValMaxIdManifest.kt](</Users/tylercovalt/Desktop/OpenRune/OpenRune-Server-241/or-cache/src/main/kotlin/dev/openrune/gamevals/GameValMaxIdManifest.kt:16>) |
| S24 | [fs: GameValReferenceIndex.kt](</Users/tylercovalt/Desktop/OpenRune/OpenRune-FileStore-main/tools/src/main/kotlin/dev/openrune/cache/tools/gameval/GameValReferenceIndex.kt:36>) |
| S25 | [rust: tile_clipboard.rs](</Users/tylercovalt/Documents/RustOSRS/crates/osrs-editor/src/app/tile_clipboard.rs:18>) |
| S26 | [fs: RecordingCache.kt](</Users/tylercovalt/Desktop/OpenRune/OpenRune-FileStore-main/tools/src/main/kotlin/dev/openrune/cache/tools/incremental/RecordingCache.kt:7>) |
| S27 | [fs: IncrementalBuild.kt](</Users/tylercovalt/Desktop/OpenRune/OpenRune-FileStore-main/tools/src/main/kotlin/dev/openrune/cache/tools/incremental/IncrementalBuild.kt:211>) |
| S28 | [fs: IncrementalSession.kt](</Users/tylercovalt/Desktop/OpenRune/OpenRune-FileStore-main/tools/src/main/kotlin/dev/openrune/cache/tools/incremental/IncrementalSession.kt:78>) |
| S29 | [server: PluginPacks.kt](</Users/tylercovalt/Desktop/OpenRune/OpenRune-Server-241/or-cache/src/main/kotlin/dev/openrune/pack/PluginPacks.kt:52>) |
| S30 | [rust: prepare.rs](</Users/tylercovalt/Documents/RustOSRS/crates/osrs-editor/src/export_flow/prepare.rs:45>) |
| S31 | [rust: mod.rs](</Users/tylercovalt/Documents/RustOSRS/crates/osrs-world/src/worldmap/mod.rs:1>) |
| S32 | [fs: WorldMapPacker.kt](</Users/tylercovalt/Desktop/OpenRune/OpenRune-FileStore-main/tools/src/main/kotlin/dev/openrune/cache/tools/WorldMapPacker.kt:124>) |
| S33 | [server: MapNpcPacker.kt](</Users/tylercovalt/Desktop/OpenRune/OpenRune-Server-241/or-cache/src/main/kotlin/dev/openrune/map/packing/MapNpcPacker.kt:22>) |
| S34 | [server: MapObjPacker.kt](</Users/tylercovalt/Desktop/OpenRune/OpenRune-Server-241/or-cache/src/main/kotlin/dev/openrune/map/packing/MapObjPacker.kt:23>) |
| S35 | [server: MapAreaPacker.kt](</Users/tylercovalt/Desktop/OpenRune/OpenRune-Server-241/or-cache/src/main/kotlin/dev/openrune/map/packing/MapAreaPacker.kt:90>) |
| S36 | [rust: animation.rs](</Users/tylercovalt/Documents/RustOSRS/crates/osrs-world/src/animation.rs:8>) |
| S37 | [rust: project.rs](</Users/tylercovalt/Documents/RustOSRS/crates/osrs-editor/src/app/project.rs:114>) |
| S38 | [rust: projectile.rs](</Users/tylercovalt/Documents/RustOSRS/crates/osrs-core/src/entity_effect/projectile.rs:28>) |
| S39 | [rust: spot.rs](</Users/tylercovalt/Documents/RustOSRS/crates/osrs-core/src/entity_effect/spot.rs:87>) |
| S40 | [rust: reference_table.rs](</Users/tylercovalt/Documents/RustOSRS/crates/osrs-cache/src/encode/reference_table.rs:125>) |
| S41 | [rust: index.rs](</Users/tylercovalt/Documents/RustOSRS/crates/osrs-filestore/src/index.rs:229>) |
