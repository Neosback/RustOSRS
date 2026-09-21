import {
    recordRustStage5Attempt,
    recordRustStage5Fallback,
    recordRustStage5Success,
} from "./RustStage5Ownership";

export type RustSkeletalSkinner = (
    verticesX: Int32Array,
    verticesY: Int32Array,
    verticesZ: Int32Array,
    vertexGroupOffsets: Uint32Array,
    boneIds: Int32Array,
    boneScales: Int32Array,
    boneMatrices: Float32Array,
) => Int32Array;

type FlattenedSkinning = {
    scalesRef: object;
    vertexCount: number;
    offsets: Uint32Array;
    boneIds: Int32Array;
    boneScales: Int32Array;
};

let skeletalSkinner: RustSkeletalSkinner | undefined;
let warnedAboutSkinningFailure = false;

const skinningCache = new WeakMap<object, FlattenedSkinning>();

export function registerRustSkeletalSkinner(
    skinner: RustSkeletalSkinner | undefined,
): void {
    skeletalSkinner = skinner;
}

function flattenSkinning(
    vertexCount: number,
    groups: Int32Array[],
    scales: Int32Array[],
): FlattenedSkinning | undefined {
    const groupKey = groups as unknown as object;
    const scaleKey = scales as unknown as object;
    const cached = skinningCache.get(groupKey);
    if (
        cached
        && cached.scalesRef === scaleKey
        && cached.vertexCount === vertexCount
    ) {
        return cached;
    }

    const offsets = new Uint32Array(vertexCount + 1);
    let influenceCount = 0;
    for (let vertex = 0; vertex < vertexCount; vertex++) {
        offsets[vertex] = influenceCount;
        const vertexGroups = groups[vertex];
        if (!vertexGroups || vertexGroups.length === 0) {
            continue;
        }
        const vertexScales = scales[vertex];
        if (!vertexScales || vertexScales.length !== vertexGroups.length) {
            return undefined;
        }
        influenceCount += vertexGroups.length;
    }
    offsets[vertexCount] = influenceCount;

    const boneIds = new Int32Array(influenceCount);
    const boneScales = new Int32Array(influenceCount);
    let offset = 0;
    for (let vertex = 0; vertex < vertexCount; vertex++) {
        const vertexGroups = groups[vertex];
        if (!vertexGroups || vertexGroups.length === 0) {
            continue;
        }
        const vertexScales = scales[vertex];
        for (let influence = 0; influence < vertexGroups.length; influence++) {
            boneIds[offset] = vertexGroups[influence] | 0;
            boneScales[offset] = vertexScales[influence] | 0;
            offset++;
        }
    }

    const flattened: FlattenedSkinning = {
        scalesRef: scaleKey,
        vertexCount,
        offsets,
        boneIds,
        boneScales,
    };
    skinningCache.set(groupKey, flattened);
    return flattened;
}

export function skinSkeletalVerticesWithRustIfReady(
    verticesX: Int32Array,
    verticesY: Int32Array,
    verticesZ: Int32Array,
    vertexCount: number,
    groups: Int32Array[] | undefined,
    scales: Int32Array[] | undefined,
    boneMatrices: Float32Array,
): Int32Array | undefined {
    if (!groups || !scales || vertexCount <= 0) {
        return undefined;
    }
    recordRustStage5Attempt("skeletal");
    if (!skeletalSkinner) {
        recordRustStage5Fallback("skeletal", "backend unavailable");
        return undefined;
    }

    const flattened = flattenSkinning(vertexCount, groups, scales);
    if (!flattened) {
        recordRustStage5Fallback("skeletal", "invalid skinning packet", true);
        return undefined;
    }

    let result: Int32Array;
    try {
        result = skeletalSkinner(
            verticesX.subarray(0, vertexCount),
            verticesY.subarray(0, vertexCount),
            verticesZ.subarray(0, vertexCount),
            flattened.offsets,
            flattened.boneIds,
            flattened.boneScales,
            boneMatrices,
        );
    } catch (error) {
        if (!warnedAboutSkinningFailure) {
            warnedAboutSkinningFailure = true;
            console.warn(
                "[RustModelTransforms] Rust skeletal skinning failed; using TypeScript fallback.",
                error,
            );
        }
        recordRustStage5Fallback("skeletal", "backend threw", true);
        return undefined;
    }
    if (result.length !== vertexCount * 3) {
        recordRustStage5Fallback("skeletal", "invalid result length", true);
        return undefined;
    }
    recordRustStage5Success("skeletal");
    return result;
}


