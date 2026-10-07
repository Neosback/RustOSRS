# DrawCallbacks

`net.runelite.api.hooks.DrawCallbacks` — interface render contract (§A10). GPU flags, passes, per-zone/terrain/frustum hooks.

Source: `/Users/tylercovalt/Documents/runelite-master/runelite-api/src/main/java/net/runelite/api/hooks/DrawCallbacks.java`
Apidocs: https://static.runelite.net/runelite-api/apidocs/net/runelite/api/hooks/DrawCallbacks.html

Declaration: `interface DrawCallbacks`

## Methods (24)

### `static int RENDER_THREADS(int num) {`

### `default void draw(Projection projection, Scene scene, Renderable renderable, int orientation, int x, int y, int z, long hash) {`

### `default void drawScenePaint(Scene scene, SceneTilePaint paint, int plane, int tileX, int tileZ) {`

### `default void drawSceneTileModel(Scene scene, SceneTileModel model, int tileX, int tileZ) {`

### `void draw(int overlayColor);`
Called when a frame should be drawn.

### `default void drawScene(double cameraX, double cameraY, double cameraZ, double cameraPitch, double cameraYaw, int plane) {`
Called before the scene is drawn

### `default void postDrawScene() {`
Called after the scene has been drawn

### `default void animate(Texture texture, int diff) {`

### `default void loadScene(Scene scene) {`

### `void swapScene(Scene scene);`

### `default boolean tileInFrustum(Scene scene, float pitchSin, float pitchCos, float yawSin, float yawCos, int cameraX, int cameraY, int cameraZ, int plane, int msx, int msy) {`

### `default boolean zoneInFrustum(int zoneX, int zoneZ, int maxY, int minY) {`

### `default void loadScene(WorldView worldView, Scene scene) {`

### `default void despawnWorldView(WorldView worldView) {`

### `default void preSceneDraw( Scene scene, Projection entityProjection, float cameraX, float cameraY, float cameraZ, float cameraPitch, float cameraYaw, int minLevel, int level, int maxLevel, Set<Integer> hideRoofIds) {`

### `default void preSceneDraw( Scene scene, float cameraX, float cameraY, float cameraZ, float cameraPitch, float cameraYaw, int minLevel, int level, int maxLevel, Set<Integer> hideRoofIds) {`

### `default void postSceneDraw(Scene scene) {`

### `default void drawPass(Projection entityProjection, Scene scene, int pass) {`

### `default void drawZoneOpaque(Projection entityProjection, Scene scene, int zx, int zz) {`

### `default void drawZoneAlpha(Projection entityProjection, Scene scene, int level, int zx, int zz) {`

### `default void drawDynamic(Projection worldProjection, Scene scene, TileObject tileObject, Renderable r, Model m, int orient, int x, int y, int z) {`

### `default void drawDynamic(int renderThreadId, Projection worldProjection, Scene scene, TileObject tileObject, Renderable r, Model m, int orient, int x, int y, int z) {`

### `default void drawTemp(Projection worldProjection, Scene scene, GameObject gameObject, Model m, int orient, int x, int y, int z) {`

### `default void invalidateZone(Scene scene, int zx, int zz) {`

## Fields (12)

- `int GPU = 0x1;` — GPU mode on.
- `int HILLSKEW = 0x2;` — GPU hillskew support.
- `int NORMALS = 0x4;` — Requests normals be computed for models.
- `int NO_VERTEX_SNAPPING = 0x8;` — Disable vertex snapping for animations
- `int ZBUF = 0x10;` — Enable zbuf renderer.
- `int ZBUF_ZONE_FRUSTUM_CHECK = 0x20;` — Enable the #zoneInFrustum(int, int, int, int) callback
- `int UNLIT_FACE_COLORS = 0x40;` — Enable the Model#getUnlitFaceColors() method
- `int RENDER_THREADS_MASK = 15;`
- `int RENDER_THREADS_SHIFT = 7;`
- `int PASS_OPAQUE = 0;`
- `int PASS_ALPHA = 1;`
- `int PRE_PASS_ALPHA = 2;`

_Generated from the local Oct-2026 snapshot by `tools/runelite-mcp/gen_api_docs.py`. Regenerate after updating the snapshot._
