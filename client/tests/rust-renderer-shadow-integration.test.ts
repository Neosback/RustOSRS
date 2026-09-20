import assert from "node:assert/strict";

import { getRustRendererShadowDiagnostics } from "../render/rust/RustShadowIntegration";

const fakeHost = {
    canvas: {},
} as any;

assert.deepEqual(getRustRendererShadowDiagnostics(fakeHost), {
    enabled: false,
    failed: false,
    residentMaps: 0,
    visibleMaps: 0,
    eligibleMaps: 0,
    mirroredMaps: 0,
    drawCalls: 0,
    submittedIndices: 0,
});

console.log("rust renderer shadow integration smoke test passed");