export type RustLegacyTransformer = (
    verticesX: Int32Array,
    verticesY: Int32Array,
    verticesZ: Int32Array,
    faceAlphas: Int8Array,
    faceColors: Uint16Array,
    vertexLabelOffsets: Uint32Array,
    vertexLabelIndices: Int32Array,
    faceLabelOffsets: Uint32Array,
    faceLabelIndices: Int32Array,
    operationFields: Int32Array,
    operationLabelOffsets: Uint32Array,
    operationLabels: Int32Array,
    initialOriginX: number,
    initialOriginY: number,
    initialOriginZ: number,
) => Int32Array;

export type LegacyTransformOperation = {
    type: number;
    labels: readonly number[];
    x: number;
    y: number;
    z: number;
};

export type LegacyTransformResult = {
    originX: number;
    originY: number;
    originZ: number;
    changedLight: boolean;
};

type FlattenedLabels = {
    groupCount: number;
    offsets: Uint32Array;
    indices: Int32Array;
};

let legacyTransformer: RustLegacyTransformer | undefined;
let warnedAboutLegacyTransformFailure = false;

const labelCache = new WeakMap<object, FlattenedLabels>();
const EMPTY_I8 = new Int8Array(0);
const EMPTY_U16 = new Uint16Array(0);
const EMPTY_LABELS: FlattenedLabels = {
    groupCount: 0,
    offsets: new Uint32Array([0]),
    indices: new Int32Array(0),
};

export function registerRustLegacyTransformer(
    transformer: RustLegacyTransformer | undefined,
): void {
    legacyTransformer = transformer;
}

function flattenLabels(groups: Int32Array[] | undefined): FlattenedLabels {
    if (!groups) {
        return EMPTY_LABELS;
    }
    const key = groups as unknown as object;
    const cached = labelCache.get(key);
    if (cached && cached.groupCount === groups.length) {
        return cached;
    }

    const offsets = new Uint32Array(groups.length + 1);
    let count = 0;
    for (let group = 0; group < groups.length; group++) {
        offsets[group] = count;
        count += groups[group]?.length ?? 0;
    }
    offsets[groups.length] = count;

    const indices = new Int32Array(count);
    let offset = 0;
    for (const group of groups) {
        if (!group) continue;
        indices.set(group, offset);
        offset += group.length;
    }

    const flattened = {
        groupCount: groups.length,
        offsets,
        indices,
    };
    labelCache.set(key, flattened);
    return flattened;
}

