import type {
    VengeanceTimerPluginConfig,
    VengeanceTimerPluginPersistence,
    VengeanceTimerPluginState,
} from "./types";

type VengeanceTimerPluginListener = () => void;

export const VENGEANCE_TIME_LIMIT_VARBIT = 2451;
export const VENGEANCE_COOLDOWN_MS = 30_000;

const DEFAULT_CONFIG: VengeanceTimerPluginConfig = Object.freeze({
    enabled: true,
});

export class VengeanceTimerPlugin {
    private readonly listeners = new Set<VengeanceTimerPluginListener>();
    private readonly persistence?: VengeanceTimerPluginPersistence;
    private config: VengeanceTimerPluginConfig;
    private cooldownActive = false;
    private cooldownEndsAt: number | null = null;
    private state: VengeanceTimerPluginState;
    private version = 0;

    constructor(persistence?: VengeanceTimerPluginPersistence) {
        this.persistence = persistence;
        this.config = this.sanitizeConfig(persistence?.load());
        this.state = this.createState();
    }

    subscribe(listener: VengeanceTimerPluginListener): () => void {
        this.listeners.add(listener);
        return () => this.listeners.delete(listener);
    }

    getState(): VengeanceTimerPluginState {
        return this.state;
    }

    setConfig(nextConfig: Partial<VengeanceTimerPluginConfig>): void {
        this.config = this.sanitizeConfig({ ...this.config, ...nextConfig });
        this.commit(true);
    }

    syncCooldownVarbit(value: number, now = Date.now()): void {
        const active = value !== 0;
        if (active === this.cooldownActive) {
            return;
        }

        this.cooldownActive = active;
        this.cooldownEndsAt = active ? now + VENGEANCE_COOLDOWN_MS : null;
        this.commit(false);
    }

    getRemainingSeconds(now = Date.now()): number {
        if (this.cooldownEndsAt === null) {
            return 0;
        }
        return Math.max(0, Math.ceil((this.cooldownEndsAt - now) / 1000));
    }

    private sanitizeConfig(
        input: Partial<VengeanceTimerPluginConfig> | undefined,
    ): VengeanceTimerPluginConfig {
        return {
            enabled: input?.enabled ?? DEFAULT_CONFIG.enabled,
        };
    }

    private createState(): VengeanceTimerPluginState {
        return {
            config: this.config,
            cooldownEndsAt: this.cooldownEndsAt,
            version: this.version,
        };
    }

    private commit(persist: boolean): void {
        this.version++;
        this.state = this.createState();
        if (persist) {
            this.persistence?.save(this.config);
        }
        for (const listener of this.listeners) {
            try {
                listener();
            } catch (err) {
                console.log("[vengeance-timer-plugin] listener failed", err);
            }
        }
    }
}
