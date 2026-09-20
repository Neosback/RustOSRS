import type { WidgetNode } from "../../../widgets/WidgetNode";
type Widget = WidgetNode;
type WidgetClickMeta = {
    widget: Widget;
    option: string;
    target?: string;
    hasDropAction: boolean;
    itemId?: number;
    slot?: number;
};

// PERF: Cached click target structure to avoid object allocation per widget per frame
type CachedClickTarget = {
    id: string;
    rect: { x: number; y: number; w: number; h: number };
    priority: number;
    hoverText?: string;
    primaryOption?: { option: string; target?: string };
    /**
     * number of minimenu options for this hover target (including Cancel).
     * Used by CS2 minimenu_* opcodes via ClientOps snapshot logic.
     */
    menuOptionsCount?: number;
    widgetUid?: number; // For OSRS-style visibility filtering during hit testing
    onDown?: (x?: number, y?: number, targetId?: string) => void;
    onClick?: (x?: number, y?: number, targetId?: string) => void;
    persist?: boolean; // If true, survives beginFrame() clearing
};
export type { WidgetClickMeta, CachedClickTarget };
