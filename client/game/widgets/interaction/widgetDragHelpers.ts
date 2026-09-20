import { getDragDepth } from "../../../widgets/WidgetFlags";
import type { WidgetManager } from "../../../widgets/WidgetManager";

/** Check if a widget UID belongs to an inventory container (type 2). */
export function isInventoryContainer(
    widgetManager: WidgetManager | undefined,
    parentUid: number,
): boolean {
    const parent = widgetManager?.getWidgetByUid(parentUid);
    if (!parent) return false;
    return parent.type === 2 || (parent.itemId !== undefined && parent.itemId >= 0);
}

export function getDragParentDepth(widgetManager: WidgetManager | undefined, w: any): number {
    const flags = widgetManager?.getWidgetFlags?.(w) ?? w?.flags ?? 0;
    return getDragDepth(flags);
}

/** Resolve clickedWidgetParent via flag-based parent climbing. */
export function resolveDragParentByFlags(
    widgetManager: WidgetManager | undefined,
    w: any,
): any | null {
    const depth = getDragParentDepth(widgetManager, w);
    if (depth === 0) return null;
    let cur: any = w;
    for (let i = 0; i < depth; i++) {
        const parentUid = cur?.parentUid;
        if (typeof parentUid !== "number" || parentUid === -1) return null;
        cur = widgetManager?.getWidgetByUid(parentUid);
        if (!cur) return null;
    }
    return cur;
}

/**
 * clickedWidgetParent selection used for clamping and script coords.
 * Only returns a drag parent if explicitly set via flag bits or dragRenderArea.
 */
export function resolveClickedWidgetParent(
    widgetManager: WidgetManager | undefined,
    w: any,
): any | null {
    if (!w) return null;
    const byFlags = resolveDragParentByFlags(widgetManager, w);
    if (byFlags) return byFlags;
    if (w.dragRenderArea) return w.dragRenderArea;
    return null;
}

export function isWidgetDraggable(widgetManager: WidgetManager | undefined, w: any): boolean {
    if (getDragParentDepth(widgetManager, w) !== 0) return true;
    if (w.dragRenderArea) return true;
    if (w.isDraggable) return true;
    if (w.eventHandlers?.onDrag || w.onDrag) return true;
    return false;
}

export function clearDragWidgetVisualState(
    widgetManager: WidgetManager | undefined,
    widget: any,
): void {
    if (!widget) return;
    delete (widget as any)._dragPickupOffsetX;
    delete (widget as any)._dragPickupOffsetY;
    delete (widget as any)._dragVisualX;
    delete (widget as any)._dragVisualY;
    delete (widget as any)._dragAbsX;
    delete (widget as any)._dragAbsY;
    delete (widget as any)._isDragActive;
    try {
        widgetManager?.invalidateWidgetRender?.(widget);
    } catch {}
}
