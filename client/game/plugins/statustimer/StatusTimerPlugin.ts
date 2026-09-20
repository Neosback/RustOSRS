export interface StatusTimerPluginConfig {
    enabled: boolean;
}

export interface StatusTimerPluginState {
    active: boolean;
    config: StatusTimerPluginConfig;
    endsAt: number | null;
    type: number;
    version: number;
}

export interface StatusTimerPluginPersistence {
    load(): Partial<StatusTimerPluginConfig> | undefined;
    save(config: StatusTimerPluginConfig): void;
}

type Listener = () => void;

export class StatusTimerPlugin {
    private readonly listeners = new Set<Listener>();
    private config: StatusTimerPluginConfig;
    private state: StatusTimerPluginState;

    constructor(private readonly persistence?: StatusTimerPluginPersistence) {
        this.config = { enabled: persistence?.load()?.enabled ?? true };
        this.state = { active: false, config: this.config, endsAt: null, type: 0, version: 0 };
    }

    subscribe(listener: Listener): () => void {
        this.listeners.add(listener);
        return () => this.listeners.delete(listener);
    }

    getState(): StatusTimerPluginState {
        return this.state;
    }

    setConfig(config: Partial<StatusTimerPluginConfig>): void {
        this.config = { enabled: config.enabled ?? this.config.enabled };
        this.commit(true, { ...this.state, config: this.config });
    }

    sync(seconds: number, type: number, now = Date.now()): void {
        if (seconds === 0) {
            if (this.state.active) this.commit(false, { ...this.state, active: false, endsAt: null, type: 0 });
            return;
        }

        if (seconds < 0) {
            if (!this.state.active || this.state.endsAt !== null || this.state.type !== type) {
                this.commit(false, { ...this.state, active: true, endsAt: null, type });
            }
            return;
        }

        const nextEndsAt = now + seconds * 1000;
        const shouldRestart = !this.state.active || this.state.type !== type || this.getRemainingSeconds(now) < seconds - 1;
        const endsAt = shouldRestart ? nextEndsAt : Math.min(this.state.endsAt ?? nextEndsAt, nextEndsAt);
        if (shouldRestart || this.state.endsAt !== endsAt) {
            this.commit(false, { ...this.state, active: true, endsAt, type });
        }
    }

    getRemainingSeconds(now = Date.now()): number {
        return this.state.endsAt === null ? 0 : Math.max(0, Math.ceil((this.state.endsAt - now) / 1000));
    }

    private commit(persist: boolean, state: StatusTimerPluginState): void {
        this.state = { ...state, version: state.version + 1 };
        if (persist) this.persistence?.save(this.config);
        for (const listener of this.listeners) listener();
    }
}
