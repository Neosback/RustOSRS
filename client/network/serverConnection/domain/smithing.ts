import type { SmithingServerPayload } from "../types";
import { cloneSmithingState, createDefaultSmithingState } from "./defaults";
import { state } from "../state";
import { sanitizeSmithingOption } from "../utils/sanitize";

export { createDefaultSmithingState, cloneSmithingState };

function clampSmithingMode(mode?: number): number {
    if (!Number.isFinite(mode)) return 0;
    return Math.max(0, Math.min(4, (mode as number) | 0));
}

function normalizeSmithingCustom(value?: number): number {
    if (!Number.isFinite(value) || (value as number) <= 0) return 0;
    return Math.max(0, Math.min(2147483647, (value as number) | 0));
}

export function handleSmithingPayload(payload: SmithingServerPayload | undefined): void {
    if (!payload) return;
    if (payload.kind === "open" || payload.kind === "update") {
        const options = Array.isArray(payload.options)
            ? payload.options.map((entry, idx) => sanitizeSmithingOption(entry, idx))
            : [];
        state.lastSmithingState = {
            open: payload.kind === "open" ? true : state.lastSmithingState.open,
            mode: payload.mode,
            title: payload.title ?? (payload.mode === "forge" ? "Smithing" : "Smelting"),
            options,
            quantityMode: clampSmithingMode(payload.quantityMode),
            customQuantity: normalizeSmithingCustom(payload.customQuantity),
        };
    } else if (payload.kind === "mode") {
        state.lastSmithingState.quantityMode = clampSmithingMode(payload.quantityMode);
        const custom = normalizeSmithingCustom(payload.customQuantity);
        if (custom > 0 || payload.kind === "mode") {
            state.lastSmithingState.customQuantity = custom;
        }
    } else if (payload.kind === "close") {
        state.lastSmithingState = createDefaultSmithingState();
    }
    const snapshot = cloneSmithingState(state.lastSmithingState);
    for (const listener of state.smithingListeners) {
        try {
            listener(snapshot);
        } catch (err) {
            console.warn("smelting listener error", err);
        }
    }
}
