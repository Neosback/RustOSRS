import type { MinimapIcon } from "../../render/loader/SdMapData";
import { unpackWorldMapCoord } from "../../rs/map/WorldMapArea";
import { getWorldMapImageKey, type WorldMapImageDeps } from "./WorldMapImageCache";
import type { WorldMapStateHolder } from "./WorldMapTypes";

export function getWorldMapIcons(
    state: WorldMapStateHolder,
    deps: WorldMapImageDeps,
    mapX: number,
    mapY: number,
    level: number = 0,
): MinimapIcon[] | undefined {
    const areaId = state.worldMapState.getCurrentMapAreaId();
    if (state.worldMapIconCacheAreaId !== areaId) {
        state.worldMapIconCache.clear();
        state.worldMapIconCacheAreaId = areaId;
    }
    const mapId = getWorldMapImageKey(state, deps, mapX, mapY, level);
    if (state.worldMapIconCache.has(mapId)) {
        return state.worldMapIconCache.get(mapId);
    }
    const sourceIcons = state.worldMapState.getIconsForTile(mapX, mapY, level);
    if (sourceIcons.length === 0) {
        state.worldMapIconCache.set(mapId, undefined);
        return undefined;
    }

    const icons: MinimapIcon[] = [];
    const seen = new Set<string>();
    for (const icon of sourceIcons) {
        const coord = unpackWorldMapCoord(icon.coord);
        const displayCoord =
            icon.displayCoord !== undefined ? unpackWorldMapCoord(icon.displayCoord) : coord;
        const localX = displayCoord.x - ((mapX | 0) << 6);
        const localY = displayCoord.y - ((mapY | 0) << 6);
        if (localX < 0 || localX >= 64 || localY < 0 || localY >= 64) continue;
        const displayPos =
            icon.displayCoord !== undefined
                ? { x: displayCoord.x, y: displayCoord.y }
                : state.worldMapState.currentArea?.position(coord.plane, coord.x, coord.y);
        if (!displayPos) continue;

        const key = `${icon.element | 0}:${localX | 0}:${localY | 0}`;
        if (seen.has(key)) continue;
        seen.add(key);
        let element: any;
        try {
            element = deps.loadMapElement(icon.element | 0);
        } catch {
            element = undefined;
        }

        icons.push({
            localX,
            localY,
            elementId: icon.element | 0,
            category: (element?.category ?? icon.category) | 0,
            spriteId: (element?.spriteId ?? icon.spriteId) | 0,
            worldMapVisible: element?.worldMapVisible,
            name: element?.name,
            textColor: element?.textColor,
            textSize: element?.textSize,
            horizontalAlignment: element?.horizontalAlignment,
            verticalAlignment: element?.verticalAlignment,
            sourcePlane: coord.plane,
            sourceX: coord.x | 0,
            sourceY: coord.y | 0,
            displayPlane: displayCoord.plane,
            displayX: displayPos.x,
            displayY: displayPos.y,
        });
    }

    const result = icons.length > 0 ? icons : undefined;
    state.worldMapIconCache.set(mapId, result);
    return result;
}
