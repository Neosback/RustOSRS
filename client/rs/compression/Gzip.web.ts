import pako from "pako";
import init, { decompress } from "wasm-gzip";

const wasmGzipUrl = require("wasm-gzip/wasm_gzip.wasm");

export class Gzip {
    static wasmLoaded = false;

    static async initWasm(): Promise<void> {
        await init(wasmGzipUrl);
        Gzip.wasmLoaded = true;
    }

    static decompress(compressed: Uint8Array): Int8Array {
        if (!Gzip.wasmLoaded) {
            const decompressed = pako.ungzip(compressed);
            return new Int8Array(decompressed.buffer, decompressed.byteOffset, decompressed.byteLength);
        }
        const decompressed = decompress(compressed);
        if (!decompressed) {
            throw new Error("Failed to decompress gzip");
        }
        // wasm-gzip returns a view into the WASM memory buffer (often 2MB+),
        // so we must use byteOffset/byteLength to get the actual data, not the
        // entire memory buffer. Copy the slice to detach it from WASM memory
        // before the next decompress() call reuses the buffer.
        return new Int8Array(
            decompressed.buffer.slice(decompressed.byteOffset, decompressed.byteOffset + decompressed.byteLength),
        );
    }
}
