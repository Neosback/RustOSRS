import type { CSSProperties, ReactNode } from "react";

export type HudDebugPanelPosition = "left-bottom" | "right-bottom";

export type HudDebugPanelProps = {
    position: HudDebugPanelPosition;
    children: ReactNode;
    minWidth?: number;
    textAlign?: CSSProperties["textAlign"];
};

/**
 * Shared HUD chrome for debug overlays (stats, tick counter, etc.).
 */
export function HudDebugPanel({
    position,
    children,
    minWidth = 220,
    textAlign,
}: HudDebugPanelProps): JSX.Element {
    return (
        <div className={`hud ${position}`}>
            <div
                className="content-text"
                style={{
                    background: "rgba(0,0,0,0.45)",
                    padding: "6px 8px",
                    borderRadius: 4,
                    lineHeight: 1.4,
                    fontSize: 12,
                    color: "#fff",
                    minWidth,
                    textAlign,
                    pointerEvents: "none",
                }}
            >
                {children}
            </div>
        </div>
    );
}
