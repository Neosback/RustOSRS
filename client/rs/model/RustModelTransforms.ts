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
    if (!skeletalSkinner || !groups || !scales || vertexCount <= 0) {
        return undefined;
    }

    const flattened = flattenSkinning(vertexCount, groups, scales);
    if (!flattened) {
        return undefined;
    }

    try {
        const result = skeletalSkinner(
            verticesX.subarray(0, vertexCount),
            verticesY.subarray(0, vertexCount),
            verticesZ.subarray(0, vertexCount),
            flattened.offsets,
            flattened.boneIds,
            flattened.boneScales,
            boneMatrices,
        );
        return result.length === vertexCount * 3 ? result : undefined;
    } catch (error) {
        if (!warnedAboutSkinningFailure) {
            warnedAboutSkinningFailure = true;
            console.warn(
                "[RustModelTransforms] Rust skeletal skinning failed; using TypeScript fallback.",
                error,
            );
        }
        return undefined;
    }
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
    if (!legacyTransformer || operations.length === 0 || vertexCount < 0) {
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
        return undefined;
    }
}
