# TileItem

`net.runelite.api.TileItem` — interface render contract (§F5). Single ground item on an ItemLayer.

Source: `/Users/tylercovalt/Documents/runelite-master/runelite-api/src/main/java/net/runelite/api/TileItem.java`
Apidocs: https://static.runelite.net/runelite-api/apidocs/net/runelite/api/TileItem.html

> Represents an item inside an ItemLayer.

Declaration: `interface TileItem extends Renderable`

## Methods (6)

### `int getId();`

### `int getQuantity();`

### `int getVisibleTime();`
Get the time, in server ticks, when the item becomes visible to other players

### `int getDespawnTime();`
Get the time, in server ticks, when the item despawns

### `int getOwnership();`
Get the item ownership

### `boolean isPrivate();`
Test whether the item is private

## Fields (4)

- `int OWNERSHIP_NONE = 0;`
- `int OWNERSHIP_SELF = 1;`
- `int OWNERSHIP_OTHER = 2;`
- `int OWNERSHIP_GROUP = 3;`

_Generated from the local Oct-2026 snapshot by `tools/runelite-mcp/gen_api_docs.py`. Regenerate after updating the snapshot._
