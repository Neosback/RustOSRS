import { SparseMemoryStore } from "../store/SparseMemoryStore";

export type Js5CoordinatorMessage = {
    type: "request" | "complete" | "failed";
    indexId: number;
    archiveId: number;
    error?: string;
};

export function isJs5CoordinatorMessage(value: unknown): value is Js5CoordinatorMessage {
    const message = value as Partial<Js5CoordinatorMessage> | undefined;
    return !!message &&
        (message.type === "request" || message.type === "complete" || message.type === "failed") &&
        Number.isInteger(message.indexId) && Number.isInteger(message.archiveId);
}

type WorkerPendingGroup = {
    indexId: number;
    archiveId: number;
    attempts: number;
    retryTimer?: ReturnType<typeof setTimeout>;
    promise: Promise<void>;
    resolve: () => void;
    reject: (error: Error) => void;
};

/** Worker-side proxy for the one JS5 queue owned by the main thread. */
export class WorkerJs5Coordinator {
    private readonly pending = new Map<string, WorkerPendingGroup>();
    private readonly channel: BroadcastChannel;

    constructor(
        readonly store: SparseMemoryStore,
        channelName: string,
        private readonly requestRetryMs: number = 1000,
        private readonly maxRequestAttempts: number = 3,
    ) {
        this.channel = new BroadcastChannel(channelName);
        this.channel.onmessage = ({ data }: MessageEvent<unknown>) => {
            if (!isJs5CoordinatorMessage(data) || data.type === "request") return;
            const key = this.groupKey(data.indexId, data.archiveId);
            const pending = this.pending.get(key);
            if (!pending) return;

            if (data.type === "failed") {
                this.failPending(
                    key,
                    pending,
                    new Error(data.error ?? "central JS5 fetch failed"),
                );
                return;
            }

            if (this.store.isGroupPresent(data.indexId, data.archiveId)) {
                this.completePending(key, pending);
            }
            // If the completion races shared-presence visibility, keep the
            // bounded retry timer alive. The next request is deduped by the
            // main-thread Js5RangeClient and cannot create duplicate downloads.
        };
        store.onMiss = (span) => {
            void this.requestGroup(span.indexId, span.archiveId);
        };
    }

    requestGroup(indexId: number, archiveId: number): Promise<void> {
        if (this.store.isGroupPresent(indexId, archiveId)) return Promise.resolve();

        const key = this.groupKey(indexId, archiveId);
        const existing = this.pending.get(key);
        if (existing) return existing.promise;

        let resolvePending!: () => void;
        let rejectPending!: (error: Error) => void;
        const promise = new Promise<void>((resolve, reject) => {
            resolvePending = resolve;
            rejectPending = reject;
        });
        promise.catch(() => {});

        const pending: WorkerPendingGroup = {
            indexId,
            archiveId,
            attempts: 0,
            promise,
            resolve: resolvePending,
            reject: rejectPending,
        };
        this.pending.set(key, pending);
        this.postRequest(key, pending);
        return promise;
    }

    async settled(): Promise<void> {
        while (this.pending.size > 0) {
            const snapshot = Array.from(this.pending.values(), (pending) => pending.promise);
            await Promise.allSettled(snapshot);
        }
    }

    close(): void {
        const error = new Error("JS5 worker coordinator closed");
        for (const [key, pending] of this.pending) {
            this.failPending(key, pending, error);
        }
        this.channel.close();
    }

    private groupKey(indexId: number, archiveId: number): string {
        return `${indexId}:${archiveId}`;
    }

    private postRequest(key: string, pending: WorkerPendingGroup): void {
        if (this.pending.get(key) !== pending) return;

        if (this.store.isGroupPresent(pending.indexId, pending.archiveId)) {
            this.completePending(key, pending);
            return;
        }

        const maxAttempts = Math.max(1, this.maxRequestAttempts | 0);
        if (pending.attempts >= maxAttempts) {
            this.failPending(
                key,
                pending,
                new Error(
                    `central JS5 fetch timed out for ${key} after ${pending.attempts} attempts`,
                ),
            );
            return;
        }

        pending.attempts++;
        if (pending.retryTimer !== undefined) clearTimeout(pending.retryTimer);
        pending.retryTimer = setTimeout(() => {
            pending.retryTimer = undefined;
            this.postRequest(key, pending);
        }, Math.max(1, this.requestRetryMs | 0));

        this.channel.postMessage({
            type: "request",
            indexId: pending.indexId,
            archiveId: pending.archiveId,
        } satisfies Js5CoordinatorMessage);
    }

    private completePending(key: string, pending: WorkerPendingGroup): void {
        if (this.pending.get(key) !== pending) return;
        this.pending.delete(key);
        if (pending.retryTimer !== undefined) clearTimeout(pending.retryTimer);
        pending.retryTimer = undefined;
        pending.resolve();
    }

    private failPending(key: string, pending: WorkerPendingGroup, error: Error): void {
        if (this.pending.get(key) !== pending) return;
        this.pending.delete(key);
        if (pending.retryTimer !== undefined) clearTimeout(pending.retryTimer);
        pending.retryTimer = undefined;
        pending.reject(error);
    }
}
