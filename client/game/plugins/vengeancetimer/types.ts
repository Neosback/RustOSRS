export interface VengeanceTimerPluginConfig {
    enabled: boolean;
}

export interface VengeanceTimerPluginState {
    config: VengeanceTimerPluginConfig;
    cooldownEndsAt: number | null;
    version: number;
}

export interface VengeanceTimerPluginPersistence {
    load(): Partial<VengeanceTimerPluginConfig> | undefined;
    save(config: VengeanceTimerPluginConfig): void;
}
