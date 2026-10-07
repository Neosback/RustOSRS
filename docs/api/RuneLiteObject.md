# RuneLiteObject

`net.runelite.api.RuneLiteObject` — class render contract (§F3). Custom scene objects: placement ghosts/markers.

Source: `/Users/tylercovalt/Documents/runelite-master/runelite-api/src/main/java/net/runelite/api/RuneLiteObject.java`
Apidocs: https://static.runelite.net/runelite-api/apidocs/net/runelite/api/RuneLiteObject.html

Declaration: `class RuneLiteObject extends RuneLiteObjectController`

## Methods (14)

### `public void setShouldLoop(boolean shouldLoop) {`
Sets whether the animation of the RuneLiteObject should loop when the animation ends.

### `public void setModel(Model baseModel) {`
Sets the model to be rendered.

### `public void setLocation(LocalPoint point, int level) {`
Sets the location in the scene for the RuneLiteObject

### `public void setAnimation(Animation animation) {`
Sets the animation of the RuneLiteObject.

### `public void setAnimationController(@Nullable AnimationController animationController) {`
Sets the animation controller of the RuneLiteObject.

### `public void setActive(boolean active) {`
Sets the state of the RuneLiteObject.

### `public boolean isActive() {`
Gets the state of the RuneLiteObject

### `public void tick(int ticksSinceLastFrame) {`
Called every frame the RuneLiteObject is registered and in the scene

### `public Model getModel() {`
Called every frame to get a model to render.

### `public boolean finished() {`

### `public void setFinished(boolean finished) {`

### `public Animation getAnimation() {`

### `public int getAnimationFrame() {`

### `private void updateAnimationControllerLooping() {`

## Fields (6)

- `private final Client client;`
- `private Model baseModel;`
- `private AnimationController animationController;` — The animation of the RuneLiteObject.
- `private AnimationController poseAnimationController;` — The optional pose animation of the RuneLiteObject.
- `private int startCycle;`
- `private Boolean shouldLoop;`

_Generated from the local Oct-2026 snapshot by `tools/runelite-mcp/gen_api_docs.py`. Regenerate after updating the snapshot._
