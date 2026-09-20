import { getMapSquareId } from "../../rs/map/MapFileIndex";
import type { DrawRange } from "../DrawRange";
import type { GroundItemGeometryBuildData } from "../ground/GroundItemMeshBuilder";
import type { SdMapData } from "../loader/SdMapData";

export const RUST_RENDERER_ABI_VERSION = 13 as const;

/**
 * Numeric-only static map-square packet for the Rust/WASM renderer.
 *
 * This is a migration boundary, not a mirror of WebGLMapSquare. Do not add
 * PicoGL resources, TypeScript class instances, callbacks, loaders or React
 * state here.
 */
export interface RustStaticGeometryPacket {
    packedVertexWords: Uint32Array;
    indices: Uint32Array;

    modelInfoOpaque: Uint16Array;
    modelInfoAlpha: Uint16Array;
    modelInfoOpaqueLod: Uint16Array;
    modelInfoAlphaLod: Uint16Array;

    opaqueDrawRanges: Uint32Array;
    opaqueDrawRangePlanes: Uint8Array;
    alphaDrawRanges: Uint32Array;
    alphaDrawRangePlanes: Uint8Array;

    opaqueLodDrawRanges: Uint32Array;
    opaqueLodDrawRangePlanes: Uint8Array;
    alphaLodDrawRanges: Uint32Array;
    alphaLodDrawRangePlanes: Uint8Array;

    locGeometry: RustStaticGeometryPacket;
    doorGeometry: RustStaticGeometryPacket;
}

export interface RustStaticScenePacket {
    abiVersion: typeof RUST_RENDERER_ABI_VERSION;

    mapKey: number;
    mapX: number;
    mapY: number;
    borderSize: number;
    heightMapSize: number;
    heightMapPlanes: number;

    packedVertexWords: Uint32Array;
    indices: Uint32Array;
    npcPackedVertexWords: Uint32Array;
    npcIndices: Uint32Array;

    modelInfoOpaque: Uint16Array;
    modelInfoAlpha: Uint16Array;
    modelInfoOpaqueLod: Uint16Array;
    modelInfoAlphaLod: Uint16Array;
    heightMap: Int16Array;
    waterMask: Uint8Array;

    opaqueDrawRanges: Uint32Array;
    opaqueDrawRangePlanes: Uint8Array;
    alphaDrawRanges: Uint32Array;
    alphaDrawRangePlanes: Uint8Array;

    opaqueLodDrawRanges: Uint32Array;
    opaqueLodDrawRangePlanes: Uint8Array;
    alphaLodDrawRanges: Uint32Array;
    alphaLodDrawRangePlanes: Uint8Array;
}

/**
 * Flattens the existing [offsetBytes, elements, instances] tuples to the
 * renderer wire format without changing their meaning.
 */
export function flattenDrawRanges(ranges: readonly DrawRange[]): Uint32Array {
    const flat = new Uint32Array(ranges.length * 3);
    for (let i = 0; i < ranges.length; i++) {
        const range = ranges[i];
        const base = i * 3;
        flat[base] = range[0] >>> 0;
        flat[base + 1] = range[1] >>> 0;
        flat[base + 2] = range[2] >>> 0;
    }
    return flat;
}

/**
 * Reinterprets the current 12-byte packed vertex buffer as u32 triplets.
 * The renderer's DataBuffer allocation is normally 4-byte aligned. A copy is
 * used only when an external view is misaligned.
 */
export function packedVertexWords(vertices: Uint8Array): Uint32Array {
    if (vertices.byteLength % 12 !== 0) {
        throw new Error(
            `Packed vertex buffer must be a multiple of 12 bytes, got ${vertices.byteLength}`,
        );
    }

    if (vertices.byteOffset % 4 === 0) {
        return new Uint32Array(
            vertices.buffer,
            vertices.byteOffset,
            vertices.byteLength >>> 2,
        );
    }

    const aligned = new Uint8Array(vertices.byteLength);
    aligned.set(vertices);
    return new Uint32Array(aligned.buffer);
}

/** Reinterprets signed JS index storage as the unsigned WebGL index packet. */
export function unsignedIndexWords(indices: Int32Array): Uint32Array {
    if (indices.byteOffset % 4 === 0) {
        return new Uint32Array(indices.buffer, indices.byteOffset, indices.length);
    }

    const copy = new Int32Array(indices);
    return new Uint32Array(copy.buffer);
}

