import {
    Framebuffer,
    PicoGL,
    Renderbuffer,
    Texture,
} from "picogl";

import type { WebGLOsrsRendererHost } from "../render/hostInterface";
import { getRustRendererRuntimeMode } from "./RustRendererRuntime";

export interface RustPixelFrame {
    width: number;
    height: number;
    pixels: Uint8Array;
}

export interface RustPixelParityMetrics {
    width: number;
    height: number;
    dimensionMatch: boolean;
    totalPixels: number;
    differentPixels: number;
    mismatchRatio: number;
    maxChannelDelta: number;
    meanAbsoluteChannelDelta: number;
    rmse: number;
    exactMatch: boolean;
}

interface PicoStaticReferenceTarget {
    framebuffer: Framebuffer;
    color: Texture;
    depth: Renderbuffer;
    width: number;
    height: number;
}

const referenceTargets = new WeakMap<
    WebGLOsrsRendererHost,
    PicoStaticReferenceTarget
>();
const captureCounters = new WeakMap<WebGLOsrsRendererHost, number>();

function getSearch(
    search: string | undefined,
): string {
    if (search !== undefined) return search;
    return typeof window !== "undefined" ? window.location.search : "";
}

export function isRustPixelParityEnabled(
    search?: string,
): boolean {
    const resolvedSearch = getSearch(search);
    if (getRustRendererRuntimeMode(resolvedSearch) !== "shadow") {
        return false;
    }

    const raw = new URLSearchParams(resolvedSearch).get(
        "rust-pixel-parity",
    );
    return raw === "1" || raw === "true";
}

export function getRustPixelParityInterval(
    search?: string,
): number {
    const raw = new URLSearchParams(getSearch(search)).get(
        "rust-pixel-every",
    );
    const parsed = raw == null ? Number.NaN : Number.parseInt(raw, 10);
    if (!Number.isFinite(parsed) || parsed < 1) {
        return 120;
    }
    return Math.min(3600, parsed | 0);
}

export function shouldCaptureRustPixelParity(
    host: WebGLOsrsRendererHost,
    search?: string,
): boolean {
    if (!isRustPixelParityEnabled(search)) return false;

    const next = (captureCounters.get(host) ?? 0) + 1;
    captureCounters.set(host, next);
    const interval = getRustPixelParityInterval(search);
    return next === 1 || next % interval === 0;
}

function deleteReferenceTarget(
    target: PicoStaticReferenceTarget,
): void {
    target.framebuffer.delete();
    target.color.delete();
    target.depth.delete();
}

function ensureReferenceTarget(
    host: WebGLOsrsRendererHost,
): PicoStaticReferenceTarget {
    const width = Math.max(1, host.sceneRenderWidth | 0);
    const height = Math.max(1, host.sceneRenderHeight | 0);
    const existing = referenceTargets.get(host);
    if (
        existing
        && existing.width === width
        && existing.height === height
    ) {
        return existing;
    }

    if (existing) {
        deleteReferenceTarget(existing);
    }

    const color = host.app.createTexture2D(width, height, {
        minFilter: PicoGL.NEAREST,
        magFilter: PicoGL.NEAREST,
    });
    const depth = host.app.createRenderbuffer(
        width,
        height,
        PicoGL.DEPTH_COMPONENT24,
        0,
    );
    const framebuffer = host.app
        .createFramebuffer()
        .colorTarget(0, color)
        .depthTarget(depth);

    const target = {
        framebuffer,
        color,
        depth,
        width,
        height,
    };
    referenceTargets.set(host, target);
    return target;
}

