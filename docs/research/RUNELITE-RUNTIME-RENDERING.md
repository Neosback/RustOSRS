# RuneLite Actor, Animation, Projectile, and Interface Runtime Study

Status: **source study / implementation input, not a parity certificate**

Primary source target:

`melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`

Checked-in mirror used for this walkthrough:

`RuneLite-melxin/runescape-client/src/main/java/`

The pinned source declares game revision 235 while the selected RustOSRS cache is revision 241. These findings are therefore authoritative for the audited ownership/data-flow relationships and for behavior already promoted into canonical specs, but they must not be used to delete or narrow newer decoder fields solely because the older source does not contain them.

## Why this study exists

RustOSRS already has strong map, loc, model, legacy-animation, and renderer foundations, but a future offline playtest/client path needs a sharper split between four layers that are easy to blur together:

1. **update/script input** changes runtime state;
2. **runtime state** owns clocks, movement, targets, vars, temporary effects, and UI state;
3. **definition/model assembly** resolves the current definition and creates a posed model;
4. **rendering** consumes the resolved model/transform/UI state without becoming its semantic owner.

The client source consistently follows that split for players, NPCs, actor spot effects, standalone graphics objects, projectiles, and widgets.

## Actor family overview

```text
packet / replay / local command
            |
            v
      Actor runtime state
 movement, path, target, sequence clocks,
 overheads, hitsplats, health bars, spots
            |
            +------------------------------+
            |                              |
            v                              v
   Player runtime                    NPC runtime
   + PlayerComposition               + NPCComposition
            |                              |
            +-----------+------------------+
                        v
                SequenceDefinition
             legacy frames or cached model
                        |
                        v
              posed/combined Model
                        |
                        v
                 render projection
```

`GraphicsObject` and `Projectile` do not inherit `Actor`, but they use the same spot-animation and sequence-definition model path. Their clocks and trajectories remain runtime state.

## Player update and runtime state

`Player extends Actor`. The inherited `Actor` state includes:

- world/local position, orientation, rotation, target index;
- idle/turn/walk/run/crawl sequence ids and variants;
- action sequence id, frame, frame cycle, delay and loop count;
- movement path arrays and traversal state;
- exact-movement timing/coordinates;
- overhead text;
- hit splats and health bars;
- actor-attached spot animations;
- temporary recolor/model-override windows.

`Player` adds appearance, username/head icons, combat/skill/team state, actions, hidden state and a temporary attached `model0` with its own cycle window and world-relative transform.

The audited `Player.read(Buffer)` path decodes appearance/equipment/body colours, movement/action sequence ids, username/combat/skill and player customization into runtime/appearance state. The exact packet-mask dispatcher that decides when each player update block is applied has **not yet been pinned in this source walkthrough**. Do not invent that protocol from field names. When network/replay update decoding is implemented, pin the exact dispatcher and masks separately.

### Player render-model assembly

`Player.getModel()` performs semantic/model work before rendering:

1. return no model if `appearance` is absent;
2. choose the active action sequence when `sequence != -1 && sequenceDelay == 0`;
3. choose a movement sequence unless the actor is effectively idle for this model build;
4. call `PlayerComposition.getModel(action, actionFrame, movement, movementFrame)`;
5. calculate bounds and record default height;
6. combine active actor spot-animation models;
7. combine the temporary attached `model0` while its cycle window is active, translating/rotating it relative to the player and then restoring its source transform;
8. apply temporary colour/override state;
9. return the final model for scene rendering.

This means the renderer should receive an already resolved player render instance. It must not own sequence selection, frame progression, equipment selection, actor spot lifetimes, or the temporary attached-model window.

## PlayerComposition and appearance caching

`PlayerComposition` owns the mutable player appearance description and the base-model cache identity:

- equipment/body-part selections;
- body colours;
- gender;
- optional NPC transform id;
- item-specific colour/texture customization overrides;
- appearance hash and cached base model.

`PlayerComposition.getModel(...)` is especially important for RustOSRS ownership:

- an NPC transform delegates to `NPCComposition`;
- active sequence definitions may override effective weapon/shield equipment for the posed instance;
- the base assembled/lit appearance model is cached by composition identity;
- kit/item model data is assembled and recoloured/retextured before lighting;
- sequence transforms are applied **after** the base model is recovered/constructed;
- one or two simultaneous sequences can be applied without mutating the canonical cached base model.

