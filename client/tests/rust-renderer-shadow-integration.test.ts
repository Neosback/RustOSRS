import assert from "node:assert/strict";

async function main(): Promise<void> {
    (globalThis as any).self = globalThis;

    const {
        createAnimatedLocDrawRangePatches,
        getRustRendererShadowDiagnostics,
    } = await import("../render/rust/RustShadowIntegration");

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
        expectedDrawCalls: 0,
        expectedSubmittedIndices: 0,
        drawStatsMatch: true,
    });

    const animatedMap = {
        locsAnimated: [
            {
                frame: 1,
                anim: {
                    frames: [
                        [0, 3, 1],
                        [12, 6, 1],
                    ],
                    framesAlpha: [
                        [24, 3, 1],
                        [36, 9, 1],
                    ],
                },
                getDrawRangeIndex(
                    alpha: boolean,
                    _interact: boolean,
                    lod: boolean,
                ): number {
                    if (lod) return alpha ? 7 : 6;
                    return alpha ? 5 : 4;
                },
            },
        ],
    } as any;

    assert.deepEqual(
        Array.from(createAnimatedLocDrawRangePatches(animatedMap, false, true)),
        [6, 12, 6, 1],
    );
    assert.deepEqual(
        Array.from(createAnimatedLocDrawRangePatches(animatedMap, true, false)),
        [5, 36, 9, 1],
    );

    console.log("rust renderer shadow integration smoke test passed");
}

void main().catch((error) => {
    console.error(error);
    process.exitCode = 1;
});
