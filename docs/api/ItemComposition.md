# ItemComposition

`net.runelite.api.ItemComposition` — interface render contract (§F5). Item models for ground-item rendering.

Source: `/Users/tylercovalt/Documents/runelite-master/runelite-api/src/main/java/net/runelite/api/ItemComposition.java`
Apidocs: https://static.runelite.net/runelite-api/apidocs/net/runelite/api/ItemComposition.html

> Represents the template of a specific item type.

Declaration: `interface ItemComposition extends ParamHolder`

## Methods (36)

### `String getName();`
Gets the item's name as it appears in game.

### `String getMembersName();`
Gets the real item name, even if the player is on a F2P server.

### `void setName(String name);`
Sets the item's name.

### `int getId();`
Gets the items ID.

### `int getNote();`
Gets a value specifying whether the item is noted.

### `int getLinkedNoteId();`
Gets the item ID of the noted or unnoted variant of this item. <p> Calling this method on a noted item will result in the ID of itself in unnoted form, and on an unnoted item its noted variant.

### `int getPlaceholderId();`
Gets the item ID of the normal or placeholder variant of this item. <p> Calling this method on a normal item will result in the ID of itself in placeholder form, and on a placeholder item its normal variant.

### `int getPlaceholderTemplateId();`
Gets a value specifying whether the item is a placeholder.

### `int getPrice();`
Gets the store price of the item. <p> Although not all items can be found in a store, they have a store price which can be used to calculate high and low alchemy values.

### `int getHaPrice();`
Get the high alchemy price for this item.

### `boolean isMembers();`
Checks whether the item is members only.

### `boolean isStackable();`
Checks whether the item is able to stack in a players inventory.

### `boolean isTradeable();`
Returns whether the item can be traded between players.

### `boolean isGeTradeable();`
Returns whether the item can be sold on the grand exchange.

### `String[] getInventoryActions();`
Gets an array of possible right-click menu actions the item has in a player inventory.

### `String[][] getSubops();`
The subops for each op, indexed by op id.

### `int getShiftClickActionIndex();`
Gets the menu action index of the shift-click action.

### `void setShiftClickActionIndex(int shiftClickActionIndex);`
Sets the menu action index of the shift-click action.

### `int getInventoryModel();`
Gets the model ID of the inventory item.

### `void setInventoryModel(int model);`
Set the model ID of the inventory item.

### `short[] getColorToReplace();`
Get the colors to be replaced on this item's model for this item.

### `void setColorToReplace(short[] colorsToReplace);`
Set the colors to be replaced on this item's model for this item.

### `short[] getColorToReplaceWith();`
Get the colors applied to this item's model for this item.

### `void setColorToReplaceWith(short[] colorToReplaceWith);`
Set the colors applied to this item's model for this item.

### `short[] getTextureToReplace();`
Get the textures to be replaced on this item's model for this item.

### `void setTextureToReplace(short[] textureToFind);`
Set the textures to be replaced on this item's model for this item.

### `short[] getTextureToReplaceWith();`
Get the textures applied to this item's model for this item.

### `void setTextureToReplaceWith(short[] textureToReplaceWith);`
Set the textures applied to this item's model for this item.

### `int getXan2d();`
Get the x angle for 2d item sprites used in the inventory.

### `int getYan2d();`
Get the y angle for 2d item sprites used in the inventory.

### `int getZan2d();`
Get the z angle for 2d item sprites used in the inventory.

### `void setXan2d(int angle);`
Set the x angle for 2d item sprites used in the inventory.

### `void setYan2d(int angle);`
Set the y angle for 2d item sprites used in the inventory.

### `void setZan2d(int angle);`
Set the z angle for 2d item sprites used in the inventory.

### `int getAmbient();`
Get the ambient light value

### `int getContrast();`
Get the contrast light value

_Generated from the local Oct-2026 snapshot by `tools/runelite-mcp/gen_api_docs.py`. Regenerate after updating the snapshot._
