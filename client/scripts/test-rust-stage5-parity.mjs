import { readFileSync } from "node:fs";
import path from "node:path";
import { pathToFileURL, fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const clientDir = path.resolve(scriptDir, "..");
const rendererDir = path.join(clientDir, "public", "rust-renderer");
const modulePath = path.join(rendererDir, "rustosrs_renderer.js");
const wasmPath = path.join(rendererDir, "rustosrs_renderer_bg.wasm");

const module = await import(pathToFileURL(modulePath).href);
await module.default(readFileSync(wasmPath));

function assert(condition, message) {
    if (!condition) throw new Error(message);
}

function arraysEqual(actual, expected, label) {
    const a = Array.from(actual);
    const e = Array.from(expected);
    assert(
        a.length === e.length && a.every((value, index) => value === e[index]),
        `${label}: ${JSON.stringify(a)} vs ${JSON.stringify(e)}`,
    );
}

function f32BitsEqual(actual, expected, label) {
    assert(actual.length === expected.length, `${label}: length mismatch`);
    const a = new Uint32Array(actual.buffer, actual.byteOffset, actual.length);
    const e = new Uint32Array(expected.buffer, expected.byteOffset, expected.length);
    for (let i = 0; i < a.length; i++) {
        assert(
            a[i] === e[i],
            `${label}: f32 bit mismatch at ${i}, 0x${a[i].toString(16)} vs 0x${e[i].toString(16)}`,
        );
    }
}

function rsTrig(angle) {
    const radians = angle * ((360 / 2048) * (Math.PI / 180));
    return [
        (65536 * Math.sin(radians)) | 0,
        (65536 * Math.cos(radians)) | 0,
    ];
}

function referenceBasicRotate(x, y, z, angle) {
    const [sin, cos] = rsTrig(angle);
    return [
        (sin * z + cos * x) >> 16,
        y,
        (cos * z - sin * x) >> 16,
    ];
}

for (let angle = 0; angle < 2048; angle++) {
    const rust = module.transform_vertices_basic(
        new Int32Array([1234]),
        new Int32Array([-567]),
        new Int32Array([891]),
        3,
        angle,
        0,
        0,
    );
    arraysEqual(
        rust,
        referenceBasicRotate(1234, -567, 891, angle),
        `basic rotation angle ${angle}`,
    );
}

function referenceLegacyRotate(x, y, z, tx, ty, tz, ox, oy, oz) {
    let vx = x - ox;
    let vy = y - oy;
    let vz = z - oz;
    const angleX = (tx & 0xff) * 8;
    const angleY = (ty & 0xff) * 8;
    const angleZ = (tz & 0xff) * 8;

    if (angleZ !== 0) {
        const [sin, cos] = rsTrig(angleZ);
        const temp = (sin * vy + cos * vx) >> 16;
        vy = (cos * vy - sin * vx) >> 16;
        vx = temp;
    }
    if (angleX !== 0) {
        const [sin, cos] = rsTrig(angleX);
        const temp = (cos * vy - sin * vz) >> 16;
        vz = (sin * vy + cos * vz) >> 16;
        vy = temp;
    }
    if (angleY !== 0) {
        const [sin, cos] = rsTrig(angleY);
        const temp = (sin * vz + cos * vx) >> 16;
        vz = (cos * vz - sin * vx) >> 16;
        vx = temp;
    }
    return [(vx + ox) | 0, (vy + oy) | 0, (vz + oz) | 0];
}

for (const tx of [0, 1, 17, 31, 64, 95, 127, 128, 159, 191, 223, 255]) {
    const ty = (tx * 3) & 0xff;
    const tz = (tx * 5) & 0xff;
    const rust = module.apply_legacy_transforms(
        new Int32Array([123]),
        new Int32Array([-45]),
        new Int32Array([67]),
        new Int8Array(),
        new Uint16Array(),
        new Uint32Array([0, 1]),
        new Int32Array([0]),
        new Uint32Array([0]),
        new Int32Array(),
        new Int32Array([2, tx, ty, tz]),
        new Uint32Array([0, 1]),
        new Int32Array([0]),
        11,
        -7,
        5,
    );
    arraysEqual(
        rust.slice(7, 10),
        referenceLegacyRotate(123, -45, 67, tx, ty, tz, 11, -7, 5),
        `legacy rotation ${tx}/${ty}/${tz}`,
    );
}

function sampleHeight(heights, width, depth, vx, vz) {
    const rx = vx & 0x7f;
    const rz = vz & 0x7f;
    const tx = vx >> 7;
    const tz = vz >> 7;
    if (tx < 0 || tz < 0 || tx + 1 >= width || tz + 1 >= depth) return undefined;
    const at = (x, z) => heights[x * depth + z];
    const h0 = (at(tx, tz) * (128 - rx) + at(tx + 1, tz) * rx) >> 7;
    const h1 = (at(tx, tz + 1) * (128 - rx) + at(tx + 1, tz + 1) * rx) >> 7;
    return (h0 * (128 - rz) + h1 * rz) >> 7;
}

function referenceContour(
    type,
    param,
    xs,
    ys,
    zs,
    used,
    base,
    bw,
    bd,
    above,
    aw,
    ad,
    sceneX,
    sceneHeight,
    sceneZ,
    denominator,
    minY,
    maxY,
) {
    const out = new Int32Array(xs.length);
    const deltaY = maxY - minY;
    for (let i = 0; i < xs.length; i++) {
        const isUsed = i < used;
        const vx = (xs[i] + sceneX) | 0;
        const vz = (zs[i] + sceneZ) | 0;
        const baseHeight = sampleHeight(base, bw, bd, vx, vz);
        const aboveHeight = sampleHeight(above, aw, ad, vx, vz);

        if (type === 1) {
            if (baseHeight !== undefined) out[i] = ys[i] + baseHeight - sceneHeight;
        } else if (type === 2) {
            const yRatio = denominator === 0
                ? 0
                : (((ys[i] << 16) / denominator) | 0);
            if (yRatio < param) {
                if (baseHeight !== undefined && param !== 0) {
                    out[i] =
                        ys[i]
                        + ((baseHeight - sceneHeight) * (param - yRatio)) / param;
                } else if (param === 0) {
                    out[i] = ys[i];
                }
            } else {
                out[i] = ys[i];
            }
        } else if (type === 3) {
            if (!isUsed) continue;
            if (baseHeight === undefined) {
                out[i] = ys[i];
                continue;
            }
            let delta = baseHeight - sceneHeight;
            const limit = Math.abs(param);
            if (limit > 0) delta = Math.max(-limit, Math.min(limit, delta));
            out[i] = ys[i] + delta;
        } else if (type === 4) {
            if (!isUsed) continue;
            if (aboveHeight !== undefined) {
                out[i] = ys[i] + aboveHeight - sceneHeight + deltaY;
            }
        } else if (type === 5) {
            if (!isUsed) continue;
            if (baseHeight !== undefined && aboveHeight !== undefined) {
                const deltaHeight = baseHeight - aboveHeight;
                out[i] =
                    (((((ys[i] << 8) / deltaY) | 0) * deltaHeight) >> 8)
                    - (sceneHeight - baseHeight);
            }
        }
    }
    return out;
}

const contourXs = new Int32Array([64, 160]);
const contourYs = new Int32Array([-20, 10]);
const contourZs = new Int32Array([64, 160]);
const contourBase = new Int32Array([
    0, 64, 128,
    128, 192, 256,
    256, 320, 384,
]);
const contourAbove = new Int32Array([
    300, 364, 428,
    428, 492, 556,
    556, 620, 684,
]);
for (const [type, param] of [[1, 0], [2, 40000], [3, 25], [4, 0], [5, 0]]) {
    const rust = module.contour_vertices_y(
        contourXs,
        contourYs,
        contourZs,
        2,
        type,
        param,
        contourBase,
        3,
        3,
        contourAbove,
        3,
        3,
        0,
        50,
        0,
        -64,
        -20,
        40,
        false,
    );
    const expected = referenceContour(
        type,
        param,
        contourXs,
        contourYs,
        contourZs,
        2,
        contourBase,
        3,
        3,
        contourAbove,
        3,
        3,
        0,
        50,
        0,
        -64,
        -20,
        40,
    );
    arraysEqual(rust, expected, `contour type ${type}`);
}

function referenceSkeletal(verticesX, verticesY, verticesZ, offsets, boneIds, scales, matrices) {
    const out = new Int32Array(verticesX.length * 3);
    for (let vertex = 0; vertex < verticesX.length; vertex++) {
        const start = offsets[vertex];
        const end = offsets[vertex + 1];
        if (start === end) {
            out[vertex * 3] = verticesX[vertex];
            out[vertex * 3 + 1] = verticesY[vertex];
            out[vertex * 3 + 2] = verticesZ[vertex];
            continue;
        }
        const transform = new Float32Array(16);
        for (let influence = start; influence < end; influence++) {
            const boneId = boneIds[influence];
            if (boneId < 0 || boneId * 16 + 15 >= matrices.length) continue;
            const weight = Math.fround(scales[influence] / 255);
            const base = boneId * 16;
            for (let i = 0; i < 16; i++) {
                const row = i & 3;
                const scaled = row === 3
                    ? matrices[base + i]
                    : Math.fround(weight * matrices[base + i]);
                // gl-matrix's scaling matrix leaves row 3 unscaled, but the
                // final vertex transform only consumes rows 0..2.
                transform[i] = Math.fround(transform[i] + scaled);
            }
        }
        const vx = verticesX[vertex];
        const vy = -verticesY[vertex];
        const vz = -verticesZ[vertex];
        out[vertex * 3] = Math.round(
            transform[0] * vx + transform[4] * vy + transform[8] * vz + transform[12],
        );
        out[vertex * 3 + 1] = -Math.round(
            transform[1] * vx + transform[5] * vy + transform[9] * vz + transform[13],
        );
        out[vertex * 3 + 2] = -Math.round(
            transform[2] * vx + transform[6] * vy + transform[10] * vz + transform[14],
        );
    }
    return out;
}

const skeletalMatrices = new Float32Array([
    0.8660254, 0.5, 0, 0,
    -0.5, 0.8660254, 0, 0,
    0, 0, 1, 0,
    12.25, -7.5, 3.75, 1,
    0.70710677, 0, -0.70710677, 0,
    0, 1, 0, 0,
    0.70710677, 0, 0.70710677, 0,
    -5.5, 9.25, 2.5, 1,
]);
const skeletalX = new Int32Array([120, -33]);
const skeletalY = new Int32Array([-48, 91]);
const skeletalZ = new Int32Array([77, -144]);
const skeletalOffsets = new Uint32Array([0, 2, 4]);
const skeletalBoneIds = new Int32Array([0, 1, 1, 0]);
const skeletalScales = new Int32Array([128, 127, 96, 159]);
arraysEqual(
    module.skin_skeletal_vertices(
        skeletalX,
        skeletalY,
        skeletalZ,
        skeletalOffsets,
        skeletalBoneIds,
        skeletalScales,
        skeletalMatrices,
    ),
    referenceSkeletal(
        skeletalX,
        skeletalY,
        skeletalZ,
        skeletalOffsets,
        skeletalBoneIds,
        skeletalScales,
        skeletalMatrices,
    ),
    "skeletal weighted matrix parity",
);

function buildUvMatrix(p, m, n, rotation, scaleX, scaleY, scaleZ) {
    const fs = new Float32Array(9);
    let a = 1;
    let b = 0;
    let c = m / 32767;
    let d = -Math.sqrt(1 - c * c);
    const e = 1 - c;
    const len = Math.sqrt(p * p + n * n);
    if (len !== 0) {
        a = -n / len;
        b = p / len;
    }
    fs[0] = c + a * a * e;
    fs[1] = b * d;
    fs[2] = b * a * e;
    fs[3] = -b * d;
    fs[4] = c;
    fs[5] = a * d;
    fs[6] = a * b * e;
    fs[7] = -a * d;
    fs[8] = c + b * b * e;

    const r = new Float32Array(9);
    c = Math.cos(rotation * 0.024543693);
    d = Math.sin(rotation * 0.024543693);
    r[0] = c; r[1] = 0; r[2] = d;
    r[3] = 0; r[4] = 1; r[5] = 0;
    r[6] = -d; r[7] = 0; r[8] = c;

    const out = new Float32Array(9);
    out[0] = r[0] * fs[0] + r[1] * fs[3] + r[2] * fs[6];
    out[1] = r[0] * fs[1] + r[1] * fs[4] + r[2] * fs[7];
    out[2] = r[0] * fs[2] + r[1] * fs[5] + r[2] * fs[8];
    out[3] = r[3] * fs[0] + r[4] * fs[3] + r[5] * fs[6];
    out[4] = r[3] * fs[1] + r[4] * fs[4] + r[5] * fs[7];
    out[5] = r[3] * fs[2] + r[4] * fs[5] + r[5] * fs[8];
    out[6] = r[6] * fs[0] + r[7] * fs[3] + r[8] * fs[6];
    out[7] = r[6] * fs[1] + r[7] * fs[4] + r[8] * fs[7];
    out[8] = r[6] * fs[2] + r[7] * fs[5] + r[8] * fs[8];
    out[0] *= scaleX; out[1] *= scaleX; out[2] *= scaleX;
    out[3] *= scaleY; out[4] *= scaleY; out[5] *= scaleY;
    out[6] *= scaleZ; out[7] *= scaleZ; out[8] *= scaleZ;
    return out;
}

function uvDirection(u, v, direction) {
    if (direction === 1) {
        const oldU = u;
        u = -v;
        v = oldU;
    } else if (direction === 2) {
        u = -u;
        v = -v;
    } else if (direction === 3) {
        const oldU = u;
        u = v;
        v = -oldU;
    }
    const pair = new Float32Array(2);
    pair[0] = u;
    pair[1] = v;
    return [pair[0], pair[1]];
}

function projectCylindrical(vertex, center, scales, scaleZ, direction, speed) {
    const x = vertex[0] - center[0];
    const y = vertex[1] - center[1];
    const z = vertex[2] - center[2];
    const a = x * scales[0] + y * scales[1] + z * scales[2];
    const b = x * scales[3] + y * scales[4] + z * scales[5];
    const c = x * scales[6] + y * scales[7] + z * scales[8];
    let u = Math.atan2(a, c) / 6.2831855 + 0.5;
    if (scaleZ !== 1) u *= scaleZ;
    return uvDirection(u, b + 0.5 + speed, direction);
}

function dominantAxis(a, b, c) {
    const aa = Math.abs(a);
    const bb = Math.abs(b);
    const cc = Math.abs(c);
    if (bb > aa && bb > cc) return b > 0 ? 0 : 1;
    if (cc > aa && cc > bb) return c > 0 ? 2 : 3;
    return a > 0 ? 4 : 5;
}

function projectPlanar(vertex, center, type, scales, direction, speed, uOffset, vOffset) {
    const x = vertex[0] - center[0];
    const y = vertex[1] - center[1];
    const z = vertex[2] - center[2];
    const a = x * scales[0] + y * scales[1] + z * scales[2];
    const b = x * scales[3] + y * scales[4] + z * scales[5];
    const c = x * scales[6] + y * scales[7] + z * scales[8];
    let u;
    let v;
    if (type === 0) {
        u = a + speed + 0.5; v = -c + vOffset + 0.5;
    } else if (type === 1) {
        u = a + speed + 0.5; v = c + vOffset + 0.5;
    } else if (type === 2) {
        u = -a + speed + 0.5; v = -b + uOffset + 0.5;
    } else if (type === 3) {
        u = a + speed + 0.5; v = -b + uOffset + 0.5;
    } else if (type === 4) {
        u = c + vOffset + 0.5; v = -b + uOffset + 0.5;
    } else {
        u = -c + vOffset + 0.5; v = -b + uOffset + 0.5;
    }
    return uvDirection(u, v, direction);
}

function projectSpherical(vertex, center, scales, direction, speed) {
    const x = vertex[0] - center[0];
    const y = vertex[1] - center[1];
    const z = vertex[2] - center[2];
    const a = x * scales[0] + y * scales[1] + z * scales[2];
    const b = x * scales[3] + y * scales[4] + z * scales[5];
    const c = x * scales[6] + y * scales[7] + z * scales[8];
    const len = Math.sqrt(a * a + b * b + c * c);
    return uvDirection(
        Math.atan2(a, c) / 6.2831855 + 0.5,
        Math.asin(b / len) / 3.1415927 + 0.5 + speed,
        direction,
    );
}

function referenceComplexUv(type, direction) {
    const vertices = [[0, 0, 0], [128, 64, 32], [-64, 96, 160]];
    const center = [32, 48, 80];
    const sx = 512;
    const sy = 256;
    const sz = 768;
    const p = 10000;
    const m = 12000;
    const n = -8000;
    const rotation = 33;
    const speed = 64 / 256;
    let scaleX;
    let scaleY;
    let scaleZ;

    if (type === 1) {
        scaleY = 64 / sy;
        if (sx === 0) {
            scaleX = 1;
            scaleZ = 1;
        } else if (sx <= 0) {
            scaleX = -sx / 1024;
            scaleZ = 1;
        } else {
            scaleX = 1;
            scaleZ = sx / 1024;
        }
    } else if (type === 2) {
        scaleX = 64 / sx;
        scaleY = 64 / sy;
        scaleZ = 64 / sz;
    } else {
        scaleX = sx / 1024;
        scaleY = sy / 1024;
        scaleZ = sz / 1024;
    }

    const scales = buildUvMatrix(p, m, n, rotation, scaleX, scaleY, scaleZ);
    let pairs;
    if (type === 1) {
        const wrap = sz / 1024;
        pairs = vertices.map((v) =>
            projectCylindrical(v, center, scales, wrap, direction, speed)
        );
        const half = wrap / 2;
        const component = direction & 1;
        for (const index of [1, 2]) {
            if (pairs[index][component] - pairs[0][component] > half) {
                pairs[index][component] -= wrap;
            } else if (pairs[0][component] - pairs[index][component] > half) {
                pairs[index][component] += wrap;
            }
        }
    } else if (type === 2) {
        const v0 = vertices[0];
        const v1 = vertices[1];
        const v2 = vertices[2];
        const dx1 = v1[0] - v0[0];
        const dy1 = v1[1] - v0[1];
        const dz1 = v1[2] - v0[2];
        const dx2 = v2[0] - v0[0];
        const dy2 = v2[1] - v0[1];
        const dz2 = v2[2] - v0[2];
        const vx = dy1 * dz2 - dy2 * dz1;
        const vy = dz1 * dx2 - dz2 * dx1;
        const vz = dx1 * dy2 - dx2 * dy1;
        const axis = dominantAxis(
            (vx * scales[0] + vy * scales[1] + vz * scales[2]) / (64 / sx),
            (vx * scales[3] + vy * scales[4] + vz * scales[5]) / (64 / sy),
            (vx * scales[6] + vy * scales[7] + vz * scales[8]) / (64 / sz),
        );
        pairs = vertices.map((v) =>
            projectPlanar(
                v,
                center,
                axis,
                scales,
                direction,
                speed,
                32 / 256,
                -48 / 256,
            )
        );
    } else {
        pairs = vertices.map((v) =>
            projectSpherical(v, center, scales, direction, speed)
        );
        const component = direction & 1;
        if (pairs[1][component] - pairs[0][component] > 0.5) {
            pairs[1][component]--;
        } else if (
            pairs[0][component] - pairs[1][component]
            > (component === 0 ? 0 : 0.5)
        ) {
            pairs[1][component]++;
        }
        if (pairs[2][component] - pairs[0][component] > 0.5) {
            pairs[2][component]--;
        } else if (pairs[0][component] - pairs[2][component] > 0.5) {
            pairs[2][component]++;
        }
    }

    return new Float32Array([
        pairs[0][0], pairs[0][1],
        pairs[1][0], pairs[1][1],
        pairs[2][0], pairs[2][1],
    ]);
}

for (const type of [1, 2, 3]) {
    for (const direction of [0, 1, 2, 3]) {
        const rust = module.compute_model_uvs(
            new Int32Array([0, 128, -64]),
            new Int32Array([0, 64, 96]),
            new Int32Array([0, 32, 160]),
            new Int32Array([0]),
            new Int32Array([1]),
            new Int32Array([2]),
            new Int16Array([7]),
            new Int8Array([0]),
            new Int8Array([type]),
            new Int16Array([10000]),
            new Int16Array([12000]),
            new Int16Array([-8000]),
            new Int32Array([512]),
            new Int32Array([256]),
            new Int32Array([768]),
            new Int8Array([33]),
            new Int8Array([direction]),
            new Int32Array([64]),
            new Int32Array([32]),
            new Int32Array([-48]),
        );
        f32BitsEqual(
            rust,
            referenceComplexUv(type, direction),
            `UV type ${type} direction ${direction}`,
        );
    }
}

function referenceNormals(xs, ys, zs, used, i1, i2, i3, types) {
    const vertexNormals = Array.from({ length: used }, () => [0, 0, 0, 0]);
    const faceNormals = Array.from({ length: i1.length }, () => [0, 0, 0, 0]);
    for (let face = 0; face < i1.length; face++) {
        const a = i1[face];
        const b = i2[face];
        const c = i3[face];
        const x1 = xs[b] - xs[a];
        const y1 = ys[b] - ys[a];
        const z1 = zs[b] - zs[a];
        const x2 = xs[c] - xs[a];
        const y2 = ys[c] - ys[a];
        const z2 = zs[c] - zs[a];
        let nx = y1 * z2 - y2 * z1;
        let ny = z1 * x2 - z2 * x1;
        let nz = x1 * y2 - x2 * y1;
        while (
            nx > 8192 || ny > 8192 || nz > 8192
            || nx < -8192 || ny < -8192 || nz < -8192
        ) {
            nx >>= 1;
            ny >>= 1;
            nz >>= 1;
        }
        let magnitude = Math.sqrt(nx * nx + ny * ny + nz * nz) | 0;
        if (magnitude <= 0) magnitude = 1;
        nx = ((nx * 256) / magnitude) | 0;
        ny = ((ny * 256) / magnitude) | 0;
        nz = ((nz * 256) / magnitude) | 0;
        const type = types.length ? types[face] : 0;
        if (type === 0) {
            for (const vertex of [a, b, c]) {
                vertexNormals[vertex][0] += nx;
                vertexNormals[vertex][1] += ny;
                vertexNormals[vertex][2] += nz;
                vertexNormals[vertex][3]++;
            }
        } else if (type === 1) {
            faceNormals[face] = [1, nx, ny, nz];
        }
    }
    return new Int32Array([
        used,
        i1.length,
        ...vertexNormals.flat(),
        ...faceNormals.flat(),
    ]);
}

const normalXs = new Int32Array([0, 1200, -300]);
const normalYs = new Int32Array([0, 700, 900]);
const normalZs = new Int32Array([0, -400, 1300]);
const normalI1 = new Int32Array([0]);
const normalI2 = new Int32Array([1]);
const normalI3 = new Int32Array([2]);
const normalTypes = new Int8Array([0]);
arraysEqual(
    module.calculate_model_normals(
        normalXs,
        normalYs,
        normalZs,
        3,
        normalI1,
        normalI2,
        normalI3,
        normalTypes,
    ),
    referenceNormals(
        normalXs,
        normalYs,
        normalZs,
        3,
        normalI1,
        normalI2,
        normalI3,
        normalTypes,
    ),
    "model normal parity",
);

function adjustLightness(hsl, lightness) {
    lightness = ((hsl & 127) * lightness) >> 7;
    if (lightness < 2) lightness = 2;
    else if (lightness > 126) lightness = 126;
    return (hsl & 0xff80) + lightness;
}

function clampLightness(lightness) {
    if (lightness < 2) lightness = 2;
    else if (lightness > 126) lightness = 126;
    return lightness | 0;
}

function referenceLightFaces(
    i1,
    i2,
    i3,
    colors,
    types,
    alphas,
    textures,
    vertexNormals,
    mergedNormals,
    faceNormals,
    ambient,
    contrast,
    lx,
    ly,
    lz,
) {
    const magnitude = Math.sqrt(lz * lz + lx * lx + ly * ly) | 0;
    const lightIntensity = (magnitude * contrast) >> 8;
    const faceIntensity = (lightIntensity >> 1) + lightIntensity;
    const out = new Int32Array(i1.length * 3);
    const normalFor = (vertex) => {
        if (mergedNormals.length && mergedNormals[vertex * 5] !== 0) {
            return [
                mergedNormals[vertex * 5 + 1],
                mergedNormals[vertex * 5 + 2],
                mergedNormals[vertex * 5 + 3],
                mergedNormals[vertex * 5 + 4],
            ];
        }
        return [
            vertexNormals[vertex * 4],
            vertexNormals[vertex * 4 + 1],
            vertexNormals[vertex * 4 + 2],
            vertexNormals[vertex * 4 + 3],
        ];
    };
    const vertexLight = (normal) =>
        ambient
        + (ly * normal[1] + lz * normal[2] + lx * normal[0])
        / (lightIntensity * normal[3]);

    for (let face = 0; face < i1.length; face++) {
        let type = types.length ? types[face] : 0;
        const alpha = alphas.length ? alphas[face] : 0;
        const texture = textures.length ? textures[face] : -1;
        if (alpha === -2) type = 3;
        if (alpha === -1) type = 2;
        const offset = face * 3;

        if (texture === -1) {
            if (type === 0) {
                const color = colors[face];
                for (const [component, vertex] of [i1[face], i2[face], i3[face]].entries()) {
                    const light = vertexLight(normalFor(vertex));
                    const packed = light << 17;
                    out[offset + component] =
                        packed | adjustLightness(color, packed >> 17);
                }
            } else if (type === 1 && faceNormals[face * 4] !== 0) {
                const nx = faceNormals[face * 4 + 1];
                const ny = faceNormals[face * 4 + 2];
                const nz = faceNormals[face * 4 + 3];
                const light =
                    ambient + (ly * ny + lz * nz + lx * nx) / faceIntensity;
                const packed = light << 17;
                out[offset] =
                    packed | adjustLightness(colors[face], packed >> 17);
                out[offset + 2] = -1;
            } else if (type === 3) {
                out[offset] = 128;
                out[offset + 2] = -1;
            } else {
                out[offset + 2] = -2;
            }
        } else if (type === 0) {
            for (const [component, vertex] of [i1[face], i2[face], i3[face]].entries()) {
                out[offset + component] =
                    clampLightness(vertexLight(normalFor(vertex)));
            }
        } else if (type === 1 && faceNormals[face * 4] !== 0) {
            const nx = faceNormals[face * 4 + 1];
            const ny = faceNormals[face * 4 + 2];
            const nz = faceNormals[face * 4 + 3];
            out[offset] = clampLightness(
                ambient + (ly * ny + lz * nz + lx * nx) / faceIntensity,
            );
            out[offset + 2] = -1;
        } else {
            out[offset + 2] = -2;
        }
    }
    return out;
}

const lightI1 = new Int32Array([0, 0, 0, 0]);
const lightI2 = new Int32Array([1, 1, 1, 1]);
const lightI3 = new Int32Array([2, 2, 2, 2]);
const lightColors = new Uint16Array([0x1234, 0x2345, 0x3456, 0x4567]);
const lightTypes = new Int8Array([0, 1, 0, 0]);
const lightAlphas = new Int8Array([0, 0, -1, -2]);
const lightTextures = new Int16Array([-1, -1, 7, -1]);
const lightVertexNormals = new Int32Array([
    0, -256, 0, 1,
    0, -256, 0, 1,
    0, -256, 0, 1,
]);
const lightMergedNormals = new Int32Array([
    1, 0, -128, 0, 2,
    0, 0, 0, 0, 0,
    0, 0, 0, 0, 0,
]);
const lightFaceNormals = new Int32Array([
    0, 0, 0, 0,
    1, 0, -256, 0,
    0, 0, 0, 0,
    0, 0, 0, 0,
]);
arraysEqual(
    module.light_model_faces(
        lightI1,
        lightI2,
        lightI3,
        lightColors,
        lightTypes,
        lightAlphas,
        lightTextures,
        lightVertexNormals,
        lightMergedNormals,
        lightFaceNormals,
        64,
        768,
        -50,
        -10,
        -50,
    ),
    referenceLightFaces(
        lightI1,
        lightI2,
        lightI3,
        lightColors,
        lightTypes,
        lightAlphas,
        lightTextures,
        lightVertexNormals,
        lightMergedNormals,
        lightFaceNormals,
        64,
        768,
        -50,
        -10,
        -50,
    ),
    "model lighting parity",
);

console.log("Rust Stage 5 WASM parity passed");
