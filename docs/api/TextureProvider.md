# TextureProvider

`net.runelite.api.TextureProvider` — interface render contract (§D5). Texture array, brightness, default colors.

Source: `/Users/tylercovalt/Documents/runelite-master/runelite-api/src/main/java/net/runelite/api/TextureProvider.java`
Apidocs: https://static.runelite.net/runelite-api/apidocs/net/runelite/api/TextureProvider.html

Declaration: `interface TextureProvider`

## Methods (5)

### `double getBrightness();`

### `void setBrightness(double brightness);`
Set the brightness for textures, clearing the texture cache. .9 is the darkest value available in the standard options .6 is the brightest value

### `Texture[] getTextures();`
Get all textures

### `int[] load(int textureId);`
Get the pixels for a texture

### `int getDefaultColor(int textureID);`
Get the HSL color used when the texture isn't loaded yet

_Generated from the local Oct-2026 snapshot by `tools/runelite-mcp/gen_api_docs.py`. Regenerate after updating the snapshot._
