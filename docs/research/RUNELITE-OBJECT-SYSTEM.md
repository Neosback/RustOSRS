# RuneLite / OSRS object system: definitions, placement, models and effects

Reviewed: **2026-10-15**. Source checkout:
`/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin`.
Git source: **`1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`**.

This is the same public deob pinned by our loc/model/morph specifications. Its build constants
declare **OSRS 235**, RuneLite `1.12.14`, OpenOSRS `4.31.2` and `cacheversion = 165`.
That last value is a separately named build constant, not evidence that every asset uses one
format version. RustOSRS's selected cache is **build 241**. Preserve that distinction when porting.
See [build constants](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/buildSrc/src/main/kotlin/Dependencies.kt:26).

This document connects the source paths; existing accepted specifications remain implementation
authority. Source-file hashes, Git state and the RustOSRS comparison baseline are recorded in
[runelite-object-sources.json](runelite-object-sources.json). No client was compiled or run for
this study, and no Java implementation or assets were copied.

Companion: [what our filestore and cache layers need](RUNELITE-FILESTORE-NEEDS.md).

## 1. An object is several different things

| Concept | What it owns | Source vocabulary |
| --- | --- | --- |
| Object definition | Reusable settings and references to models, sequences, sounds, morphs and map graphics | `ObjectComposition`; cache-tool `ObjectDefinition` |
| Placed location, or loc | Definition ID, tile, encoded plane, loc type and orientation | Decoded map locations |
| Scene object | Placement/storage category, bounds, tag, flags, one or two renderables | `BoundaryObject`, `WallDecoration`, `GameObject`, `FloorDecoration` |
| Renderable | Supplies geometry at draw time; can be static or stateful | `Renderable`, `Model`, `DynamicObject` |
| Object sound instance | Footprint, active sound IDs, countdowns, volume ramps and mixer streams | `ObjectSound` |
| Map feature | Map-element reference or a map-scene sprite-sheet entry | `WorldMapElement`, `WorldMapIcon_0/1` |

Definition IDs do not identify individual placements. A model ID is not an object ID, an animation
ID is not a frame ID, and a map-icon ID is not a sprite ID. Preserve those identities separately.

The normal dependency path is:

```text
JS5 sector/index storage → encoded group → decompress/decrypt → logical file
    → object definition + placed loc
    → model selection / morph selection / scene placement
    → static or dynamic model, collision, sound instance, map feature
    → rendering / audio / interaction
```

## 2. Where the assets live

These addresses are evidenced by this checkout; keep numeric addressing target-aware.
Java cache tooling calls a JS5 group an **archive**. In this document, an address is
`(index, group, logical file)`; sprite frames inside one decoded file are another level.

| Resource | Index / group / file | Relationship |
| --- | --- | --- |
| Object definitions | `2 / 6 / object ID` | Shared settings for map locs |
| NPC definitions | `2 / 9 / NPC ID` | Actors; a different definition family |
| Item definitions | `2 / 10 / item ID` | Ground items and equipment |
| Identity kits | `2 / 3 / kit ID` | Player appearance assembly |
| Sequences | `2 / 12 / sequence ID` | Playback, frame references, skeletal reference, events |
| Spot-animation definitions | `2 / 13 / effect ID` | Model/sequence for graphics and projectiles |
| Varbits / varps | `2 / 14`, `2 / 16` | Definition metadata; live values are runtime state |
| World-map elements | `2 / 35 / element ID` | Icon sprite, name, labels, visibility and alignment |
| Models | `7 / model ID / 0` in the audited object path | Geometry and per-face attributes |
| Legacy animation frames | index `0` | Sequence frame ID splits into group/file parts |
| Skeletons | index `1` | Referenced by frame/skeletal data |
| Maya/skeletal animations | index `22` | Distinct from config sequence definitions |
| Sprite files | index `8` | Each decoded file may contain multiple sprite frames |
| Textures | `9 / 0 / texture ID` | Refers to a sprite resource; not a bitmap itself |
| Object sound effects | `4 / sound ID / selected file` | This client supports two sound-file variants |
| World-map data | indices `18`, `19`, `20`; font metrics `13` | Geography, map descriptions/composites, ground and labels |

Sources: [index IDs](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/cache/src/main/java/net/runelite/cache/IndexType.java:28),
[config groups](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/cache/src/main/java/net/runelite/cache/ConfigType.java:28),
[object lookup](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java/AsyncHttpResponse.java:119),
[texture addressing](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java/TextureProvider.java:53),
[skeletal resource lookup](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java/class146.java:58).