**Porting rule:** cache the reusable appearance/model source independently from per-tick pose state. A current animation frame is not part of the canonical player appearance identity.

## NPC runtime and rendering

`NPC extends Actor` and holds an `NPCComposition definition` plus runtime movement/name/action overrides.

`NPC.getModel()` follows the same high-level flow as the player:

1. select action and movement sequences from actor runtime state;
2. ask the current `NPCComposition` for a model;
3. calculate actor bounds/default height;
4. combine actor spot-animation models;
5. apply temporary override state;
6. return the renderable model.

`NPCComposition` owns static definition data such as model ids, size, actions, recolours/retextures, scales, movement sequences and transform chains.

### NPC morphs

`NPCComposition.transform()` selects the active transformed definition from varbit/varp state, with the configured fallback entry when the selector is out of range.

**Porting rule:** runtime/query state owns vars and resolves the active NPC definition before rendering. `osrs-render` must never own or mutate varbits/varps to decide which NPC exists.

## SequenceDefinition and animation ownership

The pinned client supports two sequence paths:

1. **legacy frame archives** using frame ids/frame lengths/interleave data;
2. **cached-model / skeletal animation** using the cached-model id and associated transform/mask machinery.

`SequenceDefinition` exposes separate transformation paths for actors, objects, spot animations and widgets, plus dual-sequence application.

Important consequence for RustOSRS: existing M8 legacy-animation parity does **not** prove complete player/NPC cached-model or skeletal animation parity. The future entity checkpoint needs an explicit decoder, pose, masking, blending and progression audit for that second path.

Runtime sequence state and model posing are different responsibilities:

- Actor/GraphicsObject/Projectile owns current frame/time/loop progression;
- SequenceDefinition owns how a selected frame transforms a model;
- the renderer consumes the posed model and does not advance the animation clock.

## Actor-attached spot animations

`Actor` stores active spot animations in a keyed table. `updateSpotAnimation(...)` replaces the keyed attachment, records its height and start cycle, and can leave the frame at `-1` until a delayed start is reached.

When an actor model is built, the actor combines the active spot models with the base actor model. Each effect is obtained through `SpotAnimationDefinition` and vertically offset by its attachment height.

`Player.method2365(...)` also demonstrates a special path that can turn a spot-animation model into the player's temporary attached `model0`. That temporary model is distinct from the normal actor spot-animation collection and has its own world/cycle/orientation behavior.

**Porting rule:** do not create separate player and NPC spot-model pipelines. Actor-attached effects should use one shared runtime/model path.

## SpotAnimationDefinition

The audited definition decodes:

- base model archive id;
- sequence id;
- width/height scale;
- orientation;
- ambient/contrast inputs;
- recolours;
- retextures.

`getModel(frame)`:

1. loads/caches the reusable base spot model;
2. applies recolours/retextures and lighting when the base is first built;
3. applies `SequenceDefinition.transformSpotAnimationModel` for an active frame;
4. applies configured scaling;
5. applies 90/180/270 degree orientation rotation.

This is the shared model source for actor attachments, standalone graphics objects and projectiles.

## GraphicsObject

`GraphicsObject` is a standalone timed effect, not an Actor.

Runtime state owns:

- spot-animation id;
- plane and position;
- start cycle;
- sequence/frame/frame-cycle;
- finished state.

`advance(delta)` advances either the legacy-frame or cached-model sequence path and marks the object finished at the appropriate end. `getModel()` asks the spot-animation definition for the current frame.

**Porting rule:** the future playtest runtime advances the lifetime/sequence. The renderer only consumes the current effect model and transform.

## Projectile

`Projectile` is also a standalone `Renderable` with deterministic simulation state:

- source position and plane;
- cycle start/end;
- slope/start-height/end-height;
- target index;
- floating-point current x/y/z;
- x/y speed, horizontal speed, z speed and z acceleration;
- yaw/pitch;
- sequence/frame state.

`setDestination(...)` computes launch position and velocity/acceleration so the projectile reaches its target at the required cycle. `advance(delta)` integrates position, derives yaw/pitch from velocity, and advances the projectile's spot-animation sequence. `getModel()` gets the current spot model and rotates it by pitch.

**Porting rule:** trajectory integration, target tracking and lifetime are simulation/runtime concerns. They must never be recreated from GPU state or advanced by render-frame timing. A deterministic playtest tick should be the source of truth.

