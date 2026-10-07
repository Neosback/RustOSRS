# Constants

`net.runelite.api.Constants` — class render contract (§D2). Sizes, tile flags, shifts.

Source: `/Users/tylercovalt/Documents/runelite-master/runelite-api/src/main/java/net/runelite/api/Constants.java`
Apidocs: https://static.runelite.net/runelite-api/apidocs/net/runelite/api/Constants.html

> A utility class containing constant values.

Declaration: `class Constants`

## Methods (0)

## Fields (24)

- `public static final int GAME_FIXED_WIDTH = 765;` — The original width of the game when running in fixed mode.
- `public static final int GAME_FIXED_HEIGHT = 503;` — The original height of the game when running in fixed mode.
- `public static final int CLIENT_DEFAULT_ZOOM = 512;` — The default camera zoom value.
- `public static final int CHUNK_SIZE = 8;` — The width and length of a chunk (8x8 tiles).
- `public static final int REGION_SIZE = 64;` — The width and length of a map region (64x64 tiles).
- `public static final int SCENE_SIZE = 104;` — The width and length of the scene (13 chunks x 8 tiles).
- `public static final int EXTENDED_SCENE_SIZE = 184;` — Size of the extended scene.
- `public static final int MAX_Z = 4;` — The max allowed plane by the game. <p> This value is exclusive.
- `public static final int TILE_FLAG_BRIDGE = 2;`
- `public static final int TILE_FLAG_UNDER_ROOF = 4;`
- `public static final int TILE_FLAG_VIS_BELOW = 8;`
- `public static final int ROOF_FLAG_POSITION = 1;` — Flag for roof removal to remove the roofs above the player's current position.
- `public static final int ROOF_FLAG_HOVERED = 2;` — Flag for roof removal to remove the roofs above the currently hovered tile.
- `public static final int ROOF_FLAG_DESTINATION = 4;` — Flag for roof removal to remove the roofs above the player's destination tile.
- `public static final int ROOF_FLAG_BETWEEN = 8;` — Flag for roof removal to remove the roofs that are above any tile between the camera and the player.
- `public static final int OVERWORLD_MAX_Y = 4160;` — The height of the overworld, in tiles.
- `public static final int CLIENT_TICK_LENGTH = 20;` — The number of milliseconds in a client tick. <p> This is the length of a single frame when the client is running at the maximum framerate of 50 fps.
- `public static final int GAME_TICK_LENGTH = 600;` — The number of milliseconds in a server game tick. <p> This is the length of a single game cycle under ideal conditions.
- `public static final int ITEM_SPRITE_WIDTH = 36;` — Width of a standard item sprite
- `public static final int ITEM_SPRITE_HEIGHT = 32;` — Height of a standard item sprite
- `public static final float HIGH_ALCHEMY_MULTIPLIER = .6f;` — High alchemy = shop price * HIGH_ALCHEMY_MULTIPLIER
- `public static final int CLICK_ACTION_NONE = 0;`
- `public static final int CLICK_ACTION_WALK = 1;`
- `public static final int CLICK_ACTION_SET_HEADING = 2;`

_Generated from the local Oct-2026 snapshot by `tools/runelite-mcp/gen_api_docs.py`. Regenerate after updating the snapshot._