export function applyLegacyTransformsWithRustIfReady(
    verticesX: Int32Array,
    verticesY: Int32Array,
    verticesZ: Int32Array,
    vertexCount: number,
    faceAlphas: Int8Array | undefined,
    faceColors: Uint16Array | undefined,
    vertexLabels: Int32Array[] | undefined,
    faceLabels: Int32Array[] | undefined,
    operations: readonly LegacyTransformOperation[],
    initialOriginX: number,
    initialOriginY: number,
    initialOriginZ: number,
): LegacyTransformResult | undefined {
    if (operations.length === 0 || vertexCount < 0) {
        return undefined;
    }
    recordRustStage5Attempt("legacy");
    if (!legacyTransformer) {
        recordRustStage5Fallback("legacy", "backend unavailable");
        return undefined;
    }

    const vertices = Math.min(
        vertexCount | 0,
        verticesX.length,
        verticesY.length,
        verticesZ.length,
    );
    const flattenedVertices = flattenLabels(vertexLabels);
    const flattenedFaces = flattenLabels(faceLabels);

    const operationFields = new Int32Array(operations.length * 4);
    const operationLabelOffsets = new Uint32Array(operations.length + 1);
    let operationLabelCount = 0;
    for (let operation = 0; operation < operations.length; operation++) {
        const op = operations[operation];
        const field = operation * 4;
        operationFields[field] = op.type | 0;
        operationFields[field + 1] = op.x | 0;
        operationFields[field + 2] = op.y | 0;
        operationFields[field + 3] = op.z | 0;
        operationLabelOffsets[operation] = operationLabelCount;
        operationLabelCount += op.labels.length;
    }
    operationLabelOffsets[operations.length] = operationLabelCount;

    const operationLabels = new Int32Array(operationLabelCount);
    let labelOffset = 0;
    for (const operation of operations) {
        for (const label of operation.labels) {
            operationLabels[labelOffset++] = label | 0;
        }
    }

    try {
        const result = legacyTransformer(
            verticesX.subarray(0, vertices),
            verticesY.subarray(0, vertices),
            verticesZ.subarray(0, vertices),
            faceAlphas ?? EMPTY_I8,
            faceColors ?? EMPTY_U16,
            flattenedVertices.offsets,
            flattenedVertices.indices,
            flattenedFaces.offsets,
            flattenedFaces.indices,
            operationFields,
            operationLabelOffsets,
            operationLabels,
            initialOriginX | 0,
            initialOriginY | 0,
            initialOriginZ | 0,
        );

        const headerSize = 7;
        if (result.length < headerSize) {
            recordRustStage5Fallback("legacy", "result header missing", true);
            return undefined;
        }
        const resultVertexCount = result[4] | 0;
        const alphaCount = result[5] | 0;
        const colorCount = result[6] | 0;
        const expectedLength =
            headerSize + resultVertexCount * 3 + alphaCount + colorCount;
        if (
            resultVertexCount !== vertices
            || alphaCount !== (faceAlphas?.length ?? 0)
            || colorCount !== (faceColors?.length ?? 0)
            || result.length !== expectedLength
        ) {
            recordRustStage5Fallback("legacy", "invalid result packet", true);
            return undefined;
        }

        let offset = headerSize;
        for (let vertex = 0; vertex < resultVertexCount; vertex++) {
            verticesX[vertex] = result[offset++];
            verticesY[vertex] = result[offset++];
            verticesZ[vertex] = result[offset++];
        }
        if (faceAlphas) {
            for (let face = 0; face < alphaCount; face++) {
                faceAlphas[face] = result[offset++];
            }
        } else {
            offset += alphaCount;
        }
        if (faceColors) {
            for (let face = 0; face < colorCount; face++) {
                faceColors[face] = result[offset++];
            }
        }

        recordRustStage5Success("legacy");
        return {
            originX: result[0] | 0,
            originY: result[1] | 0,
            originZ: result[2] | 0,
            changedLight: (result[3] | 0) !== 0,
        };
    } catch (error) {
        if (!warnedAboutLegacyTransformFailure) {
            warnedAboutLegacyTransformFailure = true;
            console.warn(
                "[RustModelTransforms] Rust legacy model transforms failed; using TypeScript fallback.",
                error,
            );
        }
        recordRustStage5Fallback("legacy", "backend threw", true);
        return undefined;
    }
}


export type RustContourBuilder = (
    verticesX: Int32Array,
    verticesY: Int32Array,
    verticesZ: Int32Array,
    usedVertexCount: number,
    contourType: number,
    param: number,
    heightMap: Int32Array,
    heightWidth: number,
    heightDepth: number,
    heightMapAbove: Int32Array,
    aboveWidth: number,
    aboveDepth: number,
    sceneX: number,
    sceneHeight: number,
    sceneZ: number,
    type2Denominator: number,
    minY: number,
    maxY: number,
    preserveType1UnusedOob: boolean,
) => Int32Array;

type FlattenedHeightMap = {
    width: number;
    depth: number;
    data: Int32Array;
};