## 3. Loading a definition and applying defaults

`AsyncHttpResponse.getObjectDefinition(id)` checks the definition cache, loads config group `6`,
constructs an `ObjectComposition`, decodes its opcode stream if bytes exist, runs `postDecode`,
applies the solid-object interaction override, and caches the result. Missing bytes leave the
constructor defaults; they do not produce an error in this Java lookup.

Important defaults are footprint `1×1`, interaction type `2`, projectile blocking enabled,
terrain shadow/clipping participation enabled, scale axes `128`, displacement `16`, zero offsets,
no animation, no morph selectors and no map graphics. Sound IDs default absent; fade durations
default `300`, visibility `2`. Random animation start is enabled by `boolean3` until opcode `89`.

`postDecode` derives `int1` when absent from model/action presence, and `int3` from interaction
type. Afterwards the lookup makes solid/hollow objects non-blocking for its interaction fields.
Raw encoded fields, derived defaults and runtime values are different representations: do not
repack a definition by assuming the normalized runtime structure preserves its original bytes.

Sources: [constructor/defaults](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java/ObjectComposition.java:319),
[postDecode](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java/ObjectComposition.java:370),
[lookup/solid override](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java/AsyncHttpResponse.java:119).

## 4. Definition opcode map in this snapshot

`u8/u16/i8/i16` below describe logical reads, not decompiler field-storage multipliers.
Strings are CP1252 and NUL terminated. Opcode `0` terminates the stream.

| Opcode | Payload / effect |
| --- | --- |
| `1` | `u8 count`, then `(u16 model ID, u8 requested loc type)` entries |
| `2` | Name |
| `5` | `u8 count`, then untyped `u16` model IDs |
| `14`, `15` | `u8` footprint width/depth |
| `17`, `18`, `27` | Interaction/projectile-blocking flags; `17` clears both, `18` projectile blocking, `27` sets interaction type `1` |
| `19` | Explicit `u8 int1` interaction hint |
| `21`, `81` | Ground contouring: full contour (`0`) or `u8 × 256` clip setting |
| `22`, `23` | Non-flat shading / model occluder participation |
| `24` | `u16` sequence ID; `65535` explicitly becomes `-1` |
| `28` | `u8` wall-decoration displacement |
| `29`, `39` | Signed ambient adjustment; signed contrast adjustment multiplied by `25` |
| `30..34` | Five action strings; case-insensitive `Hidden` becomes absent |
| `40`, `41` | `u8 count`, then `u16 → u16` recolor or retexture pairs |
| `61` | `u16` category; runtime decoder consumes it, cache-tool loader retains it |
| `62`, `64` | Mirror/rotation flag; clear `clipped` terrain-shadow participation |
| `65..67` | Three `u16` scale axes; `128` is identity |
| `68` | `u16` map-scene sprite-sheet entry |
| `69` | `u8` access/blocking metadata; runtime consumes it, cache-tool loader retains it |
| `70..72` | Three signed `i16` model offsets in the runtime decoder |
| `73..75` | Ground obstruction flag, solid/hollow flag, `u8` item-support setting |
| `77`, `92` | Nullable varbit/varp selectors and transform IDs; `92` additionally carries explicit fallback |
| `78` | `u16` looping sound ID, `u8` distance, `u8` retain |
| `79` | Two `u16` random-delay bounds, distance, retain, count, `u16` random sound IDs |
| `82` | `u16` map-element ID |
| `89`, `90` | Disable random animation start; additional animation-change flag (`field2397`) |
| `91` | `u8` sound distance-fade curve |
| `93` | Fade-in `(u8 curve, u16 duration)` and fade-out pair |
| `94` | No payload; runtime branch does nothing, cache-tool loader retains `unknown1` |
| `95` | `u8` sound visibility enum |
| `249` | Counted string/int parameter table with 24-bit keys |

Sources: [runtime decodeNext](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java/ObjectComposition.java:413),
[cache-tool ObjectLoader](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/cache/src/main/java/net/runelite/cache/definitions/loaders/ObjectLoader.java:73).

### Revision and source traps

- This object decoder has no build-241 extended model opcodes `6/7`, or newer `42/96/100..102`.
  Our decoder already recognizes those. Do not replace it with this older switch.
