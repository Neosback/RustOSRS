# WorldView

`net.runelite.api.WorldView` — interface render contract (§C10). TOPLEVEL/instance coords, tile-height helper.

Source: `/Users/tylercovalt/Documents/runelite-master/runelite-api/src/main/java/net/runelite/api/WorldView.java`
Apidocs: https://static.runelite.net/runelite-api/apidocs/net/runelite/api/WorldView.html

Declaration: `interface WorldView`

## Methods (27)

### `int getId();`
Get the world view id

### `boolean isTopLevel();`
Test if this worldview is the top level world view.

### `Scene getScene();`
Gets the worldview's scene

### `IndexedObjectSet<? extends Player> players();`
Gets all of the Players in this view

### `IndexedObjectSet<? extends NPC> npcs();`
Gets all the Non Player Characters in this view

### `IndexedObjectSet<? extends WorldEntity> worldEntities();`
Gets all the WorldEntities in this view

### `IndexedObjectSet<? extends WorldView> worldViews();`
Get the worldviews of each worldentity in this worldview.

### `CollisionData[] getCollisionMaps();`
Gets an array of tile collision data. <p> The index into the array is the plane/z-axis coordinate.

### `int getPlane();`
Gets the current plane the player is on. <p> This value indicates the current map level above ground level, where ground level is 0.

### `int[][][] getTileHeights();`
Gets a 3D array containing the heights of tiles in the current scene.

### `byte[][][] getTileSettings();`
Gets a 3D array containing the settings of tiles in the current scene.

### `int getSizeX();`
Get the size of the world view, x-axis

### `int getSizeY();`
Get the size of the world view, y-axis

### `int getBaseX();`
Returns the x-axis base coordinate. <p> This value is the x-axis world coordinate of tile (0, 0) in the current scene (ie. the bottom-left most coordinates in the scene).

### `int getBaseY();`
Returns the y-axis base coordinate. <p> This value is the y-axis world coordinate of tile (0, 0) in the current scene (ie. the bottom-left most coordinates in the scene).

### `Projectile createProjectile(int id, int plane, int startX, int startY, int startZ, int startCycle, int endCycle, int slope, int startHeight, int endHeight, @Nullable Actor target, int targetX, int targetY);`
Create a projectile.

### `Deque<GraphicsObject> getGraphicsObjects();`
Gets a list of all graphics objects currently drawn.

### `Tile getSelectedSceneTile();`
Gets the currently selected tile. (ie. last right clicked tile)

### `boolean isInstance();`
Check if this scene is an instance

### `int[][][] getInstanceTemplateChunks();`
Contains a 3D array of template chunks for instanced areas. <p> The array returned is of format [z][x][y], where z is the plane, x and y the x-axis and y-axis coordinates of a tile divided by the size of a chunk. <p> The bits of the int value held by the coordinates are -1 if there is no data, structured in the following format: <pre>0 1 2 3 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 

### `int[] getMapRegions();`
Gets an array of map region IDs that are currently loaded.

### `boolean contains(WorldPoint point);`
Test if this worldview contains the given point

### `boolean contains(LocalPoint point);`
Test if this worldview contains the given point

### `Projection getMainWorldProjection();`
Returns a Projection to translate from this world view to the main world

### `Projection getCanvasProjection();`
Returns a Projection to translate from this world view to the canvas

### `int getYellowClickAction();`
Returns how clicking on tiles should behave for this WorldView.

### `int getTileHeight(int x, int y, int maplevel);`
Gets the tile height at the given coordinates, interpolating the height from adjacent tiles.

## Fields (1)

- `int TOPLEVEL = 0;`

_Generated from the local Oct-2026 snapshot by `tools/runelite-mcp/gen_api_docs.py`. Regenerate after updating the snapshot._
