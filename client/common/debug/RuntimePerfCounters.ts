export type RuntimeUploadCategory =
    | "actorData"
    | "dynamicNpc"
    | "dynamicGfx"
    | "dynamicProjectile"
    | "dynamicPlayer"
    | "residentPlayer"
    | "residentActor";

export interface RuntimePerfSnapshot {
    workerCount: number;
    sharedSparseCache: boolean;
    crossOriginIsolated: boolean;
    cacheAdvertisedBytes: number;
    js5GroupRequests: number;
    js5StoreHits: number;
    js5PendingDedupHits: number;
    js5HttpBatches: number;
    js5HttpBytes: number;
    js5ChainFetches: number;
    rustUploadCalls: Record<RuntimeUploadCategory, number>;
    rustUploadBytes: Record<RuntimeUploadCategory, number>;
    residentPlayerHits: number;
    residentPlayerMisses: number;
    residentPlayerEvictions: number;
    residentPlayerEntries: number;
    residentActorHits: number;
    residentActorMisses: number;
    residentActorEvictions: number;
    residentActorEntries: number;
}

const uploadCategories: RuntimeUploadCategory[] = [
    "actorData",
    "dynamicNpc",
    "dynamicGfx",
    "dynamicProjectile",
    "dynamicPlayer",
    "residentPlayer",
    "residentActor",
];

function emptyNumberRecord(): Record<RuntimeUploadCategory, number> {
    return Object.fromEntries(uploadCategories.map((name) => [name, 0])) as Record<
        RuntimeUploadCategory,
        number
    >;
}

class RuntimePerfCounters {
    private workerCount = 0;
    private sharedSparseCache = false;
    private cacheAdvertisedBytes = 0;
    private js5GroupRequests = 0;
    private js5StoreHits = 0;
    private js5PendingDedupHits = 0;
    private js5HttpBatches = 0;
    private js5HttpBytes = 0;
    private js5ChainFetches = 0;
    private rustUploadCalls = emptyNumberRecord();
    private rustUploadBytes = emptyNumberRecord();
    private residentPlayerHits = 0;
    private residentPlayerMisses = 0;
    private residentPlayerEvictions = 0;
    private residentPlayerEntries = 0;
    private residentActorHits = 0;
    private residentActorMisses = 0;
    private residentActorEvictions = 0;
    private residentActorEntries = 0;

    setWorkerPolicy(workerCount: number, sharedSparseCache: boolean): void {
        this.workerCount = Math.max(0, workerCount | 0);
        this.sharedSparseCache = !!sharedSparseCache;
    }

    setCacheAdvertisedBytes(bytes: number): void {
        this.cacheAdvertisedBytes = Math.max(0, Number.isFinite(bytes) ? bytes : 0);
    }

    recordJs5GroupRequest(storeHit: boolean, pendingDedupHit: boolean): void {
        this.js5GroupRequests++;
        if (storeHit) this.js5StoreHits++;
        if (pendingDedupHit) this.js5PendingDedupHits++;
    }

    recordJs5HttpBatch(bytes: number): void {
        this.js5HttpBatches++;
        this.js5HttpBytes += Math.max(0, bytes | 0);
    }

    recordJs5ChainFetch(): void {
        this.js5ChainFetches++;
    }

    recordRustUpload(category: RuntimeUploadCategory, bytes: number): void {
        this.rustUploadCalls[category]++;
        this.rustUploadBytes[category] += Math.max(0, bytes | 0);
    }

    recordResidentPlayerHit(): void {
        this.residentPlayerHits++;
    }

    recordResidentPlayerMiss(): void {
        this.residentPlayerMisses++;
    }

    recordResidentPlayerEviction(): void {
        this.residentPlayerEvictions++;
    }

    setResidentPlayerEntries(entries: number): void {
        this.residentPlayerEntries = Math.max(0, entries | 0);
    }

    recordResidentActorHit(): void {
        this.residentActorHits++;
    }

    recordResidentActorMiss(): void {
        this.residentActorMisses++;
    }

    recordResidentActorEviction(): void {
        this.residentActorEvictions++;
    }

    setResidentActorEntries(entries: number): void {
        this.residentActorEntries = Math.max(0, entries | 0);
    }

    snapshot(): RuntimePerfSnapshot {
        return {
            workerCount: this.workerCount,
            sharedSparseCache: this.sharedSparseCache,
            crossOriginIsolated: globalThis.crossOriginIsolated === true,
            cacheAdvertisedBytes: this.cacheAdvertisedBytes,
            js5GroupRequests: this.js5GroupRequests,
            js5StoreHits: this.js5StoreHits,
            js5PendingDedupHits: this.js5PendingDedupHits,
            js5HttpBatches: this.js5HttpBatches,
            js5HttpBytes: this.js5HttpBytes,
            js5ChainFetches: this.js5ChainFetches,
            rustUploadCalls: { ...this.rustUploadCalls },
            rustUploadBytes: { ...this.rustUploadBytes },
            residentPlayerHits: this.residentPlayerHits,
            residentPlayerMisses: this.residentPlayerMisses,
            residentPlayerEvictions: this.residentPlayerEvictions,
            residentPlayerEntries: this.residentPlayerEntries,
            residentActorHits: this.residentActorHits,
            residentActorMisses: this.residentActorMisses,
            residentActorEvictions: this.residentActorEvictions,
            residentActorEntries: this.residentActorEntries,
        };
    }

    reset(): void {
        this.js5GroupRequests = 0;
        this.js5StoreHits = 0;
        this.js5PendingDedupHits = 0;
        this.js5HttpBatches = 0;
        this.js5HttpBytes = 0;
        this.js5ChainFetches = 0;
        this.rustUploadCalls = emptyNumberRecord();
        this.rustUploadBytes = emptyNumberRecord();
        this.residentPlayerHits = 0;
        this.residentPlayerMisses = 0;
        this.residentPlayerEvictions = 0;
        this.residentActorHits = 0;
        this.residentActorMisses = 0;
        this.residentActorEvictions = 0;
    }
}

export const runtimePerfCounters = new RuntimePerfCounters();

if (typeof window !== "undefined") {
    (window as any).runtimePerf = {
        snapshot: () => runtimePerfCounters.snapshot(),
        reset: () => runtimePerfCounters.reset(),
    };
}
