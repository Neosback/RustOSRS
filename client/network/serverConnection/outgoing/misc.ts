import { send } from "../connection/send";
import { state } from "../state";

export function sendSmithingMake(recipeId: string, mode: "smelt" | "forge"): void {
    if (!state.socket || state.socket.readyState !== WebSocket.OPEN) return;
    if (typeof recipeId !== "string" || recipeId.length === 0) return;
    send({ type: "smithing_make", payload: { recipeId, mode } } as any);
}

export function sendSmithingSetMode(mode: number, customAmount?: number): void {
    if (!state.socket || state.socket.readyState !== WebSocket.OPEN) return;
    if (!Number.isFinite(mode)) return;
    const payload: { mode: number; custom?: number } = { mode: Math.max(0, Math.min(4, mode | 0)) };
    if (Number.isFinite(customAmount) && (customAmount as number) > 0) {
        payload.custom = Math.max(1, Math.min(2147483647, (customAmount as number) | 0));
    }
    send({ type: "smithing_mode", payload } as any);
}

// Dev-only helper to request projectile snapshot from renderer clients
export function requestProjectileDebugSnapshot(
    requestId: number = Math.floor(Math.random() * 1e9),
): void {
    if (!state.socket || state.socket.readyState !== WebSocket.OPEN) return;
    send({ type: "debug", payload: { kind: "projectiles_request", requestId } } as any);
}

export function sendFaceRot(rot: number): void {
    if (!state.socket || state.socket.readyState !== WebSocket.OPEN) return;
    send({ type: "face", payload: { rot: rot | 0 } } as any);
}
export function sendFaceTile(tile: { x: number; y: number }): void {
    if (!state.socket || state.socket.readyState !== WebSocket.OPEN) return;
    send({ type: "face", payload: { tile: { x: tile.x | 0, y: tile.y | 0 } } } as any);
}

export function sendEmote(index: number, loop: boolean = false): void {
    if (!state.socket || state.socket.readyState !== WebSocket.OPEN) return;
    const idx = Math.max(0, index | 0);
    send({ type: "emote", payload: { index: idx, loop: !!loop } } as any);
}
