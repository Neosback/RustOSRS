# GraphicsObject

`net.runelite.api.GraphicsObject` — interface render contract (§F4). Spotanim FX state (id, location, cycle).

Source: `/Users/tylercovalt/Documents/runelite-master/runelite-api/src/main/java/net/runelite/api/GraphicsObject.java`
Apidocs: https://static.runelite.net/runelite-api/apidocs/net/runelite/api/GraphicsObject.html

> Represents a graphics object/spotanim.

Declaration: `interface GraphicsObject extends Renderable`

## Methods (10)

### `WorldView getWorldView();`
Get the WorldEntity this spotanim is on.

### `int getId();`
The graphics object ID.

### `LocalPoint getLocation();`
The location of the object.

### `int getStartCycle();`
Get the time this spotanim starts

### `int getLevel();`
The plane the spotanim is on.

### `int getZ();`
Gets the z coordinate

### `boolean finished();`
Checks if this spotanim is done animating

### `void setFinished(boolean finished);`
Set if this spotanim is done animating.

### `Animation getAnimation();`
The animation of the spotanim

### `int getAnimationFrame();`
The frame of the current animation

_Generated from the local Oct-2026 snapshot by `tools/runelite-mcp/gen_api_docs.py`. Regenerate after updating the snapshot._
