import {
    RUST_RENDERER_ABI_VERSION,
    RustStaticGeometryPacket,
    RustStaticScenePacket,
} from "./RendererPacket";

export interface RustRendererWasm {
    abi_version(): number;
    select_static_map(mapKey: number): void;
    active_static_map_key(): number;
    resident_static_map_count(): number;
    remove_static_map(mapKey: number): void;
    clear_static_maps(): void;

    upload_geometry(vertices: Uint32Array, indices: Uint32Array): void;
    upload_npc_geometry(vertices: Uint32Array, indices: Uint32Array): void;
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
    upload_static_lod_passes(
        modelInfoOpaque: Uint16Array,
        opaqueRanges: Uint32Array,
        opaqueRangePlanes: Uint8Array,
        modelInfoAlpha: Uint16Array,
        alphaRanges: Uint32Array,
        alphaRangePlanes: Uint8Array,
    ): void;
    upload_aux_geometry(
        kind: number,
        vertices: Uint32Array,
        indices: Uint32Array,
    ): void;
    upload_aux_passes(
        kind: number,
        modelInfoOpaque: Uint16Array,
        opaqueRanges: Uint32Array,
        opaqueRangePlanes: Uint8Array,
        modelInfoAlpha: Uint16Array,
        alphaRanges: Uint32Array,
        alphaRangePlanes: Uint8Array,
    ): void;
    upload_aux_lod_passes(
        kind: number,
        modelInfoOpaque: Uint16Array,
        opaqueRanges: Uint32Array,
        opaqueRangePlanes: Uint8Array,
        modelInfoAlpha: Uint16Array,
        alphaRanges: Uint32Array,
        alphaRangePlanes: Uint8Array,
    ): void;
    patch_aux_draw_ranges(
        kind: number,
        lod: boolean,
        alpha: boolean,
        patches: Uint32Array,
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
    upload_actor_data(
        values: Uint16Array,
        width: number,
        height: number,
    ): void;

    begin_static_frame(skyRgba: Float32Array): void;
    render_active_static_map_pass(
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
        useLod: boolean,
        isNewTextureAnim: boolean,
        colorBanding: number,
        transparent: boolean,
    ): void;
    render_active_static_terrain_ghost_pass(
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
        useLod: boolean,
        isNewTextureAnim: boolean,
        colorBanding: number,
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
        useLod: boolean,
        isNewTextureAnim: boolean,
        colorBanding: number,
    ): void;

    last_draw_calls(): number;
    last_submitted_indices(): number;
    last_draw_hash(): number;
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
    useLod: boolean;
    isNewTextureAnim: boolean;
    colorBanding: number;
}

export interface RustResidentMapFrameState extends RustStaticFrameState {
    mapKey: number;
    worldEntityGhostSceneHslOverride?: Float32Array;
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
    private readonly uploadedMapKeys = new Set<number>();
    private globalResourcesRevision: number | undefined;

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

    uploadActorData(
        values: Uint16Array,
        width: number,
        height: number,
    ): void {
        if (
            !Number.isInteger(width)
            || !Number.isInteger(height)
            || width <= 0
            || height <= 0
        ) {
            throw new Error(
                `Invalid actor-data texture dimensions: ${width}x${height}`,
            );
        }

        const expected = width * height * 4;
        if (values.length !== expected) {
            throw new Error(
                `Actor-data packet has ${values.length} u16 values; expected ${expected} for ${width}x${height} RGBA16UI`,
            );
        }

        this.wasm.upload_actor_data(values, width, height);
    }

