export type RenderWorkerPolicyInput = {
    hardwareConcurrency: number;
    mobile: boolean;
    sharedCacheMemoryAvailable: boolean;
};

/**
 * Multiple render workers are only memory-safe for sparse DAT2 when its
 * backing buffer can be shared. Without cross-origin isolation, structured
 * cloning LoadedCache duplicates the full DAT2 ArrayBuffer into each worker.
 */
export function resolveRenderWorkerCount(input: RenderWorkerPolicyInput): number {
    const cores = Math.max(1, Math.floor(input.hardwareConcurrency || 1));
    if (!input.sharedCacheMemoryAvailable) {
        return 1;
    }

    const availableWorkerCores = Math.max(1, cores - 1);
    if (input.mobile) {
        return Math.min(2, availableWorkerCores);
    }
    return Math.min(4, availableWorkerCores);
}

export function canShareSparseCacheMemory(): boolean {
    return (
        globalThis.crossOriginIsolated === true &&
        typeof globalThis.SharedArrayBuffer !== "undefined"
    );
}
