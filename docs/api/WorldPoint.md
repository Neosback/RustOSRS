# WorldPoint

`net.runelite.api.coords.WorldPoint` — class render contract (§D7). Global tile coordinates.

Source: `/Users/tylercovalt/Documents/runelite-master/runelite-api/src/main/java/net/runelite/api/coords/WorldPoint.java`
Apidocs: https://static.runelite.net/runelite-api/apidocs/net/runelite/api/coords/WorldPoint.html

> A three-dimensional point representing the coordinate of a Tile. <p> WorldPoints are immutable.

Declaration: `class WorldPoint`

## Methods (36)

### `public WorldPoint dx(int dx) {`
Offsets the x-axis coordinate by the passed value.

### `public WorldPoint dy(int dy) {`
Offsets the y-axis coordinate by the passed value.

### `public WorldPoint dz(int dz) {`
Offsets the plane by the passed value.

### `public static boolean isInScene(Scene scene, int x, int y) {`
Checks whether a tile is located in the current scene.

### `public static boolean isInScene(Client client, int x, int y) {`
Checks whether a tile is located in the current scene.

### `public static boolean isInScene(WorldView wv, int x, int y) {`
Checks whether a tile is located in the current scene.

### `public boolean isInScene(Client client) {`
Checks whether this tile is located in the current scene.

### `public static WorldPoint fromLocal(Client client, LocalPoint local) {`
Gets the coordinate of the tile that contains the passed local point.

### `public static WorldPoint fromLocal(WorldView wv, int x, int y, int plane) {`
Gets the coordinate of the tile that contains the passed local point.

### `public static WorldPoint fromLocal(Scene scene, int x, int y, int plane) {`
Gets the coordinate of the tile that contains the passed local point.

### `public static WorldPoint fromLocal(Client client, int x, int y, int plane) {`
Gets the coordinate of the tile that contains the passed local point.

### `public static WorldPoint fromLocalInstance(Client client, LocalPoint localPoint) {`
Gets the coordinate of the tile that contains the passed local point, accounting for instances.

### `public static WorldPoint fromLocalInstance(Client client, LocalPoint localPoint, int plane) {`
Gets the coordinate of the tile that contains the passed local point, accounting for instances.

### `public static WorldPoint fromLocalInstance(Scene scene, LocalPoint localPoint, int plane) {`
Gets the coordinate of the tile that contains the passed local point, accounting for instances.

### `private static WorldPoint fromLocalInstance(int[][][] instanceTemplateChunks, LocalPoint localPoint, int plane) {`

### `public static Collection<WorldPoint> toLocalInstance(Client client, WorldPoint worldPoint) {`
Get occurrences of a tile on the scene, accounting for instances.

### `public static Collection<WorldPoint> toLocalInstance(WorldView wv, WorldPoint worldPoint) {`
Get occurrences of a tile on the scene, accounting for instances.

### `public static Collection<WorldPoint> toLocalInstance(Scene scene, WorldPoint worldPoint) {`
Get occurrences of a tile on the scene, accounting for instances.

### `private static Collection<WorldPoint> toLocalInstance(int[][][] instanceTemplateChunks, int baseX, int baseY, WorldPoint worldPoint) {`

### `private static WorldPoint rotate(WorldPoint point, int rotation) {`
Rotate the coordinates in the chunk according to chunk rotation

### `public int distanceTo(WorldArea other) {`
Gets the shortest distance from this point to a WorldArea.

### `public int distanceTo(WorldPoint other) {`
Gets the distance between this point and another. <p> If the other point is not on the same plane, this method will return Integer#MAX_VALUE.

### `public int distanceTo2D(WorldPoint other) {`
Find the distance from this point to another point. <p> This method disregards the plane value of the two tiles and returns the simple distance between the X-Z coordinate pairs.

### `public static WorldPoint fromScene(Client client, int x, int y, int plane) {`

### `public static WorldPoint fromScene(WorldView wv, int x, int y, int plane) {`
Converts the passed scene coordinates to a world space

### `public static WorldPoint fromScene(Scene scene, int x, int y, int plane) {`
Converts the passed scene coordinates to a world space

### `public int getRegionID() {`
Gets the ID of the region containing this tile.

### `public static WorldPoint fromRegion(int regionId, int regionX, int regionY, int plane) {`
Converts the passed region ID and coordinates to a world coordinate

### `public int getRegionX() {`
Gets the X-axis coordinate of the region coordinate

### `public int getRegionY() {`
Gets the Y-axis coordinate of the region coordinate

### `private static int getRegionOffset(final int position) {`

### `public static WorldPoint getMirrorPoint(WorldPoint worldPoint, boolean toOverworld) {`
Translate a coordinate either between overworld and real, or real and overworld

### `public boolean isInArea(WorldArea... worldAreas) {`
Checks whether this tile is located within any of the given areas.

### `public boolean isInArea2D(WorldArea... worldAreas) {`
Checks whether this tile is located within any of the given areas, disregarding any plane differences.

### `public WorldArea toWorldArea() {`
Retrieves an area consisting of only this point.

### `public static WorldPoint fromCoord(int c) {`
Create a WorldPoint from a packed Jagex coordinate

## Fields (3)

- `private final int x;` — X-axis coordinate.
- `private final int y;` — Y-axis coordinate.
- `private final int plane;` — The plane level of the Tile, also referred as z-axis coordinate.

_Generated from the local Oct-2026 snapshot by `tools/runelite-mcp/gen_api_docs.py`. Regenerate after updating the snapshot._
