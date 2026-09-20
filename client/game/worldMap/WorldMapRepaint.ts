import type { WorldMapStateHolder } from "./WorldMapTypes";

export function findWorldMapWidgetForRepaint(
    state: WorldMapStateHolder,
    getWidgetManager: () => any,
): any | undefined {
    const manager = getWidgetManager();
    if (!manager) return undefined;

    if (state.worldMapWidgetUid !== -1) {
        const cached = manager.getWidgetByUid?.(state.worldMapWidgetUid);
        if (cached && (((cached as any).contentType ?? 0) | 0) === 1400) {
            return cached;
        }
        state.worldMapWidgetUid = -1;
    }

    const scanGroup = (groupId: number): any | undefined => {
        if (groupId < 0 || typeof manager.getWidgetsForGroup !== "function") return undefined;
        const widgets = manager.getWidgetsForGroup(groupId | 0);
        for (const widget of widgets) {
            if ((((widget as any)?.contentType ?? 0) | 0) === 1400) {
                state.worldMapWidgetUid = ((widget as any).uid ?? -1) | 0;
                return widget;
            }
        }
        return undefined;
    };

    const rootWidget = scanGroup(manager.rootInterface ?? -1);
    if (rootWidget) return rootWidget;

    for (const parent of manager.interfaceParents?.values?.() ?? []) {
        const widget = scanGroup((parent as any)?.group ?? -1);
        if (widget) return widget;
    }
    return undefined;
}

export function scheduleWorldMapImageRepaint(
    state: WorldMapStateHolder,
    getWidgetManager: () => any,
): void {
    if (state.worldMapImageRepaintQueued) return;
    state.worldMapImageRepaintQueued = true;
    const repaint = () => {
        state.worldMapImageRepaintQueued = false;
        const widget = findWorldMapWidgetForRepaint(state, getWidgetManager);
        const manager = getWidgetManager() as any;
        if (widget) {
            if (typeof manager?.invalidateWidgetRect === "function") {
                manager.invalidateWidgetRect(widget, "worldmap-tile");
            } else {
                manager?.invalidateWidgetRender?.(widget, "worldmap-tile");
            }
        } else {
            manager?.invalidateAll?.();
        }
    };
    if (typeof requestAnimationFrame === "function") {
        requestAnimationFrame(repaint);
    } else {
        setTimeout(repaint, 0);
    }
}
