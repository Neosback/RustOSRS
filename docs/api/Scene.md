# Scene

`net.runelite.api.Scene` — interface render contract (§C1). Scene root: extended tiles/settings/heights/shapes/roofs/WorldView.

Source: `/Users/tylercovalt/Documents/runelite-master/runelite-api/src/main/java/net/runelite/api/Scene.java`
Apidocs: https://static.runelite.net/runelite-api/apidocs/net/runelite/api/Scene.html

> Represents a 3D scene

Declaration: `interface Scene extends Renderable`

## Methods (28)

### `Tile[][][] getTiles();`
Gets the tiles in the scene

### `Tile[][][] getExtendedTiles();`
Get the extended scene.

### `byte[][][] getExtendedTileSettings();`
Get the extended tile settings.

### `int getDrawDistance();`

### `void setDrawDistance(int drawDistance);`

### `int getWorldViewId();`
Get the world view id of this scene

### `int getMinLevel();`
Get the minimum scene level which will be rendered

### `void setMinLevel(int minLevel);`
Set the minimum scene level which will be rendered

### `void removeTile(Tile tile);`
Remove a tile from the scene

### `void removeGameObject(GameObject gameObject);`
Remove a game object from the scene

### `void buildRoofs();`

### `int[][][] getRoofs();`

### `void setRoofRemovalMode(int flags);`

### `int getRoofRemovalMode();`

### `short[][][] getUnderlayIds();`
Get the underlay ids for the scene.

### `short[][][] getOverlayIds();`
Get the overlay ids for the scene.

### `byte[][][] getTileShapes();`
Get the shapes of the tiles for the scene.

### `int[][][] getTileHeights();`
Get the heights of the tiles on the scene.

### `int getBaseX();`
Returns the x-axis base coordinate. <p> This value is the x-axis world coordinate of tile (0, 0) in this scene (ie. the bottom-left most coordinates in the scene).

### `int getBaseY();`
Returns the y-axis base coordinate. <p> This value is the y-axis world coordinate of tile (0, 0) in this scene (ie. the bottom-left most coordinates in the scene).

### `boolean isInstance();`
Check if this scene is an instance

### `int[][][] getInstanceTemplateChunks();`
Contains a 3D array of template chunks for instanced areas. <p> The array returned is of format [z][x][y], where z is the plane, x and y the x-axis and y-axis coordinates of a tile divided by the size of a chunk. <p> The bits of the int value held by the coordinates are -1 if there is no data, structured in the following format: <pre>0 1 2 3 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 

### `int[] getMapRegions();`
Gets an array of map region IDs that are currently loaded.

### `byte getOverrideAmount();`

### `byte getOverrideHue();`

### `byte getOverrideSaturation();`

### `byte getOverrideLuminance();`

### `Model getSkybox();`

_Generated from the local Oct-2026 snapshot by `tools/runelite-mcp/gen_api_docs.py`. Regenerate after updating the snapshot._
