import assert from "node:assert/strict";

import { getMousePos } from "../game/InputManager";
import { toCssEvent } from "../render/render/interact/menu2";

(globalThis as any).document = { documentElement: { dataset: {} } };

const rect = {
    left: 50,
    top: 25,
    width: 1000,
    height: 500,
};
const canvas = {
    width: 2000,
    height: 1000,
    clientWidth: 1000,
    clientHeight: 500,
    getBoundingClientRect: () => rect,
};
const host = {
    canvas,
    cachedCanvasRect: null,
    cachedCanvasRectFrame: -1,
    cachedCssEventResult: { clientX: 0, clientY: 0 },
};

const event = toCssEvent(host as any, 1500, 750, 1);
assert.ok(event);
assert.deepEqual(getMousePos(canvas as any, event as MouseEvent), [1500, 750]);

console.log("Click coordinate round-trip regression test passed");
