import assert from "node:assert/strict";

import {
    RustRendererBridge,
    RustRendererWasm,
    RustResidentMapFrameState,
    RustStaticFrameState,
} from "../render/rust/RustRendererBridge";
import {
    RUST_RENDERER_ABI_VERSION,
    RustStaticGeometryPacket,
    RustStaticScenePacket,
} from "../render/rust/RendererPacket";
import { getRustRendererGlobalResourceSnapshot } from "../render/rust/LiveResourceAdapter";
import { getRustRendererRuntimeMode } from "../render/rust/RustRendererRuntime";

assert.equal(getRustRendererRuntimeMode(""), "off");
assert.equal(getRustRendererRuntimeMode("?rust-renderer=off"), "off");
assert.equal(getRustRendererRuntimeMode("?rust-renderer=shadow"), "shadow");

class MockWasm implements RustRendererWasm {
    static abiVersion = RUST_RENDERER_ABI_VERSION;
    static last?: MockWasm;

    disposed = false;
    selectedMapKey = 0;
    residentMapKeys = new Set<number>();
    geometryUploads = 0;
    npcGeometryUploads: Array<{
        vertices: Uint32Array;
        indices: Uint32Array;
    }> = [];
    dynamicNpcGeometryUploads: Array<{
        vertices: Uint32Array;
        indices: Uint32Array;
    }> = [];
    dynamicGfxGeometryUploads: Array<{
        vertices: Uint32Array;
        indices: Uint32Array;
    }> = [];
    dynamicProjectileGeometryUploads: Array<{
        vertices: Uint32Array;
        indices: Uint32Array;
    }> = [];
    dynamicPlayerGeometryUploads: Array<{
        vertices: Uint32Array;
        indices: Uint32Array;
    }> = [];
    modelInfoUploads: Uint16Array[] = [];
    heightUploads = 0;
    waterMaskUploads = 0;
    staticStateCalls = 0;
    drawRangeUploads: Uint32Array[] = [];
    staticPassUploads = 0;
    staticLodPassUploads = 0;
    auxGeometryUploads: number[] = [];
    auxPassUploads: number[] = [];
    auxLodPassUploads: number[] = [];
    auxRangePatches: Array<{
        kind: number;
        lod: boolean;
        alpha: boolean;
        patches: Uint32Array;
    }> = [];
    textureResourceUploads = 0;
    materialResourceUploads = 0;
    waterResourceUploads = 0;
    actorDataUploads: Array<{
        values: Uint16Array;
        width: number;
        height: number;
    }> = [];
    renderFrameCalls = 0;
    beginFrameCalls = 0;
    mapPassCalls: Array<{ mapKey: number; transparent: boolean }> = [];
    ghostPassCalls: Array<{
        mapKey: number;
        opacity: number;
        hsl: number[];
    }> = [];
    npcPassCalls: Array<{
        mapKey: number;
        ranges: Uint32Array;
        npcDataOffset: number;
        modelYOffset: number;
        transparent: boolean;
        worldEntityTransform: Float32Array;
    }> = [];
    dynamicNpcPassCalls: Array<{
        mapKey: number;
        npcDataOffset: number;
        modelYOffset: number;
        transparent: boolean;
        worldEntityTransform: Float32Array;
    }> = [];
    gfxPassCalls: Array<{
        mapKey: number;
        actorDataOffset: number;
        modelYOffset: number;
        mapX: number;
        mapY: number;
        transparent: boolean;
    }> = [];
    projectilePassCalls: Array<{
        mapKey: number;
        projectileDataOffset: number;
        modelYOffset: number;
        projectileSubOffset: Float32Array;
        mapX: number;
        mapY: number;
        transparent: boolean;
        cullBackFace: boolean;
    }> = [];
    playerPassCalls: Array<{
        mapKey: number;
        playerDataOffset: number;
        playerSlots: Int32Array;
        modelYOffset: number;
        transparent: boolean;
        cullBackFace: boolean;
        worldEntityTransform: Float32Array;
    }> = [];
    passSequence: Array<{
        mapKey: number;
        pass: "opaque" | "ghost" | "transparent";
    }> = [];
    lastWorldEntityTransform?: Float32Array;
    lastWorldEntityOpacity = 1;

    private opaqueRanges = new Uint32Array();
    private alphaRanges = new Uint32Array();
    private opaqueRangePlanes = new Uint8Array();
    private alphaRangePlanes = new Uint8Array();
    private opaqueLodRanges = new Uint32Array();
    private alphaLodRanges = new Uint32Array();
    private opaqueLodRangePlanes = new Uint8Array();
    private alphaLodRangePlanes = new Uint8Array();
    private roofPlaneLimit = 3;
    private useLod = false;

    constructor(_canvas: HTMLCanvasElement) {
        MockWasm.last = this;
    }

    abi_version(): number {
        return MockWasm.abiVersion;
    }

    select_static_map(mapKey: number): void {
        this.selectedMapKey = mapKey;
        this.residentMapKeys.add(mapKey);
    }

    active_static_map_key(): number {
        return this.selectedMapKey;
    }

    resident_static_map_count(): number {
        return this.residentMapKeys.size;
    }

    remove_static_map(mapKey: number): void {
        this.residentMapKeys.delete(mapKey);
        if (this.selectedMapKey === mapKey) {
            this.selectedMapKey = this.residentMapKeys.values().next().value ?? 0;
        }
    }

    clear_static_maps(): void {
        this.residentMapKeys.clear();
        this.selectedMapKey = 0;
    }

    upload_geometry(_vertices: Uint32Array, _indices: Uint32Array): void {
        this.geometryUploads++;
    }

