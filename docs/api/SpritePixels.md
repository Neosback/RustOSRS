# SpritePixels

`net.runelite.api.SpritePixels` — interface render contract (§F5). Pixel container behind texture upload.

Source: `/Users/tylercovalt/Documents/runelite-master/runelite-api/src/main/java/net/runelite/api/SpritePixels.java`
Apidocs: https://static.runelite.net/runelite-api/apidocs/net/runelite/api/SpritePixels.html

> Represents data about the pixels of a sprite image.

Declaration: `interface SpritePixels`

## Methods (16)

### `void drawAt(int x, int y);`
Draws the pixels at the given coordinates on the canvas.

### `int getWidth();`
Gets the width of the sprite image in pixels.

### `int getHeight();`
Gets the height of the sprite image in pixels.

### `int getMaxWidth();`
Gets the max width of the sprite image in pixels.

### `int getMaxHeight();`
Gets the max height of the sprite image in pixels.

### `int getOffsetX();`
Gets the x offset of the sprite image in pixels.

### `int getOffsetY();`
Gets the y offset of the sprite image in pixels.

### `void setMaxWidth(int maxWidth);`
Sets the max width of the sprite image in pixels.

### `void setMaxHeight(int maxHeight);`
Sets the max height of the sprite image in pixels.

### `void setOffsetX(int offsetX);`
Sets the x offset of the sprite image in pixels.

### `void setOffsetY(int offsetY);`
Sets the y offset of the sprite image in pixels.

### `int[] getPixels();`
Gets an array of all pixels data in the sprite.

### `BufferedImage toBufferedImage();`
Converts the sprite into a BufferedImage.

### `void toBufferedImage(BufferedImage img) throws IllegalArgumentException;`
Writes the contents of the sprite to the given BufferedImage.

### `BufferedImage toBufferedOutline(Color color);`
Writes the contents of the SpritePixels with chosen outline to the BufferedImage

### `void toBufferedOutline(BufferedImage img, int color);`
Writes the contents of the SpritePixels with chosen outline to the BufferedImage

## Fields (1)

- `int DEFAULT_SHADOW_COLOR = 3153952;`

_Generated from the local Oct-2026 snapshot by `tools/runelite-mcp/gen_api_docs.py`. Regenerate after updating the snapshot._
