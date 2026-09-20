import type { DrawRange } from "../DrawRange";
import type { SdMapData } from "../loader/SdMapData";

export const RUST_RENDERER_ABI_VERSION = 1 as const;

/**
 * Stage-0 packet sent to the Rust/WASM renderer.
 *
 * This deliberately contains numeric buffers only. Do not add PicoGL objects,
 * TypeScript class instances, callbacks, loaders, or React state here.
 */
export interface RustStaticScenePacket {
    abiVersion: typeof RUST_RENDERER_ABI_VERSION;
    packedVertexWords: Uint32Array;
    indices: Uint32Array;
    opaqueDrawRanges: Uint32Array;
    opaqueDrawRangePlanes: Uint8Array;
    alphaDrawRanges: Uint32Array;
    alphaDrawRangePlanes: Uint8Array;
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
 * The renderer's existing DataBuffer allocation is naturally 4-byte aligned.
 * A defensive copy is used only for a misaligned external view.
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

/**
 * Stage-0 adapter for an already-decoded map square. This is intentionally
 * small so it can disappear after scene/mesh preparation itself moves to Rust.
 */
export function createRustStaticScenePacket(data: SdMapData): RustStaticScenePacket {
    return {
        abiVersion: RUST_RENDERER_ABI_VERSION,
        packedVertexWords: packedVertexWords(data.vertices),
        indices: unsignedIndexWords(data.indices),
        opaqueDrawRanges: flattenDrawRanges(data.drawRanges),
        opaqueDrawRangePlanes: data.drawRangesPlanes,
        alphaDrawRanges: flattenDrawRanges(data.drawRangesAlpha),
        alphaDrawRangePlanes: data.drawRangesAlphaPlanes,
    };
}
