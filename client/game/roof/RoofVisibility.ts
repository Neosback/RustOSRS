/**
 * Roof hiding: computes the top visible plane for the current frame. The
 * renderer culls all geometry above the limit (draw-range filter + vertex
 * shader plane cull).
 */
import { getMapIndexFromTile } from "../../rs/map/MapFileIndex";
import { Scene } from "../../rs/scene/Scene";
import type { MapManager, MapSquare } from "../MapManager";
import {
    TILE_FLAG_UNDER_ROOF,
    TileFlagMapSquare,
    getTileRenderFlagAt,
    isBridgeSurfaceLocal,
} from "../scene/TileRenderFlags";
import { clampPlane } from "../utils/PlaneUtil";

export interface TilePoint {
    x: number;
    y: number;
}

/** Camera pitch (RS units, 128-383) below which the camera/line-of-sight roof checks apply. */
const LINE_OF_SIGHT_MAX_PITCH = 310;

const TOP_PLANE = Scene.MAX_LEVELS - 1;

export interface RoofComputationInput {
    /** Player plane after bridge promotion. */
    playerRawPlane: number;
    /** Camera pitch in RS units (128 = lowest, 383 = highest). */
    cameraPitch: number;
    /** When true, every plane above the player's plane is hidden. */
    roofsHidden: boolean;
    cameraTile: TilePoint;
    playerTile: TilePoint;
    /** Camera focal tile; the camera-to-focal line is sampled for roof tiles. */
    targetTile: TilePoint;
}

export function computeRoofPlaneLimit<T extends MapSquare>(
    mapManager: MapManager<T>,
    maxLevel: number,
    input: RoofComputationInput,
): number {
    const playerPlane = resolveRoofReferencePlane(
        mapManager,
        input.playerRawPlane,
        input.playerTile,
    );
    return Math.min(computeTopVisiblePlane(mapManager, playerPlane, input), clampPlane(maxLevel));
}

function computeTopVisiblePlane<T extends MapSquare>(
    mapManager: MapManager<T>,
    playerPlane: number,
    input: RoofComputationInput,
): number {
    if (input.roofsHidden) {
        return playerPlane;
    }

    let topPlane = TOP_PLANE;

    if (input.cameraPitch < LINE_OF_SIGHT_MAX_PITCH) {
        if (
            isTileUnderRoof(mapManager, playerPlane, input.cameraTile.x, input.cameraTile.y) ||
            lineCrossesRoofTile(mapManager, playerPlane, input.cameraTile, input.targetTile)
        ) {
            topPlane = playerPlane;
        }
    }

    if (isTileUnderRoof(mapManager, playerPlane, input.playerTile.x, input.playerTile.y)) {
        topPlane = playerPlane;
    }

    return topPlane;
}

/**
 * Walks the tile grid from `from` to `to`, stepping one tile along the major
 * axis per iteration and advancing the minor axis on fixed-point accumulator
 * overflow. Returns true if any sampled tile is under a roof.
 */
function lineCrossesRoofTile<T extends MapSquare>(
    mapManager: MapManager<T>,
    plane: number,
    from: TilePoint,
    to: TilePoint,
): boolean {
    let x = from.x | 0;
    let y = from.y | 0;
    const targetX = to.x | 0;
    const targetY = to.y | 0;
    const dx = Math.abs(targetX - x);
    const dy = Math.abs(targetY - y);

    if (dx > dy) {
        const minorStep = ((dy * 65536) / dx) | 0;
        let acc = 32768;
        while (x !== targetX) {
            x += x < targetX ? 1 : -1;
            if (isTileUnderRoof(mapManager, plane, x, y)) {
                return true;
            }
            acc += minorStep;
            if (acc >= 65536) {
                acc -= 65536;
                y += y < targetY ? 1 : -1;
                if (isTileUnderRoof(mapManager, plane, x, y)) {
                    return true;
                }
            }
        }
    } else if (dy > 0) {
        const minorStep = ((dx * 65536) / dy) | 0;
        let acc = 32768;
        while (y !== targetY) {
            y += y < targetY ? 1 : -1;
            if (isTileUnderRoof(mapManager, plane, x, y)) {
                return true;
            }
            acc += minorStep;
            if (acc >= 65536) {
                acc -= 65536;
                x += x < targetX ? 1 : -1;
                if (isTileUnderRoof(mapManager, plane, x, y)) {
                    return true;
                }
            }
        }
    }

    return false;
}

function isTileUnderRoof<T extends MapSquare>(
    mapManager: MapManager<T>,
    plane: number,
    tileX: number,
    tileY: number,
): boolean {
    return (getTileRenderFlagAt(mapManager, plane, tileX, tileY) & TILE_FLAG_UNDER_ROOF) !== 0;
}

/**
 * Plane used for roof sampling. Bridge surfaces are demoted to ground level at
 * scene build time, so a bridge-promoted player plane is walked back down to
 * the surface tile's level.
 */
function resolveRoofReferencePlane<T extends MapSquare>(
    mapManager: MapManager<T>,
    rawPlane: number,
    tile: TilePoint | undefined,
): number {
    let plane = clampPlane(rawPlane);
    if (!tile) {
        return plane;
    }

    const map = mapManager.getMap(getMapIndexFromTile(tile.x), getMapIndexFromTile(tile.y)) as
        | (T & TileFlagMapSquare)
        | undefined;
    if (!map) {
        return plane;
    }

    const mask = Scene.MAP_SQUARE_SIZE - 1;
    const localTileX = tile.x & mask;
    const localTileY = tile.y & mask;

    while (plane > 0 && isBridgeSurfaceLocal(map, plane - 1, localTileX, localTileY)) {
        plane--;
    }

    return plane;
}
