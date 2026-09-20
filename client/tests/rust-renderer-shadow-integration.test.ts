import assert from "node:assert/strict";

async function main(): Promise<void> {
    (globalThis as any).self = globalThis;

    const {
        RUST_DRAW_HASH_OFFSET_BASIS,
        countExpectedDrawRanges,
        createAnimatedLocDrawRangePatches,
        createWorldEntityGhostSceneHslOverride,
        getRustRendererShadowDiagnostics,
        hashExpectedDrawRanges,
        isRustFullDynamicShadowEnabled,
        isRustFullDynamicStructuralParityMatch,
        isRustGfxShadowEnabled,
        isRustNpcShadowEnabled,
        isRustPlayerShadowEnabled,
        isRustPresentationShadowEnabled,
        isRustPrimaryRendererEnabled,
        isRustProjectileShadowEnabled,
        isRustSceneOverlayShadowEnabled,
    } = await import("../render/rust/RustShadowIntegration");
    const {
        compareRgbaFrames,
        getRustPixelParityInterval,
        isRustPixelParityEnabled,
        shouldCaptureRustPixelParity,
    } = await import("../render/rust/RustPixelParity");
    const {
        getRustRendererRuntimeMode,
        isRustPrimaryRuntime,
    } = await import("../render/rust/RustRendererRuntime");

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
        npcParityEnabled: false,
        playerParityEnabled: false,
        gfxParityEnabled: false,
        projectileParityEnabled: false,
        overlayParityEnabled: false,
        mirroredNpcPasses: 0,
        mirroredPlayerPasses: 0,
        mirroredGfxPasses: 0,
        mirroredProjectilePasses: 0,
        mirroredOverlayPasses: 0,
    });

    const structuralBase = getRustRendererShadowDiagnostics(fakeHost);
    assert.equal(
        isRustFullDynamicStructuralParityMatch(structuralBase),
        false,
    );
    assert.equal(
        isRustFullDynamicStructuralParityMatch({
            ...structuralBase,
            enabled: true,
            npcParityEnabled: true,
            playerParityEnabled: true,
            gfxParityEnabled: true,
            projectileParityEnabled: true,
            drawStatsMatch: true,
            drawSequenceMatch: true,
        }),
        true,
    );
    assert.equal(
        isRustFullDynamicStructuralParityMatch({
            ...structuralBase,
            enabled: true,
            npcParityEnabled: true,
            playerParityEnabled: true,
            gfxParityEnabled: true,
            projectileParityEnabled: true,
            drawStatsMatch: true,
            drawSequenceMatch: false,
        }),
        false,
    );

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

    assert.deepEqual(
        Array.from(
            createWorldEntityGhostSceneHslOverride(
                (12 << 10) | (3 << 7) | 64,
            )!,
        ),
        [12, 3, 64, 127],
    );
    assert.equal(
        createWorldEntityGhostSceneHslOverride(0),
        undefined,
    );

    assert.equal(
        getRustRendererRuntimeMode(""),
        "off",
    );
    assert.equal(
        getRustRendererRuntimeMode("?unrelated=1"),
        "off",
    );
    assert.equal(
        getRustRendererRuntimeMode("?rust-renderer=primary"),
        "primary",
    );
    assert.equal(
        getRustRendererRuntimeMode("?rust-renderer=shadow"),
        "shadow",
    );
    assert.equal(
        getRustRendererRuntimeMode("?rust-renderer=off"),
        "off",
    );
    assert.equal(
        isRustPrimaryRuntime("?rust-renderer=primary"),
        true,
    );
    assert.equal(
        isRustPrimaryRendererEnabled("?rust-renderer=primary"),
        true,
    );

    const primarySearch = "?rust-renderer=primary";
    assert.equal(isRustNpcShadowEnabled(primarySearch), true);
    assert.equal(isRustPlayerShadowEnabled(primarySearch), true);
    assert.equal(isRustGfxShadowEnabled(primarySearch), true);
    assert.equal(isRustProjectileShadowEnabled(primarySearch), true);
    assert.equal(isRustSceneOverlayShadowEnabled(primarySearch), true);
    assert.equal(isRustPresentationShadowEnabled(primarySearch), true);
    assert.equal(isRustFullDynamicShadowEnabled(primarySearch), true);

    assert.equal(
        isRustNpcShadowEnabled(
            "?rust-renderer=shadow&rust-npc-parity=1",
        ),
        true,
    );
    assert.equal(
        isRustNpcShadowEnabled("?rust-renderer=shadow"),
        false,
    );
    assert.equal(
        isRustNpcShadowEnabled(
            "?rust-renderer=off&rust-npc-parity=1",
        ),
        false,
    );
    assert.equal(
        isRustPlayerShadowEnabled(
            "?rust-renderer=shadow&rust-player-parity=1",
        ),
        true,
    );
    assert.equal(
        isRustPlayerShadowEnabled("?rust-renderer=shadow"),
        false,
    );
    assert.equal(
        isRustPlayerShadowEnabled(
            "?rust-renderer=off&rust-player-parity=1",
        ),
        false,
    );
    assert.equal(
        isRustGfxShadowEnabled(
            "?rust-renderer=shadow&rust-gfx-parity=1",
        ),
        true,
    );
    assert.equal(
        isRustGfxShadowEnabled("?rust-renderer=shadow"),
        false,
    );
    assert.equal(
        isRustGfxShadowEnabled(
            "?rust-renderer=off&rust-gfx-parity=1",
        ),
        false,
    );
    assert.equal(
        isRustProjectileShadowEnabled(
            "?rust-renderer=shadow&rust-projectile-parity=1",
        ),
        true,
    );
    assert.equal(
        isRustProjectileShadowEnabled("?rust-renderer=shadow"),
        false,
    );
    assert.equal(
        isRustProjectileShadowEnabled(
            "?rust-renderer=off&rust-projectile-parity=1",
        ),
        false,
    );

    assert.equal(
        isRustSceneOverlayShadowEnabled(
            "?rust-renderer=shadow&rust-overlay-parity=1",
        ),
        true,
    );
    assert.equal(
        isRustSceneOverlayShadowEnabled("?rust-renderer=shadow"),
        false,
    );
    assert.equal(
        isRustSceneOverlayShadowEnabled(
            "?rust-renderer=off&rust-overlay-parity=1",
        ),
        false,
    );

    assert.equal(
        isRustPresentationShadowEnabled(
            "?rust-renderer=shadow&rust-presentation=1",
        ),
        true,
    );
    assert.equal(
        isRustPresentationShadowEnabled("?rust-renderer=shadow"),
        false,
    );
    assert.equal(
        isRustPresentationShadowEnabled(
            "?rust-renderer=off&rust-presentation=1",
        ),
        false,
    );

    const fullDynamicSearch =
        "?rust-renderer=shadow"
        + "&rust-npc-parity=1"
        + "&rust-player-parity=1"
        + "&rust-gfx-parity=1"
        + "&rust-projectile-parity=1";
    assert.equal(
        isRustFullDynamicShadowEnabled(fullDynamicSearch),
        true,
    );
    assert.equal(
        isRustFullDynamicShadowEnabled(
            fullDynamicSearch.replace("&rust-gfx-parity=1", ""),
        ),
        false,
    );

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

    const shapeMismatch = compareRgbaFrames(
        {
            width: 2,
            height: 2,
            pixels: new Uint8Array(2 * 2 * 4),
        },
        {
            width: 1,
            height: 4,
            pixels: new Uint8Array(1 * 4 * 4),
        },
    );
    assert.equal(shapeMismatch.dimensionMatch, false);
    assert.equal(shapeMismatch.totalPixels, 4);
    assert.equal(shapeMismatch.differentPixels, 4);
    assert.equal(shapeMismatch.mismatchRatio, 1);
    assert.equal(shapeMismatch.exactMatch, false);

    console.log("rust renderer shadow integration smoke test passed");
}

void main().catch((error) => {
    console.error(error);
    process.exitCode = 1;
});