    upload_npc_geometry(
        vertices: Uint32Array,
        indices: Uint32Array,
    ): void {
        this.npcGeometryUploads.push({
            vertices: new Uint32Array(vertices),
            indices: new Uint32Array(indices),
        });
    }

    upload_dynamic_npc_geometry(
        vertices: Uint32Array,
        indices: Uint32Array,
    ): void {
        this.dynamicNpcGeometryUploads.push({
            vertices: new Uint32Array(vertices),
            indices: new Uint32Array(indices),
        });
    }

    upload_dynamic_gfx_geometry(
        vertices: Uint32Array,
        indices: Uint32Array,
    ): void {
        this.dynamicGfxGeometryUploads.push({
            vertices: new Uint32Array(vertices),
            indices: new Uint32Array(indices),
        });
    }

    upload_dynamic_projectile_geometry(
        vertices: Uint32Array,
        indices: Uint32Array,
    ): void {
        this.dynamicProjectileGeometryUploads.push({
            vertices: new Uint32Array(vertices),
            indices: new Uint32Array(indices),
        });
    }

    upload_dynamic_player_geometry(
        vertices: Uint32Array,
        indices: Uint32Array,
    ): void {
        this.dynamicPlayerGeometryUploads.push({
            vertices: new Uint32Array(vertices),
            indices: new Uint32Array(indices),
        });
    }

    upload_model_info(modelInfo: Uint16Array): void {
        this.modelInfoUploads.push(modelInfo);
    }

    upload_height_map(
        _heightMap: Int16Array,
        _size: number,
        _planes: number,
    ): void {
        this.heightUploads++;
    }

    upload_water_mask(
        _waterMask: Uint8Array,
        _size: number,
        _planes: number,
    ): void {
        this.waterMaskUploads++;
    }

    set_static_map_state(
        _mapX: number,
        _mapY: number,
        _borderSize: number,
        _heightMapSize: number,
        _heightMapPlanes: number,
        _timeLoaded: number,
    ): void {
        this.staticStateCalls++;
    }

    set_draw_ranges(flatRanges: Uint32Array): void {
        this.drawRangeUploads.push(flatRanges);
    }

    upload_static_passes(
        _modelInfoOpaque: Uint16Array,
        opaqueRanges: Uint32Array,
        opaqueRangePlanes: Uint8Array,
        _modelInfoAlpha: Uint16Array,
        alphaRanges: Uint32Array,
        alphaRangePlanes: Uint8Array,
    ): void {
        this.staticPassUploads++;
        this.opaqueRanges = opaqueRanges;
        this.alphaRanges = alphaRanges;
        this.opaqueRangePlanes = opaqueRangePlanes;
        this.alphaRangePlanes = alphaRangePlanes;
    }

    upload_static_lod_passes(
        _modelInfoOpaque: Uint16Array,
        opaqueRanges: Uint32Array,
        opaqueRangePlanes: Uint8Array,
        _modelInfoAlpha: Uint16Array,
        alphaRanges: Uint32Array,
        alphaRangePlanes: Uint8Array,
    ): void {
        this.staticLodPassUploads++;
        this.opaqueLodRanges = opaqueRanges;
        this.alphaLodRanges = alphaRanges;
        this.opaqueLodRangePlanes = opaqueRangePlanes;
        this.alphaLodRangePlanes = alphaRangePlanes;
    }

    upload_aux_geometry(
        kind: number,
        _vertices: Uint32Array,
        _indices: Uint32Array,
    ): void {
        this.auxGeometryUploads.push(kind);
    }

    upload_aux_passes(
        kind: number,
        _modelInfoOpaque: Uint16Array,
        _opaqueRanges: Uint32Array,
        _opaqueRangePlanes: Uint8Array,
        _modelInfoAlpha: Uint16Array,
        _alphaRanges: Uint32Array,
        _alphaRangePlanes: Uint8Array,
    ): void {
        this.auxPassUploads.push(kind);
    }

    upload_aux_lod_passes(
        kind: number,
        _modelInfoOpaque: Uint16Array,
        _opaqueRanges: Uint32Array,
        _opaqueRangePlanes: Uint8Array,
        _modelInfoAlpha: Uint16Array,
        _alphaRanges: Uint32Array,
        _alphaRangePlanes: Uint8Array,
    ): void {
        this.auxLodPassUploads.push(kind);
    }

    patch_aux_draw_ranges(
        kind: number,
        lod: boolean,
        alpha: boolean,
        patches: Uint32Array,
    ): void {
        this.auxRangePatches.push({
            kind,
            lod,
            alpha,
            patches: new Uint32Array(patches),
        });
    }

    upload_texture_array(
        _pixels: Uint8Array,
        _width: number,
        _height: number,
        _layers: number,
    ): void {
        this.textureResourceUploads++;
    }

    upload_materials(
        _materials: Int8Array,
        _textureCount: number,
    ): void {
        this.materialResourceUploads++;
    }

    upload_water_textures(
        _pixels: Uint8Array,
        _width: number,
        _height: number,
        _layers: number,
    ): void {
        this.waterResourceUploads++;
    }

    upload_actor_data(
        values: Uint16Array,
        width: number,
        height: number,
    ): void {
        this.actorDataUploads.push({
            values: new Uint16Array(values),
            width,
            height,
        });
    }

    begin_static_frame(_skyRgba: Float32Array): void {
        this.beginFrameCalls++;
    }

