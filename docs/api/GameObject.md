# GameObject

`net.runelite.api.GameObject` — interface render contract (§C8). Multi-tile objects, footprints, config word.

Source: `/Users/tylercovalt/Documents/runelite-master/runelite-api/src/main/java/net/runelite/api/GameObject.java`
Apidocs: https://static.runelite.net/runelite-api/apidocs/net/runelite/api/GameObject.html

> Represents a game object. <p> Most object in the RuneScape world are considered as game objects.

Declaration: `interface GameObject extends TileObject`

## Methods (9)

### `int sizeX();`
Get the size of this object, in tiles, on the x axis

### `int sizeY();`
Get the size of this object, in tiles, on the y axis

### `Point getSceneMinLocation();`
Gets the minimum x and y scene coordinate pair for this game object.

### `Point getSceneMaxLocation();`
Gets the maximum x and y scene coordinate pair for this game object. <p> This value differs from #getSceneMinLocation() when the size of the object is more than 1 tile.

### `Shape getConvexHull();`
Gets the convex hull of the object's model.

### `int getOrientation();`
Get the orientation of the object

### `Renderable getRenderable();`

### `int getModelOrientation();`
Gets the orientation of the model in JAU.

### `int getConfig();`
A bitfield containing various flags: <pre>object type = bits & 31 orientation = bits >>> 6 & 3 supports items = bits >>> 8 & 1 </pre>

_Generated from the local Oct-2026 snapshot by `tools/runelite-mcp/gen_api_docs.py`. Regenerate after updating the snapshot._
