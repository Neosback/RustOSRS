# CollisionData

`net.runelite.api.CollisionData` — interface render contract (§F2). Per-tile collision words for the validity overlay.

Source: `/Users/tylercovalt/Documents/runelite-master/runelite-api/src/main/java/net/runelite/api/CollisionData.java`
Apidocs: https://static.runelite.net/runelite-api/apidocs/net/runelite/api/CollisionData.html

> Represents tile collision data for the scene

Declaration: `interface CollisionData`

## Methods (1)

### `int[][] getFlags();`
Gets a 2D array of tile collision flags. <p> The array covers all tiles in the scene (104x104), and the index into the array is of format [x][y] where x and y are the tiles scene coordinates, respectively. <p> Collision flags are checked using the bitwise and (&amp;) operator.

_Generated from the local Oct-2026 snapshot by `tools/runelite-mcp/gen_api_docs.py`. Regenerate after updating the snapshot._
