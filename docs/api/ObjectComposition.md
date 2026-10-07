# ObjectComposition

`net.runelite.api.ObjectComposition` — interface render contract (§D6). Sizes, multiloc morph chain, ops.

Source: `/Users/tylercovalt/Documents/runelite-master/runelite-api/src/main/java/net/runelite/api/ObjectComposition.java`
Apidocs: https://static.runelite.net/runelite-api/apidocs/net/runelite/api/ObjectComposition.html

> Information about a specific net.runelite.api.gameval.ObjectID

Declaration: `interface ObjectComposition extends ParamHolder`

## Methods (14)

### `int getId();`
Gets ID for the object.

### `String getName();`
Gets the name of the object.

### `EntityOps getOps();`
The menu ops associated with this object

### `String[] getActions();`
The 5 menuops this object has when in world.

### `int getMapSceneId();`
Gets the index of this object in the Client#getMapScene() array, or -1 if it has no map scene icon

### `void setMapSceneId(int mapSceneId);`
Set the map scene index into the Client#getMapScene() array, or -1 if it has no map scene icon

### `int getMapIconId();`
Gets the index of this object in the Client#getMapIcons() array, or -1 if it has no full map icon

### `void setMapIconId(int mapIconId);`
Set the index of the object in the Client#getMapIcons() array, or -1 if it has no map icon

### `int[] getImpostorIds();`
Get the net.runelite.api.gameval.ObjectIDs of objects this can transform into, depending on a net.runelite.api.gameval.VarbitID or net.runelite.api.gameval.VarPlayerID

### `ObjectComposition getImpostor();`
Get the object composition the player's state says this object should transmogrify into.

### `int getVarbitId();`
Gets the net.runelite.api.gameval.VarbitID used to switch this multiloc, or -1 if this is not switched by a Varbit

### `int getVarPlayerId();`
Gets the net.runelite.api.gameval.VarPlayerID used to switch this multiloc, or -1 if this is not switched by a VarPlayer

### `int getSizeX();`
Get the size of the object on the X-axis in tiles

### `int getSizeY();`
Get the size of the object on the Y-axis in tiles

_Generated from the local Oct-2026 snapshot by `tools/runelite-mcp/gen_api_docs.py`. Regenerate after updating the snapshot._
