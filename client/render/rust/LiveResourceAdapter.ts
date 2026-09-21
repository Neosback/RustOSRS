import {
    MATERIAL_TEXTURE_ROWS,
    TEXTURE_SIZE,
    WATER_TEXTURE_ASSETS,
    WATER_TEXTURE_SIZE,
} from "../render/constants";
import type { RustRendererGlobalResources } from "./RustRendererBridge";

export interface RustRendererGlobalResourceHost {
    textureArrayPixels?: Uint8Array;
    textureMaterialBytes?: Int8Array;
    waterTexturePixels?: Uint8Array;
    textureLayerCount: number;
    rustGlobalResourcesRevision: number;
}

export interface RustRendererGlobalResourceSnapshot {
    revision: number;
    resources: RustRendererGlobalResources;
}

function requireByteLength(name: string, actual: number, expected: number): void {
    if (actual !== expected) {
        throw new Error(
            `${name} has ${actual} bytes; expected ${expected}`,
        );
    }
}

/**
 * Returns the exact CPU-side resource bytes used by the live PicoGL renderer.
 *
 * Missing resources mean initialization is not complete yet and return
 * undefined. Once all resources exist, malformed dimensions are treated as a
 * hard error because silently uploading truncated GPU data makes A/B parity
 * results meaningless.
 */
export function getRustRendererGlobalResourceSnapshot(
    host: RustRendererGlobalResourceHost,
): RustRendererGlobalResourceSnapshot | undefined {
    const texturePixels = host.textureArrayPixels;
    const materialBytes = host.textureMaterialBytes;
    const waterPixels = host.waterTexturePixels;
    const textureLayers = host.textureLayerCount | 0;

    if (!texturePixels || !materialBytes || !waterPixels || textureLayers <= 0) {
        return undefined;
    }

    const textureBytes =
        TEXTURE_SIZE * TEXTURE_SIZE * textureLayers * 4;
    requireByteLength("texture array", texturePixels.byteLength, textureBytes);

    const materialByteLength =
        textureLayers * MATERIAL_TEXTURE_ROWS * 4;
    requireByteLength(
        "material table",
        materialBytes.byteLength,
        materialByteLength,
    );

    const waterLayers = WATER_TEXTURE_ASSETS.length;
    const waterByteLength =
        WATER_TEXTURE_SIZE * WATER_TEXTURE_SIZE * waterLayers * 4;
    requireByteLength("water texture array", waterPixels.byteLength, waterByteLength);

    return {
        revision: host.rustGlobalResourcesRevision | 0,
        resources: {
            texturePixels,
            textureWidth: TEXTURE_SIZE,
            textureHeight: TEXTURE_SIZE,
            textureLayers,
            materialBytes,
            materialCount: textureLayers,
            waterPixels,
            waterWidth: WATER_TEXTURE_SIZE,
            waterHeight: WATER_TEXTURE_SIZE,
            waterLayers,
        },
    };
}