- The cache-tool `ObjectLoader.configureForRevision` takes the **object group's revision**;
  its sound-layout threshold is `1673`, not game build `220` passed as an integer.
  [ObjectManager supplies that group revision](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/cache/src/main/java/net/runelite/cache/ObjectManager.java:54).
- The cache-tool loader reads offsets `70..72` unsigned; the runtime reads them signed. Its
  `randomizeAnimStart` naming/assignment also differs from runtime `boolean3`. Use verified
  runtime semantics, not convenient field names, for negative offsets and animation start.
- Animation/morph nullable IDs are explicitly normalized. Sound opcode `78` does **not** contain
  an equivalent `65535 → -1` conversion here. A universal sentinel rule is not established.
- Decompiled code contains encoded-field arithmetic, including `Buffer.offset` multipliers.
  Use logical field operations and our pinned differential cases; do not copy those numbers
  into Rust. Contour fixed-point arithmetic needs its existing verified spec, not simplification.
- The old `decodeNext` body in `RSObjectCompositionMixin` has its replacement annotations
  **commented out**. It is not evidence that the injected client uses that switch. The active
  mixin posts `PostObjectComposition` and resizes a model cache.
  [Mixin entry points](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runelite-mixins/src/main/java/net/runelite/mixins/RSObjectCompositionMixin.java:40).

## 5. Decoding placements and putting them in the scene

The locations stream delta-encodes object IDs starting from `-1`. Within each ID it delta-encodes
packed positions: local Y is bits `0..5`, local X bits `6..11`, plane above bit `11`.
Each placement has an attribute byte: `type = byte >> 2`, orientation `byte & 3`.
The region decoder selects the collision plane separately when bridge settings require it.
That is not permission to subtract one from every plane used elsewhere.

Sources: [runtime location stream](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java/DynamicObject.java:218),
[cache-tool stream decoder](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/cache/src/main/java/net/runelite/cache/definitions/loaders/LocationsLoader.java:43).

`FriendSystem.addObjects` rotates the footprint for orientations `1/3`, obtains integer height
samples and the 128-unit-space center, builds scene tags/flags, creates a sound instance when
`hasSound()` permits one, then dispatches by loc type:

| Loc type | Scene storage / special behavior |
| --- | --- |
| `0`, `1`, `3` | Single boundary arm; straight or diagonal orientation mask |
| `2` | Two boundary arms, with distinct model orientations |
| `4..8` | Wall decorations; full/half existing-wall displacement or opposite-side orientation |
| `9` | Game-object storage, 1×1 scene insertion path |
| `10`, `11` | Game object using rotated definition footprint; `11` adds rotation/flag `256` |
| `12..21` | Game-object 1×1 insertion path; some types also update occlusion grids |
| `22` | Floor decoration with its own interaction/collision condition |

Collision footprints, shadow-grid writes, occlusion flags and scene insertion dimensions are
related but distinct. `clipped = false` is not a complete policy for all HD shadow rendering.
Initial static placement uses `getEntity`; pending/live replacement has a separate removal and
replacement path using lit models. Replacing a door must update the old category's collision and
sound lifecycle as well as its mesh.

Sources: [initial placement](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java/FriendSystem.java:358),
[live replacement](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java/class150.java:135),
[sound removal matching](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java/WorldView.java:262).
Our precise contracts are [loc placement](../specs/loc-placement.md) and
[planes/bridges](../specs/planes-bridges.md).

## 6. Selecting and building the model

`ObjectComposition.getModelData(type, orientation)` first selects geometry. An untyped model
list accepts requested type `10` and combines its parts. A typed list selects the matching type;
a miss returns no geometry. There is no fallback to the first model.

The typed mirror choice is `isRotated XOR (orientation > 3)`. The old client separates mirror
cache entries by adding `65536` to a 16-bit model ID; that encoding must not be reused for modern
32-bit IDs. RustOSRS already uses a typed model/variant/provenance key.

After resolving raw model data and obtaining a working copy, the order is:

1. Special type-4 diagonal decoration recenter: rotate `256` JAU, offset `(45, 0, -45)`.
2. Ordinary orientation rotation using `orientation & 3`.
3. Apply recolor pairs to matching face color values.
4. Apply retexture pairs to matching face texture IDs.
5. Resize by definition scales relative to `128`.
6. Apply signed definition translation.

