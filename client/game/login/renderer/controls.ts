import type { LoginRendererHost, RenderContext } from "./host";
import { getTitleMuteDrawPosition } from "./layout/geometry";
import { drawSprite } from "./render/drawUtils";

export function isTitleMuteHit(host: LoginRendererHost, x: number, y: number) {

        const mutePos = getTitleMuteDrawPosition(host);
        // Keep the original 50x50 hit area, anchored to the actual draw position.
        return (
            x >= mutePos.x - 10 &&
            y >= mutePos.y - 10 &&
            x < mutePos.x + 40 &&
            y < mutePos.y + 40
        );
    
}

export function drawTitleMuteButton(host: LoginRendererHost, ctx: RenderContext, titleMusicDisabled: boolean) {

        if (!host.titleMuteSprites) {
            return;
        }

        const muteSprite = titleMusicDisabled ? host.titleMuteSprites[1] : host.titleMuteSprites[0];
        if (!muteSprite) {
            return;
        }

        const mutePos = getTitleMuteDrawPosition(host);
        drawSprite(host, ctx, muteSprite, mutePos.x, mutePos.y);
    
}
