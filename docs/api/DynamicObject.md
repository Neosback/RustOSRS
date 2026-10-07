# DynamicObject

`net.runelite.api.DynamicObject` — interface render contract (§C9). Animated locs (model swap per frame).

Source: `/Users/tylercovalt/Documents/runelite-master/runelite-api/src/main/java/net/runelite/api/DynamicObject.java`
Apidocs: https://static.runelite.net/runelite-api/apidocs/net/runelite/api/DynamicObject.html

> An animated object

Declaration: `interface DynamicObject extends Renderable`

## Methods (5)

### `Animation getAnimation();`
Get the animation applied to the object

### `int getAnimFrame();`
Get the frame of the current animation

### `int getAnimCycle();`
Get the frame cycle.

### `Model getModelZbuf();`
Like #getModel() but is threadsafe and doesn't support animations.

### `ObjectComposition getRecordedObjectComposition();`
The object composition for the model returned by #getModelZbuf()

_Generated from the local Oct-2026 snapshot by `tools/runelite-mcp/gen_api_docs.py`. Regenerate after updating the snapshot._
