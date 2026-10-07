# JagexColor

`net.runelite.api.JagexColor` — class render contract (§D3). HSL pack/unpack + gamma conversion.

Source: `/Users/tylercovalt/Documents/runelite-master/runelite-api/src/main/java/net/runelite/api/JagexColor.java`
Apidocs: https://static.runelite.net/runelite-api/apidocs/net/runelite/api/JagexColor.html

Declaration: `class JagexColor`

## Methods (6)

### `public static short packHSL(int hue, int saturation, int luminance) {`

### `public static int unpackHue(short hsl) {`

### `public static int unpackSaturation(short hsl) {`

### `public static int unpackLuminance(short hsl) {`

### `public static String formatHSL(short hsl) {`

### `public static short rgbToHSL(int rgb, double brightness) {`

## Fields (3)

- `public static final int HUE_MAX = 63;`
- `public static final int SATURATION_MAX = 7;`
- `public static final int LUMINANCE_MAX = 127;`

_Generated from the local Oct-2026 snapshot by `tools/runelite-mcp/gen_api_docs.py`. Regenerate after updating the snapshot._
