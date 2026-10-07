# Model

`net.runelite.api.Model` — interface render contract (§D4). Renderable mesh: faces, priorities, bias, textures, alpha.

Source: `/Users/tylercovalt/Documents/runelite-master/runelite-api/src/main/java/net/runelite/api/Model.java`
Apidocs: https://static.runelite.net/runelite-api/apidocs/net/runelite/api/Model.html

> Represents the model of an object.

Declaration: `interface Model extends Mesh<Model>, Renderable`

## Methods (35)

### `int[] getFaceColors1();`

### `int[] getFaceColors2();`

### `int[] getFaceColors3();`

### `short[] getUnlitFaceColors();`

### `int getSceneId();`

### `void setSceneId(int sceneId);`

### `int getBufferOffset();`

### `void setBufferOffset(int bufferOffset);`

### `int getUvBufferOffset();`

### `void setUvBufferOffset(int bufferOffset);`

### `int getBottomY();`

### `void calculateBoundsCylinder();`

### `byte getTransparency();`

### `byte[] getFaceRenderPriorities();`

### `byte[] getFaceBias();`

### `int getRadius();`

### `int getDiameter();`

### `void calculateExtreme(int orientation);`

### `AABB getAABB(int orientation);`

### `int getXYZMag();`

### `boolean useBoundingBox();`

### `int[] getVertexNormalsX();`

### `int[] getVertexNormalsY();`

### `int[] getVertexNormalsZ();`

### `byte getOverrideAmount();`

### `byte getOverrideHue();`

### `byte getOverrideSaturation();`

### `byte getOverrideLuminance();`

### `byte[] getTextureFaces();`

### `int[] getTexIndices1();`

### `int[] getTexIndices2();`

### `int[] getTexIndices3();`

### `Model getUnskewedModel();`

### `void drawFrustum(int zero, int xRotate, int yRotate, int zRotate, int xCamera, int yCamera, int zCamera);`

### `void drawOrtho(int zero, int xRotate, int yRotate, int zRotate, int xCamera, int yCamera, int zCamera, int zoom);`

_Generated from the local Oct-2026 snapshot by `tools/runelite-mcp/gen_api_docs.py`. Regenerate after updating the snapshot._
