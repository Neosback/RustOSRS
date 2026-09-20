import { send } from "../connection/send";
import { state } from "../state";

export async function requestPath(
    from: { x: number; y: number; plane: number },
    to: { x: number; y: number },
    size: number = 1,
    timeoutMs: number = 3000,
): Promise<{ ok: boolean; waypoints?: { x: number; y: number }[]; message?: string }> {
    if (!state.socket || state.socket.readyState !== WebSocket.OPEN) {
        return { ok: false, message: "ws not connected" };
    }
    const id = state.nextReqId++;
    const payload = { id, from, to, size } as any;
    const p = new Promise<{
        ok: boolean;
        waypoints?: { x: number; y: number }[];
        message?: string;
    }>((resolve) => {
        state.pending.set(id, resolve);
    });
    send({ type: "pathfind", payload });
    const toPromise = new Promise<{
        ok: boolean;
        waypoints?: { x: number; y: number }[];
        message?: string;
    }>((resolve) => {
        const t = setTimeout(() => {
            if (state.pending.has(id)) {
                state.pending.delete(id);
                resolve({ ok: false, message: "timeout" });
            }
        }, timeoutMs);
        p.then((r) => {
            clearTimeout(t);
            resolve(r);
        });
    });
    return toPromise;
}
