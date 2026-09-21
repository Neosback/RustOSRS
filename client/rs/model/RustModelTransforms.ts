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
