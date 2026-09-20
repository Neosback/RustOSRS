export function scaleLogicalPixels(scale: number, logicalPixels: number): number {
    const safeScale = Number.isFinite(scale) && scale > 0 ? scale : 1;
    return Math.max(1, Math.round(logicalPixels * safeScale));
}
