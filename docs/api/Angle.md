# Angle

`net.runelite.api.coords.Angle` — class render contract (§D7). JAU angle convention.

Source: `/Users/tylercovalt/Documents/runelite-master/runelite-api/src/main/java/net/runelite/api/coords/Angle.java`
Apidocs: https://static.runelite.net/runelite-api/apidocs/net/runelite/api/coords/Angle.html

> Represents an in-game orientation that uses fixed point arithmetic. <p> Angles are represented as an int value ranging from 0-2047, where the following is true: <ul> <li>0 is true South</li> <li>512 is true West</li> <li>1024 is true North</li> <li>1536 is true East</li> </ul>

Declaration: `class Angle`

## Methods (1)

### `public Direction getNearestDirection() {`
Converts the angle value to the nearest cardinal direction. <p> Each cardinal direction contains 512 angles, ranging between -256 and +256 of it's true value.

## Fields (1)

- `private final int angle;` — The raw angle value.

_Generated from the local Oct-2026 snapshot by `tools/runelite-mcp/gen_api_docs.py`. Regenerate after updating the snapshot._
