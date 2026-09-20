import type { LoginRendererHost, RenderContext } from "../host";
import { drawSprite, drawText } from "../render/drawUtils";

export function drawSortColumn(host: LoginRendererHost, ctx: RenderContext, x: number, label: string, sortIndex: number) {

        if (!host.worldSelectArrowSprites || !host.fontBold12) return;

        // Draw up arrow (ascending)
        const upArrowIdx =
            host.worldSortOption === sortIndex && host.worldSortDirection === 0 ? 2 : 0;
        drawSprite(host, ctx, host.worldSelectArrowSprites[upArrowIdx], x, 4);

        // Draw down arrow (descending)
        const downArrowIdx =
            host.worldSortOption === sortIndex && host.worldSortDirection === 1 ? 3 : 1;
        drawSprite(host, ctx, host.worldSelectArrowSprites[downArrowIdx], x + 15, 4);

        // Draw label
        drawText(host, ctx, host.fontBold12, label, x + 32, 17, 0xffffff);
    
}