let contourBuilder: RustContourBuilder | undefined;
let warnedAboutContourFailure = false;
const heightMapCache = new WeakMap<object, FlattenedHeightMap>();
const EMPTY_HEIGHT_MAP: FlattenedHeightMap = {
    width: 0,
    depth: 0,
    data: new Int32Array(0),
};

export function registerRustContourBuilder(
    builder: RustContourBuilder | undefined,
): void {
    contourBuilder = builder;
}

function flattenHeightMap(heightMap: Int32Array[] | undefined): FlattenedHeightMap | undefined {
    if (!heightMap || heightMap.length === 0) {
        return heightMap ? EMPTY_HEIGHT_MAP : undefined;
    }
    const key = heightMap as unknown as object;
    const cached = heightMapCache.get(key);
    const width = heightMap.length;
    const depth = heightMap[0]?.length ?? 0;
    if (cached && cached.width === width && cached.depth === depth) {
        return cached;
    }
    if (depth === 0) {
        return EMPTY_HEIGHT_MAP;
    }
    for (const column of heightMap) {
        if (!column || column.length !== depth) {
            return undefined;
        }
    }
    const data = new Int32Array(width * depth);
    for (let x = 0; x < width; x++) {
        data.set(heightMap[x], x * depth);
    }
    const flattened = { width, depth, data };
    heightMapCache.set(key, flattened);
    return flattened;
}

export function contourVerticesWithRustIfReady(
    verticesX: Int32Array,
    verticesY: Int32Array,
    verticesZ: Int32Array,
    usedVertexCount: number,
    contourType: number,
    param: number,
    heightMap: Int32Array[],
    heightMapAbove: Int32Array[] | undefined,
    sceneX: number,
    sceneHeight: number,
    sceneZ: number,
    type2Denominator: number,
    minY: number,
    maxY: number,
    preserveType1UnusedOob: boolean,
): Int32Array | undefined {
    if (contourType < 1 || contourType > 5) {
        return undefined;
    }
    recordRustStage5Attempt("contour");
    if (!contourBuilder) {
        recordRustStage5Fallback("contour", "backend unavailable");
        return undefined;
    }
    const base = flattenHeightMap(heightMap);
    const above = heightMapAbove ? flattenHeightMap(heightMapAbove) : EMPTY_HEIGHT_MAP;
    if (!base || !above) {
        recordRustStage5Fallback("contour", "invalid height-map packet", true);
        return undefined;
    }
    let result: Int32Array;
    try {
        result = contourBuilder(
            verticesX,
            verticesY,
            verticesZ,
            usedVertexCount | 0,
            contourType | 0,
            param | 0,
            base.data,
            base.width | 0,
            base.depth | 0,
            above.data,
            above.width | 0,
            above.depth | 0,
            sceneX | 0,
            sceneHeight | 0,
            sceneZ | 0,
            type2Denominator | 0,
            minY | 0,
            maxY | 0,
            preserveType1UnusedOob,
        );
    } catch (error) {
        if (!warnedAboutContourFailure) {
            warnedAboutContourFailure = true;
            console.warn(
                "[RustModelTransforms] Rust contour transform failed; using TypeScript fallback.",
                error,
            );
        }
        recordRustStage5Fallback("contour", "backend threw", true);
        return undefined;
    }
    if (result.length !== verticesX.length) {
        recordRustStage5Fallback("contour", "invalid result length", true);
        return undefined;
    }
    recordRustStage5Success("contour");
    return result;
}


export const RustBasicTransformMode = {
    ROTATE_90: 0,
    ROTATE_180: 1,
    ROTATE_270: 2,
    ROTATE_ANGLE: 3,
    TRANSLATE: 4,
    SCALE: 5,
} as const;

export type RustBasicVertexTransformer = (
    verticesX: Int32Array,
    verticesY: Int32Array,
    verticesZ: Int32Array,
    mode: number,
    a: number,
    b: number,
    c: number,
) => Int32Array;

let basicVertexTransformer: RustBasicVertexTransformer | undefined;
let warnedAboutBasicTransformFailure = false;

