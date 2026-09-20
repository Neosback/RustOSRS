import { LOGIN_LAYOUT } from "./constants";
import type { LoginRendererHost } from "./host";
import { getLogicalFirePositions } from "./layout/geometry";
import { updateLayout } from "./layout/updateLayout";

export function getCanvas(host: LoginRendererHost, width: number, height: number) {

        if (!host.canvas) {
            host.canvas = document.createElement("canvas");
        }
        if (host.canvas.width !== width || host.canvas.height !== height) {
            host.canvas.width = width;
            host.canvas.height = height;
            host.ctx = host.canvas.getContext("2d", { willReadFrequently: true }) || undefined;
        }
        return host.canvas;
    
}

export function getContext(host: LoginRendererHost) {

        return host.ctx;
    
}

export function tick(host: LoginRendererHost) {

        host.cycle++;

        // Time-based caret blink
        const now = performance.now();
        if (host.lastTickTime === 0) {
            host.lastTickTime = now;
        }
        const elapsed = now - host.lastTickTime;
        host.lastTickTime = now;
        host.caretBlinkMs =
            (host.caretBlinkMs + elapsed) % (LOGIN_LAYOUT.CARET_BLINK_INTERVAL_MS * 2);
    
}

export function isCaretVisible(host: LoginRendererHost) {

        return host.caretBlinkMs < LOGIN_LAYOUT.CARET_BLINK_INTERVAL_MS;
    
}

export function getFireAnimation(host: LoginRendererHost) {

        return host.loginScreenRunesAnimation;
    
}

export function getFirePositions(host: LoginRendererHost) {

        const logical = getLogicalFirePositions(host);
        const scale = host.renderScale;
        const leftX = logical.leftX * scale + host.renderOffsetX;
        const rightX = logical.rightX * scale + host.renderOffsetX;
        const y = logical.y * scale + host.renderOffsetY;
        return { leftX, rightX, y, scale: logical.scale * scale };
    
}

export function initCanvas(host: LoginRendererHost): void {
    updateLayout(host, host.SCENE_WIDTH, host.SCENE_HEIGHT);
}
