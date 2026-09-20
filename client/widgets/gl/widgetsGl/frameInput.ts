import type { InputManager } from "../../../game/InputManager";
import { GLRenderer } from "../renderer";
import { ensureInput } from "../ui-input";
import type { CachedClickTarget, WidgetClickMeta } from "./clickTypes";
export function beginWidgetUiFrame(glr: GLRenderer): void {
    try {
        const input = ensureInput(glr, () => {}, undefined);
        input.beginFrame();
    } catch {}
}

export function processWidgetUiInput(
    glr: GLRenderer,
    inputManager: InputManager | undefined,
): void {
    if (!inputManager) return;
    try {
        const input = ensureInput(glr, () => {}, undefined);
        const targetCount = input.getClicks().getTargetCount();
        if (targetCount > 0) {
            input.processInput(inputManager);
        }
    } catch (e) {
        console.error("[widgets-gl] processWidgetUiInput error:", e);
    }
}

export function detachGLUI(glr: GLRenderer) {
    const canvas = glr.canvas as any;
    try {
        if (typeof canvas.__detachUI === "function") canvas.__detachUI();
        if (canvas.__input) {
            try {
                canvas.__input.detach();
            } catch {}
            canvas.__input = undefined;
        }
        if (canvas.__scrolls) canvas.__scrolls = undefined;
        if (canvas.__clicks) canvas.__clicks = undefined;
    } catch {}
}

/**
 * Clean up click targets and metadata for a closed interface group.
 * Call this when an interface closes to prevent stale click targets from persisting.
 */
export function cleanupInterfaceClickTargets(canvas: HTMLCanvasElement, groupId: number): void {
    const canvasAny = canvas as any;

    // Clean clickTargetCache (cached CachedClickTarget objects)
    const clickTargetCache: Map<number, CachedClickTarget> | undefined =
        canvasAny.__clickTargetCache;
    if (clickTargetCache) {
        for (const uid of [...clickTargetCache.keys()]) {
            if (((uid >>> 16) & 0xffff) === groupId) {
                clickTargetCache.delete(uid);
            }
        }
    }

    // Clean clickMetaMap (widget click metadata)
    const clickMetaMap: Map<number, WidgetClickMeta> | undefined = canvasAny.__clickMetaMap;
    if (clickMetaMap) {
        for (const uid of [...clickMetaMap.keys()]) {
            if (((uid >>> 16) & 0xffff) === groupId) {
                clickMetaMap.delete(uid);
            }
        }
    }

    // Clean from ClickRegistry via unregisterWidgetGroup
    // Access clicks via __input.getClicks() first (UIInputBridge), then fallback to __clicks
    const clicks =
        (canvasAny.__input as any)?.getClicks?.() ||
        (canvasAny.__inputBridge as any)?.getClicks?.() ||
        canvasAny.__clicks;
    if (clicks?.unregisterWidgetGroup) {
        clicks.unregisterWidgetGroup(groupId);
    }
}
