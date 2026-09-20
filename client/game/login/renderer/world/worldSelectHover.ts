import type { LoginState } from "../../LoginState";
import type { LoginRendererHost, RenderContext } from "../host";
import { drawCenteredText, measureText } from "../render/drawUtils";
import { getGridLayout, getSortedWorlds, findHoveredWorld } from "./worldData";

export function drawWorldSelectHoverOnly(host: LoginRendererHost, ctx: RenderContext, state: LoginState, _width: number, _height: number) {

        if (!host.fontPlain11) return;

        const sortedWorlds = getSortedWorlds(host);
        const worldCount = sortedWorlds.length;

        // Use cached grid layout (consolidates duplicate calculation)
        const layout = getGridLayout(host, worldCount);
        const { rowWidth, rowHeight } = layout;

        // Use consolidated hover detection (eliminates duplicate loop)
        const hoverResult = findHoveredWorld(host, sortedWorlds, layout, state.worldSelectPage);
        state.hoveredWorldId = hoverResult.world ? hoverResult.world.id : -1;

        // Draw hover highlight
        if (hoverResult.world) {
            ctx.save();
            ctx.globalAlpha = 0.5;
            ctx.fillStyle = "#ffffff";
            ctx.fillRect(hoverResult.x, hoverResult.y, rowWidth, rowHeight);
            ctx.restore();
        }

        // Draw tooltip
        if (hoverResult.world) {
            const activity = hoverResult.world.activity || "-";
            const tooltipWidth = measureText(host, host.fontPlain11, activity) + 6;
            const tooltipHeight = host.fontPlain11.lineHeight + 8;
            let tooltipY = host.mouseY + 25;
            if (tooltipHeight + tooltipY > 480) {
                tooltipY = host.mouseY - 25 - tooltipHeight;
            }
            const tooltipX = host.mouseX - tooltipWidth / 2;

            ctx.fillStyle = "#ffff70";
            ctx.fillRect(tooltipX, tooltipY, tooltipWidth, tooltipHeight);
            ctx.strokeStyle = "#000000";
            ctx.lineWidth = 1;
            ctx.strokeRect(tooltipX, tooltipY, tooltipWidth, tooltipHeight);

            drawCenteredText(host, 
                ctx,
                host.fontPlain11,
                activity,
                host.mouseX,
                tooltipY + host.fontPlain11.lineHeight + 4,
                0x000000,
            );
        }
    
}
