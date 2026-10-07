# SceneTilePaint

`net.runelite.api.SceneTilePaint` — interface render contract (§C3). Flat underlay quads (corner HSL, texture, flat flag).

Source: `/Users/tylercovalt/Documents/runelite-master/runelite-api/src/main/java/net/runelite/api/SceneTilePaint.java`
Apidocs: https://static.runelite.net/runelite-api/apidocs/net/runelite/api/SceneTilePaint.html

> Represents the paint of a tile in the current scene.

Declaration: `interface SceneTilePaint`

## Methods (18)

### `int getRBG();`
Gets the RGB value of the paint.

### `int getSwColor();`
Gets the color of the south-west corner of the tile.

### `void setSwColor(int color);`
Sets the color of the south-west corner of the tile.

### `int getSeColor();`
Gets the color of the south-east corner of the tile.

### `void setSeColor(int color);`
Sets the color of the south-east corner of the tile.

### `int getNwColor();`
Gets the color of the north-west corner of the tile.

### `void setNwColor(int color);`
Sets the color of the north-west corner of the tile.

### `int getNeColor();`
Gets the color of the north-east corner of the tile.

### `void setNeColor(int color);`
Sets the color of the north-east corner of the tile.

### `int getTexture();`
Gets the texture to be rendered for the tile.

### `void setTexture(int texture);`
Sets the texture to be rendered for the tile.

### `boolean isFlat();`

### `int getBufferOffset();`

### `void setBufferOffset(int bufferOffset);`

### `int getUvBufferOffset();`

### `void setUvBufferOffset(int bufferOffset);`

### `int getBufferLen();`

### `void setBufferLen(int bufferLen);`

_Generated from the local Oct-2026 snapshot by `tools/runelite-mcp/gen_api_docs.py`. Regenerate after updating the snapshot._
