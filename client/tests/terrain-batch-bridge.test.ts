import assert from "node:assert/strict";

import { SceneBuffer } from "../render/buffer/SceneBuffer";
import type { VertexBatchBuilder } from "../render/buffer/VertexBuffer";

let captured: any[] | undefined;
let vertexCount = 6;

const builder: VertexBatchBuilder = {
    clear(): void {},
    vertex_count(): number {
        return vertexCount;
    },
    push_batch(): Uint32Array {
        throw new Error("unexpected push_batch");
    },
    push_terrain_batch(...args: any[]): Uint32Array {
        captured = args;
        return new Uint32Array([0, 1, 2, 3, 4, 5]);
    },
    packed_vertices(): Uint32Array {
        return new Uint32Array(vertexCount * 3);
    },
};

const sceneBuffer = new SceneBuffer({} as any, new Map(), 16, builder);

const tileA = {
    tileModel: {
        vertexX: new Int32Array([0, 128, 0]),
        vertexY: new Int32Array([0, 0, 0]),
        vertexZ: new Int32Array([0, 0, 128]),
        facesA: new Int32Array([0]),
        facesB: new Int32Array([1]),
        facesC: new Int32Array([2]),
        faceColorsA: new Int32Array([0x1111]),
        faceColorsB: new Int32Array([0x2222]),
        faceColorsC: new Int32Array([0x3333]),
        faceTextures: undefined,
    },
};

const tileB = {
    tileModel: {
        vertexX: new Int32Array([128, 256, 128]),
        vertexY: new Int32Array([0, 0, 0]),
        vertexZ: new Int32Array([0, 0, 128]),
        facesA: new Int32Array([0]),
        facesB: new Int32Array([1]),
        facesC: new Int32Array([2]),
        faceColorsA: new Int32Array([0x4444]),
        faceColorsB: new Int32Array([0x5555]),
        faceColorsC: new Int32Array([0x6666]),
        faceTextures: new Int32Array([7]),
    },
};

(sceneBuffer as any).addTerrainTilesBatch([tileA, tileB], -128, -256);

assert.ok(captured, "terrain batch builder was not called");
const [
    tileVertexOffsets,
    tileFaceOffsets,
    verticesX,
    verticesY,
    verticesZ,
    facesA,
    facesB,
    facesC,
    colorsA,
    colorsB,
    colorsC,
    textureIds,
    tileX,
    tileZ,
    offsetX,
    offsetZ,
] = captured!;

assert.deepEqual(Array.from(tileVertexOffsets), [0, 3, 6]);
assert.deepEqual(Array.from(tileFaceOffsets), [0, 1, 2]);
assert.deepEqual(Array.from(verticesX), [0, 128, 0, 128, 256, 128]);
assert.deepEqual(Array.from(verticesY), [0, 0, 0, 0, 0, 0]);
assert.deepEqual(Array.from(verticesZ), [0, 0, 128, 0, 0, 128]);
assert.deepEqual(Array.from(facesA), [0, 0]);
assert.deepEqual(Array.from(facesB), [1, 1]);
assert.deepEqual(Array.from(facesC), [2, 2]);
assert.deepEqual(Array.from(colorsA), [0x1111, 0x4444]);
assert.deepEqual(Array.from(colorsB), [0x2222, 0x5555]);
assert.deepEqual(Array.from(colorsC), [0x3333, 0x6666]);
assert.deepEqual(Array.from(textureIds), [-1, 7]);
assert.deepEqual(Array.from(tileX), [0, 128]);
assert.deepEqual(Array.from(tileZ), [0, 0]);
assert.equal(offsetX, -128);
assert.equal(offsetZ, -256);
assert.deepEqual(sceneBuffer.indices, [0, 1, 2, 3, 4, 5]);
assert.equal(sceneBuffer.vertexCount(), vertexCount);


// The non-Rust fallback must still route through the legacy tile packer.
// This guards against accidentally recursing through emitTerrainTile when
// push_terrain_batch is unavailable.
const fallbackSceneBuffer = new SceneBuffer({} as any, new Map(), 16);
const fallbackTile = {
    tileModel: {
        faces: [
            {
                vertices: [
                    { x: 0, y: 0, z: 0, hsl: 0x1111, textureId: -1, u: 0, v: 0 },
                    { x: 128, y: 0, z: 0, hsl: 0x2222, textureId: -1, u: 0, v: 0 },
                    { x: 0, y: 0, z: 128, hsl: 0x3333, textureId: -1, u: 0, v: 0 },
                ],
            },
        ],
    },
    skipRender: false,
    isBridgeSurface: false,
} as any;
const fallbackScene = {
    levels: 1,
    tileRenderFlags: [
        [new Uint8Array(1)],
        [new Uint8Array(1)],
    ],
    tiles: [[[fallbackTile]]],
    isPlayerLevel(): boolean {
        return true;
    },
} as any;

const fallbackVertexCount = fallbackSceneBuffer.addTerrain(
    fallbackScene,
    0,
    0,
    1,
    0,
);
assert.equal(fallbackVertexCount, 3);
assert.deepEqual(fallbackSceneBuffer.indices, [0, 1, 2]);

console.log("Terrain batch bridge regression test passed");