Recoloring is not replacing a texture image; retexturing is not replacing a face color. Model
decoding also carries alpha, render types, priorities, texture coordinates/triangles, skin labels
and skeletal data. Dropping those arrays changes appearance or animation even if vertices match.
The `ModelData` constructor selects model formats from trailing marker bytes (`FF FD`, `FF FE`,
`FF FF`, or legacy), rather than assigning one decoder to all models in an index.

Sources: [model build](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java/ObjectComposition.java:776),
[model format dispatch](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java/ModelData.java:322),
[our typed raw-model keys](/Users/tylercovalt/Documents/RustOSRS/crates/osrs-cache/src/model_repository.rs:31).
See [model-build](../specs/model-build.md) and [face materials](../specs/face-materials.md).

### Lighting, contouring and the four caches

The source allocates caches of `4096` definitions, `500` raw model variants, `256` entity variants
and `256` lit model variants. These are entry-count policies, not suitable Rust memory budgets by
themselves. Geometry keys also include the requested type where relevant and model orientation.

`getEntity` can retain `ModelData` for non-flat shading and scene normal reconciliation; otherwise
it lights a model using ambient `+64`, contrast `+768`, and light vector `(-50, -10, -50)`.
`getModel` builds/caches the lit path. `getModelDynamic` starts from a lit base, obtains a safe
working model for a sequence or contour operation, then contours the result against terrain.
Keep shared source data immutable; animation or terrain deformation must not leak to another loc.

Sources: [caches](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java/ObjectComposition.java:311),
[getEntity](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java/ObjectComposition.java:654),
[dynamic model](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java/ObjectComposition.java:734).
Normative details: [normals/lighting](../specs/normals-lighting.md),
[morph/animation/contour](../specs/morph-animation-contouring.md).

## 7. Dynamic objects, morphs and animation

A loc becomes a `DynamicObject` when it has an animation **or** a transforms table. A morphing
object can therefore be dynamic while having no animation ID.

Morph selection uses varbit first, otherwise varp; an in-range selector picks its entry, otherwise
the final fallback. A selected null entry removes the active model; it is not a request to use the
fallback. The placed base object ID and current active definition ID remain separate identities.

`DynamicObject.getModel` resolves the active definition before calculating footprint and height.
It advances two `AnimationSequence` slots using elapsed `Client.cycle`, with explicit replacement,
restart and fallback state. Equal sequence IDs preserve primary state only when `restartMode == 0`.
Eligible initial animations randomize phase; the source uses `Math.random()`, while a deterministic
editor/playtest needs a seeded equivalent with recorded start state. Sequence events pass through
the client event context; advancing only the mesh frame is not full playback parity.

Legacy sequences reference frame group/file pairs and skeletons. Cached/Maya sequences use a
separate resource and skeletal path. `transformObjectModel` rotates the model into the animation's
orientation, poses it, and rotates it back; alpha transforms affect whether face-alpha data can
be shared. RuneLite adds optional interpolation and animation-change callbacks on top of this.
Keep interpolation settings distinct from vanilla discrete-frame behavior.

Sources: [DynamicObject](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java/DynamicObject.java:73),
[morph selector](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java/ObjectComposition.java:917),
[random phase](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java/AnimationSequence.java:245),
[sequence posing](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java/SequenceDefinition.java:467),
[RuneLite interpolation/event hook](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runelite-mixins/src/main/java/net/runelite/mixins/RSDynamicObjectMixin.java:54).

## 8. Object sound is a runtime system

`hasSound()` examines the definition or its transform alternatives, so a base definition without
an audible sound can still require an instance. `WorldView` owns sound instances independently of
the renderable. The constructor rotates the definition footprint and records a rectangle in
128-unit local coordinates; the sound is not just a point at the object's origin.

The listener path converts that rectangle through world-view coordinates, calculates **Manhattan
distance outside the rectangle**, and subtracts `64` local units, clamped at zero. The constructor's
inner range is `max((retain - 1) × 128, 0)` and its outer range is `distance × 128`. A configured
distance curve produces target volume; separate fade-in/out curves and durations ramp the streams.
Thus a Euclidean circle around the center does not reproduce the audited attenuation.

Looping sound uses one stream with loop count `-1`. Intermittent sound uses a second stream,
random selection and a countdown, and resets its delay after starting. Plane/world-view visibility,
area-sound volume, changing morphs and cleanup affect both. Removal queues a fade and eventually
unlinks the streams and instance; an off-screen mesh alone is not a reason to destroy an audible
sound.