    render_active_static_map_pass(
        _viewMatrix: Float32Array,
        _projectionMatrix: Float32Array,
        worldEntityTransform: Float32Array,
        worldEntityOpacity: number,
        _skyRgba: Float32Array,
        _sceneHslOverride: Float32Array,
        _playerPos: Float32Array,
        _renderDistance: number,
        _fogDepth: number,
        _currentTime: number,
        _brightness: number,
        roofPlaneLimit: number,
        useLod: boolean,
        _isNewTextureAnim: boolean,
        _colorBanding: number,
        transparent: boolean,
    ): void {
        this.roofPlaneLimit = roofPlaneLimit;
        this.useLod = useLod;
        this.lastWorldEntityTransform = worldEntityTransform;
        this.lastWorldEntityOpacity = worldEntityOpacity;
        this.mapPassCalls.push({
            mapKey: this.selectedMapKey,
            transparent,
        });
        this.passSequence.push({
            mapKey: this.selectedMapKey,
            pass: transparent ? "transparent" : "opaque",
        });
    }

    render_active_npc_pass(
        drawRanges: Uint32Array,
        _viewMatrix: Float32Array,
        _projectionMatrix: Float32Array,
        worldEntityTransform: Float32Array,
        _worldEntityOpacity: number,
        _skyRgba: Float32Array,
        _sceneHslOverride: Float32Array,
        _playerPos: Float32Array,
        _renderDistance: number,
        _fogDepth: number,
        _currentTime: number,
        _brightness: number,
        _isNewTextureAnim: boolean,
        _colorBanding: number,
        npcDataOffset: number,
        modelYOffset: number,
        transparent: boolean,
    ): void {
        this.npcPassCalls.push({
            mapKey: this.selectedMapKey,
            ranges: new Uint32Array(drawRanges),
            npcDataOffset,
            modelYOffset,
            transparent,
            worldEntityTransform: new Float32Array(worldEntityTransform),
        });
    }

    render_active_dynamic_npc_pass(
        _viewMatrix: Float32Array,
        _projectionMatrix: Float32Array,
        worldEntityTransform: Float32Array,
        _worldEntityOpacity: number,
        _skyRgba: Float32Array,
        _sceneHslOverride: Float32Array,
        _playerPos: Float32Array,
        _renderDistance: number,
        _fogDepth: number,
        _currentTime: number,
        _brightness: number,
        _isNewTextureAnim: boolean,
        _colorBanding: number,
        npcDataOffset: number,
        modelYOffset: number,
        transparent: boolean,
    ): void {
        this.dynamicNpcPassCalls.push({
            mapKey: this.selectedMapKey,
            npcDataOffset,
            modelYOffset,
            transparent,
            worldEntityTransform: new Float32Array(worldEntityTransform),
        });
    }

    render_active_gfx_pass(
        _viewMatrix: Float32Array,
        _projectionMatrix: Float32Array,
        _skyRgba: Float32Array,
        _sceneHslOverride: Float32Array,
        _playerPos: Float32Array,
        _renderDistance: number,
        _fogDepth: number,
        _currentTime: number,
        _brightness: number,
        _isNewTextureAnim: boolean,
        _colorBanding: number,
        actorDataOffset: number,
        modelYOffset: number,
        mapX: number,
        mapY: number,
        transparent: boolean,
    ): void {
        this.gfxPassCalls.push({
            mapKey: this.selectedMapKey,
            actorDataOffset,
            modelYOffset,
            mapX,
            mapY,
            transparent,
        });
    }

    render_active_projectile_pass(
        _viewMatrix: Float32Array,
        _projectionMatrix: Float32Array,
        _skyRgba: Float32Array,
        _sceneHslOverride: Float32Array,
        _playerPos: Float32Array,
        _renderDistance: number,
        _fogDepth: number,
        _currentTime: number,
        _brightness: number,
        _isNewTextureAnim: boolean,
        _colorBanding: number,
        projectileDataOffset: number,
        modelYOffset: number,
        projectileSubOffset: Float32Array,
        mapX: number,
        mapY: number,
        transparent: boolean,
        cullBackFace: boolean,
    ): void {
        this.projectilePassCalls.push({
            mapKey: this.selectedMapKey,
            projectileDataOffset,
            modelYOffset,
            projectileSubOffset: new Float32Array(projectileSubOffset),
            mapX,
            mapY,
            transparent,
            cullBackFace,
        });
    }

    render_active_player_pass(
        _viewMatrix: Float32Array,
        _projectionMatrix: Float32Array,
        worldEntityTransform: Float32Array,
        _skyRgba: Float32Array,
        _sceneHslOverride: Float32Array,
        _playerPos: Float32Array,
        _renderDistance: number,
        _fogDepth: number,
        _currentTime: number,
        _brightness: number,
        _isNewTextureAnim: boolean,
        _colorBanding: number,
        playerDataOffset: number,
        playerSlots: Int32Array,
        modelYOffset: number,
        transparent: boolean,
        cullBackFace: boolean,
    ): void {
        this.playerPassCalls.push({
            mapKey: this.selectedMapKey,
            playerDataOffset,
            playerSlots: new Int32Array(playerSlots),
            modelYOffset,
            transparent,
            cullBackFace,
            worldEntityTransform: new Float32Array(worldEntityTransform),
        });
    }

    render_active_static_terrain_ghost_pass(
        _viewMatrix: Float32Array,
        _projectionMatrix: Float32Array,
        worldEntityTransform: Float32Array,
        worldEntityOpacity: number,
        _skyRgba: Float32Array,
        sceneHslOverride: Float32Array,
        _playerPos: Float32Array,
        _renderDistance: number,
        _fogDepth: number,
        _currentTime: number,
        _brightness: number,
        roofPlaneLimit: number,
        useLod: boolean,
        _isNewTextureAnim: boolean,
        _colorBanding: number,
    ): void {
        this.roofPlaneLimit = roofPlaneLimit;
        this.useLod = useLod;
        this.lastWorldEntityTransform = worldEntityTransform;
        this.lastWorldEntityOpacity = worldEntityOpacity;
        this.ghostPassCalls.push({
            mapKey: this.selectedMapKey,
            opacity: worldEntityOpacity,
            hsl: Array.from(sceneHslOverride),
        });
        this.passSequence.push({
            mapKey: this.selectedMapKey,
            pass: "ghost",
        });
    }

