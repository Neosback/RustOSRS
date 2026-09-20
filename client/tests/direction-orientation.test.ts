import assert from "node:assert/strict";
import { DIRECTION_TO_ORIENTATION } from "../common/Direction";
import { computeFacingRotation } from "../game/utils/rotation";

assert.deepEqual(DIRECTION_TO_ORIENTATION, [256, 0, 1792, 512, 1536, 768, 1024, 1280]);
assert.equal(computeFacingRotation(0, -128), DIRECTION_TO_ORIENTATION[6]);
assert.equal(computeFacingRotation(0, 128), DIRECTION_TO_ORIENTATION[1]);

console.log("direction orientation check passed");