    /**
     * Upload renderer-global resources only when the live resource mirror
     * revision changes. Returns true when WASM received a new snapshot.
     */
    syncGlobalResources(
        resources: RustRendererGlobalResources,
        revision: number,
    ): boolean {
        const normalizedRevision = revision | 0;
        if (this.globalResourcesRevision === normalizedRevision) {
            return false;
        }

        this.uploadGlobalResources(resources);
        this.globalResourcesRevision = normalizedRevision;
        return true;
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

        this.wasm.select_static_map(packet.mapKey);
        this.wasm.upload_geometry(
            packet.packedVertexWords,
            packet.indices,
        );
        this.wasm.upload_npc_geometry(
            packet.npcPackedVertexWords,
            packet.npcIndices,
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
        this.wasm.upload_static_lod_passes(
            packet.modelInfoOpaqueLod,
            packet.opaqueLodDrawRanges,
            packet.opaqueLodDrawRangePlanes,
            packet.modelInfoAlphaLod,
            packet.alphaLodDrawRanges,
            packet.alphaLodDrawRangePlanes,
        );
        this.uploadAuxStaticGeometry(0, packet.locGeometry);
        this.uploadAuxStaticGeometry(1, packet.doorGeometry);
        this.uploadedMapKeys.add(packet.mapKey);
        this.uploadedPacket = packet;
    }

    renderStatic(frame: RustStaticFrameState): void {
        const packet = this.uploadedPacket;
        if (!packet) {
            throw new Error("Rust static scene has not been uploaded");
        }

        this.wasm.select_static_map(packet.mapKey);
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
            frame.useLod,
            frame.isNewTextureAnim,
            frame.colorBanding,
        );
    }

    beginStaticFrame(
        frames: readonly RustResidentMapFrameState[],
    ): boolean {
        if (frames.length === 0) {
            return false;
        }
        this.assertResidentFramesUploaded(frames);
        this.wasm.begin_static_frame(frames[0].skyRgba);
        return true;
    }

    renderOpaqueStaticMaps(
        frames: readonly RustResidentMapFrameState[],
    ): void {
        if (frames.length === 0) return;
        this.assertResidentFramesUploaded(frames);

        for (const frame of frames) {
            this.renderResidentMapPass(frame, false);
            if (frame.worldEntityGhostSceneHslOverride) {
                this.renderResidentTerrainGhostPass(frame);
            }
        }
    }

    renderTransparentStaticMaps(
        frames: readonly RustResidentMapFrameState[],
    ): void {
        if (frames.length === 0) return;
        this.assertResidentFramesUploaded(frames);

        for (let i = frames.length - 1; i >= 0; i--) {
            this.renderResidentMapPass(frames[i], true);
        }
    }

    renderStaticMaps(frames: readonly RustResidentMapFrameState[]): void {
        if (!this.beginStaticFrame(frames)) return;
        this.renderOpaqueStaticMaps(frames);
        this.renderTransparentStaticMaps(frames);
    }

    getLastStats(): { drawCalls: number; submittedIndices: number } {
        return {
            drawCalls: this.wasm.last_draw_calls(),
            submittedIndices: this.wasm.last_submitted_indices(),
        };
    }

    getLastDrawHash(): number {
        return this.wasm.last_draw_hash() >>> 0;
    }

    private assertResidentFramesUploaded(
        frames: readonly RustResidentMapFrameState[],
    ): void {
        for (const frame of frames) {
            if (!this.uploadedMapKeys.has(frame.mapKey)) {
                throw new Error(
                    `Rust static map ${frame.mapKey} has not been uploaded`,
                );
            }
        }
    }

    private renderResidentMapPass(
        frame: RustResidentMapFrameState,
        transparent: boolean,
    ): void {
        this.wasm.select_static_map(frame.mapKey);
        this.wasm.render_active_static_map_pass(
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
            frame.useLod,
            frame.isNewTextureAnim,
            frame.colorBanding,
            transparent,
        );
    }

