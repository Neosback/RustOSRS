import type { LoginRendererHost, RenderContext } from "../host";
import { getTitleBackgroundLayout } from "../layout/geometry";

export function drawTitleBackgroundToCtx(host: LoginRendererHost, ctx: RenderContext) {

        if (!host.titleBackgroundImage) return;

        const bg = getTitleBackgroundLayout(host);
        ctx.drawImage(
            host.titleBackgroundImage,
            bg.drawX,
            bg.drawY,
            bg.drawW,
            bg.drawH,
        );
    
}
