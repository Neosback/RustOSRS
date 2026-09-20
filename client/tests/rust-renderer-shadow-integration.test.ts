import assert from "node:assert/strict";

async function main(): Promise<void> {
    (globalThis as any).self = globalThis;

    const {
        RUST_DRAW_HASH_OFFSET_BASIS,
        countExpectedDrawRanges,
        createAnimatedLocDrawRangePatches,
        getRustRendererShadowDiagnostics,
        hashExpectedDrawRanges,
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
        drawHash: 0,
        expectedDrawHash: 0,
        drawSequenceMatch: true,
        staticParityMatch: true,
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

    assert.deepEqual(
        countExpectedDrawRanges(
            [
                [0, 3, 1],
                [12, 6, 2],
                [36, 9, 1],
            ],
            new Uint8Array([0, 2, 1]),
            1,
        ),
        {
            drawCalls: 2,
            submittedIndices: 12,
        },
    );

    assert.deepEqual(
        countExpectedDrawRanges(
            [
                [0, 3, 1],
                [12, 6, 2],
            ],
            new Uint8Array([0, 0]),
            3,
            new Uint32Array([
                1,
                48,
                9,
                3,
            ]),
        ),
        {
            drawCalls: 2,
            submittedIndices: 30,
        },
    );

    assert.deepEqual(
        countExpectedDrawRanges(
            [
                [0, 3, 1],
                [12, 0, 1],
                [12, 6, 0],
            ],
            undefined,
            3,
        ),
        {
            drawCalls: 1,
            submittedIndices: 3,
        },
    );

    let drawHash = hashExpectedDrawRanges(
        RUST_DRAW_HASH_OFFSET_BASIS,
        0x3232,
        false,
        false,
        0,
        [[0, 3, 1]],
        new Uint8Array([0]),
        3,
    );
    drawHash = hashExpectedDrawRanges(
        drawHash,
        0x3232,
        true,
        false,
        1,
        [
            [0, 0, 1],
            [0, 3, 1],
            [48, 6, 1],
        ],
        new Uint8Array([0, 2, 1]),
        1,
    );
    assert.equal(drawHash, 0xdceda6f5);

    console.log("rust renderer shadow integration smoke test passed");
}

void main().catch((error) => {
    console.error(error);
    process.exitCode = 1;
});