export function capturePicoStaticReference(
    host: WebGLOsrsRendererHost,
    restoreFramebuffer: Framebuffer,
): RustPixelFrame {
    const target = ensureReferenceTarget(host);
    const blendEnabled = host.gl.isEnabled(PicoGL.BLEND);
    const sceneHslOverride = new Float32Array(
        host.sceneHslOverride as Float32Array,
    );
    const counters = {
        frameIndices: host._frameIndices,
        frameBatches: host._frameBatches,
        roofFiltered: host.frameRoofFilteredRangeCount,
        roofTotal: host.frameRoofTotalRangeCount,
        lodVisible: host.lastLodVisibleMapCount,
        fullDetailVisible: host.lastFullDetailVisibleMapCount,
        lodThreshold: host.lastLodThreshold,
        distanceCulled: host.lastDistanceCulledVisibleMapCount,
    };

    try {
        host.app.drawFramebuffer(target.framebuffer);
        host.app.viewport(0, 0, target.width, target.height);

        const sceneViewport = host.getSceneViewportWidgetRect();
        const framebufferViewport =
            host.scaleViewportRectToSceneBuffer(sceneViewport);
        host.clearSceneFramebuffer(framebufferViewport);

        host.app.disable(PicoGL.BLEND);
        host.renderOpaquePass();
        host.app.enable(PicoGL.BLEND);
        host.renderTransparentPass();

        host.app.readFramebuffer(target.framebuffer);
        host.gl.readBuffer(PicoGL.COLOR_ATTACHMENT0);

        const pixels = new Uint8Array(
            target.width * target.height * 4,
        );
        host.gl.readPixels(
            0,
            0,
            target.width,
            target.height,
            PicoGL.RGBA,
            PicoGL.UNSIGNED_BYTE,
            pixels,
        );

        return {
            width: target.width,
            height: target.height,
            pixels,
        };
    } finally {
        host.sceneHslOverride.set(sceneHslOverride);
        host.sceneUniformBuffer
            ?.set(4, host.sceneHslOverride as Float32Array)
            .update();

        if (blendEnabled) {
            host.app.enable(PicoGL.BLEND);
        } else {
            host.app.disable(PicoGL.BLEND);
        }

        host.app.drawFramebuffer(restoreFramebuffer);
        host.app.readFramebuffer(restoreFramebuffer);
        host.app.viewport(
            0,
            0,
            host.sceneRenderWidth | 0,
            host.sceneRenderHeight | 0,
        );

        host._frameIndices = counters.frameIndices;
        host._frameBatches = counters.frameBatches;
        host.frameRoofFilteredRangeCount = counters.roofFiltered;
        host.frameRoofTotalRangeCount = counters.roofTotal;
        host.lastLodVisibleMapCount = counters.lodVisible;
        host.lastFullDetailVisibleMapCount =
            counters.fullDetailVisible;
        host.lastLodThreshold = counters.lodThreshold;
        host.lastDistanceCulledVisibleMapCount =
            counters.distanceCulled;
    }
}

export function readCanvasRgbaPixels(
    canvas: HTMLCanvasElement,
): RustPixelFrame | undefined {
    const gl = canvas.getContext("webgl2");
    if (!gl) return undefined;

    const width = Math.max(1, canvas.width | 0);
    const height = Math.max(1, canvas.height | 0);
    const pixels = new Uint8Array(width * height * 4);
    gl.readPixels(
        0,
        0,
        width,
        height,
        gl.RGBA,
        gl.UNSIGNED_BYTE,
        pixels,
    );

    return { width, height, pixels };
}

export function compareRgbaFrames(
    reference: RustPixelFrame,
    candidate: RustPixelFrame,
    tolerance: number = 0,
): RustPixelParityMetrics {
    const dimensionMatch =
        reference.width === candidate.width
        && reference.height === candidate.height;
    const width = Math.min(reference.width, candidate.width);
    const height = Math.min(reference.height, candidate.height);
    const totalPixels = width * height;
    const channelCount = totalPixels * 4;
    const allowedDelta = Math.max(0, Math.min(255, tolerance | 0));

    let differentPixels = 0;
    let maxChannelDelta = 0;
    let absoluteDeltaSum = 0;
    let squaredDeltaSum = 0;

    for (let pixel = 0; pixel < totalPixels; pixel++) {
        const offset = pixel * 4;
        let pixelDifferent = false;

        for (let channel = 0; channel < 4; channel++) {
            const delta = Math.abs(
                reference.pixels[offset + channel]
                - candidate.pixels[offset + channel],
            );
            maxChannelDelta = Math.max(maxChannelDelta, delta);
            absoluteDeltaSum += delta;
            squaredDeltaSum += delta * delta;
            if (delta > allowedDelta) {
                pixelDifferent = true;
            }
        }

        if (pixelDifferent) {
            differentPixels++;
        }
    }

    if (!dimensionMatch) {
        differentPixels += Math.abs(
            reference.width * reference.height
            - candidate.width * candidate.height,
        );
    }

    const comparisonPixels = Math.max(
        reference.width * reference.height,
        candidate.width * candidate.height,
        1,
    );
    const denominator = Math.max(channelCount, 1);
    const mismatchRatio = Math.min(
        1,
        differentPixels / comparisonPixels,
    );

    return {
        width,
        height,
        dimensionMatch,
        totalPixels: comparisonPixels,
        differentPixels,
        mismatchRatio,
        maxChannelDelta,
        meanAbsoluteChannelDelta: absoluteDeltaSum / denominator,
        rmse: Math.sqrt(squaredDeltaSum / denominator),
        exactMatch:
            dimensionMatch
            && differentPixels === 0
            && maxChannelDelta === 0,
    };
}

export function disposeRustPixelParity(
    host: WebGLOsrsRendererHost,
): void {
    const target = referenceTargets.get(host);
    if (target) {
        deleteReferenceTarget(target);
        referenceTargets.delete(host);
    }
    captureCounters.delete(host);
}
