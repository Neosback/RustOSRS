import assert from "node:assert/strict";

import {
    BridgePlaneStrategy,
    resolveBridgePlaneForLocal,
    resolveBridgePromotedPlane,
    resolveCollisionSamplePlaneForLocal,
    resolveGroundItemStackPlane,
    resolveHeightSamplePlaneForLocal,
    resolveInteractionPlaneForLocal,
    resolveInteractionPlaneForWorldTile,
} from "../game/scene/PlaneResolver";
import { TILE_FLAG_BRIDGE } from "../game/scene/TileRenderFlags";

function localMap(options?: { bridgeSurfacePlane?: number; bridgeLevels?: number[] }) {
    const bridgeSurfacePlane = options?.bridgeSurfacePlane;
    const bridgeLevels = new Set(options?.bridgeLevels ?? [1]);
    return {
        getTileRenderFlag(level: number): number {
            return bridgeLevels.has(level | 0) ? TILE_FLAG_BRIDGE : 0;
        },
        isBridgeSurface(level: number): boolean {
            return bridgeSurfacePlane !== undefined && (level | 0) === bridgeSurfacePlane;
        },
    } as any;
}

{
    const map = localMap({ bridgeLevels: [1] });
    assert.equal(resolveHeightSamplePlaneForLocal(map, 0, 10, 20), 1);
    assert.equal(resolveCollisionSamplePlaneForLocal(map, 0, 10, 20), 1);
    assert.equal(resolveInteractionPlaneForLocal(map, 0, 10, 20), 1);
    assert.equal(
        resolveBridgePlaneForLocal(map, 0, 10, 20, BridgePlaneStrategy.RENDER),
        1,
    );
    assert.equal(
        resolveBridgePlaneForLocal(map, 0, 10, 20, BridgePlaneStrategy.OCCUPANCY),
        1,
    );
    assert.equal(
        resolveBridgePlaneForLocal(map, 0, 10, 20, BridgePlaneStrategy.EFFECTIVE),
        1,
    );
}

{
    const bridgeSurface = localMap({ bridgeSurfacePlane: 0, bridgeLevels: [1] });
    assert.equal(
        resolveInteractionPlaneForLocal(bridgeSurface, 0, 5, 5),
        0,
        "a tile already identified as the bridge surface stays on its effective plane",
    );
    assert.equal(
        resolveCollisionSamplePlaneForLocal(bridgeSurface, 0, 5, 5),
        0,
        "bridge-surface collision must not be promoted twice",
    );
    assert.equal(
        resolveHeightSamplePlaneForLocal(bridgeSurface, 0, 5, 5),
        1,
        "render height still samples the bridge geometry plane",
    );
}

{
    const noBridge = localMap({ bridgeLevels: [] });
    assert.equal(resolveHeightSamplePlaneForLocal(noBridge, 0, 1, 1), 0);
    assert.equal(resolveCollisionSamplePlaneForLocal(noBridge, 0, 1, 1), 0);
    assert.equal(resolveInteractionPlaneForLocal(noBridge, 0, 1, 1), 0);
}

{
    assert.equal(resolveGroundItemStackPlane(-1), 0);
    assert.equal(resolveGroundItemStackPlane(2), 2);
    assert.equal(resolveGroundItemStackPlane(99), 3);
}

{
    const map = localMap({ bridgeLevels: [1, 2] });
    const manager = {
        getMap(): any {
            return map;
        },
    } as any;
    assert.equal(
        resolveBridgePromotedPlane(manager, 0, { x: 3200, y: 3200 }),
        2,
        "consecutive bridge flags promote the visual plane one level at a time",
    );
}

{
    const map = localMap({ bridgeLevels: [1] });
    let requestedMapX = -1;
    let requestedMapY = -1;
    const manager = {
        getMap(mapX: number, mapY: number): any {
            requestedMapX = mapX;
            requestedMapY = mapY;
            return map;
        },
    } as any;
    assert.equal(resolveInteractionPlaneForWorldTile(manager, 0, 3210, 3220), 1);
    assert.ok(requestedMapX >= 0 && requestedMapY >= 0);
}

console.log("Plane resolver regressions passed");
