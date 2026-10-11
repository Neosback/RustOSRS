# Animation review (client deob "Java Deob/build/decompiled-vineflower", build 241 cache)

Sources: `DynamicObject`, `AnimationSequence`, `SequenceDefinition`, `GrandExchangeOfferUnitPriceComparator.method8811`
(sequence stepping), `Model.animate/transform`, `ObjectComposition.getModelDynamic`, `SpotAnimationDefinition`,
`GraphicsObject`, `class146` (skeletal). Census: `crates/osrs-world/examples/anim_census.rs`.

## What can animate in map data

| Kind | Where | Status here |
|---|---|---|
| Legacy frame animation (`Frames`, vertex-group transforms: origin 0, translate 1, rotate 2, scale 3, alpha 5) | any loc type with `animationId` (walls, decorations, game objects, floor decorations all become `DynamicObject`) | ported (`animation_pose.rs`, `animation.rs`); 11,247 looping + 1,065 one-shot (object, sequence) pairs |
| Skeletal ("Maya") animation: sequence opcode 13 `cachedModelId`, `class146` bone curves, `Model.method5924` skinning | 108 (object, sequence) pairs, e.g. objects 29497/29505, "Sails" 29506-29515+ (sailing content) | **not ported**: these locs show the static model. Playback stepping exists in `osrs-core::animation`; the bone/curve decode and skinning do not |
| Texture animation (scrolling water, lava...) | texture definitions | ported (`tick` uniform) |
| Varbit/varp transforms (multiloc) | `ObjectComposition.transforms` | default state only (no varps in an editor) |

Not map data (cannot appear in a static scene): `GraphicsObject` (GFX / spot animation at a tile: spot-anim model +
sequence, optional recolor/retexture, width/height scale, orientation, ambient/contrast; created by server packets),
`ActorSpotAnim` on NPCs/players, projectiles, ground items, hit splats, `ObjectSound`. GFX would only matter for
previewing server effects; `SpotAnimationDefinition` (opcodes 1 model, 2 sequence, 4/5 scale, 6 orientation, 7/8 ambient/
contrast, 40/41 recolor/retexture; 241 adds 42 fullRecolor, 10 not clickable) is not decoded for rendering yet.

## Client behaviour (DynamicObject) and where we stand

* Start: looping sequences (`frameStep != -1`) of objects without opcode 89 start at a **random frame and cycle**
  (`AnimationSequence.method9880`). Was: all at frame 0. **Fixed** (hash of placement, so rebuilds are stable).
* One-shot sequences (no frame step): after one pass `DynamicObject.getModel` resets the sequence and the loc shows its
  **static unposed model**. Was: restarted at frame 0 forever. **Fixed** (the 1,065 one-shot pairs, e.g. party balloons,
  crate 3206, now stop like the game).
* Looping sequences ignore `maxLoops` (only callbacks see the flag); they loop forever. Matches the stepping port.
* Sequence stepping (`method8811`): strict `>` frame-length compare, `% totalDelay` fast-forward when `frameStep > 0`,
  skeletal variant counts frames not cycles. Matches `osrs-core::animation`.
* Posing order: transform on the lit model (lighting is not recomputed), then ground contour. Matches.
* Two `AnimationSequence`s per loc (current + carried-over) blend when a loc changes sequence at runtime (varbit morph);
  irrelevant to static map data.
* `restartMode` only matters for replacement; interleave masks (opcodes 3/17) are for actors, not objects.
* RuneLite GPU flag `NO_VERTEX_SNAPPING` (default on): the closed client mixin presumably keeps float vertices during
  posing; our pose truncates like the vanilla int path (<= 1 unit differences). Unverifiable from the open sources.

## Spit roast (seq 1017) and torch (seq 473)

Both are ordinary looping legacy sequences (9 frames step 9; 2 frames step 2), so they were already stepping
correctly; what differed from the game was that every instance started in phase. They now start de-synchronised.

## Open

1. Skeletal animations (108 pairs): decode `class146`/`class139` curves + `Skeleton` hierarchy, port `Model.method5924`
   and `method5996` skinning.
2. Float posing if RuneLite's `NO_VERTEX_SNAPPING` can be confirmed.
3. `SpotAnimationDefinition` decode + a GFX preview for server effects (editor feature, not map data).
