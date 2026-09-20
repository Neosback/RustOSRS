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
    const {
        compareRgbaFrames,
        getRustPixelParityInterval,
        isRustPixelParityEnabled,
        shouldCaptureRustPixelParity,
    } = await import("../render/rust/RustPixelParity");

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
        expectedWorldEntityGhostPasses: 0,
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

    assert.equal(
        isRustPixelParityEnabled(
            "?rust-renderer=shadow&rust-pixel-parity=1",
        ),
        true,
    );
    assert.equal(
        isRustPixelParityEnabled(
            "?rust-renderer=off&rust-pixel-parity=1",
        ),
        false,
    );
    assert.equal(getRustPixelParityInterval(""), 120);
    assert.equal(
        getRustPixelParityInterval("?rust-pixel-every=3"),
        3,
    );

    const cadenceHost = {} as any;
    const cadenceSearch =
        "?rust-renderer=shadow&rust-pixel-parity=1&rust-pixel-every=3";
    assert.equal(
        shouldCaptureRustPixelParity(cadenceHost, cadenceSearch),
        true,
    );
    assert.equal(
        shouldCaptureRustPixelParity(cadenceHost, cadenceSearch),
        false,
    );
    assert.equal(
        shouldCaptureRustPixelParity(cadenceHost, cadenceSearch),
        false,
    );
    assert.equal(
        shouldCaptureRustPixelParity(cadenceHost, cadenceSearch),
        true,
    );

    const pixelMetrics = compareRgbaFrames(
        {
            width: 2,
            height: 1,
            pixels: new Uint8Array([
                0, 10, 20, 255,
                50, 60, 70, 255,
            ]),
        },
        {
            width: 2,
            height: 1,
            pixels: new Uint8Array([
                0, 10, 20, 255,
                50, 65, 70, 255,
            ]),
        },
    );
    assert.equal(pixelMetrics.dimensionMatch, true);
    assert.equal(pixelMetrics.totalPixels, 2);
    assert.equal(pixelMetrics.differentPixels, 1);
    assert.equal(pixelMetrics.mismatchRatio, 0.5);
    assert.equal(pixelMetrics.maxChannelDelta, 5);
    assert.equal(pixelMetrics.exactMatch, false);

    console.log("rust renderer shadow integration smoke test passed");
}

void main().catch((error) => {
    console.error(error);
    process.exitCode = 1;
});