    render_static_frame(
        _viewMatrix: Float32Array,
        _projectionMatrix: Float32Array,
        worldEntityTransform: Float32Array,
        worldEntityOpacity: number,
        _skyRgba: Float32Array,
        _sceneHslOverride: Float32Array,
        _playerPos: Float32Array,
        _renderDistance: number,
        _fogDepth: number,
        _currentTime: number,
        _brightness: number,
        roofPlaneLimit: number,
        useLod: boolean,
        _isNewTextureAnim: boolean,
        _colorBanding: number,
    ): void {
        this.renderFrameCalls++;
        this.roofPlaneLimit = roofPlaneLimit;
        this.useLod = useLod;
        this.lastWorldEntityTransform = worldEntityTransform;
        this.lastWorldEntityOpacity = worldEntityOpacity;
    }

    last_draw_calls(): number {
        if (this.renderFrameCalls === 0) return 0;
        const opaqueRanges = this.useLod ? this.opaqueLodRanges : this.opaqueRanges;
        const alphaRanges = this.useLod ? this.alphaLodRanges : this.alphaRanges;
        const opaquePlanes = this.useLod ? this.opaqueLodRangePlanes : this.opaqueRangePlanes;
        const alphaPlanes = this.useLod ? this.alphaLodRangePlanes : this.alphaRangePlanes;
        return (
            this.countDrawCalls(opaqueRanges, opaquePlanes)
            + this.countDrawCalls(alphaRanges, alphaPlanes)
        );
    }

    last_submitted_indices(): number {
        if (this.renderFrameCalls === 0) return 0;
        const opaqueRanges = this.useLod ? this.opaqueLodRanges : this.opaqueRanges;
        const alphaRanges = this.useLod ? this.alphaLodRanges : this.alphaRanges;
        const opaquePlanes = this.useLod ? this.opaqueLodRangePlanes : this.opaqueRangePlanes;
        const alphaPlanes = this.useLod ? this.alphaLodRangePlanes : this.alphaRangePlanes;
        return (
            this.countSubmittedIndices(opaqueRanges, opaquePlanes)
            + this.countSubmittedIndices(alphaRanges, alphaPlanes)
        );
    }

    last_draw_hash(): number {
        return 0;
    }

    dispose(): void {
        this.disposed = true;
    }

    private isVisible(rangeIndex: number, planes: Uint8Array): boolean {
        return this.roofPlaneLimit >= 3 || (planes[rangeIndex] ?? 0) <= this.roofPlaneLimit;
    }

    private countDrawCalls(ranges: Uint32Array, planes: Uint8Array): number {
        let calls = 0;
        for (let i = 0; i + 2 < ranges.length; i += 3) {
            const rangeIndex = i / 3;
            if (
                ranges[i + 1] > 0
                && ranges[i + 2] > 0
                && this.isVisible(rangeIndex, planes)
            ) {
                calls++;
            }
        }
        return calls;
    }

    private countSubmittedIndices(ranges: Uint32Array, planes: Uint8Array): number {
        let indices = 0;
        for (let i = 0; i + 2 < ranges.length; i += 3) {
            const rangeIndex = i / 3;
            if (this.isVisible(rangeIndex, planes)) {
                indices += ranges[i + 1] * ranges[i + 2];
            }
        }
        return indices;
    }
}

function emptyGeometryPacket(): RustStaticGeometryPacket {
    return {
        packedVertexWords: new Uint32Array(),
        indices: new Uint32Array(),
        modelInfoOpaque: new Uint16Array(64),
        modelInfoAlpha: new Uint16Array(64),
        modelInfoOpaqueLod: new Uint16Array(64),
        modelInfoAlphaLod: new Uint16Array(64),
        opaqueDrawRanges: new Uint32Array(),
        opaqueDrawRangePlanes: new Uint8Array(),
        alphaDrawRanges: new Uint32Array(),
        alphaDrawRangePlanes: new Uint8Array(),
        opaqueLodDrawRanges: new Uint32Array(),
        opaqueLodDrawRangePlanes: new Uint8Array(),
        alphaLodDrawRanges: new Uint32Array(),
        alphaLodDrawRangePlanes: new Uint8Array(),
    };
}

function packet(): RustStaticScenePacket {
    return {
        abiVersion: RUST_RENDERER_ABI_VERSION,
        mapKey: (50 << 8) | 51,
        mapX: 50,
        mapY: 51,
        borderSize: 1,
        heightMapSize: 2,
        heightMapPlanes: 1,
        packedVertexWords: new Uint32Array([1, 2, 3]),
        indices: new Uint32Array([0, 0, 0, 0, 0, 0]),
        npcPackedVertexWords: new Uint32Array([7, 8, 9]),
        npcIndices: new Uint32Array([0, 0, 0]),
        modelInfoOpaque: new Uint16Array(64),
        modelInfoAlpha: new Uint16Array(64),
        modelInfoOpaqueLod: new Uint16Array(64),
        modelInfoAlphaLod: new Uint16Array(64),
        heightMap: new Int16Array(4),
        waterMask: new Uint8Array(16),
        opaqueDrawRanges: new Uint32Array([0, 3, 1]),
        opaqueDrawRangePlanes: new Uint8Array([0]),
        alphaDrawRanges: new Uint32Array([0, 3, 1]),
        alphaDrawRangePlanes: new Uint8Array([0]),
        opaqueLodDrawRanges: new Uint32Array([0, 6, 1]),
        opaqueLodDrawRangePlanes: new Uint8Array([0]),
        alphaLodDrawRanges: new Uint32Array(),
        alphaLodDrawRangePlanes: new Uint8Array(),
        locGeometry: emptyGeometryPacket(),
        doorGeometry: emptyGeometryPacket(),
    };
}

