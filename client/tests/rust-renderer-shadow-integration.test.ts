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
        WebGLMapSquare,
        createDeferredDrawCallRange,
        dematerializeDrawCallRange,
        materializeDrawCallRange,
        releaseDrawCallRange,
    } = await import("../render/WebGLMapSquare");
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
    const {
        buildModelInfoTextureDataWithRust,
        flattenModelInfoCommands,
    } = await import("../render/rust/RustGeometryPreparation");

    const fakeHost = {
        canvas: {},
    } as any;

    const modelInfoCommands = [
        {
            instances: [
                {
                    sceneX: 1,
                    sceneZ: 2,
                    heightOffset: 3,
                    level: 1,
                    contourGround: 2,
                    priority: 3,
                    interactType: 4,
                    interactId: 5,
                },
                {
                    sceneX: 6,
                    sceneZ: 7,
                    heightOffset: 8,
                    level: 0,
                    planeCullLevel: 2,
                    contourGround: 1,
                    priority: 5,
                    interactType: 6,
                    interactId: 0x12345,
                },
            ],
        },
        {
            instances: [],
        },
    ] as any;

    const flatModelInfo = flattenModelInfoCommands(modelInfoCommands);
    assert.deepEqual(Array.from(flatModelInfo.commandInstanceCounts), [2, 0]);
    assert.deepEqual(Array.from(flatModelInfo.instanceFields), [
        1, 2, 3, 1, 1, 2, 3, 4, 5,
        6, 7, 8, 0, 2, 1, 5, 6, 0x12345,
    ]);

    let capturedCounts: Uint32Array | undefined;
    let capturedFields: Int32Array | undefined;
    const rustModelInfoResult = buildModelInfoTextureDataWithRust(
        modelInfoCommands,
        (counts, fields) => {
            capturedCounts = counts;
            capturedFields = fields;
            return new Uint16Array([7, 8, 9]);
        },
    );
    assert.deepEqual(Array.from(rustModelInfoResult), [7, 8, 9]);
    assert.deepEqual(Array.from(capturedCounts!), [2, 0]);
    assert.deepEqual(Array.from(capturedFields!), Array.from(flatModelInfo.instanceFields));

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
        structuralParityEnabled: false,
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

    const deferredRanges = [[12, 6, 1]] as any;
    let deferredCreations = 0;
    const deferred = createDeferredDrawCallRange(
        deferredRanges,
        () => {
            deferredCreations++;
            return { marker: "legacy" } as any;
        },
        false,
    );
    assert.equal(deferredCreations, 0);
    assert.equal(deferred.drawCall, undefined);
    assert.equal(deferred.materializeDrawCall !== undefined, true);

    const materialized = materializeDrawCallRange(deferred);
    assert.equal(deferredCreations, 1);
    assert.equal((materialized.drawCall as any).marker, "legacy");
    assert.equal(materialized.drawRanges, deferredRanges);
    assert.equal(materialized.materializeDrawCall !== undefined, true);

    const materializedAgain = materializeDrawCallRange(deferred);
    assert.equal(deferredCreations, 1);
    assert.equal(materializedAgain.drawCall, materialized.drawCall);

    dematerializeDrawCallRange(deferred);
    assert.equal(deferred.drawCall, undefined);
    assert.equal(deferred.materializeDrawCall !== undefined, true);
    const rematerialized = materializeDrawCallRange(deferred);
    assert.equal(deferredCreations, 2);
    assert.equal((rematerialized.drawCall as any).marker, "legacy");

    // Simulate the Stage 4 primary-to-legacy transition at the real map accessors.
    // Primary leaves these ranges deferred; the first legacy accessor must
    // materialize exactly once and subsequent fallback frames must reuse it.
    const fallbackMap = Object.create(WebGLMapSquare.prototype) as any;
    fallbackMap.mapX = 0;
    fallbackMap.mapY = 0;
    fallbackMap.legacyMapTextureState = { resources: undefined };
    const makeFallbackRange = (marker: string) => {
        let creations = 0;
        const range = createDeferredDrawCallRange(
            [[0, 3, 1]] as any,
            () => {
                creations++;
                return { marker } as any;
            },
            false,
        );
        return { range, creations: () => creations };
    };

    const terrainFallback = makeFallbackRange("terrain");
    fallbackMap.drawCall = terrainFallback.range;
    assert.equal(terrainFallback.range.drawCall, undefined);
    assert.equal((fallbackMap.getDrawCall(false, false, false).drawCall as any).marker, "terrain");
    assert.equal(terrainFallback.creations(), 1);
    fallbackMap.getDrawCall(false, false, false);
    assert.equal(terrainFallback.creations(), 1);

    const locFallback = makeFallbackRange("loc");
    fallbackMap.loc = { drawCall: locFallback.range };
    assert.equal((fallbackMap.getLocDrawCall(false, false, false)!.drawCall as any).marker, "loc");
    assert.equal(locFallback.creations(), 1);
    fallbackMap.getLocDrawCall(false, false, false);
    assert.equal(locFallback.creations(), 1);

    const doorFallback = makeFallbackRange("door");
    fallbackMap.door = { drawCall: doorFallback.range };
    assert.equal((fallbackMap.getDoorDrawCall(false, false, false)!.drawCall as any).marker, "door");
    assert.equal(doorFallback.creations(), 1);
    fallbackMap.getDoorDrawCall(false, false, false);
    assert.equal(doorFallback.creations(), 1);

    const groundFallback = makeFallbackRange("ground");
    fallbackMap.groundItems = { drawCall: groundFallback.range };
    assert.equal(
        (fallbackMap.getGroundItemDrawCall(false, false, false)!.drawCall as any).marker,
        "ground",
    );
    assert.equal(groundFallback.creations(), 1);
    fallbackMap.getGroundItemDrawCall(false, false, false);
    assert.equal(groundFallback.creations(), 1);

    // Rust-primary recovery must reclaim any Pico resources materialized by a
    // temporary fallback without destroying the factories needed for a later
    // context-loss fallback.
    fallbackMap.releaseLegacySceneGpuResources();
    assert.equal(terrainFallback.range.drawCall, undefined);
    assert.equal(locFallback.range.drawCall, undefined);
    assert.equal(doorFallback.range.drawCall, undefined);
    assert.equal(groundFallback.range.drawCall, undefined);
    assert.equal(terrainFallback.range.materializeDrawCall !== undefined, true);
    assert.equal(locFallback.range.materializeDrawCall !== undefined, true);
    assert.equal(doorFallback.range.materializeDrawCall !== undefined, true);
    assert.equal(groundFallback.range.materializeDrawCall !== undefined, true);

    fallbackMap.getDrawCall(false, false, false);
    fallbackMap.getLocDrawCall(false, false, false);
    fallbackMap.getDoorDrawCall(false, false, false);
    fallbackMap.getGroundItemDrawCall(false, false, false);
    assert.equal(terrainFallback.creations(), 2);
    assert.equal(locFallback.creations(), 2);
    assert.equal(doorFallback.creations(), 2);
    assert.equal(groundFallback.creations(), 2);

    let eagerCreations = 0;
    const eager = createDeferredDrawCallRange(
        [[0, 3, 1]] as any,
        () => {
            eagerCreations++;
            return { marker: "eager" } as any;
        },
        true,
    );
    assert.equal(eagerCreations, 1);
    assert.equal((eager.drawCall as any).marker, "eager");
    assert.equal(eager.materializeDrawCall, undefined);

    releaseDrawCallRange(deferred);
    assert.equal(deferred.drawCall, undefined);
    assert.equal(deferred.materializeDrawCall, undefined);
    assert.equal(deferred.drawRanges.length, 0);

    assert.equal(
        getRustRendererRuntimeMode(""),
        "primary",
    );
    assert.equal(
        getRustRendererRuntimeMode("?unrelated=1"),
        "primary",
    );
    assert.equal(
        getRustRendererRuntimeMode("?rust-renderer=unknown"),
        "primary",
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
        isRustPrimaryRuntime(""),
        true,
    );
    assert.equal(
        isRustPrimaryRuntime("?rust-renderer=primary"),
        true,
    );
    assert.equal(
        isRustPrimaryRuntime("?rust-renderer=off"),
        false,
    );
    assert.equal(
        isRustPrimaryRendererEnabled(""),
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
