import type { TradeServerPayload } from "../types";
import { cloneTradeState, createDefaultTradeState } from "./defaults";
import { state } from "../state";
import { sanitizeTradePartyMessage } from "../utils/sanitize";

export { createDefaultTradeState, cloneTradeState };

export function handleTradePayload(payload: TradeServerPayload | undefined): void {
    if (!payload) return;
    if (payload.kind === "request") {
        state.lastTradeState = {
            ...createDefaultTradeState(),
            requestFrom: { playerId: payload.fromId | 0, name: payload.fromName },
        };
    } else if (payload.kind === "close") {
        state.lastTradeState = createDefaultTradeState();
        if (payload.reason) {
            state.lastTradeState.infoMessage = String(payload.reason);
        }
    } else if (payload.kind === "open" || payload.kind === "update") {
        const stage = payload.stage === "confirm" ? "confirm" : "offer";
        state.lastTradeState = {
            open: true,
            sessionId: payload.sessionId,
            stage,
            self: sanitizeTradePartyMessage(payload.self),
            other: sanitizeTradePartyMessage(payload.other),
            infoMessage: payload.info ? String(payload.info) : undefined,
            requestFrom: undefined,
        };
    }
    const snapshot = cloneTradeState(state.lastTradeState);
    for (const listener of state.tradeListeners) {
        try {
            listener(snapshot);
        } catch (err) {
            console.warn("trade listener error", err);
        }
    }
}