function frame(): RustStaticFrameState {
    return {
        viewMatrix: new Float32Array(16),
        projectionMatrix: new Float32Array(16),
        worldEntityTransform: new Float32Array([
            1, 0, 0, 0,
            0, 1, 0, 0,
            0, 0, 1, 0,
            0, 0, 0, 1,
        ]),
        worldEntityOpacity: 1,
        skyRgba: new Float32Array([0, 0, 0, 1]),
        sceneHslOverride: new Float32Array([-1, -1, -1, 0]),
        playerPos: new Float32Array([3200, 3200]),
        renderDistance: 50,
        fogDepth: 40,
        currentTime: 1,
        brightness: 0.8,
        roofPlaneLimit: 3,
        useLod: false,
        isNewTextureAnim: false,
        colorBanding: 255,
    };
}

{
    const source = {
        textureArrayPixels: new Uint8Array(128 * 128 * 2 * 4),
        textureMaterialBytes: new Int8Array(2 * 6 * 4),
        waterTexturePixels: new Uint8Array(128 * 128 * 5 * 4),
        textureLayerCount: 2,
        rustGlobalResourcesRevision: 7,
    };
    const snapshot = getRustRendererGlobalResourceSnapshot(source);
    assert.ok(snapshot);
    assert.equal(snapshot.revision, 7);
    assert.equal(snapshot.resources.textureWidth, 128);
    assert.equal(snapshot.resources.textureLayers, 2);
    assert.equal(snapshot.resources.materialCount, 2);
    assert.equal(snapshot.resources.waterLayers, 5);

    const bridge = new RustRendererBridge(
        {} as HTMLCanvasElement,
        MockWasm,
    );
    assert.equal(
        bridge.syncGlobalResources(snapshot.resources, snapshot.revision),
        true,
    );
    const wasm = MockWasm.last!;
    assert.equal(wasm.textureResourceUploads, 1);
    assert.equal(wasm.materialResourceUploads, 1);
    assert.equal(wasm.waterResourceUploads, 1);

    assert.equal(
        bridge.syncGlobalResources(snapshot.resources, snapshot.revision),
        false,
    );
    assert.equal(wasm.textureResourceUploads, 1);
    assert.equal(wasm.materialResourceUploads, 1);
    assert.equal(wasm.waterResourceUploads, 1);

    assert.equal(
        bridge.syncGlobalResources(snapshot.resources, snapshot.revision + 1),
        true,
    );
    assert.equal(wasm.textureResourceUploads, 2);
    assert.equal(wasm.materialResourceUploads, 2);
    assert.equal(wasm.waterResourceUploads, 2);
    bridge.dispose();

    assert.equal(
        getRustRendererGlobalResourceSnapshot({
            textureLayerCount: 0,
            rustGlobalResourcesRevision: 0,
        }),
        undefined,
    );

    assert.throws(
        () =>
            getRustRendererGlobalResourceSnapshot({
                ...source,
                textureMaterialBytes: new Int8Array(1),
            }),
        /material table has 1 bytes/,
    );
}

{
    const bridge = new RustRendererBridge(
        {} as HTMLCanvasElement,
        MockWasm,
    );
    const actorData = new Uint16Array(16 * 1 * 4);
    actorData[0] = 3200;
    actorData[1] = 3210;
    actorData[7] = 127;

    bridge.uploadActorData(actorData, 16, 1);
    const wasm = MockWasm.last!;
    assert.equal(wasm.actorDataUploads.length, 1);
    assert.equal(wasm.actorDataUploads[0].width, 16);
    assert.equal(wasm.actorDataUploads[0].height, 1);
    assert.deepEqual(
        Array.from(wasm.actorDataUploads[0].values),
        Array.from(actorData),
    );

    actorData[0] = 9999;
    assert.equal(wasm.actorDataUploads[0].values[0], 3200);

    assert.throws(
        () => bridge.uploadActorData(new Uint16Array(3), 16, 1),
        /Actor-data packet has 3 u16 values; expected 64/,
    );
    assert.throws(
        () => bridge.uploadActorData(new Uint16Array(), 0, 1),
        /Invalid actor-data texture dimensions/,
    );
    bridge.dispose();
}

{
    const bridge = new RustRendererBridge(
        {} as HTMLCanvasElement,
        MockWasm,
    );
    bridge.uploadStaticScene(packet(), 3.25);
    bridge.renderStatic(frame());

    const wasm = MockWasm.last!;
    assert.equal(wasm.selectedMapKey, (50 << 8) | 51);
    assert.equal(wasm.resident_static_map_count(), 1);
    assert.equal(wasm.geometryUploads, 1);
    assert.equal(wasm.npcGeometryUploads.length, 1);
    assert.deepEqual(
        Array.from(wasm.npcGeometryUploads[0].vertices),
        [7, 8, 9],
    );
    assert.deepEqual(
        Array.from(wasm.npcGeometryUploads[0].indices),
        [0, 0, 0],
    );
    assert.equal(wasm.heightUploads, 1);
    assert.equal(wasm.waterMaskUploads, 1);
    assert.equal(wasm.staticStateCalls, 1);
    assert.equal(wasm.staticPassUploads, 1);
    assert.equal(wasm.staticLodPassUploads, 1);
    assert.deepEqual(wasm.auxGeometryUploads, [0, 1]);
    assert.deepEqual(wasm.auxPassUploads, []);
    assert.deepEqual(wasm.auxLodPassUploads, []);
    assert.equal(wasm.modelInfoUploads.length, 0);
    assert.equal(wasm.drawRangeUploads.length, 0);
    assert.equal(wasm.renderFrameCalls, 1);
    assert.equal(wasm.lastWorldEntityOpacity, 1);
    assert.equal(wasm.lastWorldEntityTransform?.[0], 1);
    assert.equal(wasm.lastWorldEntityTransform?.[15], 1);
    assert.deepEqual(bridge.getLastStats(), {
        drawCalls: 2,
        submittedIndices: 6,
    });
    assert.equal(bridge.getLastDrawHash(), 0);

    bridge.renderStatic(frame());
    assert.equal(wasm.staticPassUploads, 1);
    assert.equal(wasm.staticLodPassUploads, 1);
    assert.equal(wasm.renderFrameCalls, 2);

    bridge.dispose();
    assert.equal(wasm.disposed, true);
}

