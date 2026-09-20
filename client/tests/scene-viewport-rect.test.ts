import assert from "node:assert/strict";

import { computeSceneViewportRect } from "../render/render/viewportRect";

// Logged in: the viewport widget's layout-unit size scales up to device pixels.
assert.deepEqual(
    computeSceneViewportRect({
        fallbackWidth: 2560,
        fallbackHeight: 1426,
        layoutWidth: 1280,
        layoutHeight: 713,
        viewport: { x: 4, y: 4, width: 512, height: 334 },
    }),
    { x: 8, y: 8, width: 1024, height: 668 },
);

// No viewport widget (login screen, scene preview) on a HiDPI display: the rect
// must be the canvas, not double it.
assert.deepEqual(
    computeSceneViewportRect({
        fallbackWidth: 2560,
        fallbackHeight: 1426,
        layoutWidth: 1280,
        layoutHeight: 713,
    }),
    { x: 0, y: 0, width: 2560, height: 1426 },
);

// Same, at devicePixelRatio 1, where the bug was invisible.
assert.deepEqual(
    computeSceneViewportRect({
        fallbackWidth: 1280,
        fallbackHeight: 713,
        layoutWidth: 1280,
        layoutHeight: 713,
    }),
    { x: 0, y: 0, width: 1280, height: 713 },
);

console.log("Scene viewport rect tests passed");