export function registerRustBasicVertexTransformer(
    transformer: RustBasicVertexTransformer | undefined,
): void {
    basicVertexTransformer = transformer;
}

export function transformVerticesWithRustIfReady(
    verticesX: Int32Array,
    verticesY: Int32Array,
    verticesZ: Int32Array,
    vertexCount: number,
    mode: number,
    a: number = 0,
    b: number = 0,
    c: number = 0,
): boolean {
    if (vertexCount <= 0) {
        return false;
    }
    recordRustStage5Attempt("basic");
    if (!basicVertexTransformer) {
        recordRustStage5Fallback("basic", "backend unavailable");
        return false;
    }

    const count = Math.min(
        vertexCount | 0,
        verticesX.length,
        verticesY.length,
        verticesZ.length,
    );
    try {
        const transformed = basicVertexTransformer(
            verticesX.subarray(0, count),
            verticesY.subarray(0, count),
            verticesZ.subarray(0, count),
            mode | 0,
            a | 0,
            b | 0,
            c | 0,
        );
        if (transformed.length !== count * 3) {
            recordRustStage5Fallback("basic", "invalid result length", true);
            return false;
        }

        for (let vertex = 0, offset = 0; vertex < count; vertex++) {
            verticesX[vertex] = transformed[offset++];
            verticesY[vertex] = transformed[offset++];
            verticesZ[vertex] = transformed[offset++];
        }
        recordRustStage5Success("basic");
        return true;
    } catch (error) {
        if (!warnedAboutBasicTransformFailure) {
            warnedAboutBasicTransformFailure = true;
            console.warn(
                "[RustModelTransforms] Rust basic vertex transform failed; using TypeScript fallback.",
                error,
            );
        }
        recordRustStage5Fallback("basic", "backend threw", true);
        return false;
    }
}


export type RustMirrorModelGeometry = (
    verticesZ: Int32Array,
    indices1: Int32Array,
    indices3: Int32Array,
) => Int32Array;

let mirrorModelGeometry: RustMirrorModelGeometry | undefined;
let warnedAboutMirrorFailure = false;

export function registerRustMirrorModelGeometry(
    mirror: RustMirrorModelGeometry | undefined,
): void {
    mirrorModelGeometry = mirror;
}

export function mirrorModelGeometryWithRustIfReady(
    verticesZ: Int32Array,
    indices1: Int32Array,
    indices3: Int32Array,
    vertexCount: number,
    faceCount: number,
): boolean {
    recordRustStage5Attempt("mirror");
    if (!mirrorModelGeometry) {
        recordRustStage5Fallback("mirror", "backend unavailable");
        return false;
    }
    const vertices = Math.min(vertexCount | 0, verticesZ.length);
    const faces = Math.min(faceCount | 0, indices1.length, indices3.length);
    try {
        const result = mirrorModelGeometry(
            verticesZ.subarray(0, vertices),
            indices1.subarray(0, faces),
            indices3.subarray(0, faces),
        );
        const expectedLength = 2 + vertices + faces * 2;
        if (
            result.length !== expectedLength
            || (result[0] | 0) !== vertices
            || (result[1] | 0) !== faces
        ) {
            recordRustStage5Fallback("mirror", "invalid result packet", true);
            return false;
        }

        let offset = 2;
        for (let vertex = 0; vertex < vertices; vertex++) {
            verticesZ[vertex] = result[offset++];
        }
        for (let face = 0; face < faces; face++) {
            indices1[face] = result[offset++];
        }
        for (let face = 0; face < faces; face++) {
            indices3[face] = result[offset++];
        }
        recordRustStage5Success("mirror");
        return true;
    } catch (error) {
        if (!warnedAboutMirrorFailure) {
            warnedAboutMirrorFailure = true;
            console.warn(
                "[RustModelTransforms] Rust model mirror failed; using TypeScript fallback.",
                error,
            );
        }
        recordRustStage5Fallback("mirror", "backend threw", true);
        return false;
    }
}
