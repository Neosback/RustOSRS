/**
 * Holds widget updates that arrive for a server-defined interface before its definition
 * has finished loading.
 *
 * The server sends an interface's contents - text, items, hidden flags - in the same batch
 * as the open packet. A cache interface mounts synchronously so those land on widgets that
 * already exist; a server-defined one has to be fetched first, and anything that arrives in
 * the meantime would otherwise be applied to widgets that do not exist yet and be lost.
 */
export class PendingInterfaceUpdates {
    private readonly queues = new Map<number, unknown[]>();

    get size(): number {
        return this.queues.size;
    }

    /** Starts holding updates for a group whose definition is being fetched. */
    open(groupId: number): void {
        const id = groupId | 0;
        if (!this.queues.has(id)) {
            this.queues.set(id, []);
        }
    }

    /** True when the payload was held; false when it should be applied now. */
    defer(payload: unknown): boolean {
        if (this.queues.size === 0) {
            return false;
        }
        const event = payload as { action?: unknown; uid?: unknown; targetUid?: unknown };
        const uid = event.action === "open_sub" ? event.targetUid : event.uid;
        if (typeof uid !== "number") {
            return false;
        }
        const queued = this.queues.get((uid >>> 16) & 0xffff);
        if (!queued) {
            return false;
        }
        queued.push(payload);
        return true;
    }

    /** Applies everything held for a group, in arrival order, and stops holding. */
    flush(groupId: number, apply: (payload: unknown) => void): void {
        const id = groupId | 0;
        const queued = this.queues.get(id);
        this.queues.delete(id);
        for (const payload of queued ?? []) {
            apply(payload);
        }
    }

    /** Drops anything held for a group that will never mount. */
    cancel(groupId: number): void {
        this.queues.delete(groupId | 0);
    }
}
