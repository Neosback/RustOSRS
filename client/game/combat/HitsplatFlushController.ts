import type { HitsplatServerPayload } from "../../network/ServerConnection";
import type { GameRenderer } from "../GameRenderer";

export type HitsplatFlushControllerDeps = {
    getRenderer: () => GameRenderer | undefined;
};

type HealthBarEntry = { serverId: number; bar: any };

/**
 * Buffers hitsplat and health-bar updates until the renderer is ready, then flushes them.
 */
export class HitsplatFlushController {
    private pendingHitsplats: HitsplatServerPayload[] = [];
    private pendingPlayerHealthBars: HealthBarEntry[] = [];
    private pendingNpcHealthBars: HealthBarEntry[] = [];

    constructor(private readonly deps: HitsplatFlushControllerDeps) {}

    queueHitsplat(payload: HitsplatServerPayload): void {
        this.pendingHitsplats.push(payload);
    }

    queuePlayerHealthBar(payload: HealthBarEntry): void {
        this.pendingPlayerHealthBars.push(payload);
    }

    queueNpcHealthBar(entry: HealthBarEntry): void {
        this.pendingNpcHealthBars.push(entry);
    }

    flushPendingHitsplats(): void {
        const renderer = this.deps.getRenderer();
        if (!renderer || this.pendingHitsplats.length === 0) return;
        const pending = this.pendingHitsplats.splice(0, this.pendingHitsplats.length);
        for (const event of pending) {
            try {
                renderer.registerHitsplat(event);
            } catch (err) {
                console.warn("[OsrsClient] registerHitsplat failed", err);
            }
        }
    }

    flushPendingPlayerHealthBars(): void {
        const renderer = this.deps.getRenderer() as any;
        if (!renderer || this.pendingPlayerHealthBars.length === 0) return;
        const pending = this.pendingPlayerHealthBars.splice(0, this.pendingPlayerHealthBars.length);
        for (const entry of pending) {
            try {
                renderer.registerPlayerHealthBarUpdate?.(entry);
            } catch (err) {
                console.warn("[OsrsClient] registerPlayerHealthBarUpdate failed", err);
            }
        }
    }

    flushPendingNpcHealthBars(): void {
        const renderer = this.deps.getRenderer() as any;
        if (!renderer || this.pendingNpcHealthBars.length === 0) return;
        const pending = this.pendingNpcHealthBars.splice(0, this.pendingNpcHealthBars.length);
        for (const entry of pending) {
            try {
                renderer.registerNpcHealthBarUpdate?.(entry);
            } catch (err) {
                console.warn("[OsrsClient] registerNpcHealthBarUpdate failed", err);
            }
        }
    }

    flushAll(): void {
        this.flushPendingHitsplats();
        this.flushPendingPlayerHealthBars();
        this.flushPendingNpcHealthBars();
    }
}
