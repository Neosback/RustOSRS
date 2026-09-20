import assert from "node:assert/strict";

import {
    RustRendererBridge,
    RustRendererWasm,
    RustStaticFrameState,
} from "../render/rust/RustRendererBridge";
import {
    RUST_RENDERER_ABI_VERSION,
    RustStaticScenePacket,
} from "../render/rust/RendererPacket";

type RenderCall = {
    discardAlpha: boolean;
    clearFrame: boolean;
};

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
    renderCalls: RenderCall[] = [];

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

    upload_texture_array(
        _pixels: Uint8Array,
        _width: number,
        _height: number,
        _layers: number,
    ): void {}

    upload_materials(
        _materials: Int8Array,
        _textureCount: number,
    ): void {}

    upload_water_textures(
        _pixels: Uint8Array,
        _width: number,
        _height: number,
        _layers: number,
    ): void {}

    render_static(
        _viewMatrix: Float32Array,
        _projectionMatrix: Float32Array,
        _skyRgba: Float32Array,
        _sceneHslOverride: Float32Array,
        _playerPos: Float32Array,
        _renderDistance: number,
        _fogDepth: number,
        _currentTime: number,
        _brightness: number,
        _roofPlaneLimit: number,
        _isNewTextureAnim: boolean,
        _colorBanding: number,
        discardAlpha: boolean,
        clearFrame: boolean,
    ): void {
        this.renderCalls.push({ discardAlpha, clearFrame });
    }

    last_draw_calls(): number {
        return this.renderCalls.length;
    }

    last_submitted_indices(): number {
        return this.renderCalls.length * 3;
    }

    dispose(): void {
        this.disposed = true;
    }
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
        indices: new Uint32Array([0]),
        modelInfoOpaque: new Uint16Array(64),
        modelInfoAlpha: new Uint16Array(64),
        heightMap: new Int16Array(4),
        waterMask: new Uint8Array(16),
        opaqueDrawRanges: new Uint32Array([0, 1, 1]),
        opaqueDrawRangePlanes: new Uint8Array([0]),
        alphaDrawRanges: new Uint32Array([0, 1, 1]),
        alphaDrawRangePlanes: new Uint8Array([0]),
    };
}

function frame(): RustStaticFrameState {
    return {
        viewMatrix: new Float32Array(16),
        projectionMatrix: new Float32Array(16),
        skyRgba: new Float32Array([0, 0, 0, 1]),
        sceneHslOverride: new Float32Array([-1, -1, -1, 0]),
        playerPos: new Float32Array([3200, 3200]),
        renderDistance: 50,
        fogDepth: 40,
        currentTime: 1,
        brightness: 0.8,
        roofPlaneLimit: 3,
        isNewTextureAnim: false,
        colorBanding: 255,
    };
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
    assert.equal(wasm.modelInfoUploads.length, 2);
    assert.equal(wasm.drawRangeUploads.length, 2);
    assert.deepEqual(wasm.renderCalls, [
        { discardAlpha: false, clearFrame: true },
        { discardAlpha: true, clearFrame: false },
    ]);
    assert.deepEqual(bridge.getLastStats(), {
        drawCalls: 2,
        submittedIndices: 6,
    });

    bridge.dispose();
    assert.equal(wasm.disposed, true);
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
