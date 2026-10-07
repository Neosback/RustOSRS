# GroundObject

`net.runelite.api.GroundObject` — interface render contract (§C7). Floor decals at sampled height.

Source: `/Users/tylercovalt/Documents/runelite-master/runelite-api/src/main/java/net/runelite/api/GroundObject.java`
Apidocs: https://static.runelite.net/runelite-api/apidocs/net/runelite/api/GroundObject.html

> Represents an object on the ground of a tile.

Declaration: `interface GroundObject extends TileObject`

## Methods (3)

### `Renderable getRenderable();`

### `Shape getConvexHull();`
Gets the convex hull of the objects model.

### `int getConfig();`
A bitfield containing various flags: <pre>object type id = bits & 0x20 orientation (0-3) = bits >>> 6 & 3 supports items = bits >>> 8 & 1 </pre>

_Generated from the local Oct-2026 snapshot by `tools/runelite-mcp/gen_api_docs.py`. Regenerate after updating the snapshot._
