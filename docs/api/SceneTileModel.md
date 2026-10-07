# SceneTileModel

`net.runelite.api.SceneTileModel` — interface render contract (§C4). Shaped overlay cuts (shape/rotation/faces/colors).

Source: `/Users/tylercovalt/Documents/runelite-master/runelite-api/src/main/java/net/runelite/api/SceneTileModel.java`
Apidocs: https://static.runelite.net/runelite-api/apidocs/net/runelite/api/SceneTileModel.html

> Represents the model of a tile in the current scene.

Declaration: `interface SceneTileModel`

## Methods (21)

### `int getModelUnderlay();`
Gets the underlay color of the tile.

### `int getModelOverlay();`
Gets the overlay color of the tile.

### `int getShape();`
Gets the shape mask type.

### `int getRotation();`
Gets the rotation of the tile.

### `int[] getFaceX();`

### `int[] getFaceY();`

### `int[] getFaceZ();`

### `int[] getVertexX();`

### `int[] getVertexY();`

### `int[] getVertexZ();`

### `int[] getTriangleColorA();`

### `int[] getTriangleColorB();`

### `int[] getTriangleColorC();`

### `int[] getTriangleTextureId();`

### `boolean isFlat();`

### `int getBufferOffset();`

### `void setBufferOffset(int bufferOffset);`

### `int getUvBufferOffset();`

### `void setUvBufferOffset(int bufferOffset);`

### `int getBufferLen();`

### `void setBufferLen(int bufferLen);`

_Generated from the local Oct-2026 snapshot by `tools/runelite-mcp/gen_api_docs.py`. Regenerate after updating the snapshot._