{
    const noAlpha = packet();
    noAlpha.alphaDrawRanges = new Uint32Array();
    noAlpha.alphaDrawRangePlanes = new Uint8Array();

    const bridge = new RustRendererBridge(
        {} as HTMLCanvasElement,
        MockWasm,
    );
    bridge.uploadStaticScene(noAlpha, 4.5);
    bridge.renderStatic(frame());

    assert.deepEqual(bridge.getLastStats(), {
        drawCalls: 1,
        submittedIndices: 3,
    });
    bridge.dispose();
}

{
    const bridge = new RustRendererBridge(
        {} as HTMLCanvasElement,
        MockWasm,
    );
    bridge.uploadStaticScene(packet(), 5.0);
    const worldFrame = frame();
    worldFrame.worldEntityOpacity = 0.25;
    worldFrame.worldEntityTransform[12] = 12;
    worldFrame.worldEntityTransform[13] = -3;
    bridge.renderStatic(worldFrame);

    const wasm = MockWasm.last!;
    assert.equal(wasm.lastWorldEntityOpacity, 0.25);
    assert.equal(wasm.lastWorldEntityTransform?.[12], 12);
    assert.equal(wasm.lastWorldEntityTransform?.[13], -3);
    bridge.dispose();
}

{
    const roofFiltered = packet();
    roofFiltered.alphaDrawRangePlanes = new Uint8Array([2]);

    const bridge = new RustRendererBridge(
        {} as HTMLCanvasElement,
        MockWasm,
    );
    bridge.uploadStaticScene(roofFiltered, 5.5);
    const roofFrame = frame();
    roofFrame.roofPlaneLimit = 0;
    bridge.renderStatic(roofFrame);

    assert.deepEqual(bridge.getLastStats(), {
        drawCalls: 1,
        submittedIndices: 3,
    });
    bridge.dispose();
}

{
    const bridge = new RustRendererBridge(
        {} as HTMLCanvasElement,
        MockWasm,
    );
    bridge.uploadStaticScene(packet(), 6.0);
    const lodFrame = frame();
    lodFrame.useLod = true;
    bridge.renderStatic(lodFrame);

    assert.deepEqual(bridge.getLastStats(), {
        drawCalls: 1,
        submittedIndices: 6,
    });
    bridge.dispose();
}

{
    const withLoc = packet();
    withLoc.locGeometry = {
        packedVertexWords: new Uint32Array([1, 2, 3]),
        indices: new Uint32Array([0, 0, 0]),
        modelInfoOpaque: new Uint16Array(64),
        modelInfoAlpha: new Uint16Array(64),
        modelInfoOpaqueLod: new Uint16Array(64),
        modelInfoAlphaLod: new Uint16Array(64),
        opaqueDrawRanges: new Uint32Array([0, 3, 1]),
        opaqueDrawRangePlanes: new Uint8Array([0]),
        alphaDrawRanges: new Uint32Array(),
        alphaDrawRangePlanes: new Uint8Array(),
        opaqueLodDrawRanges: new Uint32Array([0, 3, 1]),
        opaqueLodDrawRangePlanes: new Uint8Array([0]),
        alphaLodDrawRanges: new Uint32Array(),
        alphaLodDrawRangePlanes: new Uint8Array(),
    };

    const bridge = new RustRendererBridge(
        {} as HTMLCanvasElement,
        MockWasm,
    );
    bridge.uploadStaticScene(withLoc, 7.0);

    const wasm = MockWasm.last!;
    assert.deepEqual(wasm.auxGeometryUploads, [0, 1]);
    assert.deepEqual(wasm.auxPassUploads, [0]);
    assert.deepEqual(wasm.auxLodPassUploads, [0]);
    bridge.dispose();
}

{
    const bridge = new RustRendererBridge(
        {} as HTMLCanvasElement,
        MockWasm,
    );
    const first = packet();
    first.mapKey = 1001;
    const second = packet();
    second.mapKey = 1002;

    bridge.uploadStaticScene(first, 1.0);
    bridge.uploadStaticScene(second, 2.0);

    assert.equal(bridge.getResidentStaticMapCount(), 2);
    assert.equal(MockWasm.last?.active_static_map_key(), 1002);

    bridge.removeStaticMap(1001);
    assert.equal(bridge.getResidentStaticMapCount(), 1);

    bridge.clearStaticMaps();
    assert.equal(bridge.getResidentStaticMapCount(), 0);
    bridge.dispose();
}

