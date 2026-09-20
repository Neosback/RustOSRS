import { state } from "./state";

export function registerAnimDebugProvider(fn: (() => any) | null): void {
    state.animDebugProvider = fn;
}
