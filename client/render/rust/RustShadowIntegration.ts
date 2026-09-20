import { getMapSquareId } from "../../rs/map/MapFileIndex";
import type { DrawRange } from "../DrawRange";
import { WebGLMapSquare } from "../WebGLMapSquare";
import type { GroundItemGeometryBuildData } from "../ground/GroundItemMeshBuilder";
import type { SdMapData } from "../loader/SdMapData";
import type { WebGLOsrsRendererHost } from "../render/hostInterface";
import { getRustRendererGlobalResourceSnapshot } from "./LiveResourceAdapter";
import {
    createRustDoorGeometryPacket,
    createRustGroundItemGeometryPacket,
    createRustLocGeometryPacket,
    createRustStaticScenePacket,
    type RustStaticGeometryPacket,
} from "./RendererPacket";
import type { RustResidentMapFrameState } from "./RustRendererBridge";
import {
    createRustRendererShadowRuntime,
    syncRustShadowCanvasSize,
    type RustRendererShadowRuntime,
} from "./RustRendererRuntime";
import {
    compareRgbaFrames,
    disposeRustPixelParity,
    readCanvasRgbaPixels,
    type RustPixelFrame,
    type RustPixelParityMetrics,
} from "./RustPixelParity";

const runtimes = new WeakMap<WebGLOsrsRendererHost, RustRendererShadowRuntime>();
const failedHosts = new WeakSet<WebGLOsrsRendererHost>();
const pendingGroundGeometry = new WeakMap<
    WebGLOsrsRendererHost,
    Map<number, RustStaticGeometryPacket | null>
>();

export interface RustRendererShadowDiagnostics {
    enabled: boolean;
    failed: boolean;
    residentMaps: number;
    visibleMaps: number;
    eligibleMaps: number;
    mirroredMaps: number;
    drawCalls: number;
    submittedIndices: number;
    expectedDrawCalls: number;
    expectedSubmittedIndices: number;
    drawStatsMatch: boolean;
    drawHash: number;
    expectedDrawHash: number;
    drawSequenceMatch: boolean;
    staticParityMatch: boolean;
    expectedWorldEntityGhostPasses: number;
    pixelParity?: RustPixelParityMetrics;
}

const diagnostics = new WeakMap<
    WebGLOsrsRendererHost,
    RustRendererShadowDiagnostics
>();

function publishDiagnostics(
    host: WebGLOsrsRendererHost,
    value: RustRendererShadowDiagnostics,
): void {
    diagnostics.set(host, value);
    (host.canvas as HTMLCanvasElement & {
        __rustRendererShadowDiagnostics?: RustRendererShadowDiagnostics;
    }).__rustRendererShadowDiagnostics = value;
}

export function getRustRendererShadowDiagnostics(
    host: WebGLOsrsRendererHost,
): RustRendererShadowDiagnostics {
    return diagnostics.get(host) ?? {
        enabled: false,
        failed: failedHosts.has(host),
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
    };
}


function disableShadow(
    host: WebGLOsrsRendererHost,
    phase: string,
    error: unknown,
): void {
    if (!failedHosts.has(host)) {
        console.warn(`[RustRenderer] shadow ${phase} disabled`, error);
    }
    failedHosts.add(host);
    publishDiagnostics(host, {
        ...getRustRendererShadowDiagnostics(host),
        enabled: false,
        failed: true,
    });

    const runtime = runtimes.get(host);
    if (runtime) {
        try {
            runtime.bridge.dispose();
        } catch {}
        runtimes.delete(host);
    }
    disposeRustPixelParity(host);
}

function getRuntime(
    host: WebGLOsrsRendererHost,
): RustRendererShadowRuntime | undefined {
    if (failedHosts.has(host)) return undefined;
    return runtimes.get(host);
}

function syncGlobalResources(
    host: WebGLOsrsRendererHost,
    runtime: RustRendererShadowRuntime,
): void {
    const snapshot = getRustRendererGlobalResourceSnapshot(host);
    if (!snapshot) return;

    runtime.bridge.syncGlobalResources(
        snapshot.resources,
        snapshot.revision,
    );
}

