import { GLRenderer } from "../renderer";
export function drawLine(
    glr: GLRenderer,
    x1: number,
    y1: number,
    x2: number,
    y2: number,
    color: [number, number, number, number],
): void {
    x1 = x1 | 0;
    y1 = y1 | 0;
    x2 = x2 | 0;
    y2 = y2 | 0;

    const dx = Math.abs(x2 - x1);
    const dy = Math.abs(y2 - y1);
    const sx = x1 < x2 ? 1 : -1;
    const sy = y1 < y2 ? 1 : -1;
    let err = dx - dy;

    let x = x1;
    let y = y1;

    while (true) {
        glr.drawRect(x, y, 1, 1, color);
        if (x === x2 && y === y2) break;

        const e2 = err * 2;
        if (e2 > -dy) {
            err -= dy;
            x += sx;
        }
        if (e2 < dx) {
            err += dx;
            y += sy;
        }
    }
}

/**
 * Draw a thick line
 * Draws thick lines using perpendicular expansion
 */
export function drawThickLine(
    glr: GLRenderer,
    x1: number,
    y1: number,
    x2: number,
    y2: number,
    width: number,
    color: [number, number, number, number],
): void {
    x1 = x1 | 0;
    y1 = y1 | 0;
    x2 = x2 | 0;
    y2 = y2 | 0;
    width = Math.max(1, width | 0);

    // For thick lines, draw multiple parallel lines
    // Calculate perpendicular offset
    const dx = x2 - x1;
    const dy = y2 - y1;
    const len = Math.sqrt(dx * dx + dy * dy);

    if (len < 0.001) {
        // Point - just draw a square
        glr.drawRect(x1 - (width >> 1), y1 - (width >> 1), width, width, color);
        return;
    }

    // Unit perpendicular vector
    const px = -dy / len;
    const py = dx / len;

    // Draw lines offset perpendicular to the main line
    const halfWidth = width / 2;
    for (let i = -halfWidth; i <= halfWidth; i++) {
        const ox = Math.round(px * i);
        const oy = Math.round(py * i);
        drawLine(glr, x1 + ox, y1 + oy, x2 + ox, y2 + oy, color);
    }
}