{
    const bridge = new RustRendererBridge(
        {} as HTMLCanvasElement,
        MockWasm,
    );
    const first = packet();
    first.mapKey = 2001;
    const second = packet();
    second.mapKey = 2002;
    bridge.uploadStaticScene(first, 1.0);
    bridge.uploadStaticScene(second, 2.0);

    const firstFrame: RustResidentMapFrameState = {
        ...frame(),
        mapKey: 2001,
        worldEntityGhostSceneHslOverride: new Float32Array([
            12, 3, 64, 127,
        ]),
    };
    const secondFrame: RustResidentMapFrameState = {
        ...frame(),
        mapKey: 2002,
        useLod: true,
    };
    bridge.renderStaticMaps([firstFrame, secondFrame]);

    const wasm = MockWasm.last!;
    assert.equal(wasm.beginFrameCalls, 1);
    assert.deepEqual(wasm.mapPassCalls, [
        { mapKey: 2001, transparent: false },
        { mapKey: 2002, transparent: false },
        { mapKey: 2002, transparent: true },
        { mapKey: 2001, transparent: true },
    ]);
    assert.deepEqual(wasm.ghostPassCalls, [
        {
            mapKey: 2001,
            opacity: 0.01,
            hsl: [12, 3, 64, 127],
        },
    ]);
    assert.deepEqual(wasm.passSequence, [
        { mapKey: 2001, pass: "opaque" },
        { mapKey: 2001, pass: "ghost" },
        { mapKey: 2002, pass: "opaque" },
        { mapKey: 2002, pass: "transparent" },
        { mapKey: 2001, pass: "transparent" },
    ]);

    wasm.passSequence.length = 0;
    wasm.mapPassCalls.length = 0;
    wasm.ghostPassCalls.length = 0;
    const beginCallsBeforeSplit = wasm.beginFrameCalls;

    assert.equal(
        bridge.beginStaticFrame([firstFrame, secondFrame]),
        true,
    );
    assert.equal(wasm.beginFrameCalls, beginCallsBeforeSplit + 1);
    assert.deepEqual(wasm.passSequence, []);

    bridge.renderOpaqueStaticMaps([firstFrame, secondFrame]);
    assert.deepEqual(wasm.passSequence, [
        { mapKey: 2001, pass: "opaque" },
        { mapKey: 2001, pass: "ghost" },
        { mapKey: 2002, pass: "opaque" },
    ]);

    bridge.renderTransparentStaticMaps([firstFrame, secondFrame]);
    assert.deepEqual(wasm.passSequence, [
        { mapKey: 2001, pass: "opaque" },
        { mapKey: 2001, pass: "ghost" },
        { mapKey: 2002, pass: "opaque" },
        { mapKey: 2002, pass: "transparent" },
        { mapKey: 2001, pass: "transparent" },
    ]);
    assert.equal(bridge.beginStaticFrame([]), false);

    const npcTransform = new Float32Array([
        1, 0, 0, 0,
        0, 1, 0, 0,
        0, 0, 1, 0,
        2, 3, 4, 1,
    ]);
    bridge.renderNpcPass({
        ...firstFrame,
        drawRanges: new Uint32Array([
            0, 3, 1,
            12, 6, 1,
        ]),
        npcDataOffset: 24,
        modelYOffset: 0.5,
        transparent: false,
        worldEntityTransform: npcTransform,
    });
    assert.equal(wasm.npcPassCalls.length, 1);
    assert.equal(wasm.npcPassCalls[0].mapKey, 2001);
    assert.deepEqual(
        Array.from(wasm.npcPassCalls[0].ranges),
        [0, 3, 1, 12, 6, 1],
    );
    assert.equal(wasm.npcPassCalls[0].npcDataOffset, 24);
    assert.equal(wasm.npcPassCalls[0].modelYOffset, 0.5);
    assert.equal(wasm.npcPassCalls[0].transparent, false);
    assert.deepEqual(
        Array.from(wasm.npcPassCalls[0].worldEntityTransform),
        Array.from(npcTransform),
    );
    assert.throws(
        () => bridge.renderNpcPass({
            ...firstFrame,
            drawRanges: new Uint32Array([0, 3]),
            npcDataOffset: 0,
            modelYOffset: 0,
            transparent: false,
        }),
        /offset\/element\/instance triples/,
    );

    const dynamicVertices = new Uint32Array([11, 12, 13]);
    const dynamicIndices = new Uint32Array([0, 0, 0]);
    bridge.renderDynamicNpcPass(
        {
            ...firstFrame,
            npcDataOffset: 31,
            modelYOffset: 1.25,
            transparent: true,
            worldEntityTransform: npcTransform,
        },
        dynamicVertices,
        dynamicIndices,
    );
    assert.equal(wasm.dynamicNpcGeometryUploads.length, 1);
    assert.deepEqual(
        Array.from(wasm.dynamicNpcGeometryUploads[0].vertices),
        [11, 12, 13],
    );
    assert.deepEqual(
        Array.from(wasm.dynamicNpcGeometryUploads[0].indices),
        [0, 0, 0],
    );
    assert.equal(wasm.dynamicNpcPassCalls.length, 1);
    assert.deepEqual(
        wasm.dynamicNpcPassCalls[0],
        {
            mapKey: 2001,
            npcDataOffset: 31,
            modelYOffset: 1.25,
            transparent: true,
            worldEntityTransform: npcTransform,
        },
    );

    const gfxVertices = new Uint32Array([15, 16, 17]);
    const gfxIndices = new Uint32Array([0, 0, 0]);
    bridge.renderGfxPass(
        {
            ...firstFrame,
            actorDataOffset: 44,
            modelYOffset: -96,
            mapX: 50,
            mapY: 51,
            transparent: true,
        },
        gfxVertices,
        gfxIndices,
    );
    assert.equal(wasm.dynamicGfxGeometryUploads.length, 1);
    assert.deepEqual(
        Array.from(wasm.dynamicGfxGeometryUploads[0].vertices),
        [15, 16, 17],
    );
    assert.deepEqual(
        wasm.gfxPassCalls[0],
        {
            mapKey: 2001,
            actorDataOffset: 44,
            modelYOffset: -96,
            mapX: 50,
            mapY: 51,
            transparent: true,
        },
    );

    const projectileVertices = new Uint32Array([18, 19, 20]);
    const projectileIndices = new Uint32Array([0, 0, 0]);
    bridge.renderProjectilePass(
        {
            ...firstFrame,
            projectileDataOffset: 52,
            modelYOffset: -144,
            projectileSubOffset: new Float32Array([0.25, 0.75]),
            mapX: 60,
            mapY: 61,
            transparent: true,
            cullBackFace: false,
        },
        projectileVertices,
        projectileIndices,
    );
    assert.equal(wasm.dynamicProjectileGeometryUploads.length, 1);
    assert.deepEqual(
        Array.from(wasm.dynamicProjectileGeometryUploads[0].vertices),
        [18, 19, 20],
    );
    assert.deepEqual(
        wasm.projectilePassCalls[0],
        {
            mapKey: 2001,
            projectileDataOffset: 52,
            modelYOffset: -144,
            projectileSubOffset: new Float32Array([0.25, 0.75]),
            mapX: 60,
            mapY: 61,
            transparent: true,
            cullBackFace: false,
        },
    );

    const playerVertices = new Uint32Array([21, 22, 23]);
    const playerIndices = new Uint32Array([0, 0, 0]);
    bridge.renderDynamicPlayerPass(
        {
            ...firstFrame,
            playerDataOffset: 9,
            playerSlots: new Int32Array(),
            modelYOffset: 2.5,
            transparent: false,
            cullBackFace: true,
            worldEntityTransform: npcTransform,
        },
        playerVertices,
        playerIndices,
    );
    assert.equal(wasm.dynamicPlayerGeometryUploads.length, 1);
    assert.deepEqual(
        Array.from(wasm.dynamicPlayerGeometryUploads[0].vertices),
        [21, 22, 23],
    );
    assert.deepEqual(
        Array.from(wasm.dynamicPlayerGeometryUploads[0].indices),
        [0, 0, 0],
    );
    assert.equal(wasm.playerPassCalls.length, 1);
    assert.deepEqual(
        wasm.playerPassCalls[0],
        {
            mapKey: 2001,
            playerDataOffset: 9,
            playerSlots: new Int32Array(),
            modelYOffset: 2.5,
            transparent: false,
            cullBackFace: true,
            worldEntityTransform: npcTransform,
        },
    );

    bridge.renderDynamicPlayerPass(
        {
            ...firstFrame,
            playerDataOffset: 40,
            playerSlots: new Int32Array([1, 4, 7]),
            modelYOffset: 0.75,
            transparent: true,
            cullBackFace: false,
            worldEntityTransform: npcTransform,
        },
        playerVertices,
        playerIndices,
    );
    assert.equal(wasm.playerPassCalls.length, 2);
    assert.deepEqual(
        wasm.playerPassCalls[1],
        {
            mapKey: 2001,
            playerDataOffset: 40,
            playerSlots: new Int32Array([1, 4, 7]),
            modelYOffset: 0.75,
            transparent: true,
            cullBackFace: false,
            worldEntityTransform: npcTransform,
        },
    );

    bridge.dispose();
}

