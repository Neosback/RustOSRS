# InstanceTemplates

`net.runelite.api.InstanceTemplates` — enum render contract (§F6). Instanced chunk templates + matcher.

Source: `/Users/tylercovalt/Documents/runelite-master/runelite-api/src/main/java/net/runelite/api/InstanceTemplates.java`
Apidocs: https://static.runelite.net/runelite-api/apidocs/net/runelite/api/InstanceTemplates.html

> An enumeration of possible instance templates and the area they occupy.

Declaration: `enum InstanceTemplates`

## Methods (1)

### `public static InstanceTemplates findMatch(int chunkData) {`
Matches chunk data of an instance to the instance it belongs.

## Fields (5)

- `private final int baseX;` — The base x-axis coordinate of the instance area.
- `private final int baseY;` — The base y-axis coordinate of the instance area.
- `private final int plane;` — The plane the instance is on.
- `private final int width;` — The width of the instance area.
- `private final int height;` — The height of the instance area.

_Generated from the local Oct-2026 snapshot by `tools/runelite-mcp/gen_api_docs.py`. Regenerate after updating the snapshot._