function syncCurrentActorData(
    host: WebGLOsrsRendererHost,
    runtime: RustRendererShadowRuntime,
): void {
    const height = host.actorDataLastTexHeight | 0;
    const currentTexture =
        host.actorDataTextures[host.actorDataCurrentIndex];
    if (height <= 0 || !currentTexture) return;

    const width = 16;
    const requiredU16 = width * height * 4;
    if (host.actorRenderData.length < requiredU16) {
        throw new Error(
            `Current actor-data buffer has ${host.actorRenderData.length} u16 values; expected at least ${requiredU16}`,
        );
    }

    runtime.bridge.uploadActorData(
        host.actorRenderData.subarray(0, requiredU16),
        width,
        height,
    );
}

export async function initRustRendererShadow(
    host: WebGLOsrsRendererHost,
): Promise<void> {
    if (failedHosts.has(host) || runtimes.has(host)) return;

    try {
        const runtime = await createRustRendererShadowRuntime(host.canvas);
        if (!runtime) return;

        runtimes.set(host, runtime);
        syncGlobalResources(host, runtime);
        syncCurrentActorData(host, runtime);
        publishDiagnostics(host, {
            enabled: true,
            failed: false,
            residentMaps: runtime.bridge.getResidentStaticMapCount(),
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
        console.info("[RustRenderer] shadow renderer enabled");
    } catch (error) {
        disableShadow(host, "initialization", error);
    }
}

export function disposeRustRendererShadow(
    host: WebGLOsrsRendererHost,
): void {
    const runtime = runtimes.get(host);
    if (runtime) {
        try {
            runtime.bridge.dispose();
        } catch {}
        runtimes.delete(host);
    }
    failedHosts.delete(host);
    diagnostics.delete(host);
    pendingGroundGeometry.delete(host);
    disposeRustPixelParity(host);
    delete (host.canvas as HTMLCanvasElement & {
        __rustRendererShadowDiagnostics?: RustRendererShadowDiagnostics;
    }).__rustRendererShadowDiagnostics;
}

export function mirrorRustStaticMap(
    host: WebGLOsrsRendererHost,
    data: SdMapData,
    timeLoaded: number,
): void {
    const runtime = getRuntime(host);
    if (!runtime) return;

    try {
        syncGlobalResources(host, runtime);
        const mapKey = getMapSquareId(data.mapX, data.mapY);

        if (data.doorOnly) {
            runtime.bridge.updateDoorGeometry(
                mapKey,
                createRustDoorGeometryPacket(data),
            );
            return;
        }

        if (data.locOnly) {
            runtime.bridge.updateLocGeometry(
                mapKey,
                createRustLocGeometryPacket(data),
            );
            return;
        }

        runtime.bridge.uploadStaticScene(
            createRustStaticScenePacket(data),
            timeLoaded,
        );

        const pendingGround = pendingGroundGeometry.get(host);
        if (pendingGround?.has(mapKey)) {
            const geometry = pendingGround.get(mapKey) ?? undefined;
            runtime.bridge.updateGroundGeometry(mapKey, geometry);
            pendingGround.delete(mapKey);
            if (pendingGround.size === 0) {
                pendingGroundGeometry.delete(host);
            }
        }

        publishDiagnostics(host, {
            ...getRustRendererShadowDiagnostics(host),
            residentMaps: runtime.bridge.getResidentStaticMapCount(),
        });
    } catch (error) {
        disableShadow(host, "map mirror", error);
    }
}

export function removeRustStaticMap(
    host: WebGLOsrsRendererHost,
    mapX: number,
    mapY: number,
): void {
    const runtime = getRuntime(host);
    if (!runtime) return;

    try {
        const mapKey = getMapSquareId(mapX, mapY);
        runtime.bridge.removeStaticMap(mapKey);
        pendingGroundGeometry.get(host)?.delete(mapKey);
        publishDiagnostics(host, {
            ...getRustRendererShadowDiagnostics(host),
            residentMaps: runtime.bridge.getResidentStaticMapCount(),
        });
    } catch (error) {
        disableShadow(host, "map removal", error);
    }
}

export function mirrorRustActorData(
    host: WebGLOsrsRendererHost,
    values: Uint16Array,
    width: number,
    height: number,
): void {
    const runtime = getRuntime(host);
    if (!runtime) return;

    try {
        runtime.bridge.uploadActorData(values, width, height);
    } catch (error) {
        disableShadow(host, "actor-data mirror", error);
    }
}

export function mirrorRustGroundItemGeometry(
    host: WebGLOsrsRendererHost,
    mapKey: number,
    data?: GroundItemGeometryBuildData,
): void {
    const runtime = getRuntime(host);
    if (!runtime) return;

    try {
        syncGlobalResources(host, runtime);
        const geometry = data
            ? createRustGroundItemGeometryPacket(data)
            : undefined;

        if (runtime.bridge.updateGroundGeometry(mapKey, geometry)) {
            pendingGroundGeometry.get(host)?.delete(mapKey);
            return;
        }

        let pending = pendingGroundGeometry.get(host);
        if (!pending) {
            pending = new Map<number, RustStaticGeometryPacket | null>();
            pendingGroundGeometry.set(host, pending);
        }
        pending.set(mapKey, geometry ?? null);
    } catch (error) {
        disableShadow(host, "ground-item mirror", error);
    }
}

interface ShadowCamera {
    viewMatrix: Float32Array;
    projectionMatrix: Float32Array;
}

export interface RustStaticDrawStats {
    drawCalls: number;
    submittedIndices: number;
}

export const RUST_DRAW_HASH_OFFSET_BASIS = 0x811c9dc5;
const RUST_DRAW_HASH_PRIME = 0x01000193;

export function hashRustDrawWord(hash: number, value: number): number {
    return Math.imul(
        (hash ^ (value >>> 0)) >>> 0,
        RUST_DRAW_HASH_PRIME,
    ) >>> 0;
}

export function hashExpectedDrawRanges(
    hash: number,
    mapKey: number,
    transparent: boolean,
    useLod: boolean,
    batchKind: number,
    ranges: readonly DrawRange[],
    planes: Uint8Array | undefined,
    roofPlaneLimit: number,
    patches?: Uint32Array,
): number {
    const overrides = new Map<number, DrawRange>();
    if (patches) {
        for (let i = 0; i + 3 < patches.length; i += 4) {
            overrides.set(patches[i] | 0, [
                patches[i + 1] >>> 0,
                patches[i + 2] >>> 0,
                patches[i + 3] >>> 0,
            ]);
        }
    }

    const flags = (transparent ? 1 : 0) | (useLod ? 2 : 0);
    const roofLimit = Math.max(0, Math.min(3, roofPlaneLimit | 0));

    for (let index = 0; index < ranges.length; index++) {
        const range = overrides.get(index) ?? ranges[index];
        const offset = range?.[0] ?? 0;
        const elements = range?.[1] ?? 0;
        const instances = range?.[2] ?? 0;
        if (elements <= 0 || instances <= 0) continue;

        const plane = planes?.[index] ?? 0;
        if (roofLimit < 3 && plane > roofLimit) continue;

        for (const value of [
            mapKey,
            flags,
            batchKind,
            index,
            offset,
            elements,
            instances,
            plane,
        ]) {
            hash = hashRustDrawWord(hash, value);
        }
    }

    return hash >>> 0;
}

export function countExpectedDrawRanges(
    ranges: readonly DrawRange[],
    planes: Uint8Array | undefined,
    roofPlaneLimit: number,
    patches?: Uint32Array,
): RustStaticDrawStats {
    const overrides = new Map<number, DrawRange>();
    if (patches) {
        for (let i = 0; i + 3 < patches.length; i += 4) {
            overrides.set(patches[i] | 0, [
                patches[i + 1] >>> 0,
                patches[i + 2] >>> 0,
                patches[i + 3] >>> 0,
            ]);
        }
    }

    const roofLimit = Math.max(0, Math.min(3, roofPlaneLimit | 0));
    let drawCalls = 0;
    let submittedIndices = 0;

    for (let index = 0; index < ranges.length; index++) {
        const range = overrides.get(index) ?? ranges[index];
        const elements = range?.[1] ?? 0;
        const instances = range?.[2] ?? 0;
        if (elements <= 0 || instances <= 0) continue;

        const plane = planes?.[index] ?? 0;
        if (roofLimit < 3 && plane > roofLimit) continue;

        drawCalls++;
        submittedIndices += elements * instances;
    }

    return { drawCalls, submittedIndices };
}

function addStats(
    target: RustStaticDrawStats,
    source: RustStaticDrawStats,
): void {
    target.drawCalls += source.drawCalls;
    target.submittedIndices += source.submittedIndices;
}

export function createWorldEntityGhostSceneHslOverride(
    packedHsl: number,
): Float32Array | undefined {
    if ((packedHsl | 0) <= 0) return undefined;

    return new Float32Array([
        (packedHsl >> 10) & 63,
        (packedHsl >> 7) & 7,
        packedHsl & 127,
        127,
    ]);
}

function getWorldEntityGhostSceneHslOverride(
    host: WebGLOsrsRendererHost,
    map: WebGLMapSquare,
): Float32Array | undefined {
    if (
        !host.sceneUniformBuffer
        || !host.mapManager.worldEntityMapIds.has(map.id)
    ) {
        return undefined;
    }

    const entityIndex = host.getWorldEntityIndexForMapId(map.id);
    if (entityIndex === undefined) return undefined;

    const entity =
        host.osrsClient.worldViewManager.getWorldEntity(entityIndex);
    if (!entity || entity.drawMode !== 1) return undefined;

    const worldView =
        host.osrsClient.worldViewManager.getWorldView(entityIndex);
    if (
        !worldView
        || (worldView.npcIds.size === 0 && worldView.playerIds.size === 0)
    ) {
        return undefined;
    }

    const overlay = host.worldEntityOverlays.get(entityIndex);
    const worldEntityType =
        overlay?.configId !== undefined && overlay.configId >= 0
            ? host.osrsClient.worldEntityTypeLoader?.load(overlay.configId)
            : undefined;
    return createWorldEntityGhostSceneHslOverride(
        worldEntityType?.sceneTintHsl ?? 0,
    );
}

function countExpectedMapStaticDraws(
    map: WebGLMapSquare,
    useLod: boolean,
    roofPlaneLimit: number,
    worldEntityGhostPass: boolean,
): RustStaticDrawStats {
    const total: RustStaticDrawStats = {
        drawCalls: 0,
        submittedIndices: 0,
    };

    for (const transparent of [false, true]) {
        const terrain = map.getDrawCall(transparent, false, useLod);
        addStats(
            total,
            countExpectedDrawRanges(
                terrain.drawRanges,
                map.getDrawRangesPlanes(transparent, false, useLod),
                roofPlaneLimit,
            ),
        );

        const loc = map.getLocDrawCall(transparent, false, useLod);
        if (loc) {
            addStats(
                total,
                countExpectedDrawRanges(
                    loc.drawRanges,
                    map.getLocDrawRangesPlanes(transparent, false, useLod),
                    roofPlaneLimit,
                    createAnimatedLocDrawRangePatches(
                        map,
                        transparent,
                        useLod,
                    ),
                ),
            );
        }

        const ground = map.getGroundItemDrawCall(
            transparent,
            false,
            useLod,
        );
        if (ground) {
            addStats(
                total,
                countExpectedDrawRanges(
                    ground.drawRanges,
                    map.getGroundItemDrawRangesPlanes(
                        transparent,
                        false,
                        useLod,
                    ),
                    roofPlaneLimit,
                ),
            );
        }

        const door = map.getDoorDrawCall(transparent, false, useLod);
        if (door) {
            addStats(
                total,
                countExpectedDrawRanges(
                    door.drawRanges,
                    map.getDoorDrawRangesPlanes(
                        transparent,
                        false,
                        useLod,
                    ),
                    roofPlaneLimit,
                ),
            );
        }

        if (!transparent && worldEntityGhostPass) {
            addStats(
                total,
                countExpectedDrawRanges(
                    terrain.drawRanges,
                    map.getDrawRangesPlanes(false, false, useLod),
                    roofPlaneLimit,
                ),
            );
        }
    }

    return total;
}

function hashExpectedMapStaticPass(
    hash: number,
    map: WebGLMapSquare,
    useLod: boolean,
    transparent: boolean,
    roofPlaneLimit: number,
    worldEntityGhostPass: boolean,
): number {
    const mapKey = map.id >>> 0;
    const terrain = map.getDrawCall(transparent, false, useLod);
    hash = hashExpectedDrawRanges(
        hash,
        mapKey,
        transparent,
        useLod,
        0,
        terrain.drawRanges,
        map.getDrawRangesPlanes(transparent, false, useLod),
        roofPlaneLimit,
    );

    const loc = map.getLocDrawCall(transparent, false, useLod);
    if (loc) {
        hash = hashExpectedDrawRanges(
            hash,
            mapKey,
            transparent,
            useLod,
            1,
            loc.drawRanges,
            map.getLocDrawRangesPlanes(transparent, false, useLod),
            roofPlaneLimit,
            createAnimatedLocDrawRangePatches(
                map,
                transparent,
                useLod,
            ),
        );
    }

    const ground = map.getGroundItemDrawCall(
        transparent,
        false,
        useLod,
    );
    if (ground) {
        hash = hashExpectedDrawRanges(
            hash,
            mapKey,
            transparent,
            useLod,
            2,
            ground.drawRanges,
            map.getGroundItemDrawRangesPlanes(
                transparent,
                false,
                useLod,
            ),
            roofPlaneLimit,
        );
    }

    const door = map.getDoorDrawCall(transparent, false, useLod);
    if (door) {
        hash = hashExpectedDrawRanges(
            hash,
            mapKey,
            transparent,
            useLod,
            3,
            door.drawRanges,
            map.getDoorDrawRangesPlanes(
                transparent,
                false,
                useLod,
            ),
            roofPlaneLimit,
        );
    }

    if (!transparent && worldEntityGhostPass) {
        hash = hashExpectedDrawRanges(
            hash,
            mapKey,
            false,
            useLod,
            4,
            terrain.drawRanges,
            map.getDrawRangesPlanes(false, false, useLod),
            roofPlaneLimit,
        );
    }

    return hash >>> 0;
}

export function createAnimatedLocDrawRangePatches(
    map: Pick<WebGLMapSquare, "locsAnimated">,
    transparent: boolean,
    useLod: boolean,
): Uint32Array {
    if (map.locsAnimated.length === 0) {
        return new Uint32Array();
    }

    const values: number[] = [];
    for (const loc of map.locsAnimated) {
        const frames = transparent ? loc.anim.framesAlpha : loc.anim.frames;
        if (!frames) continue;

        const frame = frames[loc.frame | 0];
        if (!frame) continue;

        const rangeIndex = loc.getDrawRangeIndex(
            transparent,
            false,
            useLod,
        );
        if (rangeIndex < 0) continue;

        values.push(
            rangeIndex >>> 0,
            frame[0] >>> 0,
            frame[1] >>> 0,
            frame[2] >>> 0,
        );
    }

    return new Uint32Array(values);
}


export function renderRustStaticShadowFrame(
    host: WebGLOsrsRendererHost,
    camera: ShadowCamera,
    renderDistance: number,
    fogDepth: number,
    currentTime: number,
    pixelReference?: RustPixelFrame,
): void {
    const runtime = getRuntime(host);
    if (!runtime) return;

    try {
        syncRustShadowCanvasSize(
            runtime.canvas,
            host.sceneRenderWidth,
            host.sceneRenderHeight,
        );
        syncGlobalResources(host, runtime);

        const count = host.mapManager.visibleMapCount | 0;
        if (count <= 0) {
            publishDiagnostics(host, {
                ...getRustRendererShadowDiagnostics(host),
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
            return;
        }

        const cullTile = host.getRenderCullTile();
        const renderDistanceTiles = Math.max(0, renderDistance | 0);
        const lodThresholdTiles = Math.max(
            0,
            host.getFrameLodThresholdTiles() | 0,
        );
        const roofPlaneLimit = host.getRoofPlaneLimit();
        const frames: RustResidentMapFrameState[] = [];
        const expectedStats: RustStaticDrawStats = {
            drawCalls: 0,
            submittedIndices: 0,
        };
        const mirroredStaticMaps: Array<{
            map: WebGLMapSquare;
            useLod: boolean;
            worldEntityGhostPass: boolean;
        }> = [];
        let expectedWorldEntityGhostPasses = 0;
        let eligibleMaps = 0;

        for (let i = 0; i < count; i++) {
            const map = host.mapManager.visibleMaps[i];
            if (
                !host.isMapWithinRenderDistance(
                    map,
                    cullTile.x,
                    cullTile.y,
                    renderDistanceTiles,
                    0,
                )
            ) {
                continue;
            }

            eligibleMaps++;
            const mapKey = map.id | 0;
            if (!runtime.bridge.hasStaticMap(mapKey)) {
                continue;
            }

            const tileDistance = host.getMapTileDistanceFromPoint(
                map,
                cullTile.x,
                cullTile.y,
            );
            const useLod = tileDistance > lodThresholdTiles;

            runtime.bridge.patchLocDrawRanges(
                mapKey,
                useLod,
                false,
                createAnimatedLocDrawRangePatches(map, false, useLod),
            );
            runtime.bridge.patchLocDrawRanges(
                mapKey,
                useLod,
                true,
                createAnimatedLocDrawRangePatches(map, true, useLod),
            );
            const worldEntityGhostSceneHslOverride =
                getWorldEntityGhostSceneHslOverride(host, map);
            const worldEntityGhostPass =
                !!worldEntityGhostSceneHslOverride;
            if (worldEntityGhostPass) {
                expectedWorldEntityGhostPasses++;
            }
            addStats(
                expectedStats,
                countExpectedMapStaticDraws(
                    map,
                    useLod,
                    roofPlaneLimit,
                    worldEntityGhostPass,
                ),
            );

            let worldEntityTransform: Float32Array =
                WebGLMapSquare.IDENTITY_MAT4;
            if (host.mapManager.worldEntityMapIds.has(map.id)) {
                const entityIndex = host.getWorldEntityIndexForMapId(map.id);
                if (entityIndex !== undefined) {
                    worldEntityTransform =
                        host.worldEntityAnimator?.getTransform(entityIndex)
                        ?? WebGLMapSquare.IDENTITY_MAT4;
                }
            }

            mirroredStaticMaps.push({
                map,
                useLod,
                worldEntityGhostPass,
            });
            frames.push({
                mapKey,
                viewMatrix: camera.viewMatrix,
                projectionMatrix: camera.projectionMatrix,
                worldEntityTransform,
                worldEntityOpacity: 1,
                skyRgba: host.skyColor as Float32Array,
                sceneHslOverride: host.sceneHslOverride as Float32Array,
                playerPos: host.playerPosUni as Float32Array,
                renderDistance,
                fogDepth,
                currentTime,
                brightness: host.brightness,
                roofPlaneLimit,
                useLod,
                isNewTextureAnim: !!host.osrsClient.isNewTextureAnim,
                colorBanding: host.colorBanding,
                worldEntityGhostSceneHslOverride,
            });
        }

        let expectedDrawHash = RUST_DRAW_HASH_OFFSET_BASIS;
        for (const entry of mirroredStaticMaps) {
            expectedDrawHash = hashExpectedMapStaticPass(
                expectedDrawHash,
                entry.map,
                entry.useLod,
                false,
                roofPlaneLimit,
                entry.worldEntityGhostPass,
            );
        }
        for (let i = mirroredStaticMaps.length - 1; i >= 0; i--) {
            const entry = mirroredStaticMaps[i];
            expectedDrawHash = hashExpectedMapStaticPass(
                expectedDrawHash,
                entry.map,
                entry.useLod,
                true,
                roofPlaneLimit,
                entry.worldEntityGhostPass,
            );
        }

        runtime.bridge.renderStaticMaps(frames);
        const stats =
            frames.length > 0
                ? runtime.bridge.getLastStats()
                : { drawCalls: 0, submittedIndices: 0 };
        const drawHash =
            frames.length > 0
                ? runtime.bridge.getLastDrawHash()
                : 0;
        if (frames.length === 0) {
            expectedDrawHash = 0;
        }
        const drawStatsMatch =
            stats.drawCalls === expectedStats.drawCalls
            && stats.submittedIndices
                === expectedStats.submittedIndices;
        const drawSequenceMatch =
            drawHash === (expectedDrawHash >>> 0);
        const previousDiagnostics =
            getRustRendererShadowDiagnostics(host);
        let pixelParity = previousDiagnostics.pixelParity;
        if (pixelReference) {
            const rustPixels = readCanvasRgbaPixels(runtime.canvas);
            if (rustPixels) {
                pixelParity = compareRgbaFrames(
                    pixelReference,
                    rustPixels,
                );
            }
        }

        publishDiagnostics(host, {
            enabled: true,
            failed: false,
            residentMaps: runtime.bridge.getResidentStaticMapCount(),
            visibleMaps: count,
            eligibleMaps,
            mirroredMaps: frames.length,
            drawCalls: stats.drawCalls,
            submittedIndices: stats.submittedIndices,
            expectedDrawCalls: expectedStats.drawCalls,
            expectedSubmittedIndices: expectedStats.submittedIndices,
            drawStatsMatch,
            drawHash,
            expectedDrawHash: expectedDrawHash >>> 0,
            drawSequenceMatch,
            staticParityMatch: drawStatsMatch && drawSequenceMatch,
            expectedWorldEntityGhostPasses,
            pixelParity,
        });
    } catch (error) {
        disableShadow(host, "frame render", error);
    }
}
