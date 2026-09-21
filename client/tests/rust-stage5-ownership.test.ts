import {
    calculateModelNormalsWithRustIfReady,
    lightModelFacesWithRustIfReady,
    registerRustModelFaceLighter,
    registerRustModelNormalCalculator,
} from "../rs/model/RustModelLighting";
import assert from "node:assert/strict";

import {
    applyLegacyTransformsWithRustIfReady,
    contourVerticesWithRustIfReady,
    mirrorModelGeometryWithRustIfReady,
    registerRustBasicVertexTransformer,
    registerRustContourBuilder,
    registerRustLegacyTransformer,
    registerRustMirrorModelGeometry,
    registerRustSkeletalSkinner,
    RustBasicTransformMode,
    skinSkeletalVerticesWithRustIfReady,
    transformVerticesWithRustIfReady,
} from "../rs/model/RustModelTransforms";
import {
    getRustStage5OwnershipStats,
    resetRustStage5OwnershipStats,
    setRustStage5StrictMode,
} from "../rs/model/RustStage5Ownership";
import {
    computeTextureCoordsWithRustIfReady,
    registerRustTextureMapper,
} from "../rs/model/RustTextureMapper";

resetRustStage5OwnershipStats();
setRustStage5StrictMode(false);

registerRustBasicVertexTransformer((x, y, z, _mode, a, b, c) => {
    return new Int32Array([x[0] + a, y[0] + b, z[0] + c]);
});
const basicX = new Int32Array([10]);
const basicY = new Int32Array([20]);
const basicZ = new Int32Array([30]);
assert.equal(
    transformVerticesWithRustIfReady(
        basicX,
        basicY,
        basicZ,
        1,
        RustBasicTransformMode.TRANSLATE,
        5,
        -3,
        7,
    ),
    true,
);
assert.deepEqual(Array.from(basicX), [15]);
assert.deepEqual(Array.from(basicY), [17]);
assert.deepEqual(Array.from(basicZ), [37]);

registerRustSkeletalSkinner(() => new Int32Array([11, 22, 33]));
assert.deepEqual(
    Array.from(
        skinSkeletalVerticesWithRustIfReady(
            new Int32Array([1]),
            new Int32Array([2]),
            new Int32Array([3]),
            1,
            [new Int32Array([0])],
            [new Int32Array([255])],
            new Float32Array(16),
        )!,
    ),
    [11, 22, 33],
);

registerRustLegacyTransformer(() =>
    new Int32Array([
        4, 5, 6, 1,
        1, 1, 1,
        10, 20, 30,
        8,
        0x1234,
    ]),
);
const legacyX = new Int32Array([1]);
const legacyY = new Int32Array([2]);
const legacyZ = new Int32Array([3]);
const legacyAlpha = new Int8Array([0]);
const legacyColor = new Uint16Array([0]);
const legacy = applyLegacyTransformsWithRustIfReady(
    legacyX,
    legacyY,
    legacyZ,
    1,
    legacyAlpha,
    legacyColor,
    [new Int32Array([0])],
    [new Int32Array([0])],
    [{ type: 1, labels: [0], x: 1, y: 2, z: 3 }],
    0,
    0,
    0,
);
assert.deepEqual(legacy, {
    originX: 4,
    originY: 5,
    originZ: 6,
    changedLight: true,
});
assert.deepEqual(Array.from(legacyX), [10]);
assert.deepEqual(Array.from(legacyY), [20]);
assert.deepEqual(Array.from(legacyZ), [30]);
assert.deepEqual(Array.from(legacyAlpha), [8]);
assert.deepEqual(Array.from(legacyColor), [0x1234]);

registerRustContourBuilder((_x, y) => new Int32Array(y));
assert.deepEqual(
    Array.from(
        contourVerticesWithRustIfReady(
            new Int32Array([64]),
            new Int32Array([10]),
            new Int32Array([64]),
            1,
            1,
            0,
            [new Int32Array([0, 0]), new Int32Array([0, 0])],
            undefined,
            0,
            0,
            0,
            -1,
            -20,
            20,
            true,
        )!,
    ),
    [10],
);

registerRustMirrorModelGeometry((z, i1, i3) =>
    new Int32Array([z.length, i1.length, -z[0], i3[0], i1[0]]),
);
const mirrorZ = new Int32Array([9]);
const mirrorI1 = new Int32Array([1]);
const mirrorI3 = new Int32Array([3]);
assert.equal(
    mirrorModelGeometryWithRustIfReady(mirrorZ, mirrorI1, mirrorI3, 1, 1),
    true,
);
assert.deepEqual(Array.from(mirrorZ), [-9]);
assert.deepEqual(Array.from(mirrorI1), [3]);
assert.deepEqual(Array.from(mirrorI3), [1]);

