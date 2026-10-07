# WorldArea

`net.runelite.api.coords.WorldArea` — class render contract (§D7). Footprint rectangles.

Source: `/Users/tylercovalt/Documents/runelite-master/runelite-api/src/main/java/net/runelite/api/coords/WorldArea.java`
Apidocs: https://static.runelite.net/runelite-api/apidocs/net/runelite/api/coords/WorldArea.html

> Represents an area on the world.

Declaration: `class WorldArea`

## Methods (23)

### `public WorldArea(int x, int y, int width, int height, int plane) {`

### `public WorldArea(WorldPoint location, int width, int height) {`

### `private Point getAxisDistances(WorldArea other) {`
Computes the shortest distance to another area.

### `public int distanceTo(WorldArea other) {`
Computes the shortest distance to another area.

### `public int distanceTo(WorldPoint other) {`
Computes the shortest distance to a world coordinate.

### `public int distanceTo2D(WorldArea other) {`
Computes the shortest distance to another area while ignoring the plane.

### `public int distanceTo2D(WorldPoint other) {`
Computes the shortest distance to a world coordinate.

### `public boolean contains(WorldPoint worldPoint) {`
Checks whether a tile is contained within the area and in the same plane.

### `public boolean contains2D(WorldPoint worldPoint) {`
Checks whether a tile is contained within the area while ignoring the plane.

### `public boolean isInMeleeDistance(WorldArea other) {`
Checks whether this area is within melee distance of another. <p> Melee distance is exactly 1 tile, so this method computes and returns whether the shortest distance to the passed area is directly on the outside of this areas edge.

### `public boolean isInMeleeDistance(WorldPoint other) {`
Checks whether a coordinate is within melee distance of this area.

### `public boolean intersectsWith(WorldArea other) {`
Checks whether this area intersects with another.

### `public boolean canTravelInDirection(WorldView wv, int dx, int dy) {`
Determines if the area can travel in one of the 9 directions by using the standard collision detection algorithm. <p> Note that this method does not consider other actors as a collision, but most non-boss NPCs do check for collision with some actors.

### `public boolean canTravelInDirection(WorldView wv, int dx, int dy, Predicate<? super WorldPoint> extraCondition) {`
Determines if the area can travel in one of the 9 directions by using the standard collision detection algorithm. <p> The passed x and y axis directions indicate the direction to travel in. <p> Note that this method does not normally consider other actors as a collision, but most non-boss NPCs do check for collision with some actors.

### `private Point getComparisonPoint(WorldArea other) {`
Gets the point within this area that is closest to another.

### `public boolean hasLineOfSightTo(WorldView wv, WorldArea other) {`
Determine if this WorldArea has line of sight to another WorldArea. <p> Note that the reverse isn't necessarily true, meaning this can return true while the other WorldArea does not have line of sight to this WorldArea.

### `private static boolean hasLineOfSightTo(WorldView wv, Tile from, Tile to) {`

### `public boolean hasLineOfSightTo(WorldView wv, WorldPoint other) {`
Determine if this WorldArea has line of sight to another WorldArea. <p> Note that the reverse isn't necessarily true, meaning this can return true while the other WorldArea does not have line of sight to this WorldArea.

### `private List<WorldPoint> getVisibleCandidates(WorldArea other) {`
Gets the points of another worldArea that may be viewable by this WorldArea The points are sorted by their distance to the closest point in this WorldArea

### `private static boolean isEdgePoint(WorldArea wa, WorldPoint p) {`
Checks if the given point is on the edge of the provided WorldArea

### `private boolean isVisibleCandidate(WorldArea other, WorldPoint p) {`
Prunes potentially visible WorldArea points in the other WorldArea by checking if the given point is on a visible edge.

### `public WorldPoint toWorldPoint() {`
Retrieves the southwestern most point of this WorldArea.

### `public List<WorldPoint> toWorldPointList() {`
Accumulates all the WorldPoints that this WorldArea contains and returns them as a list

## Fields (5)

- `private int x;` — The western most point of the area.
- `private int y;` — The southern most point of the area.
- `private int width;` — The width of the area.
- `private int height;` — The height of the area.
- `private int plane;` — The plane the area is on.

_Generated from the local Oct-2026 snapshot by `tools/runelite-mcp/gen_api_docs.py`. Regenerate after updating the snapshot._
