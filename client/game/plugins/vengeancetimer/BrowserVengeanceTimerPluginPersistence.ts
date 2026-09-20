import type {
    VengeanceTimerPluginConfig,
    VengeanceTimerPluginPersistence,
} from "./types";

export function createBrowserVengeanceTimerPluginPersistence(
    storageKey: string,
): VengeanceTimerPluginPersistence | undefined {
    if (typeof window === "undefined" || typeof window.localStorage === "undefined") {
        return undefined;
    }

    return {
        load: (): Partial<VengeanceTimerPluginConfig> | undefined => {
            try {
                const raw = window.localStorage.getItem(storageKey);
                return raw ? (JSON.parse(raw) as Partial<VengeanceTimerPluginConfig>) : undefined;
            } catch {
                return undefined;
            }
        },
        save: (config: VengeanceTimerPluginConfig): void => {
            try {
                window.localStorage.setItem(storageKey, JSON.stringify(config));
            } catch {}
        },
    };
}
