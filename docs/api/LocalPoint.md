# LocalPoint

`net.runelite.api.coords.LocalPoint` — class render contract (§D7). Scene-unit coordinates.

Source: `/Users/tylercovalt/Documents/runelite-master/runelite-api/src/main/java/net/runelite/api/coords/LocalPoint.java`
Apidocs: https://static.runelite.net/runelite-api/apidocs/net/runelite/api/coords/LocalPoint.html

> A two-dimensional point in the local coordinate space. <p> Local points are immutable, however since the local coordinate space moves, it is not safe to keep a LocalPoint after a loading zone. <p> The unit of a LocalPoint is 1/128th of a tile.

Declaration: `class LocalPoint`

## Methods (17)

### `public LocalPoint(int x, int y, WorldView wv) {`

### `public LocalPoint(int x, int y) {`

### `public static LocalPoint fromWorld(Client client, WorldPoint point) {`

### `public static LocalPoint fromWorld(WorldView wv, WorldPoint world) {`
Gets the local coordinate at the center of the passed tile.

### `public static LocalPoint fromWorld(Client client, int x, int y) {`

### `public static LocalPoint fromWorld(WorldView wv, int x, int y) {`
Gets the local coordinate at the center of the passed tile.

### `public static LocalPoint fromWorld(Scene scene, int x, int y) {`
Gets the local coordinate at the center of the passed tile.

### `public int distanceTo(LocalPoint other) {`
Gets the distance between this point and another.

### `public boolean isInScene() {`
Test if this point is in the basic 104x104 tile scene.

### `public static LocalPoint fromScene(int x, int y) {`
Gets the coordinate at the center of the passed tile.

### `public static LocalPoint fromScene(int x, int y, Scene scene) {`
Gets the coordinate at the center of the passed tile.

### `public static LocalPoint fromScene(int x, int y, WorldView wv) {`
Gets the coordinate at the center of the passed tile.

### `public int getSceneX() {`
Gets the x-axis coordinate in scene space (tiles).

### `public int getSceneY() {`
Gets the y-axis coordinate in scene space (tiles).

### `public LocalPoint dx(int dx) {`

### `public LocalPoint dy(int dy) {`

### `public LocalPoint plus(int dx, int dy) {`

## Fields (3)

- `private final int x, y;` — X and Y axis coordinates.
- `private final int x, y;` — X and Y axis coordinates.
- `private final int worldView;`

_Generated from the local Oct-2026 snapshot by `tools/runelite-mcp/gen_api_docs.py`. Regenerate after updating the snapshot._
