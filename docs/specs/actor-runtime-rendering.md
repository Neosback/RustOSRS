# Actor, Entity Effect, and Interface Runtime Specifications

Primary target: `melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`

Primary audited sources: `Actor.java`, `Player.java`, `PlayerComposition.java`, `NPC.java`, `NPCComposition.java`, `SequenceDefinition.java`, `SpotAnimationDefinition.java`, `GraphicsObject.java`, `Projectile.java`, `Widget.java`, `Interpreter.java`, and `Script.java` from the checked-in `RuneLite-melxin` mirror.

Revision note: this source declares game revision 235 while RustOSRS currently targets cache revision 241. These contracts therefore preserve the audited ownership/data-flow behavior without treating absence from the older source as proof that newer decoder fields do not exist.

## SPEC: ACTOR-RUNTIME-001

**Domain:** `OSRS_SEMANTIC`  
**Status:** `DERIVED`; exact packet-update differential fixtures **REQUIRED** before network/update parity is claimed  
**Applies to:** player/NPC live actor state and sequence progression  
**Does not apply to:** packet encryption/network transport, full combat/economy, renderer frame timing

### Required behavior

Live actor state owns movement, orientation/rotation/targeting, movement/action sequence state, path traversal, exact-movement timing, overhead/combat-display state, temporary override windows and actor-attached spot animations.

Player and NPC model selection consumes that runtime state. The renderer does not advance actor paths or animation clocks and does not infer them from GPU state.

The player/NPC update decoder, when implemented, must mutate the owning runtime state through a deterministic tick/update boundary. The exact packet-mask dispatcher is not yet pinned by this specification and must not be guessed from field names.

### Invariants

- one deterministic runtime clock is authoritative for actor progression;
- render cadence cannot change sequence or movement semantics;
- replaying identical initial state plus updates/ticks produces identical actor state;
- source/revision changes invalidate stale runtime projections;
- `MapDocument` remains map-editing truth and does not become live actor state.

### Failure signature

Animation speed changes with FPS, actors move differently between headless and rendered runs, effects start/end on different ticks, or renderer state becomes necessary to reconstruct simulation state.

### Required tests

- deterministic movement/path replay;
- action and movement sequence progression independent of render rate;
- exact-movement start/end boundaries;
- delayed actor spot start/removal;
- pinned packet-update mask fixtures once the dispatcher is identified.

### Related specs

`ACTOR-MODEL-001`, `ENTITY-EFFECT-001`, `MORPH-001`.

---

## SPEC: ACTOR-MODEL-001

**Domain:** `OSRS_SEMANTIC`  
**Status:** `DERIVED`; player/NPC appearance and dual-sequence differential fixtures **REQUIRED**  
**Primary evidence:** `Player.getModel`, `PlayerComposition.getModel`, `NPC.getModel`, `NPCComposition.getModel`, `SequenceDefinition` transform methods

### Required player behavior

Player model assembly must preserve this ownership/order:

1. resolve player appearance or return no model when appearance is unavailable;
2. select active action and movement sequences from actor runtime state;
3. obtain the reusable base appearance model from player composition state;
4. apply one/two sequence transforms to a working/posed model without corrupting the cached base appearance;
5. combine active actor spot-animation models;
6. combine any active temporary attached player model using its runtime cycle/world/orientation state;
7. apply active temporary colour/model overrides;
8. publish the resolved render instance.

`PlayerComposition` base-cache identity is appearance/equipment/customization state, not the current animation frame. Sequence transforms occur after the reusable base model is selected/assembled.

### Required NPC behavior

NPC model assembly must:

1. resolve the active NPC definition from runtime varbit/varp context when the definition has transforms;
2. select action/movement sequence state from the Actor runtime;
3. obtain/construct the reusable base NPC model;
4. apply current sequence pose without mutating the shared base;
5. apply configured scale/instance behavior in the audited model path;
6. combine actor spot models and temporary runtime overrides;
7. publish the resolved render instance.

### Animation-system requirement

The target source contains both legacy frame animation and cached-model/skeletal animation paths. Legacy M8 parity does not certify cached-model/skeletal player/NPC parity.

A complete implementation must explicitly preserve decoder metadata, progression, masks/blending and pose behavior for both paths or mark the unsupported path as an acceptance blocker.

### Invariants

- no posed frame may mutate a shared canonical base model;
- player appearance cache identity is independent of current frame clocks;
- NPC var state is resolved outside `osrs-render`;
- no renderer fallback silently chooses another player/NPC model when semantic model resolution yields none.

### Failure signature

All players sharing an appearance inherit one actor's pose, equipment changes fail to invalidate appearance, NPC morphs ignore vars, dual animations corrupt each other, or cached/skeletal sequences render static while legacy sequences animate.

### Required tests

- player appearance cache reuse with two different poses;
- equipment/customization invalidation;
- one and two simultaneous actor sequences;
- NPC varbit/varp transform and fallback/null cases;
- actor spot composition on player and NPC;
- temporary player attached-model cycle/orientation case;
- cached-model/skeletal decoder and pose fixtures.

