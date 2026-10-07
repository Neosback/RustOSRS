# TileObject

`net.runelite.api.TileObject` — interface render contract (§C9). Base id/position/plane contract.

Source: `/Users/tylercovalt/Documents/runelite-master/runelite-api/src/main/java/net/runelite/api/TileObject.java`
Apidocs: https://static.runelite.net/runelite-api/apidocs/net/runelite/api/TileObject.html

> Represents an object on a Tile

Declaration: `interface TileObject`

## Methods (17)

### `long getHash();`
A bitfield containing various flags: <pre>worldView = bits >> 52 & 4095 id = bits >> 20 & 0xffffffff wall = bits >> 19 & 1 type = bits >> 16 & 7 plane = bits >> 14 & 3 scene y = bits >> 7 & 127 scene x = bits >> 0 & 127 </pre> Type 0 = player, 1 = npc, 2 = game object, 3 = item, 4 = world entity

### `int getX();`
Gets the x-axis coordinate of the object in local context.

### `int getY();`
Gets the y-axis coordinate of the object in local context.

### `int getZ();`
Gets the vertical coordinate of this object

### `int getPlane();`
Gets the plane of the tile that the object is on.

### `WorldView getWorldView();`
Gets the WorldView this TileObject is a part of.

### `int getId();`
Gets the ID of the object.

### `WorldPoint getWorldLocation();`
Get the world location for this object.

### `LocalPoint getLocalLocation();`
Get the local location for this object.

### `Point getCanvasLocation();`
Calculates the position of the center of this tile on the canvas

### `Point getCanvasLocation(int zOffset);`
Calculates the position of the center of this tile on the canvas

### `Polygon getCanvasTilePoly();`
Creates a polygon outlining the tile this object is on

### `Point getCanvasTextLocation(Graphics2D graphics, String text, int zOffset);`
Calculates the canvas point to center text above the tile this object is on.

### `Point getMinimapLocation();`
Gets a point on the canvas of where this objects mini-map indicator should appear.

### `Shape getClickbox();`
Calculate the on-screen clickable area of the object.

### `String getOpOverride(int index);`
Get the text override for a certain action

### `boolean isOpShown(int index);`
Gets if an action is shown in the minimenu.

## Fields (1)

- `int HASH_PLANE_SHIFT = 14;`

_Generated from the local Oct-2026 snapshot by `tools/runelite-mcp/gen_api_docs.py`. Regenerate after updating the snapshot._
