import type { CacheInfo } from "../../rs/cache/CacheInfo";
import type { VarManager } from "../../rs/config/vartype/VarManager";
import {
    getBrowserVarcsStorageKey,
    loadBrowserVarcs,
    saveBrowserVarcs,
} from "../BrowserVarcsPersistence";

export type VarcPersistenceDeps = {
    getVarManager: () => VarManager | undefined;
};

/**
 * Debounced browser persistence for persistent varcs (localStorage).
 */
export class VarcPersistence {
    private storageKey?: string;
    private unwrittenChanges = false;
    private lastWriteTimeMs = 0;
    private readonly handlePageLifecycleFlush = (): void => {
        this.writeVarcs();
    };

    constructor(private readonly deps: VarcPersistenceDeps) {}

    bindPageLifecycle(): void {
        if (typeof window !== "undefined" && typeof window.addEventListener === "function") {
            window.addEventListener("pagehide", this.handlePageLifecycleFlush);
            window.addEventListener("beforeunload", this.handlePageLifecycleFlush);
        }
    }

    unbindPageLifecycle(): void {
        if (typeof window !== "undefined" && typeof window.removeEventListener === "function") {
            window.removeEventListener("pagehide", this.handlePageLifecycleFlush);
            window.removeEventListener("beforeunload", this.handlePageLifecycleFlush);
        }
    }

    initStorageKey(cacheInfo: CacheInfo): string {
        this.storageKey = getBrowserVarcsStorageKey(cacheInfo);
        return this.storageKey;
    }

    restoreFromBrowser(): void {
        if (!this.storageKey) return;
        this.deps.getVarManager()?.restorePersistentVarcs(loadBrowserVarcs(this.storageKey));
    }

    getStorageKey(): string | undefined {
        return this.storageKey;
    }

    markVarcsChanged(): void {
        if (!this.storageKey || !this.deps.getVarManager()) {
            return;
        }
        this.unwrittenChanges = true;
    }

    writeVarcs(): void {
        const varManager = this.deps.getVarManager();
        if (!this.unwrittenChanges || !this.storageKey || !varManager) {
            return;
        }
        saveBrowserVarcs(this.storageKey, varManager.snapshotPersistentVarcs());
        this.unwrittenChanges = false;
        this.lastWriteTimeMs = Date.now();
    }

    tryWriteVarcs(): void {
        if (!this.unwrittenChanges) {
            return;
        }
        const now = Date.now();
        if (this.lastWriteTimeMs < now - 60000) {
            this.writeVarcs();
        }
    }

    resetWriteTracking(): void {
        this.unwrittenChanges = false;
        this.lastWriteTimeMs = 0;
    }

    dispose(): void {
        this.writeVarcs();
        this.unbindPageLifecycle();
        this.storageKey = undefined;
        this.unwrittenChanges = false;
        this.lastWriteTimeMs = 0;
    }
}