### Related specs

`ACTOR-RUNTIME-001`, `MORPH-001`, `ENTITY-EFFECT-001`, `MODEL-BUILD-005`.

---

## SPEC: ENTITY-EFFECT-001

**Domain:** `OSRS_SEMANTIC`  
**Status:** `DERIVED`; deterministic effect/projectile fixtures **REQUIRED**  
**Primary evidence:** `Actor` spot-animation methods, `SpotAnimationDefinition`, `GraphicsObject`, `Projectile`

### Shared spot-animation model path

Actor attachments, standalone graphics objects and projectiles must share the same spot-animation definition/model semantics rather than maintaining three divergent model pipelines.

A spot-animation definition owns reusable model/sequence/scaling/orientation/recolour/retexture information. Runtime instances own their start/lifetime/current-frame/position state.

### Actor attachments

An actor may hold multiple keyed spot-animation attachments. Update replaces the corresponding keyed attachment and may schedule a delayed start. Model assembly combines active spot models with the actor model using each attachment's vertical offset.

### Graphics objects

A graphics object owns its position, start cycle, sequence/frame state and finished state. Runtime tick progression advances its sequence and completion. Rendering consumes the current model/transform only.

### Projectiles

Projectile runtime owns source/destination timing, current position, velocity/acceleration, target identity, yaw/pitch and sequence state.

Destination setup and tick advancement determine the trajectory and orientation. The renderer must not integrate projectile physics or use render-frame delta as the authoritative simulation step.

### Invariants

- identical simulation ticks produce identical projectile/effect state independent of rendering;
- spot base models remain reusable while animation poses are per instance;
- actor attachment, graphics-object and projectile model lookup share the canonical spot-definition path;
- delayed effects are not rendered before their runtime start condition;
- completed graphics objects/projectiles cannot remain alive merely because a renderer still holds an old GPU resource.

### Failure signature

Projectiles travel differently at different FPS, spot effects use inconsistent recolours/scales depending on owner type, delayed effects start early, or an actor animation mutates the spot-model cache.

### Required tests

- spot definition decode/model transform fixture;
- delayed actor attachment lifecycle;
- standalone graphics-object legacy and cached-model sequence progression;
- deterministic projectile destination/advance/orientation case;
- shared-source immutability across two simultaneous spot instances.

### Related specs

`ACTOR-RUNTIME-001`, `ACTOR-MODEL-001`.

---

## SPEC: UI-CS2-001

**Domain:** `OSRS_SEMANTIC`  
**Status:** `DERIVED`; exact opcode-routing and interface-raster fixtures **REQUIRED**  
**Primary evidence:** `Script`, `Interpreter`, `Widget`, `SequenceDefinition.transformWidgetModel`

### Required ownership split

CS2 execution and interface rendering are separate concerns.

- `Script` is parsed executable data: opcodes, operands, locals/arguments and switch tables.
- `Interpreter` owns VM stacks, frames, arrays and active-widget references.
- opcode handlers/client logic query or mutate widget/client UI state.
- `Widget` owns declarative/runtime interface state such as geometry, scrolling, colour/fill/transparency, sprite/text/model state, item state, sequence/frame state, actions and listeners.
- interface rendering consumes resolved widget state and resources.

`Widget.getModel(...)` may resolve raw models, NPC/player composition models, items and other supported model sources, cache a reusable base model, and apply widget sequence transforms. That model resolution is not evidence that the CS2 interpreter itself rasterizes widgets.

### Renderer boundary

Neither the scene renderer nor an eventual interface renderer may become the authority for:

- CS2 VM stacks/frames;
- widget business state or event execution;
- actor/NPC var state;
- map editing history.

The exact recursive interface traversal/raster helper and complete CS2 opcode-dispatch table are not yet pinned in this checkpoint. Exact interface/CS2 parity remains blocked until those anchors and executable fixtures exist.

### Invariants

- executing a script can be tested without a GPU;
- interface layout/state can be snapshotted before rasterization;
- rendering the same resolved widget tree twice does not mutate semantic widget/script state merely to produce pixels;
- widget model animation uses runtime sequence state but does not transfer that clock to the renderer.

### Failure signature

UI behavior changes with renderer backend/FPS, scripts cannot run headlessly, rendering a widget advances its animation twice, or scene-renderer code starts owning CS2 stacks/listeners.

### Required tests

- pinned script decode/execution fixture;
- widget state mutation/event fixture;
- widget model-source and sequence-transform fixture;
- headless widget-tree snapshot fixture;
- independent interface raster fixture once the recursive draw path is pinned.

### Related specs

`ACTOR-MODEL-001` and the project renderer/UI architecture documents.

---

## RustOSRS implementation boundary

These specs define semantic/runtime ownership only. `osrs-render` remains a disposable projection that consumes already resolved instances. A future deterministic `osrs-playtest` runtime is the natural owner for actor/effect clocks, movement, vars and projectile simulation. CS2/widget runtime should remain a separate UI execution/state subsystem rather than being embedded in `MapDocument` or the GPU renderer.
