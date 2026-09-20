import { state } from "../state";

export function subscribeDisconnect(
    cb: (evt: { code: number; reason: string; willReconnect: boolean }) => void,
): () => void {
    state.disconnectListeners.add(cb);
    return () => state.disconnectListeners.delete(cb);
}

export function subscribeReconnectFailed(cb: () => void): () => void {
    state.reconnectFailedListeners.add(cb);
    return () => state.reconnectFailedListeners.delete(cb);
}
