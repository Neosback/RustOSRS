import assert from "node:assert/strict";

import { ModelData } from "../rs/model/ModelData";
import {
    registerRustModelFaceLighter,
    registerRustModelNormalCalculator,
} from "../rs/model/RustModelLighting";
import { VertexNormal } from "../rs/model/VertexNormal";

const data = new ModelData();
data.verticesCount = 3;
data.usedVertexCount = 3;
data.faceCount = 1;
data.textureFaceCount = 0;
data.verticesX = new Int32Array([0, 128, 0]);
data.verticesY = new Int32Array([0, 0, 0]);
data.verticesZ = new Int32Array([0, 0, 128]);
data.contourVerticesY = new Int32Array([10, 20, 30]);
data.indices1 = new Int32Array([0]);
data.indices2 = new Int32Array([1]);
data.indices3 = new Int32Array([2]);
data.faceColors = new Uint16Array([0x1234]);
data.faceRenderPriorities = new Int8Array([0]);
data.faceAlphas = new Int8Array([0]);
data.priority = 0;

let normalCalls = 0;
registerRustModelNormalCalculator(
    (
        verticesX,
        verticesY,
        verticesZ,
        usedVertexCount,
        indices1,
        indices2,
        indices3,
        faceRenderTypes,
    ) => {
        normalCalls++;
        assert.deepEqual(Array.from(verticesX), [0, 128, 0]);
        assert.deepEqual(Array.from(verticesY), [10, 20, 30]);
        assert.deepEqual(Array.from(verticesZ), [0, 0, 128]);
        assert.equal(usedVertexCount, 3);
        assert.deepEqual(Array.from(indices1), [0]);
        assert.deepEqual(Array.from(indices2), [1]);
        assert.deepEqual(Array.from(indices3), [2]);
        assert.equal(faceRenderTypes.length, 0);
        return new Int32Array([
            3, 1,
            11, 12, 13, 1,
            21, 22, 23, 2,
            31, 32, 33, 3,
            1, 41, 42, 43,
        ]);
    },
);

data.calculateVertexNormals();
assert.equal(normalCalls, 1);
assert(data.normals);
assert.equal(data.normals.length, 3);
assert.deepEqual(
    data.normals.map((normal) => [
        normal.x,
        normal.y,
        normal.z,
        normal.magnitude,
    ]),
    [
        [11, 12, 13, 1],
        [21, 22, 23, 2],
        [31, 32, 33, 3],
    ],
);
assert(data.faceNormals);
assert.deepEqual(
    [
        data.faceNormals[0].x,
        data.faceNormals[0].y,
        data.faceNormals[0].z,
    ],
    [41, 42, 43],
);

data.mergedNormals = new Array(3);
const merged = new VertexNormal();
merged.x = 101;
merged.y = 102;
merged.z = 103;
merged.magnitude = 4;
data.mergedNormals[1] = merged;

let lightingCalls = 0;
registerRustModelFaceLighter(
    (
        indices1,
        indices2,
        indices3,
        faceColors,
        faceRenderTypes,
        faceAlphas,
        faceTextures,
        vertexNormals,
        mergedNormals,
        faceNormals,
        ambient,
        contrast,
        lightX,
        lightY,
        lightZ,
    ) => {
        lightingCalls++;
        assert.deepEqual(Array.from(indices1), [0]);
        assert.deepEqual(Array.from(indices2), [1]);
        assert.deepEqual(Array.from(indices3), [2]);
        assert.deepEqual(Array.from(faceColors), [0x1234]);
        assert.equal(faceRenderTypes.length, 0);
        assert.deepEqual(Array.from(faceAlphas), [0]);
        assert.equal(faceTextures.length, 0);
        assert.deepEqual(
            Array.from(vertexNormals),
            [
                11, 12, 13, 1,
                21, 22, 23, 2,
                31, 32, 33, 3,
            ],
        );
        assert.deepEqual(
            Array.from(mergedNormals),
            [
                0, 0, 0, 0, 0,
                1, 101, 102, 103, 4,
                0, 0, 0, 0, 0,
            ],
        );
        assert.deepEqual(Array.from(faceNormals), [1, 41, 42, 43]);
        assert.equal(ambient, 64);
        assert.equal(contrast, 768);
        assert.equal(lightX, -50);
        assert.equal(lightY, -10);
        assert.equal(lightZ, -50);
        return new Int32Array([0x111111, 0x222222, -1]);
    },
);

const textureLoader = {
    isSd: () => true,
};

const lit = data.light(
    textureLoader as any,
    64,
    768,
    -50,
    -10,
    -50,
);
assert.equal(lightingCalls, 1);
assert.deepEqual(Array.from(lit.faceColors1), [0x111111]);
assert.deepEqual(Array.from(lit.faceColors2), [0x222222]);
assert.deepEqual(Array.from(lit.faceColors3), [-1]);
assert.equal(lit.verticesX, data.verticesX);
assert.equal(lit.verticesY, data.verticesY);
assert.equal(lit.verticesZ, data.verticesZ);

registerRustModelNormalCalculator(undefined);
registerRustModelFaceLighter(undefined);

console.log("Rust ModelData lighting integration tests passed");
