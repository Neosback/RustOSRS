# Renderable

`net.runelite.api.Renderable` — interface render contract (§C9). Drawable marker inherited by all locs.

Source: `/Users/tylercovalt/Documents/runelite-master/runelite-api/src/main/java/net/runelite/api/Renderable.java`
Apidocs: https://static.runelite.net/runelite-api/apidocs/net/runelite/api/Renderable.html

> Represents an object that can be rendered.

Declaration: `interface Renderable extends Node`

## Methods (5)

### `Model getModel();`
Gets the model of the object.

### `int getModelHeight();`
Gets the height of the model.

### `void setModelHeight(int modelHeight);`

### `int getAnimationHeightOffset();`

### `int getRenderMode();`

## Fields (5)

- `int RENDERMODE_DEFAULT = 0;`
- `int RENDERMODE_SORTED = 1;`
- `int RENDERMODE_SORTED_NO_DEPTH = 2;`
- `int RENDERMODE_UNSORTED = 3;`
- `int RENDERMODE_UNSORTED_NO_DEPTH = 4;`

_Generated from the local Oct-2026 snapshot by `tools/runelite-mcp/gen_api_docs.py`. Regenerate after updating the snapshot._
