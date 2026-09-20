import { GLRenderer } from "../renderer";
import type { WidgetNode } from "../../../widgets/WidgetNode";
import { ARC_FULL_RADIANS, ARC_TURN_UNITS } from "./constants";
import { scaleLogicalPixels } from "./scaleLogicalPixels";
type Widget = WidgetNode;
type ArcRenderCache = {
    canvas: HTMLCanvasElement;
    key: string;
    stateKey: string;
};

export function arcUnitToRadians(unit: number): number {
    return ((unit / ARC_TURN_UNITS) * ARC_FULL_RADIANS - Math.PI / 2) % ARC_FULL_RADIANS;
}

export function getClockwiseArcSpan(start: number, end: number): number {
    const rawSpan = end - start;
    if (rawSpan === 0) return 0;
    if (Math.abs(rawSpan) >= ARC_TURN_UNITS) return ARC_TURN_UNITS;
    return ((rawSpan % ARC_TURN_UNITS) + ARC_TURN_UNITS) % ARC_TURN_UNITS;
}

export function renderArcWidget(
    glr: GLRenderer,
    w: Widget,
    x: number,
    y: number,
    width: number,
    height: number,
    rootScaleX: number,
    rootScaleY: number,
): void {
    const canvasWidth = Math.max(1, Math.ceil(Math.abs(width)));
    const canvasHeight = Math.max(1, Math.ceil(Math.abs(height)));
    const start = (w.arcStart ?? 0) | 0;
    const end = (w.arcEnd ?? 0) | 0;
    const span = getClockwiseArcSpan(start, end);
    if (span <= 0) {
        return;
    }

    const trans = w.transparency ?? w.opacity ?? 0;
    if (trans >= 255) {
        return;
    }

    const color = (w.textColor ?? w.color ?? 0xffffff) | 0;
    const alpha = Math.max(0, Math.min(1, (255 - (trans & 255)) / 255));
    const r = (color >>> 16) & 0xff;
    const g = (color >>> 8) & 0xff;
    const b = color & 0xff;
    const filled = !!w.filled;
    const lineWidth = Math.max(
        1,
        Math.round(((w.lineWidth ?? 1) || 1) * Math.max(rootScaleX, rootScaleY)),
    );
    const stateKey = `${canvasWidth}:${canvasHeight}:${start}:${end}:${color}:${trans}:${
        filled ? 1 : 0
    }:${lineWidth}`;

    const wAny = w as any;
    let cache = wAny.__arcRenderCache as ArcRenderCache | undefined;
    if (!cache) {
        const canvas = document.createElement("canvas");
        const uid = w.uid ?? w.id ?? 0;
        cache = {
            canvas,
            key: `__widget_arc_${uid}_${w.childIndex ?? -1}`,
            stateKey: "",
        };
        wAny.__arcRenderCache = cache;
    }

    if (
        cache.stateKey !== stateKey ||
        cache.canvas.width !== canvasWidth ||
        cache.canvas.height !== canvasHeight
    ) {
        const canvas = cache.canvas;
        canvas.width = canvasWidth;
        canvas.height = canvasHeight;
        const ctx = canvas.getContext("2d");
        if (!ctx) {
            return;
        }

        ctx.clearRect(0, 0, canvasWidth, canvasHeight);
        ctx.fillStyle = `rgba(${r},${g},${b},${alpha})`;
        ctx.strokeStyle = `rgba(${r},${g},${b},${alpha})`;
        ctx.lineWidth = lineWidth;
        ctx.lineCap = "butt";

        const cx = canvasWidth / 2;
        const cy = canvasHeight / 2;
        const inset = filled ? 0 : lineWidth / 2;
        const radiusX = Math.max(0.5, canvasWidth / 2 - inset);
        const radiusY = Math.max(0.5, canvasHeight / 2 - inset);
        const startAngle = arcUnitToRadians(start);
        const endAngle = startAngle + (span / ARC_TURN_UNITS) * ARC_FULL_RADIANS;

        ctx.beginPath();
        if (filled) {
            ctx.moveTo(cx, cy);
            ctx.ellipse(cx, cy, radiusX, radiusY, 0, startAngle, endAngle, false);
            ctx.closePath();
            ctx.fill();
        } else {
            ctx.ellipse(cx, cy, radiusX, radiusY, 0, startAngle, endAngle, false);
            ctx.stroke();
        }

        glr.updateTextureFromCanvas(cache.key, canvas);
        cache.stateKey = stateKey;
    }

    const tex = glr.getTexture(cache.key) ?? glr.updateTextureFromCanvas(cache.key, cache.canvas);
    glr.drawTexture(tex, x, y, width, height, 1, 1);
}

