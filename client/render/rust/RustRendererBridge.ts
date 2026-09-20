import {
    RUST_RENDERER_ABI_VERSION,
    RustStaticScenePacket,
} from "./RendererPacket";

export interface RustRendererWasm {
    abi_version(): number;

    upload_geometry(vertices: Uint32Array, indices: Uint32Array): void;
    upload_model_info(modelInfo: Uint16Array): void;
    upload_height_map(heightMap: Int16Array, size: number, planes: number): void;
    upload_water_mask(waterMask: Uint8Array, size: number, planes: number): void;
    set_static_map_state(
        mapX: number,
        mapY: number,
        borderSize: number,
        heightMapSize: number,
        heightMapPlanes: number,
        timeLoaded: number,
    ): void;
    set_draw_ranges(flatRanges: Uint32Array): void;
    upload_static_passes(
        modelInfoOpaque: Uint16Array,
        opaqueRanges: Uint32Array,
        opaqueRangePlanes: Uint8Array,
        modelInfoAlpha: Uint16Array,
        alphaRanges: Uint32Array,
        alphaRangePlanes: Uint8Array,
    ): void;

    upload_texture_array(
        pixels: Uint8Array,
        width: number,
        height: number,
        layers: number,
    ): void;
    upload_materials(materials: Int8Array, textureCount: number): void;
    upload_water_textures(
        pixels: Uint8Array,
        width: number,
        height: number,
        layers: number,
    ): void;

    render_static_frame(
        viewMatrix: Float32Array,
        projectionMatrix: Float32Array,
        worldEntityTransform: Float32Array,
        worldEntityOpacity: number,
        skyRgba: Float32Array,
        sceneHslOverride: Float32Array,
        playerPos: Float32Array,
        renderDistance: number,
        fogDepth: number,
        currentTime: number,
        brightness: number,
        roofPlaneLimit: number,
        isNewTextureAnim: boolean,
        colorBanding: number,
    ): void;

    last_draw_calls(): number;
    last_submitted_indices(): number;
    dispose(): void;
}

export type RustRendererWasmConstructor = new (
    canvas: HTMLCanvasElement,
) => RustRendererWasm;

export interface RustRendererGlobalResources {
    texturePixels: Uint8Array;
    textureWidth: number;
    textureHeight: number;
    textureLayers: number;

    materialBytes: Int8Array;
    materialCount: number;

    waterPixels: Uint8Array;
    waterWidth: number;
    waterHeight: number;
    waterLayers: number;
}

export interface RustStaticFrameState {
    viewMatrix: Float32Array;
    projectionMatrix: Float32Array;
    worldEntityTransform: Float32Array;
    worldEntityOpacity: number;
    skyRgba: Float32Array;
    sceneHslOverride: Float32Array;
    playerPos: Float32Array;

    renderDistance: number;
    fogDepth: number;
    currentTime: number;
    brightness: number;
    roofPlaneLimit: number;
    isNewTextureAnim: boolean;
    colorBanding: number;
}

/**
 * Thin TypeScript host for the Rust/WASM renderer.
 *
 * This class intentionally does not know about PicoGL, WebGLMapSquare or any
 * loader class. It only translates the versioned numeric scene packet into the
 * wasm-bindgen surface. That keeps the boundary removable once scene building
 * itself moves to Rust.
 */
export class RustRendererBridge {
    readonly wasm: RustRendererWasm;

    private uploadedPacket?: RustStaticScenePacket;

    constructor(
        canvas: HTMLCanvasElement,
        Renderer: RustRendererWasmConstructor,
    ) {
        this.wasm = new Renderer(canvas);
        const abiVersion = this.wasm.abi_version();
        if (abiVersion !== RUST_RENDERER_ABI_VERSION) {
            this.wasm.dispose();
            throw new Error(
                `Rust renderer ABI mismatch: TypeScript expects ${RUST_RENDERER_ABI_VERSION}, wasm exports ${abiVersion}`,
            );
        }
    }

    uploadGlobalResources(resources: RustRendererGlobalResources): void {
        this.wasm.upload_texture_array(
            resources.texturePixels,
            resources.textureWidth,
            resources.textureHeight,
            resources.textureLayers,
        );
        this.wasm.upload_materials(
            resources.materialBytes,
            resources.materialCount,
        );
        this.wasm.upload_water_textures(
            resources.waterPixels,
            resources.waterWidth,
            resources.waterHeight,
            resources.waterLayers,
        );
    }

    uploadStaticScene(
        packet: RustStaticScenePacket,
        timeLoaded: number,
    ): void {
        if (packet.abiVersion !== RUST_RENDERER_ABI_VERSION) {
            throw new Error(
                `Static scene packet ABI mismatch: expected ${RUST_RENDERER_ABI_VERSION}, got ${packet.abiVersion}`,
            );
        }

        this.wasm.upload_geometry(
            packet.packedVertexWords,
            packet.indices,
        );
        this.wasm.upload_height_map(
            packet.heightMap,
            packet.heightMapSize,
            packet.heightMapPlanes,
        );
        this.wasm.upload_water_mask(
            packet.waterMask,
            packet.heightMapSize,
            packet.heightMapPlanes,
        );
        this.wasm.set_static_map_state(
            packet.mapX,
            packet.mapY,
            packet.borderSize,
            packet.heightMapSize,
            packet.heightMapPlanes,
            timeLoaded,
        );
        this.wasm.upload_static_passes(
            packet.modelInfoOpaque,
            packet.opaqueDrawRanges,
            packet.opaqueDrawRangePlanes,
            packet.modelInfoAlpha,
            packet.alphaDrawRanges,
            packet.alphaDrawRangePlanes,
        );
        this.uploadedPacket = packet;
    }

    renderStatic(frame: RustStaticFrameState): void {
        const packet = this.uploadedPacket;
        if (!packet) {
            throw new Error("Rust static scene has not been uploaded");
        }

        this.wasm.render_static_frame(
            frame.viewMatrix,
            frame.projectionMatrix,
            frame.worldEntityTransform,
            frame.worldEntityOpacity,
            frame.skyRgba,
            frame.sceneHslOverride,
            frame.playerPos,
            frame.renderDistance,
            frame.fogDepth,
            frame.currentTime,
            frame.brightness,
            frame.roofPlaneLimit,
            frame.isNewTextureAnim,
            frame.colorBanding,
        );
    }

    getLastStats(): { drawCalls: number; submittedIndices: number } {
        return {
            drawCalls: this.wasm.last_draw_calls(),
            submittedIndices: this.wasm.last_submitted_indices(),
        };
    }

    dispose(): void {
        this.uploadedPacket = undefined;
        this.wasm.dispose();
    }

}
