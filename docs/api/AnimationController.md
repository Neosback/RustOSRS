# AnimationController

`net.runelite.api.AnimationController` — class render contract (§F4). tick()/animate(Model) skeletal posing.

Source: `/Users/tylercovalt/Documents/runelite-master/runelite-api/src/main/java/net/runelite/api/AnimationController.java`
Apidocs: https://static.runelite.net/runelite-api/apidocs/net/runelite/api/AnimationController.html

Declaration: `class AnimationController`

## Methods (9)

### `public AnimationController(Client client, int animationID) {`

### `public AnimationController(Client client, Animation animation) {`

### `public void setAnimation(@Nullable Animation animation) {`

### `public void reset() {`

### `public void loop() {`

### `public void tick(int ticks) {`

### `public Model animate(Model model) {`

### `public Model animate(Model model, @Nullable AnimationController other) {`

### `private int getPackedFrame() {`

## Fields (5)

- `private final Client client;`
- `private Animation animation;`
- `private Consumer<AnimationController> onFinished = AnimationController::loop;`
- `private int frame;`
- `private int elapsedTicks;`

_Generated from the local Oct-2026 snapshot by `tools/runelite-mcp/gen_api_docs.py`. Regenerate after updating the snapshot._