export function inferHeightMapPlanes(heightMap: Int16Array, size: number): number {
    if (!Number.isInteger(size) || size <= 0) {
        throw new Error(`Invalid height-map size: ${size}`);
    }
    const planeSamples = size * size;
    if (heightMap.length === 0 || heightMap.length % planeSamples !== 0) {
        throw new Error(
            `Height-map packet has ${heightMap.length} samples; expected a multiple of ${planeSamples}`,
        );
    }
    return heightMap.length / planeSamples;
}

export function validateWaterMask(
    waterMask: Uint8Array,
    size: number,
    planes: number,
): void {
    const expected = size * size * planes * 4;
    if (waterMask.length !== expected) {
        throw new Error(
            `Water-mask packet has ${waterMask.length} bytes; expected ${expected}`,
        );
    }
}

export function createRustStaticGeometryPacket(input: {
    vertices: Uint8Array;
    indices: Int32Array;
    modelInfoOpaque: Uint16Array;
    modelInfoAlpha: Uint16Array;
    modelInfoOpaqueLod: Uint16Array;
    modelInfoAlphaLod: Uint16Array;
    opaqueDrawRanges: readonly DrawRange[];
    opaqueDrawRangePlanes: Uint8Array;
    alphaDrawRanges: readonly DrawRange[];
    alphaDrawRangePlanes: Uint8Array;
    opaqueLodDrawRanges: readonly DrawRange[];
    opaqueLodDrawRangePlanes: Uint8Array;
    alphaLodDrawRanges: readonly DrawRange[];
    alphaLodDrawRangePlanes: Uint8Array;
}): RustStaticGeometryPacket {
    return {
        packedVertexWords: packedVertexWords(input.vertices),
        indices: unsignedIndexWords(input.indices),
        modelInfoOpaque: input.modelInfoOpaque,
        modelInfoAlpha: input.modelInfoAlpha,
        modelInfoOpaqueLod: input.modelInfoOpaqueLod,
        modelInfoAlphaLod: input.modelInfoAlphaLod,
        opaqueDrawRanges: flattenDrawRanges(input.opaqueDrawRanges),
        opaqueDrawRangePlanes: input.opaqueDrawRangePlanes,
        alphaDrawRanges: flattenDrawRanges(input.alphaDrawRanges),
        alphaDrawRangePlanes: input.alphaDrawRangePlanes,
        opaqueLodDrawRanges: flattenDrawRanges(input.opaqueLodDrawRanges),
        opaqueLodDrawRangePlanes: input.opaqueLodDrawRangePlanes,
        alphaLodDrawRanges: flattenDrawRanges(input.alphaLodDrawRanges),
        alphaLodDrawRangePlanes: input.alphaLodDrawRangePlanes,
    };
}

export function createRustLocGeometryPacket(
    data: SdMapData,
): RustStaticGeometryPacket {
    return createRustStaticGeometryPacket({
        vertices: data.loc.vertices,
        indices: data.loc.indices,
        modelInfoOpaque: data.loc.modelTextureData,
        modelInfoAlpha: data.loc.modelTextureDataAlpha,
        modelInfoOpaqueLod: data.loc.modelTextureDataLod,
        modelInfoAlphaLod: data.loc.modelTextureDataLodAlpha,
        opaqueDrawRanges: data.loc.drawRanges,
        opaqueDrawRangePlanes: data.loc.drawRangesPlanes,
        alphaDrawRanges: data.loc.drawRangesAlpha,
        alphaDrawRangePlanes: data.loc.drawRangesAlphaPlanes,
        opaqueLodDrawRanges: data.loc.drawRangesLod,
        opaqueLodDrawRangePlanes: data.loc.drawRangesLodPlanes,
        alphaLodDrawRanges: data.loc.drawRangesLodAlpha,
        alphaLodDrawRangePlanes: data.loc.drawRangesLodAlphaPlanes,
    });
}

