# DecorativeObject

`net.runelite.api.DecorativeObject` — interface render contract (§C6). Wall decor + standoff offsets.

Source: `/Users/tylercovalt/Documents/runelite-master/runelite-api/src/main/java/net/runelite/api/DecorativeObject.java`
Apidocs: https://static.runelite.net/runelite-api/apidocs/net/runelite/api/DecorativeObject.html

> Represents a decorative object, such as an object on a wall.

Declaration: `interface DecorativeObject extends TileObject`

## Methods (9)

### `Shape getConvexHull();`
Gets the convex hull of the objects model.

### `Shape getConvexHull2();`

### `Renderable getRenderable();`

### `Renderable getRenderable2();`

### `int getXOffset();`
Decorative object x offset.

### `int getYOffset();`
Decorative object y offset.

### `int getXOffset2();`

### `int getYOffset2();`

### `int getConfig();`
A bitfield containing various flags: <pre>object type id = bits & 0x20 orientation (0-3) = bits >>> 6 & 3 supports items = bits >>> 8 & 1 </pre>

_Generated from the local Oct-2026 snapshot by `tools/runelite-mcp/gen_api_docs.py`. Regenerate after updating the snapshot._
