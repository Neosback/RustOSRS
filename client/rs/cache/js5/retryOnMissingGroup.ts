import { isGroupMissingError } from "./GroupMissingError";

/**
 * Run a synchronous cache read, waiting for on-demand fetches to land when it
 * hits a not-yet-downloaded group. Each failed attempt re-queues the fetch
 * (deduped by the range client), so this simply polls until the data arrives.
 * Returns undefined if the data never arrives within the attempt budget.
 * Non-miss errors propagate unchanged.
 */
export async function retryOnMissingGroup<T>(
    read: () => T,
    attempts: number = 40,
    delayMs: number = 250,
): Promise<T | undefined> {
    for (let i = 0; i < attempts; i++) {
        try {
            return read();
        } catch (e) {
            if (!isGroupMissingError(e)) {
                throw e;
            }
            await new Promise((resolve) => setTimeout(resolve, delayMs));
        }
    }
    return undefined;
}
