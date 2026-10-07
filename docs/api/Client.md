# Client

`net.runelite.api.Client` — interface render contract (§D7). Accessor shapes only (heights, camera, provider, views).

Source: `/Users/tylercovalt/Documents/runelite-master/runelite-api/src/main/java/net/runelite/api/Client.java`
Apidocs: https://static.runelite.net/runelite-api/apidocs/net/runelite/api/Client.html

> Represents the RuneScape client.

Declaration: `interface Client extends OAuthApi, GameEngine`

## Methods (328)

_Large interface — render-relevant accessors called out; see MCP `api_class(Client)` for all 328 methods._

### Render-relevant subset (46 shown)

### `GameState getGameState();`
Gets the current game state.

### `void setGameState(GameState gameState);`
Sets the current game state

### `Canvas getCanvas();`

### `int getCameraX();`
Gets the x-axis coordinate of the camera. <p> This value is a local coordinate value similar to #getLocalDestinationLocation().

### `float getCameraFpX();`
Floating point camera position, x-axis

### `int getCameraY();`
Gets the y-axis coordinate of the camera. <p> This value is a local coordinate value similar to #getLocalDestinationLocation().

### `float getCameraFpY();`
Floating point camera position, y-axis

### `int getCameraZ();`
Gets the z-axis coordinate of the camera. <p> This value is a local coordinate value similar to #getLocalDestinationLocation().

### `float getCameraFpZ();`
Floating point camera position, z-axis

### `int getCameraPitch();`
Gets the pitch of the camera. <p> The value returned by this method is measured in JAU14, or Jagex Angle Unit (14 bit), where each unit is equivalent to 2π/(2^14) radians.

### `float getCameraFpPitch();`
Floating point camera pitch.

### `int getCameraYaw();`
Gets the yaw of the camera. <p> The value returned by this method is measured in JAU14, or Jagex Angle Unit (14 bit), where each unit is equivalent to 2π/(2^14) radians.

### `float getCameraFpYaw();`
Floating point camera yaw

### `int getCanvasHeight();`
Gets the canvas height

### `int getCanvasWidth();`
Gets the canvas width

### `Point getMouseCanvasPosition();`
Gets the current position of the mouse on the canvas.

### `int getGameCycle();`
Gets the local clients game cycle. <p> Note: This value is incremented every 20ms.

### `int getCameraYawTarget();`
Get the target camera yaw.

### `int getCameraPitchTarget();`
Get the target camera pitch The target pitch is the pitch the camera should use based on player input.

### `void setCameraYawTarget(int cameraYawTarget);`
Set the target camera yaw

### `void setCameraPitchTarget(int cameraPitchTarget);`
Set the target camera pitch

### `void setCameraSpeed(float speed);`
Sets the camera speed

### `void setCameraMouseButtonMask(int mask);`
Sets the mask for which mouse buttons control the camera.

### `void setCameraPitchRelaxerEnabled(boolean enabled);`
Sets whether the camera pitch can exceed the normal limits set by the RuneScape client.

### `int getCameraMode();`
Get the camera mode

### `void setCameraMode(int mode);`
Set the camera mode

### `float getCameraFocalPointX();`
Get the camera focus point x Typically this is the player position, but can be other points in cutscenes or in free camera mode.

### `void setCameraFocalPointX(float x);`
Sets the camera focus point x.

### `float getCameraFocalPointY();`
Get the camera focus point y Typically this is the player position, but can be other points in cutscenes or in free camera mode.

### `void setCameraFocalPointY(float y);`
Sets the camera focus point y.

### `float getCameraFocalPointZ();`
Get the camera focus point z Typically this is the player position, but can be other points in cutscenes or in free camera mode.

### `void setCameraFocalPointZ(float z);`
Sets the camera focus point z.

### `void setFreeCameraSpeed(int speed);`
Sets the normal moving speed when using oculus orb (default value is 12)

### `void setSkyboxColor(int skyboxColor);`
Sets the RGB color of the skybox

### `int getSkyboxColor();`
Gets the RGB color of the skybox

### `void setExpandedMapLoading(int chunks);`

### `int getExpandedMapLoading();`

### `TextureProvider getTextureProvider();`

### `WorldView getWorldView(int id);`
Get worldview by id

### `WorldView getTopLevelWorldView();`
Get the top level world view

### `boolean isCameraShakeDisabled();`
Whether camera shaking effects are disabled at e.g.

### `void setCameraShakeDisabled(boolean disabled);`
Set whether to disable camera shaking effects at e.g.

### `default int getPlane() {`
Gets the current plane the player is on. <p> This value indicates the current map level above ground level, where ground level is 0.

### `default int[][][] getTileHeights() {`
Gets a 3D array containing the heights of tiles in the current scene.

### `CameraFocusableEntity getCameraFocusEntity();`
Get the entity that the camera is focused on

### `WorldView findWorldViewFromWorldPoint(WorldPoint point);`
Find the worldview a given worldpoint belongs in

## Fields (4)

- `int DRAW_2D_ALL = ~0;` — Draw all 2D extras.
- `int DRAW_2D_NONE = 0;` — Hide all 2D extras.
- `int DRAW_2D_OVERHEAD_TEXT = 1;` — Render overhead text.
- `int DRAW_2D_OTHERS = 1 << 30;` — Render elements not otherwise specified in this bitflag.

_Generated from the local Oct-2026 snapshot by `tools/runelite-mcp/gen_api_docs.py`. Regenerate after updating the snapshot._
