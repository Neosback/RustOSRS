# ItemLayer

`net.runelite.api.ItemLayer` — interface render contract (§C9). Ground-item stacks (must not crash the uploader).

Source: `/Users/tylercovalt/Documents/runelite-master/runelite-api/src/main/java/net/runelite/api/ItemLayer.java`
Apidocs: https://static.runelite.net/runelite-api/apidocs/net/runelite/api/ItemLayer.html

> Represents a pile of items held by a tile.

Declaration: `interface ItemLayer extends TileObject`

## Methods (4)

### `int getHeight();`
Gets the height of the layer.

### `Renderable getBottom();`
Gets the item at the bottom of the pile.

### `Renderable getMiddle();`
Gets the item at the middle of the pile.

### `Renderable getTop();`
Gets the item at the top of the pile.

_Generated from the local Oct-2026 snapshot by `tools/runelite-mcp/gen_api_docs.py`. Regenerate after updating the snapshot._