{
    const bridge = new RustRendererBridge(
        {} as HTMLCanvasElement,
        MockWasm,
    );
    const scene = packet();
    scene.mapKey = 3001;
    bridge.uploadStaticScene(scene, 1.0);

    const patches = new Uint32Array([2, 48, 6, 1]);
    assert.equal(
        bridge.patchLocDrawRanges(3001, true, false, patches),
        true,
    );
    assert.deepEqual(MockWasm.last?.auxRangePatches, [
        {
            kind: 0,
            lod: true,
            alpha: false,
            patches,
        },
    ]);

    assert.equal(
        bridge.patchLocDrawRanges(
            9999,
            false,
            false,
            new Uint32Array([0, 0, 3, 1]),
        ),
        false,
    );
    bridge.dispose();
}

{
    const bridge = new RustRendererBridge(
        {} as HTMLCanvasElement,
        MockWasm,
    );
    const scene = packet();
    scene.mapKey = 4001;
    bridge.uploadStaticScene(scene, 1.0);

    const ground = emptyGeometryPacket();
    ground.packedVertexWords = new Uint32Array([1, 2, 3]);
    ground.indices = new Uint32Array([0, 0, 0]);
    ground.opaqueDrawRanges = new Uint32Array([0, 3, 1]);
    ground.opaqueDrawRangePlanes = new Uint8Array([0]);
    ground.opaqueLodDrawRanges = new Uint32Array([0, 3, 1]);
    ground.opaqueLodDrawRangePlanes = new Uint8Array([0]);

    assert.equal(bridge.updateGroundGeometry(4001, ground), true);
    const wasm = MockWasm.last!;
    assert.deepEqual(wasm.auxGeometryUploads, [0, 1, 2]);
    assert.deepEqual(wasm.auxPassUploads, [2]);
    assert.deepEqual(wasm.auxLodPassUploads, [2]);

    assert.equal(bridge.updateGroundGeometry(4001), true);
    assert.deepEqual(wasm.auxGeometryUploads, [0, 1, 2, 2]);
    bridge.dispose();
}

{
    MockWasm.abiVersion = RUST_RENDERER_ABI_VERSION + 1;
    assert.throws(
        () => new RustRendererBridge({} as HTMLCanvasElement, MockWasm),
        /ABI mismatch/,
    );
    assert.equal(MockWasm.last?.disposed, true);
    MockWasm.abiVersion = RUST_RENDERER_ABI_VERSION;
}

console.log("rust renderer bridge tests passed");