## CS2, widgets, and interface rendering

The source makes a useful separation that our documents should preserve:

```text
Script definition
     |
     v
Interpreter / opcode handlers
 stacks, locals, active widget, events
     |
     v
Widget/client UI state mutations
 geometry, scroll, text, sprite/model ids,
 colours, visibility, sequence/frame, listeners
     |
     v
widget resource/model resolution
     |
     v
interface raster/render traversal
```

### Script and Interpreter

`Script` stores parsed opcodes/operands, local/argument counts and switch tables. `Interpreter` owns VM stacks, arrays, script frames and the active/dot widgets.

These classes execute or support CS2 behavior. They **do not constitute the interface renderer**.

### Widget

`Widget` stores the state that interface scripts and client logic can modify or inspect, including:

- parent/position/size/alignment and scroll state;
- colour/fill/transparency;
- sprite state;
- text/font state;
- model type/id, rotations, zoom and offsets;
- primary/alternate sequence ids and model frame state;
- item id/quantity;
- actions and listener/trigger arrays;
- legacy versus IF3 decoding state.

`Widget.getModel(...)` resolves several model-source families, including raw model archives, NPC composition, player composition, item models and NPC/custom override paths. It caches the reusable base model and applies `SequenceDefinition.transformWidgetModel` for the selected widget animation frame.

### What is still not pinned

This walkthrough has not yet pinned:

- the exact recursive interface traversal/raster helper for this source revision;
- the complete CS2 opcode-dispatch/helper table;
- the exact player/NPC packet update-mask dispatchers.

Those anchors should be added before RustOSRS claims exact CS2/interface or network-update parity. The ownership conclusion does not depend on guessing those names: CS2/script execution mutates or queries UI/client state; interface rendering consumes that state.

## RustOSRS ownership map

### `osrs-cache`

Own/retain decoded immutable definitions and assets required by the runtime/model layer:

- NPC definitions and transforms;
- sequence definitions, including cached-model/skeletal metadata;
- spot-animation definitions;
- kits/items/player appearance inputs;
- widget/interface definitions/resources when that subsystem is implemented.

It does not own live actor clocks, paths, projectile positions, var state or CS2 stacks.

### `osrs-core` / semantic model layer

Own reusable model/pose/animation primitives and immutable identities. Base models must remain safe to share while per-instance transforms create working/posed instances.

### future `osrs-playtest`

Own deterministic runtime state:

- player and NPC actor state;
- movement/path/exact-movement progression;
- target/orientation state;
- var context used for morph resolution;
- sequence clocks and loop state;
- actor spot attachments;
- temporary player attached-model state;
- graphics-object lifetimes;
- projectile trajectories/targets/lifetimes;
- deterministic tick, seed, replay and source revision identity.

### UI/CS2 runtime

Own script VM state, widget/client state transitions and event execution. This is not `MapDocument` editing authority and not scene-renderer authority.

### `osrs-render`

Consume resolved render instances:

- posed model or immutable render-model handle;
- world transform/orientation/scale;
- resolved material/face state;
- effect/UI render state already selected by the owning runtime.

The renderer must not become the owner of packet masks, varbits/varps, player equipment semantics, sequence clocks, spot lifetimes, projectile physics, CS2 execution or widget business state.

## Implementation order implied by the source

1. retain/decode missing entity, sequence, spot and appearance metadata;
2. complete cached-model/skeletal animation semantics before claiming full actor animation parity;
3. implement reusable player/NPC appearance model assembly with immutable base caches;
4. add deterministic Actor runtime progression and morph resolution;
5. add shared spot-animation instances, GraphicsObject and Projectile runtime;
6. extract already resolved actor/effect instances to `osrs-render`;
7. treat CS2/widget runtime as a separate UI subsystem, then pin its exact interface traversal/raster source before claiming UI parity.

## Source files audited in this checkpoint

- `Player.java`
- `Actor.java`
- `PlayerComposition.java`
- `NPC.java`
- `NPCComposition.java`
- `SequenceDefinition.java`
- `SpotAnimationDefinition.java`
- `GraphicsObject.java`
- `Projectile.java`
- `Widget.java`
- `Interpreter.java`
- `Script.java`

The source study is evidence for documentation and implementation planning. Promote exact behavior into `docs/specs/` only with an explicit status and required executable evidence.