# CollisionDataFlag

`net.runelite.api.CollisionDataFlag` — class render contract (§F2). Collision flag constants.

Source: `/Users/tylercovalt/Documents/runelite-master/runelite-api/src/main/java/net/runelite/api/CollisionDataFlag.java`
Apidocs: https://static.runelite.net/runelite-api/apidocs/net/runelite/api/CollisionDataFlag.html

> A utility class containing collision data flags for tiles.

Declaration: `class CollisionDataFlag`

## Methods (0)

## Fields (17)

- `public static final int BLOCK_MOVEMENT_NORTH_WEST = 0x1;` — Directional movement blocking flags.
- `public static final int BLOCK_MOVEMENT_NORTH = 0x2;`
- `public static final int BLOCK_MOVEMENT_NORTH_EAST = 0x4;`
- `public static final int BLOCK_MOVEMENT_EAST = 0x8;`
- `public static final int BLOCK_MOVEMENT_SOUTH_EAST = 0x10;`
- `public static final int BLOCK_MOVEMENT_SOUTH = 0x20;`
- `public static final int BLOCK_MOVEMENT_SOUTH_WEST = 0x40;`
- `public static final int BLOCK_MOVEMENT_WEST = 0x80;`
- `public static final int BLOCK_MOVEMENT_OBJECT = 0x100;` — Movement blocking type flags.
- `public static final int BLOCK_MOVEMENT_FLOOR_DECORATION = 0x40000;`
- `public static final int BLOCK_MOVEMENT_FLOOR = 0x200000;`
- `public static final int BLOCK_MOVEMENT_FULL = BLOCK_MOVEMENT_OBJECT | BLOCK_MOVEMENT_FLOOR_DECORATION | BLOCK_MOVEMENT_FLOOR;`
- `public static final int BLOCK_LINE_OF_SIGHT_NORTH = BLOCK_MOVEMENT_NORTH << 9;` — Directional line of sight blocking flags.
- `public static final int BLOCK_LINE_OF_SIGHT_EAST = BLOCK_MOVEMENT_EAST << 9;`
- `public static final int BLOCK_LINE_OF_SIGHT_SOUTH = BLOCK_MOVEMENT_SOUTH << 9;`
- `public static final int BLOCK_LINE_OF_SIGHT_WEST = BLOCK_MOVEMENT_WEST << 9;`
- `public static final int BLOCK_LINE_OF_SIGHT_FULL = 0x20000;`

_Generated from the local Oct-2026 snapshot by `tools/runelite-mcp/gen_api_docs.py`. Regenerate after updating the snapshot._
