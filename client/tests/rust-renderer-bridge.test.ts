import assert from "node:assert/strict";

import {
    RustRendererBridge,
    RustRendererWasm,
    RustStaticFrameState,
} from "../render/rust/RustRendererBridge";
import {
    RUST_RENDERER_ABI_VERSION,
    RustStaticGeometryPacket,
    RustStaticScenePacket,
} from "../render/rust/RendererPacket";
import { getRustRendererGlobalResourceSnapshot } from "../render/rust/LiveResourceAdapter";

class MockWasm implements RustRendererWasm {
    static abiVersion = RUST_RENDERER_ABI_VERSION;
    static last?: MockWasm;

    disposed = false;
    geometryUploads = 0;
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
    textureResourceUploads = 0;
    materialResourceUploads = 0;
    waterResourceUploads = 0;
    renderFrameCalls = 0;
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

    upload_geometry(_vertices: Uint32Array, _indices: Uint32Array): void {
        this.geometryUploads++;
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
        mapX: 50,
        mapY: 51,
        borderSize: 1,
        heightMapSize: 2,
        heightMapPlanes: 1,
        packedVertexWords: new Uint32Array([1, 2, 3]),
        indices: new Uint32Array([0, 0, 0, 0, 0, 0]),
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
    bridge.uploadStaticScene(packet(), 3.25);
    bridge.renderStatic(frame());

    const wasm = MockWasm.last!;
    assert.equal(wasm.geometryUploads, 1);
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
    MockWasm.abiVersion = RUST_RENDERER_ABI_VERSION + 1;
    assert.throws(
        () => new RustRendererBridge({} as HTMLCanvasElement, MockWasm),
        /ABI mismatch/,
    );
    assert.equal(MockWasm.last?.disposed, true);
    MockWasm.abiVersion = RUST_RENDERER_ABI_VERSION;
}

console.log("rust renderer bridge tests passed");
