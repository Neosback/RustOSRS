import { BitmapFont } from "../../../rs/font/BitmapFont";
import { getBitmapFontAtlas } from "../../../ui/text/BitmapFontAtlas";
import { GLRenderer } from "../renderer";
import { WORLD_MAP_LABEL_QUADS, WORLD_MAP_LABEL_TINT } from "./constants";
export type WorldMapLabelMetrics = {
    lines: string[];
    lineWidths: number[];
    logicalWidth: number;
    lineHeight: number;
    logicalHeight: number;
};

export type WorldMapLabelDraw = {
    text: string;
    fontId: number;
    font: BitmapFont | undefined;
    metrics: WorldMapLabelMetrics;
    x: number;
    y: number;
    width: number;
    height: number;
    color: number;
    scaleX: number;
    scaleY: number;
};
export function getWorldMapFlashTexture(glr: GLRenderer) {
    const key = "__worldmap_flash_marker";
    const cached = glr.getTexture(key);
    if (cached) return cached;
    const canvas = document.createElement("canvas");
    canvas.width = 30;
    canvas.height = 30;
    const ctx = canvas.getContext("2d") as CanvasRenderingContext2D | null;
    if (!ctx) return undefined;
    ctx.clearRect(0, 0, 30, 30);
    ctx.beginPath();
    ctx.arc(15, 15, 15, 0, Math.PI * 2);
    ctx.fillStyle = "rgba(255, 255, 0, 0.5)";
    ctx.fill();
    ctx.beginPath();
    ctx.arc(15, 15, 7, 0, Math.PI * 2);
    ctx.fillStyle = "rgba(255, 255, 255, 1)";
    ctx.fill();
    return glr.createTextureFromCanvas(key, canvas);
}

export function ensureWorldMapLabelQuadCapacity(addQuads: number): void {
    const requiredQuads = WORLD_MAP_LABEL_QUADS.quadCount + addQuads;
    const currentQuads = WORLD_MAP_LABEL_QUADS.data.length / 16;
    if (requiredQuads <= currentQuads) return;

    let nextQuads = Math.max(1, currentQuads);
    while (nextQuads < requiredQuads) {
        nextQuads <<= 1;
    }

    const next = new Float32Array(nextQuads * 16);
    next.set(WORLD_MAP_LABEL_QUADS.data.subarray(0, WORLD_MAP_LABEL_QUADS.quadCount * 16));
    WORLD_MAP_LABEL_QUADS.data = next;
}

export function appendWorldMapLabelGlyphQuad(
    originX: number,
    originY: number,
    scaleX: number,
    scaleY: number,
    left: number,
    top: number,
    right: number,
    bottom: number,
    u0: number,
    v0: number,
    u1: number,
    v1: number,
): void {
    const x0 = originX + Math.round(left * scaleX);
    const y0 = originY + Math.round(top * scaleY);
    let x1 = originX + Math.round(right * scaleX);
    let y1 = originY + Math.round(bottom * scaleY);
    if (right > left && x1 <= x0) x1 = x0 + 1;
    if (bottom > top && y1 <= y0) y1 = y0 + 1;
    if (x1 <= x0 || y1 <= y0) return;

    ensureWorldMapLabelQuadCapacity(1);
    const data = WORLD_MAP_LABEL_QUADS.data;
    const offset = WORLD_MAP_LABEL_QUADS.quadCount * 16;
    data[offset + 0] = x0;
    data[offset + 1] = y0;
    data[offset + 2] = u0;
    data[offset + 3] = v0;
    data[offset + 4] = x1;
    data[offset + 5] = y0;
    data[offset + 6] = u1;
    data[offset + 7] = v0;
    data[offset + 8] = x1;
    data[offset + 9] = y1;
    data[offset + 10] = u1;
    data[offset + 11] = v1;
    data[offset + 12] = x0;
    data[offset + 13] = y1;
    data[offset + 14] = u0;
    data[offset + 15] = v1;
    WORLD_MAP_LABEL_QUADS.quadCount++;
}

export function drawWorldMapLabelGL(
    glr: GLRenderer,
    font: BitmapFont,
    metrics: WorldMapLabelMetrics,
    x: number,
    y: number,
    color: number,
    scaleX: number,
    scaleY: number,
): void {
    const atlas = getBitmapFontAtlas(glr, font);

    const originX = x | 0;
    const originY = y | 0;
    const ascent = (font.maxAscent || font.ascent || 0) | 0;
    const appendLabelQuads = (offsetX: number, offsetY: number) => {
        for (let lineIndex = 0; lineIndex < metrics.lines.length; lineIndex++) {
            const line = metrics.lines[lineIndex];
            let penX =
                (((metrics.logicalWidth - (metrics.lineWidths[lineIndex] ?? 0)) / 2) | 0) + offsetX;
            const baselineY = ascent + lineIndex * metrics.lineHeight + offsetY;
            let previous = -1;
            for (let i = 0; i < line.length; i++) {
                let ch = line.charCodeAt(i) & 0xff;
                if (ch === 160) ch = 32;
                if (font.kerning && previous !== -1) {
                    penX += font.kerning[(previous << 8) + ch] || 0;
                }

                const glyph = atlas.glyphs[ch];
                if (glyph?.drawable) {
                    const left = penX + glyph.lb;
                    const top = baselineY - atlas.ascent + glyph.tb;
                    appendWorldMapLabelGlyphQuad(
                        originX,
                        originY,
                        scaleX,
                        scaleY,
                        left,
                        top,
                        left + glyph.w,
                        top + glyph.h,
                        glyph.u0,
                        glyph.v0,
                        glyph.u1,
                        glyph.v1,
                    );
                }
                penX += glyph?.adv ?? font.advances[ch] ?? glyph?.w ?? 0;
                previous = ch;
            }
        }
    };

    WORLD_MAP_LABEL_QUADS.quadCount = 0;
    appendLabelQuads(1, 1);
    if (WORLD_MAP_LABEL_QUADS.quadCount > 0) {
        WORLD_MAP_LABEL_TINT[0] = 0;
        WORLD_MAP_LABEL_TINT[1] = 0;
        WORLD_MAP_LABEL_TINT[2] = 0;
        glr.drawTextureQuads(
            atlas.texture,
            WORLD_MAP_LABEL_QUADS.data,
            WORLD_MAP_LABEL_QUADS.quadCount,
            1,
            WORLD_MAP_LABEL_TINT,
            1,
        );
    }

    WORLD_MAP_LABEL_QUADS.quadCount = 0;
    appendLabelQuads(0, 0);
    if (WORLD_MAP_LABEL_QUADS.quadCount <= 0) return;
    WORLD_MAP_LABEL_TINT[0] = ((color >>> 16) & 0xff) / 255;
    WORLD_MAP_LABEL_TINT[1] = ((color >>> 8) & 0xff) / 255;
    WORLD_MAP_LABEL_TINT[2] = (color & 0xff) / 255;
    glr.drawTextureQuads(
        atlas.texture,
        WORLD_MAP_LABEL_QUADS.data,
        WORLD_MAP_LABEL_QUADS.quadCount,
        1,
        WORLD_MAP_LABEL_TINT,
        1,
    );
}
