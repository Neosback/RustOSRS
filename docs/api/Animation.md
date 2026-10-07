# Animation

`net.runelite.api.Animation` — interface render contract (§F4). Animation state (id, frames, duration) for previews.

Source: `/Users/tylercovalt/Documents/runelite-master/runelite-api/src/main/java/net/runelite/api/Animation.java`
Apidocs: https://static.runelite.net/runelite-api/apidocs/net/runelite/api/Animation.html

> Represents an animation of a renderable

Declaration: `interface Animation`

## Methods (10)

### `int getId();`
Get the id for this animation

### `boolean isMayaAnim();`
Is this animation a newer-style "maya" animation

### `int getNumFrames();`
Get how many distinct frames this animation has.

### `int getRestartMode();`
How this animation behaves when its restarted during playback

### `void setRestartMode(int restartMode);`

### `int getDuration();`
How many frames the animation lasts

### `int getFrameStep();`
How many frames to go back when looping

### `int[] getFrameLengths();`
How many ticks each frame is. null for #isMayaAnim() animations

### `int getLeftHandItem();`
Get the left hand item for this animation

### `int getRightHandItem();`
Get the right hand item for this animation

_Generated from the local Oct-2026 snapshot by `tools/runelite-mcp/gen_api_docs.py`. Regenerate after updating the snapshot._