export function createRustDoorGeometryPacket(
    data: SdMapData,
): RustStaticGeometryPacket {
    return createRustStaticGeometryPacket({
        vertices: data.doorVertices,
        indices: data.doorIndices,
        modelInfoOpaque: data.doorModelTextureData,
        modelInfoAlpha: data.doorModelTextureDataAlpha,
        modelInfoOpaqueLod: data.doorModelTextureDataLod,
        modelInfoAlphaLod: data.doorModelTextureDataLodAlpha,
        opaqueDrawRanges: data.doorDrawRanges,
        opaqueDrawRangePlanes: data.doorDrawRangesPlanes,
        alphaDrawRanges: data.doorDrawRangesAlpha,
        alphaDrawRangePlanes: data.doorDrawRangesAlphaPlanes,
        opaqueLodDrawRanges: data.doorDrawRangesLod,
        opaqueLodDrawRangePlanes: data.doorDrawRangesLodPlanes,
        alphaLodDrawRanges: data.doorDrawRangesLodAlpha,
        alphaLodDrawRangePlanes: data.doorDrawRangesLodAlphaPlanes,
    });
}

export function createRustGroundItemGeometryPacket(
    data: GroundItemGeometryBuildData,
): RustStaticGeometryPacket {
    return createRustStaticGeometryPacket({
        vertices: data.vertices,
        indices: data.indices,
        modelInfoOpaque: data.modelTextureData,
        modelInfoAlpha: data.modelTextureDataAlpha,
        modelInfoOpaqueLod: data.modelTextureDataLod,
        modelInfoAlphaLod: data.modelTextureDataLodAlpha,
        opaqueDrawRanges: data.drawRanges,
        opaqueDrawRangePlanes: data.planes.main,
        alphaDrawRanges: data.drawRangesAlpha,
        alphaDrawRangePlanes: data.planes.alpha,
        opaqueLodDrawRanges: data.drawRangesLod,
        opaqueLodDrawRangePlanes: data.planes.lod,
        alphaLodDrawRanges: data.drawRangesLodAlpha,
        alphaLodDrawRangePlanes: data.planes.lodAlpha,
    });
}

/**
 * Adapter for an already-decoded map square.
 *
 * Static scene preparation remains TypeScript in this migration phase, but
 * every value crossing into Rust is POD/numeric and can later be produced
 * directly in WASM memory.
 */
export function createRustStaticScenePacket(data: SdMapData): RustStaticScenePacket {
    const heightMapSize = data.heightMapSize;
    const heightMapPlanes = inferHeightMapPlanes(data.heightMapTextureData, heightMapSize);
    validateWaterMask(data.waterMaskTextureData, heightMapSize, heightMapPlanes);

    return {
        abiVersion: RUST_RENDERER_ABI_VERSION,

        mapKey: getMapSquareId(data.mapX, data.mapY),
        mapX: data.renderPosX ?? data.mapX,
        mapY: data.renderPosY ?? data.mapY,
        borderSize: data.borderSize,
        heightMapSize,
        heightMapPlanes,

        packedVertexWords: packedVertexWords(data.vertices),
        indices: unsignedIndexWords(data.indices),
        npcPackedVertexWords: packedVertexWords(data.npcVertices),
        npcIndices: unsignedIndexWords(data.npcIndices),

        modelInfoOpaque: data.modelTextureData,
        modelInfoAlpha: data.modelTextureDataAlpha,
        modelInfoOpaqueLod: data.modelTextureDataLod,
        modelInfoAlphaLod: data.modelTextureDataLodAlpha,
        heightMap: data.heightMapTextureData,
        waterMask: data.waterMaskTextureData,

        opaqueDrawRanges: flattenDrawRanges(data.drawRanges),
        opaqueDrawRangePlanes: data.drawRangesPlanes,
        alphaDrawRanges: flattenDrawRanges(data.drawRangesAlpha),
        alphaDrawRangePlanes: data.drawRangesAlphaPlanes,

        opaqueLodDrawRanges: flattenDrawRanges(data.drawRangesLod),
        opaqueLodDrawRangePlanes: data.drawRangesLodPlanes,
        alphaLodDrawRanges: flattenDrawRanges(data.drawRangesLodAlpha),
        alphaLodDrawRangePlanes: data.drawRangesLodAlphaPlanes,

        locGeometry: createRustLocGeometryPacket(data),
        doorGeometry: createRustDoorGeometryPacket(data),
    };
}
