import type { CollectionLogServerPayload, InventoryServerUpdate } from "../types";
import { state } from "../state";

export function emitInventory(update: InventoryServerUpdate): void {
    if (update.kind === "snapshot") {
        state.lastInventorySnapshot = update.slots.map((slot) => ({ ...slot }));
    } else if (update.kind === "slot") {
        if (!state.lastInventorySnapshot) state.lastInventorySnapshot = [];
        const idx = state.lastInventorySnapshot.findIndex(
            (s) => (s.slot | 0) === (update.slot.slot | 0),
        );
        if (idx >= 0) state.lastInventorySnapshot[idx] = { ...update.slot };
        else state.lastInventorySnapshot.push({ ...update.slot });
    }

    for (const listener of state.inventoryListeners) {
        try {
            if (update.kind === "snapshot") {
                listener({ kind: "snapshot", slots: update.slots.map((slot) => ({ ...slot })) });
            } else if (update.kind === "slot") {
                listener({ kind: "slot", slot: { ...update.slot } });
            }
        } catch (err) {
            console.warn("inventory listener error", err);
        }
    }
}

export function emitCollectionLog(update: CollectionLogServerPayload): void {
    state.lastCollectionLogSnapshot = update.slots.map((slot) => ({ ...slot }));

    for (const listener of state.collectionLogListeners) {
        try {
            listener({ kind: "snapshot", slots: update.slots.map((slot) => ({ ...slot })) });
        } catch (err) {
            console.warn("collection log listener error", err);
        }
    }
}