For ordered distinct delay bounds, `class190.method4416` computes
`min + floor(random × (max - min))`: the upper bound is **exclusive**. Equal bounds return that
constant. Do not turn a convenient field comment into an inclusive random range.
See [delay selection](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java/class190.java:36).

The fade ramp clock is **milliseconds** (`ParamComposition.method4708`, backed by
`System.currentTimeMillis` with backwards-clock compensation). Do not interpret its default
`300` as 300 game ticks. Random-sound countdowns are a different input (`var5` delta); keep those
units separate. The stream-ramp duration is also adjusted for volume difference.

The sound loader is particularly relevant to filestore addressing: it selects
`getGroupFileCount(soundId) - 1`, accepts file variant **0 or 1**, then reads that logical file.
Variant 0 synthesizes instrument data; variant 1 has an additional header/setup and compressed
sample path. Do not assume every sound is `index 4 / sound ID / file 0`, or a WAV file.
Here `getGroupFileCount` is the allocated file-slot array length, not the number of populated
IDs. For sparse files, translating it to Rust's `file_ids.len()` would select the wrong variant.
See [slot-count implementation](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java/AbstractArchive.java:475).

There are source details needing target-specific tests before copying: the constructor applies
`retain - 1` but `ObjectSound.set()` assigns transformed retain without that subtraction; the
instance's visibility enum is final and comes from the base definition. The constructor also
assigns base fade settings after its initial transform refresh. Preserve these as observed paths,
not an invented universal sound formula.

Sources: [hasSound](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java/ObjectComposition.java:961),
[instance/update/playback](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java/ObjectSound.java:154),
[distance](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java/class204.java:97),
[Manhattan metric](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java/class147.java:91),
[fade clock](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java/ParamComposition.java:105),
[sound variants](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java/SoundEffect.java:194),
[retirement matching](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java/WorldView.java:262).

## 9. Map icons, map-scene sprites and labels

**Map icon** (`82`) is a `WorldMapElement` ID. That config definition supplies sprite IDs, label
name/color/size, visibility flags, category, alignment and menu metadata. Its sprite is loaded
from index `8`. **Map scene** (`68`) indexes an already-decoded `mapSceneSprites[]` sheet loaded
from a sprite group. It is not a direct texture ID or a separate JS5 file for every sheet entry.

The world-map renderer scans geography decorations, checks candidate definitions and their
transforms for icons, and creates a static or dynamic icon. `WorldMapIcon_1.init()` resolves the
current transformed definition's icon and refreshes its label and dimensions. `getIconsForTiles`
stops after selecting an icon for that tile; it does not emit every candidate without limits.

The minimap has another path: scene-object tags and loc flags produce wall lines or map-scene
stamps, and a floor-decoration scan collects eligible map elements. Its base-definition lookup
is not identical to the world-map dynamic-icon path. World-map ground colors/geography and label
fonts are separate resources; deriving all map output only from the current 3D mesh misses data.

Sources: [element decode/sprite](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java/WorldMapElement.java:164),
[world-map extraction](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java/WorldMapRegion.java:139),
[dynamic icon](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java/WorldMapIcon_1.java:63),
[minimap drawing](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java/HealthBarDefinition.java:214),
[minimap icons](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java/SecureRandomCallable.java:88).

## 10. Texture animation and color handling

In the revision-233+ layout used here, a texture definition is seven bytes: sprite/file ID `u16`,
average color `u16`, a flag byte, animation direction and animation speed. `Texture.load` obtains
the indexed sprite, normalizes its crop/offsets, brightens its palette, and builds 64/128-pixel
texture data. Older cache-tool layouts allow multiple sprites and combine arrays; those fields
must not be read from this seven-byte layout.

Texture scrolling is independent of a loc's skeletal/frame animation. Directions `1/2` shift whole
rows in opposite directions; `3/4` shift within rows. `TextureProvider` retains an LRU of expanded
pixels, clears it when brightness changes, and marks textures used for animation. Animation and
brightness must be included in projection/reuse decisions even when no object vertices moved.

Vanilla face colors/palette lighting, texture IDs, sprite palette entries, per-face alpha, and
117HD material overrides are separate data. HD normal/roughness maps and object-specific HD
overrides do not become fields of an official object definition just because a plugin uses them.

