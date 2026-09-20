import assert from "node:assert/strict";
Object.assign(globalThis, { self: globalThis });
const { WebGLMapSquare } = require("../render/WebGLMapSquare");

const heights = new Int16Array([32, 32, 32, 32]);
const vertices = new Float32Array([0, -2, 0]);
const uploaded: unknown[] = [];
const stop = new Error("stop before unrelated geometry allocation");
const map = {
    mapX: 50,
    mapY: 50,
    heightMapData: new Int16Array([10, 20, 30, 40]),
    heightMapTexture: { data: (data: unknown) => uploaded.push(data) },
    waterMaskTexture: { data: (data: unknown) => uploaded.push(data) },
    get collisionMaps(): never { throw stop; },
} as any;
const data = {
    mapX: 50, mapY: 50,
    heightMapTextureData: heights,
    waterMaskTextureData: new Uint8Array(16),
    terrainPickVertices: vertices,
    terrainPickTileOffsets: new Uint32Array([0, 1]),
    terrainPickPlanes: new Uint8Array([0]),
};
assert.throws(() => WebGLMapSquare.prototype.refreshSceneGeometry.call(
    map, ...([...Array(9).fill(undefined), data, 0] as any),
), (error) => error === stop);
assert.deepEqual(map.heightMapData, heights);
assert.equal(uploaded[0], heights);
assert.equal(map.terrainPickVertices, vertices);
console.log("Scene refresh updates GPU and picking heights");
