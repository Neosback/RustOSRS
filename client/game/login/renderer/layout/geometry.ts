import { LOGIN_LAYOUT } from "../constants";
import type { LoginRendererHost, RenderContext } from "../host";

export function getTitleBackgroundLayout(host: LoginRendererHost) {

        const targetW = Math.max(1, host.containerWidth);
        const targetH = Math.max(1, host.containerHeight);
        const img = host.titleBackgroundImage;
        const srcW = img?.width || LOGIN_LAYOUT.TITLE_BG_WIDTH;
        const srcH = img?.height || LOGIN_LAYOUT.MAX_BG_HEIGHT;
        const scale = Math.max(targetW / srcW, targetH / srcH);
        const drawW = srcW * scale;
        const drawH = srcH * scale;
        return {
            drawX: host.containerX + (targetW - drawW) / 2,
            drawY: (targetH - drawH) / 2,
            drawW,
            drawH,
            scale,
        };
    
}

export function getLogicalFirePositions(host: LoginRendererHost) {

        // Classic 765-scene fire edges mapped into the 1089-wide title art:
        //   cropX = (1089 - 765) / 2 = 162
        //   left  = 162 + (-22) = 140
        //   right = 162 + (765 - 128 + 22) = 821
        // Track those art-space X values through the cover-fit background transform.
        const bg = getTitleBackgroundLayout(host);
        const artW = LOGIN_LAYOUT.TITLE_BG_WIDTH;
        const leftSrcX = LOGIN_LAYOUT.TITLE_BG_CROP_X - 22;
        const rightSrcX = LOGIN_LAYOUT.TITLE_BG_CROP_X + host.SCENE_WIDTH - 128 + 22;
        // Scale fire sprites with the background so bowl size matches the art.
        const fireScale = bg.drawW / artW;
        return {
            leftX: bg.drawX + (leftSrcX / artW) * bg.drawW,
            rightX: bg.drawX + (rightSrcX / artW) * bg.drawW,
            y: bg.drawY,
            scale: fireScale,
        };
    
}

export function getVisibleLayoutRightEdge(host: LoginRendererHost) {

        const scale = Math.max(host.renderScale, 0.0001);
        return (host.renderSurfaceWidth - host.renderOffsetX) / scale;
    
}

export function getBottomControlsY(host: LoginRendererHost) {

        return Math.max(8, host.canvasHeight - 40);
    
}

export function getServerListButtonPosition(host: LoginRendererHost) {

        return {
            x: host.containerX + 5,
            y: getBottomControlsY(host),
        };
    
}

export function getTitleMuteDrawPosition(host: LoginRendererHost) {

        // Anchor the music toggle to the visible right edge, same baseline as server button.
        const defaultRightEdge = host.containerX + host.containerWidth;
        const titleRightEdge = Math.min(defaultRightEdge, getVisibleLayoutRightEdge(host));
        return {
            x: Math.floor(titleRightEdge) - 40,
            y: getBottomControlsY(host),
        };
    
}

function getTitleBoxLayout(host: LoginRendererHost) {

        const width = host.titleboxSprite?.subWidth || host.TITLEBOX_FALLBACK_WIDTH;
        const height = host.titleboxSprite?.subHeight || host.TITLEBOX_FALLBACK_HEIGHT;
        const x = host.LOGIN_BOX_X;
        const y = host.TITLEBOX_Y;
        return {
            x,
            y,
            width,
            height,
            centerX: x + Math.floor(width / 2),
        };
    
}

export function getWelcomeLayout(host: LoginRendererHost) {

        const box = getTitleBoxLayout(host);
        return {
            centerX: box.centerX,
            titleY: box.y + Math.round(box.height * 0.405),
            buttonY: box.y + Math.round(box.height * 0.605),
            buttonSpacing: Math.max(60, Math.round(box.width * 0.222)),
        };
    
}

export function toContentPoint(host: LoginRendererHost, layoutX: number, layoutY: number) {

        const scale = host.contentScale > 0 ? host.contentScale : 1;
        return {
            x: (layoutX - host.contentOriginX) / scale,
            y: (layoutY - host.contentOriginY) / scale,
        };
    
}

export function mapPointerToContent(host: LoginRendererHost, layoutX: number, layoutY: number) {

        return toContentPoint(host, layoutX, layoutY);
    
}

export function withContentTransform(host: LoginRendererHost, ctx: RenderContext, drawFn: () => void): void {

        ctx.save();
        ctx.translate(host.contentOriginX, host.contentOriginY);
        if (host.contentScale !== 1) {
            ctx.scale(host.contentScale, host.contentScale);
        }
        try {
            drawFn();
        } finally {
            ctx.restore();
        }
    
}