Sources: [Texture](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java/Texture.java:37),
[TextureProvider](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java/TextureProvider.java:123),
[historical texture layout](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/cache/src/main/java/net/runelite/cache/definitions/loaders/TextureLoader.java:38).

## 11. Other things that look like objects

These share model/animation/rendering machinery but are not loc definitions in config group `6`.
This section identifies their dependencies; it is not an exhaustive actor or network protocol audit.

| Family | Definition / model path | Runtime state that cannot live in filestore |
| --- | --- | --- |
| Ground item (`TileItem`) | Item definition, quantity-dependent model/overrides | Quantity, location, visibility/lifetime |
| NPC | NPC definition, optional transforms/model overrides, actor sequences | Movement, facing, variables, animation and attached effects |
| Player | Appearance assembled from kits/items/body colors, sequences | Equipment, colors, movement, pose and temporary attached models |
| Projectile | Spot-animation definition/model/sequence | Start/end cycles, target, trajectory, pitch and removal |
| Graphics object | Spot-animation definition/model/sequence | Start cycle, elapsed animation and completion |
| Attached actor effect | Effect model plus actor transforms | Attachment point, active cycle range and ownership |
| RuneLite custom object | Caller-supplied model/animation in a graphics-object lifecycle | Activation, loop, position, orientation; may have no cache definition |
| World entity / sub-world | Separate world-view/configuration and scene | Coordinate transforms, plane/context and attached scenery/sound |

Sources: [ground item](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java/TileItem.java:102),
[NPC](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java/NPC.java:89),
[player/attachments](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java/Player.java:398),
[projectile](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java/Projectile.java:159),
[graphics object](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java/GraphicsObject.java:118),
[spot animation](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java/SpotAnimationDefinition.java:178),
[custom-object lifecycle](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java/RuneLiteObject.java:27).

RuneLite's tile mixins emit separate wall/decorative/ground/game-object spawn, change and despawn
events; dynamic-object animation and post-definition events are additional hooks. Event consumers
must correlate an instance and its lifecycle, not treat an object definition cache entry as a spawn.
See [tile event hooks](/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runelite-mixins/src/main/java/net/runelite/mixins/RSTileMixin.java:397).

## 12. RustOSRS comparison and next verification cases

Already present: revision-gated object/model/sequence/texture/sprite decoding; typed raw-model
variants; model construction, morph and contour semantics; legacy animated-loc posing; world-map
elements, map scenes and optional 3D map icons; decoded object-sound metadata and an inspector.
Do not plan a second implementation of those systems.

Important limits in the inspected code:

- The animated-loc runtime explicitly falls back to static geometry for skeletal sequences.
- Sequence event metadata and some object metadata are consumed but not retained canonically.
- The sound panel does not synthesize/decode and play the referenced audio files.
- The world-map `object_icon` helper uses a first-transform/default-state approximation; its
  `.or(fallback)` also needs a null-transform case checked against `MORPH-001` before it can serve
  as a general runtime variable resolver.
- NPC/player/effect playback and their light lifecycles require the separate offline runtime.
- Existing decoder tests or screenshot captures do not establish all of this behavior for 241.

Code pointers: [object decoder](/Users/tylercovalt/Documents/RustOSRS/crates/osrs-cache/src/decode/object.rs:19),
[sequence decoder](/Users/tylercovalt/Documents/RustOSRS/crates/osrs-cache/src/decode/sequence.rs:14),
[legacy runtime limitation](/Users/tylercovalt/Documents/RustOSRS/crates/osrs-world/src/animation.rs:8),
[world-map icon selection](/Users/tylercovalt/Documents/RustOSRS/crates/osrs-world/src/worldmap/mod.rs:1208),
[sound inspector data](/Users/tylercovalt/Documents/RustOSRS/crates/osrs-editor/src/sound/mod.rs:122).

The highest-value new cases are: morph-to-null with a non-null fallback; morph changing footprint,
icon and sound; two-arm walls; negative offsets with recolor/retexture/scale; a skeletal sequence
with events; a texture scrolling on a stationary loc; sound file variants 0 and 1; constructor vs
morph sound retain; millisecond fades; removal during playback; sparse logical file IDs; and a
source switch while a model/audio/icon request is outstanding. Keep fixture source/build identities
with each case and promote verified behavior through the existing spec/parity process.
