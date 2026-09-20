import { state } from "../state";

export function clearLoginConnectRetryTimer(): void {
    if (!state.loginConnectRetryTimer) return;
    try {
        clearTimeout(state.loginConnectRetryTimer);
    } catch {}
    state.loginConnectRetryTimer = null;
}
