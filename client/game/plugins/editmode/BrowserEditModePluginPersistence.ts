import type { EditModePluginConfig, EditModePluginPersistence } from "./types";

export function createBrowserEditModePluginPersistence(
    storageKey: string,
): EditModePluginPersistence | undefined {
    if (typeof window === "undefined" || typeof window.localStorage === "undefined") {
        return undefined;
    }

    return {
        load: (): Partial<EditModePluginConfig> | undefined => {
            try {
                const raw = window.localStorage.getItem(storageKey);
                return raw
                    ? { ...(JSON.parse(raw) as Partial<EditModePluginConfig>), edits: [] }
                    : undefined;
            } catch {
                return undefined;
            }
        },
        save: (config: EditModePluginConfig): void => {
            try {
                window.localStorage.setItem(storageKey, JSON.stringify({ ...config, edits: [] }));
            } catch {}
        },
    };
}
