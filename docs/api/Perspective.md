# Perspective

`net.runelite.api.Perspective` — class render contract (§D1). Tile math, trig tables, canvas projection.

Source: `/Users/tylercovalt/Documents/runelite-master/runelite-api/src/main/java/net/runelite/api/Perspective.java`
Apidocs: https://static.runelite.net/runelite-api/apidocs/net/runelite/api/Perspective.html

> A utility class containing methods to help with conversion between in-game features to canvas areas.

Declaration: `class Perspective`

## Methods (25)

### `public static Point localToCanvas(@Nonnull Client client, @Nonnull LocalPoint point, int plane) {`
Translates two-dimensional ground coordinates within the 3D world to their corresponding coordinates on the game screen.

### `public static Point localToCanvas(@Nonnull Client client, @Nonnull LocalPoint point, int plane, int heightOffset) {`
Translates two-dimensional ground coordinates within the 3D world to their corresponding coordinates on the game screen.

### `public static Point localToCanvas(@Nonnull Client client, int x, int y, int z) {`
Translates three-dimensional local coordinates within the 3D world to their corresponding coordinates on the game screen.

### `public static Point localToCanvas(@Nonnull Client client, int worldId, int x, int y, int z) {`

### `private static Point localToCanvasCpu(Client client, int x, int y, int z) {`

### `private static Point localToCanvasGpu(Client client, int x, int y, int z) {`

### `public static void modelToCanvas(Client client, int end, int x3dCenter, int y3dCenter, int z3dCenter, int rotate, float[] x3d, float[] y3d, float[] z3d, int[] x2d, int[] y2d) {`

### `public static void modelToCanvas(Client client, WorldView wv, int end, int x3dCenter, int y3dCenter, int z3dCenter, int rotate, float[] x3d, float[] y3d, float[] z3d, int[] x2d, int[] y2d) {`
Translates a model's vertices into 2d space.

### `private static void modelToCanvasProjection(Client client, WorldView wv, int end, int x3dCenter, int y3dCenter, int z3dCenter, int rotate, float[] x3d, float[] y3d, float[] z3d, int[] x2d, int[] y2d) {`

### `private static void modelToCanvasGpu(Client client, int end, int x3dCenter, int y3dCenter, int z3dCenter, int rotate, float[] x3d, float[] y3d, float[] z3d, int[] x2d, int[] y2d) {`

### `private static void modelToCanvasCpu(Client client, int end, int x3dCenter, int y3dCenter, int z3dCenter, int rotate, float[] x3d, float[] y3d, float[] z3d, int[] x2d, int[] y2d) {`

### `public static Point localToMinimap(@Nonnull Client client, @Nonnull LocalPoint point) {`
Translates two-dimensional ground coordinates within the 3D world to their corresponding coordinates on the Minimap.

### `public static Point localToMinimap(@Nonnull Client client, @Nonnull LocalPoint point, int distance) {`
Translates two-dimensional ground coordinates within the 3D world to their corresponding coordinates on the Minimap.

### `public static int getTileHeight(@Nonnull Client client, @Nonnull LocalPoint point, int plane) {`
Calculates the above ground height of a tile point.

### `public static int getFootprintTileHeight(@Nonnull Client client, @Nonnull LocalPoint p, int level, int footprintSize) {`

### `public static Polygon getCanvasTilePoly(@Nonnull Client client, @Nonnull LocalPoint localLocation) {`
Calculates a tile polygon from offset worldToScreen() points.

### `public static Polygon getCanvasTilePoly(@Nonnull Client client, @Nonnull LocalPoint localLocation, int zOffset) {`
Calculates a tile polygon from offset worldToScreen() points.

### `public static Polygon getCanvasTileAreaPoly(@Nonnull Client client, @Nonnull LocalPoint localLocation, int size) {`
Returns a polygon representing an area.

### `public static Polygon getCanvasTileAreaPoly( int sizeX, int sizeY, int level, int heightOffset) {`
Returns a polygon representing an area.

### `public static Point getCanvasTextLocation( int zOffset) {`
Calculates text position and centers depending on string length.

### `public static Point getCanvasImageLocation( int zOffset) {`
Calculates image position and centers depending on image size.

### `public static Point getCanvasSpriteLocation( int zOffset) {`
Calculates sprite position and centers depending on sprite size.

### `public static Shape getClickbox(@Nonnull Client client, WorldView wv, Model model, int orientation, int x, int y, int z) {`
You don't want this.

### `private static SimplePolygon calculateAABB(Client client, WorldView wv, Model m, int jauOrient, int x, int y, int z) {`

### `private static Shapes<SimplePolygon> calculate2DBounds(Client client, WorldView wv, Model m, int jauOrient, int x, int y, int z) {`

## Fields (14)

- `public static final double UNIT = 0.0030679615d;`
- `public static final double UNIT14 = 3.834951969714103E-4D;`
- `public static final int LOCAL_COORD_BITS = 7;`
- `public static final int LOCAL_TILE_SIZE = 1 << LOCAL_COORD_BITS;`
- `public static final int LOCAL_HALF_TILE_SIZE = LOCAL_TILE_SIZE / 2;`
- `public static final int SCENE_SIZE = Constants.SCENE_SIZE;`
- `public static final int[] SINE = new int[2048];`
- `public static final int[] COSINE = new int[2048];`
- `public static final float[] SINEF = new float[2048];`
- `public static final float[] COSINEF = new float[2048];`
- `public static final int[] SINE14 = new int[0x4000];`
- `public static final int[] COSINE14 = new int[0x4000];`
- `public static final float[] SINEF14 = new float[0x4000];`
- `public static final float[] COSINEF14 = new float[0x4000];`

_Generated from the local Oct-2026 snapshot by `tools/runelite-mcp/gen_api_docs.py`. Regenerate after updating the snapshot._
