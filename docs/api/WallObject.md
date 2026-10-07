# WallObject

`net.runelite.api.WallObject` — interface render contract (§C5). Dual-slot walls + orientation flags.

Source: `/Users/tylercovalt/Documents/runelite-master/runelite-api/src/main/java/net/runelite/api/WallObject.java`
Apidocs: https://static.runelite.net/runelite-api/apidocs/net/runelite/api/WallObject.html

> Represents one or two walls on a tile

Declaration: `interface WallObject extends TileObject`

## Methods (7)

### `int getOrientationA();`
A bitfield with the orientation of the first wall 1 = West 2 = North 4 = East 8 = South 16 = North-west 32 = North-east 64 = South-east 128 = South-west

### `int getOrientationB();`
A bitfield with the orientation of the second wall 1 = West 2 = North 4 = East 8 = South 16 = North-west 32 = North-east 64 = South-east 128 = South-west

### `int getConfig();`
A bitfield containing various flags: <pre>object type id = bits & 0x20 orientation (0-3) = bits >>> 6 & 3 supports items = bits >>> 8 & 1 </pre>

### `Shape getConvexHull();`
Gets the convex hull of the objects model.

### `Shape getConvexHull2();`

### `Renderable getRenderable1();`

### `Renderable getRenderable2();`

_Generated from the local Oct-2026 snapshot by `tools/runelite-mcp/gen_api_docs.py`. Regenerate after updating the snapshot._
