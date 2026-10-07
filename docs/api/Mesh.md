# Mesh

`net.runelite.api.Mesh` — interface render contract (§D4). Shared mesh base (float verts, indices, textures).

Source: `/Users/tylercovalt/Documents/runelite-master/runelite-api/src/main/java/net/runelite/api/Mesh.java`
Apidocs: https://static.runelite.net/runelite-api/apidocs/net/runelite/api/Mesh.html

> A Model or ModelData

Declaration: `interface Mesh <T extends Mesh<T>>`

## Methods (15)

### `int getVerticesCount();`

### `float[] getVerticesX();`

### `float[] getVerticesY();`

### `float[] getVerticesZ();`

### `int getFaceCount();`

### `int[] getFaceIndices1();`

### `int[] getFaceIndices2();`

### `int[] getFaceIndices3();`

### `byte[] getFaceTransparencies();`

### `short[] getFaceTextures();`

### `T rotateY90Ccw();`
Rotates this model 90 degrees around the vertical axis.

### `T rotateY180Ccw();`
Rotates this model 180 degrees around the vertical axis.

### `T rotateY270Ccw();`
Rotates this model 270 degrees around the vertical axis.

### `T translate(int x, int y, int z);`
Offsets this model by the passed amount (1/128ths of a tile).

### `T scale(int x, int y, int z);`
Resizes this model by the passed amount (1/128ths).

_Generated from the local Oct-2026 snapshot by `tools/runelite-mcp/gen_api_docs.py`. Regenerate after updating the snapshot._
