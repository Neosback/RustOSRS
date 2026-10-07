# RuneLiteObjectController

`net.runelite.api.RuneLiteObjectController` — class render contract (§F3). Controller side of custom scene objects.

Source: `/Users/tylercovalt/Documents/runelite-master/runelite-api/src/main/java/net/runelite/api/RuneLiteObjectController.java`
Apidocs: https://static.runelite.net/runelite-api/apidocs/net/runelite/api/RuneLiteObjectController.html

Declaration: `class RuneLiteObjectController`

## Methods (4)

### `public void setLocation(LocalPoint point, int level) {`
Sets the location in the scene for the RuneLiteObjectController

### `public LocalPoint getLocation() {`

### `public void tick(int ticksSinceLastFrame) {`
Called every frame the RuneLiteObject is registered and in the scene

### `public abstract Model getModel();`
Called every frame to get a model to render.

## Fields (8)

- `private int x;`
- `private int y;`
- `private int z;`
- `private int worldView = -1;`
- `private int level;`
- `private int radius = 60;` — The radius is offset from the object position to form a 2d rectangle, and the tiles the corners are in are used to determine the min and max scene x/y the object is on.
- `private boolean drawFrontTilesFirst = false;` — If true, the rectangle computed from the radius has 1 or 2 of its sides expanded by a full tile based on the orientation the object is facing.
- `private int orientation = 0;` — The object orientation

_Generated from the local Oct-2026 snapshot by `tools/runelite-mcp/gen_api_docs.py`. Regenerate after updating the snapshot._
