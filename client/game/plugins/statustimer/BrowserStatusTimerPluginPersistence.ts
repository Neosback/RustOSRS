import type { StatusTimerPluginConfig, StatusTimerPluginPersistence } from "./StatusTimerPlugin";

export function createBrowserStatusTimerPluginPersistence(storageKey: string): StatusTimerPluginPersistence | undefined {
    if (typeof window === "undefined" || typeof window.localStorage === "undefined") return undefined;
    return {
        load: () => {
            try {
                const raw = window.localStorage.getItem(storageKey);
                return raw ? (JSON.parse(raw) as Partial<StatusTimerPluginConfig>) : undefined;
            } catch {
                return undefined;
            }
        },
        save: (config) => {
            try {
                window.localStorage.setItem(storageKey, JSON.stringify(config));
            } catch {}
        },
    };
}