    private renderResidentTerrainGhostPass(
        frame: RustResidentMapFrameState,
    ): void {
        const sceneHslOverride =
            frame.worldEntityGhostSceneHslOverride;
        if (!sceneHslOverride) return;

        this.wasm.select_static_map(frame.mapKey);
        this.wasm.render_active_static_terrain_ghost_pass(
            frame.viewMatrix,
            frame.projectionMatrix,
            frame.worldEntityTransform,
            0.01,
            frame.skyRgba,
            sceneHslOverride,
            frame.playerPos,
            frame.renderDistance,
            frame.fogDepth,
            frame.currentTime,
            frame.brightness,
            frame.roofPlaneLimit,
            frame.useLod,
            frame.isNewTextureAnim,
            frame.colorBanding,
        );
    }

    private uploadAuxStaticGeometry(
        kind: number,
        geometry: RustStaticGeometryPacket,
    ): void {
        this.wasm.upload_aux_geometry(
            kind,
            geometry.packedVertexWords,
            geometry.indices,
        );

        if (
            geometry.packedVertexWords.length === 0
            || geometry.indices.length === 0
        ) {
            return;
        }

        this.wasm.upload_aux_passes(
            kind,
            geometry.modelInfoOpaque,
            geometry.opaqueDrawRanges,
            geometry.opaqueDrawRangePlanes,
            geometry.modelInfoAlpha,
            geometry.alphaDrawRanges,
            geometry.alphaDrawRangePlanes,
        );
        this.wasm.upload_aux_lod_passes(
            kind,
            geometry.modelInfoOpaqueLod,
            geometry.opaqueLodDrawRanges,
            geometry.opaqueLodDrawRangePlanes,
            geometry.modelInfoAlphaLod,
            geometry.alphaLodDrawRanges,
            geometry.alphaLodDrawRangePlanes,
        );
    }

    getResidentStaticMapCount(): number {
        return this.wasm.resident_static_map_count();
    }

    hasStaticMap(mapKey: number): boolean {
        return this.uploadedMapKeys.has(mapKey);
    }

    updateLocGeometry(
        mapKey: number,
        geometry: RustStaticGeometryPacket,
    ): boolean {
        if (!this.uploadedMapKeys.has(mapKey)) return false;
        this.wasm.select_static_map(mapKey);
        this.uploadAuxStaticGeometry(0, geometry);
        return true;
    }

    updateDoorGeometry(
        mapKey: number,
        geometry: RustStaticGeometryPacket,
    ): boolean {
        if (!this.uploadedMapKeys.has(mapKey)) return false;
        this.wasm.select_static_map(mapKey);
        this.uploadAuxStaticGeometry(1, geometry);
        return true;
    }

    updateGroundGeometry(
        mapKey: number,
        geometry?: RustStaticGeometryPacket,
    ): boolean {
        if (!this.uploadedMapKeys.has(mapKey)) return false;
        this.wasm.select_static_map(mapKey);

        if (!geometry) {
            this.wasm.upload_aux_geometry(
                2,
                new Uint32Array(),
                new Uint32Array(),
            );
            return true;
        }

        this.uploadAuxStaticGeometry(2, geometry);
        return true;
    }

    patchLocDrawRanges(
        mapKey: number,
        useLod: boolean,
        transparent: boolean,
        patches: Uint32Array,
    ): boolean {
        if (patches.length === 0) return true;
        if (!this.uploadedMapKeys.has(mapKey)) return false;
        this.wasm.select_static_map(mapKey);
        this.wasm.patch_aux_draw_ranges(
            0,
            useLod,
            transparent,
            patches,
        );
        return true;
    }

    removeStaticMap(mapKey: number): void {
        this.wasm.remove_static_map(mapKey);
        this.uploadedMapKeys.delete(mapKey);
        if (this.uploadedPacket?.mapKey === mapKey) {
            this.uploadedPacket = undefined;
        }
    }

    clearStaticMaps(): void {
        this.wasm.clear_static_maps();
        this.uploadedMapKeys.clear();
        this.uploadedPacket = undefined;
    }

    dispose(): void {
        this.uploadedMapKeys.clear();
        this.uploadedPacket = undefined;
        this.globalResourcesRevision = undefined;
        this.wasm.dispose();
    }

}
