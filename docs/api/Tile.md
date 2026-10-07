# Tile

`net.runelite.api.Tile` — interface render contract (§C2). Per-tile slots + bridge/render-level contract.

Source: `/Users/tylercovalt/Documents/runelite-master/runelite-api/src/main/java/net/runelite/api/Tile.java`
Apidocs: https://static.runelite.net/runelite-api/apidocs/net/runelite/api/Tile.html

> Represents a tile in the game.

Declaration: `interface Tile`

## Methods (17)

### `DecorativeObject getDecorativeObject();`
Gets the decoration on the tile.

### `GameObject[] getGameObjects();`
Gets all game objects on the tile.

### `ItemLayer getItemLayer();`
Gets the items held on this tile.

### `GroundObject getGroundObject();`
Gets the object on the ground layer of the tile.

### `void setGroundObject(GroundObject groundObject);`
Sets the object on the ground layer of the tile.

### `WallObject getWallObject();`
Gets the wall of the tile.

### `SceneTilePaint getSceneTilePaint();`
Gets the scene paint of the tile.

### `void setSceneTilePaint(SceneTilePaint paint);`
Sets the scene paint of the tile.

### `SceneTileModel getSceneTileModel();`
Gets the model of the tile in the scene.

### `void setSceneTileModel(SceneTileModel model);`
Sets the model of the tile in the scene.

### `WorldPoint getWorldLocation();`
Gets the location coordinate of the tile in the world.

### `Point getSceneLocation();`
Gets the location coordinate of the tile in scene coords

### `LocalPoint getLocalLocation();`
Gets the local coordinate of the tile.

### `int getPlane();`
Gets the plane that this tile is on.

### `int getRenderLevel();`
Get the plane this tile is rendered on, which is where the tile heights are from.

### `List<TileItem> getGroundItems();`
Get all the ground items for this tile

### `Tile getBridge();`
Return the tile under this one, if this tile is a bridge

_Generated from the local Oct-2026 snapshot by `tools/runelite-mcp/gen_api_docs.py`. Regenerate after updating the snapshot._
