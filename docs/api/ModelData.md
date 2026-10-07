# ModelData

`net.runelite.api.ModelData` — interface render contract (§D4). Mutable mesh: clones, normals, recolor, resize, toModel.

Source: `/Users/tylercovalt/Documents/runelite-master/runelite-api/src/main/java/net/runelite/api/ModelData.java`
Apidocs: https://static.runelite.net/runelite-api/apidocs/net/runelite/api/ModelData.html

> An unlit model

Declaration: `interface ModelData extends Mesh<ModelData>, Renderable`

## Methods (11)

### `short[] getFaceColors();`
Gets colors as Jagex HSL

### `Model light(int ambient, int contrast, int x, int y, int z);`
Lights a model.

### `Model light();`
Lights a model with default values

### `ModelData recolor(short colorToReplace, short colorToReplaceWith);`
Applies a recolor using Jagex's HSL format.

### `ModelData retexture(short find, short replace);`
Applies a retexture, changing texture ids.

### `ModelData shallowCopy();`
Shallow-copies a model.

### `ModelData cloneVertices();`
Clones #getVerticesX(), #getVerticesY(), and #getVerticesZ() so they can be safely mutated

### `ModelData cloneColors();`
Clones #getFaceColors() so they can be safely mutated

### `ModelData cloneTextures();`
Clones #getFaceTextures() so they can be safely mutated

### `ModelData cloneTransparencies();`
Clones #getFaceTransparencies() so they can be safely mutated

### `ModelData cloneTransparencies(boolean force);`
Clones #getFaceTransparencies() so they can be safely mutated

## Fields (5)

- `int DEFAULT_AMBIENT = 64;`
- `int DEFAULT_CONTRAST = 768;`
- `int DEFAULT_X = -50;`
- `int DEFAULT_Y = -10;`
- `int DEFAULT_Z = -50;`

_Generated from the local Oct-2026 snapshot by `tools/runelite-mcp/gen_api_docs.py`. Regenerate after updating the snapshot._
