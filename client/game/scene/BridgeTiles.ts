import type { Scene } from "../../rs/scene/Scene";
import type { SceneTile } from "../../rs/scene/SceneTile";
import { TILE_FLAG_BRIDGE, TILE_FLAG_FORCE_LOWEST_PLANE } from "./TileRenderFlags";

export function isBridgeColumn(scene: Scene, tileX: number, tileY: number): boolean {
    const row = scene?.tileRenderFlags?.[1]?.[tileX];
    const flag = row ? row[tileY] | 0 : 0;
    return (flag & TILE_FLAG_BRIDGE) !== 0;
}

export function isBridgeSurfaceTile(tile: SceneTile | undefined): boolean {
    return !!tile?.isBridgeSurface;
}

export function getBridgeLinkedBelow(tile: SceneTile | undefined): SceneTile | undefined {
    return tile?.linkedBelow;
}

/**
 * Plane used for roof culling of a tile's geometry.
 *
 * Bridge replica tiles (originalLevel == tileLevel at planes above 0) keep their
 * force-lowest-plane flag at plane 0, so the flag is checked there for them.
 */
export function getBridgeAdjustedPlane(
    scene: Scene,
    tile: SceneTile,
    tileLevel: number,
    tileX: number,
    tileY: number,
): number {
    const originLevel = typeof tile.originalLevel === "number" ? tile.originalLevel : tileLevel;
    const hasBridgeColumn = isBridgeColumn(scene, tileX, tileY);

    const isBridgeReplica = hasBridgeColumn && originLevel === tileLevel && tileLevel > 0;
    const flagCheckLevel = isBridgeReplica ? 0 : tileLevel;
    const renderFlags = scene.tileRenderFlags[flagCheckLevel]?.[tileX]?.[tileY] ?? 0;
    if ((renderFlags & TILE_FLAG_FORCE_LOWEST_PLANE) !== 0) {
        return 0;
    }

    // Bridge-demoted tiles cull at their demoted level.
    const minLevel = scene.getTileMinLevel(tileLevel, tileX, tileY);
    return minLevel < tileLevel ? minLevel : tileLevel;
}