registerRustTextureMapper(() => new Float32Array([0, 0, 1, 0, 0, 1]));
const fakeModel = {
    faceCount: 1,
    verticesX: new Int32Array([0, 128, 0]),
    verticesY: new Int32Array([0, 0, 128]),
    verticesZ: new Int32Array([0, 0, 0]),
    indices1: new Int32Array([0]),
    indices2: new Int32Array([1]),
    indices3: new Int32Array([2]),
    textureCoords: new Int8Array([-1]),
    textureRenderTypes: new Int8Array(),
    textureMappingP: new Int16Array(),
    textureMappingM: new Int16Array(),
    textureMappingN: new Int16Array(),
    textureScaleX: new Int32Array(),
    textureScaleY: new Int32Array(),
    textureScaleZ: new Int32Array(),
    textureRotation: new Int8Array(),
    textureDirection: new Int8Array(),
    textureSpeed: new Int32Array(),
    textureTransU: new Int32Array(),
    textureTransV: new Int32Array(),
};
assert.deepEqual(
    Array.from(
        computeTextureCoordsWithRustIfReady(
            fakeModel as any,
            new Int16Array([7]),
        )!,
    ),
    [0, 0, 1, 0, 0, 1],
);

registerRustModelNormalCalculator(() =>
    new Int32Array([
        1, 1,
        1, 2, 3, 4,
        1, 5, 6, 7,
    ]),
);
const normals = calculateModelNormalsWithRustIfReady(
    new Int32Array([0]),
    new Int32Array([0]),
    new Int32Array([0]),
    1,
    new Int32Array([0]),
    new Int32Array([0]),
    new Int32Array([0]),
    undefined,
);
assert(normals);
assert.equal(normals.usedVertexCount, 1);
assert.equal(normals.faceCount, 1);
assert.deepEqual(Array.from(normals.vertexNormals), [1, 2, 3, 4]);
assert.deepEqual(Array.from(normals.faceNormals), [1, 5, 6, 7]);

registerRustModelFaceLighter(() => new Int32Array([11, 22, 33]));
assert.deepEqual(
    Array.from(
        lightModelFacesWithRustIfReady(
            new Int32Array([0]),
            new Int32Array([0]),
            new Int32Array([0]),
            new Uint16Array([0x1234]),
            undefined,
            undefined,
            undefined,
            normals.vertexNormals,
            undefined,
            normals.faceNormals,
            64,
            768,
            -50,
            -10,
            -50,
        )!,
    ),
    [11, 22, 33],
);

let stats = getRustStage5OwnershipStats();
for (const path of [
    "skeletal",
    "legacy",
    "contour",
    "basic",
    "mirror",
    "uv",
    "normals",
    "lighting",
] as const) {
    assert.equal(stats[path].attempts, 1, `${path} attempt count`);
    assert.equal(stats[path].successes, 1, `${path} success count`);
    assert.equal(stats[path].fallbacks, 0, `${path} fallback count`);
    assert.equal(stats[path].failures, 0, `${path} failure count`);
}

registerRustBasicVertexTransformer(undefined);
assert.equal(
    transformVerticesWithRustIfReady(
        new Int32Array([1]),
        new Int32Array([2]),
        new Int32Array([3]),
        1,
        RustBasicTransformMode.TRANSLATE,
        1,
        1,
        1,
    ),
    false,
);
stats = getRustStage5OwnershipStats();
assert.equal(stats.basic.attempts, 2);
assert.equal(stats.basic.successes, 1);
assert.equal(stats.basic.fallbacks, 1);
assert.equal(stats.basic.failures, 0);

setRustStage5StrictMode(true);
assert.throws(
    () =>
        transformVerticesWithRustIfReady(
            new Int32Array([1]),
            new Int32Array([2]),
            new Int32Array([3]),
            1,
            RustBasicTransformMode.TRANSLATE,
            1,
            1,
            1,
        ),
    /\[RustStage5\] basic fallback: backend unavailable/,
);
setRustStage5StrictMode(false);

registerRustModelNormalCalculator(undefined);
registerRustModelFaceLighter(undefined);
registerRustSkeletalSkinner(undefined);
registerRustLegacyTransformer(undefined);
registerRustContourBuilder(undefined);
registerRustMirrorModelGeometry(undefined);
registerRustTextureMapper(undefined);

console.log("rust Stage 5 ownership tests passed");
