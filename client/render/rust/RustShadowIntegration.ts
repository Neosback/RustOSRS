import { getMapSquareId } from "../../rs/map/MapFileIndex";
import { WebGLMapSquare } from "../WebGLMapSquare";
import type { SdMapData } from "../loader/SdMapData";
import type { WebGLOsrsRendererHost } from "../render/hostInterface";
import { getRustRendererGlobalResourceSnapshot } from "./LiveResourceAdapter";
import {
    createRustDoorGeometryPacket,
    createRustLocGeometryPacket,
    createRustStaticScenePacket,
} from "./RendererPacket";
import type { RustResidentMapFrameState } from "./RustRendererBridge";
import {
    createRustRendererShadowRuntime,
    syncRustShadowCanvasSize,
    type RustRendererShadowRuntime,
} from "./RustRendererRuntime";

const runtimes = new WeakMap<WebGLOsrsRendererHost, RustRendererShadowRuntime>();
const failedHosts = new WeakSet<WebGLOsrsRendererHost>();
export interface RustRendererShadowDiagnostics {
    enabled: boolean;
    failed: boolean;
    residentMaps: number;
    visibleMaps: number;
    eligibleMaps: number;
    mirroredMaps: number;
    drawCalls: number;
    submittedIndices: number;
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

export async function initRustRendererShadow(
    host: WebGLOsrsRendererHost,
): Promise<void> {
    if (failedHosts.has(host) || runtimes.has(host)) return;

    try {
        const runtime = await createRustRendererShadowRuntime(host.canvas);
        if (!runtime) return;

        runtimes.set(host, runtime);
        syncGlobalResources(host, runtime);
        publishDiagnostics(host, {
            enabled: true,
            failed: false,
            residentMaps: runtime.bridge.getResidentStaticMapCount(),
            visibleMaps: 0,
            eligibleMaps: 0,
            mirroredMaps: 0,
            drawCalls: 0,
            submittedIndices: 0,
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
        runtime.bridge.removeStaticMap(getMapSquareId(mapX, mapY));
        publishDiagnostics(host, {
            ...getRustRendererShadowDiagnostics(host),
            residentMaps: runtime.bridge.getResidentStaticMapCount(),
        });
    } catch (error) {
        disableShadow(host, "map removal", error);
    }
}

interface ShadowCamera {
    viewMatrix: Float32Array;
    projectionMatrix: Float32Array;
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
): void {
    const runtime = getRuntime(host);
    if (!runtime) return;

    try {
        syncRustShadowCanvasSize(runtime.canvas, host.canvas);
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
            });
        }

        runtime.bridge.renderStaticMaps(frames);
        const stats =
            frames.length > 0
                ? runtime.bridge.getLastStats()
                : { drawCalls: 0, submittedIndices: 0 };
        publishDiagnostics(host, {
            enabled: true,
            failed: false,
            residentMaps: runtime.bridge.getResidentStaticMapCount(),
            visibleMaps: count,
            eligibleMaps,
            mirroredMaps: frames.length,
            drawCalls: stats.drawCalls,
            submittedIndices: stats.submittedIndices,
        });
    } catch (error) {
        disableShadow(host, "frame render", error);
    }
}
