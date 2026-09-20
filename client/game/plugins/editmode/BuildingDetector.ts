export interface BuildingMapLoc {
    id: number;
    level: number;
    typeRot?: number;
}

export interface BuildingTile {
    x: number;
    y: number;
}

export interface DetectedBuilding {
    minX: number;
    maxX: number;
    minY: number;
    maxY: number;
    minPlane: number;
    maxPlane: number;
    wallId?: number;
    shape: "Rectangle" | "Irregular";
    tiles: BuildingTile[];
    structureTiles: BuildingTile[];
}

interface WallEdge {
    a: string;
    b: string;
    x: number;
    y: number;
    id: number;
    level: number;
    door: boolean;
    type: number;
}

const MAX_ROOF_TILES = 4096;
// ponytail: widest roofless interior observed in cache buildings; raise with map evidence.
const MAX_ROOF_GAP = 4;
const WEST = 1;
const NORTH = 2;
const EAST = 4;
const SOUTH = 8;
const point = (x: number, y: number): string => `${x}:${y}`;
const tile = (key: string): BuildingTile => {
    const [x, y] = key.split(":").map(Number);
    return { x, y };
};
const isRoof = (loc: BuildingMapLoc): boolean => {
    const type = (loc.typeRot ?? -1) & 0x3f;
    return type >= 12 && type <= 21;
};
const wallSides = (loc: BuildingMapLoc): number => {
    const type = (loc.typeRot ?? -1) & 0x3f;
    const rotation = ((loc.typeRot ?? 0) >>> 6) & 3;
    if (type === 0) return [WEST, NORTH, EAST, SOUTH][rotation];
    if (type === 2) return [WEST | NORTH, NORTH | EAST, EAST | SOUTH, SOUTH | WEST][rotation];
    return 0;
};

const edge = (
    x: number,
    y: number,
    ax: number,
    ay: number,
    bx: number,
    by: number,
): [string, string, number, number] => [point(ax, ay), point(bx, by), x, y];

const wallSegments = (x: number, y: number, typeRot = -1): Array<[string, string, number, number]> => {
    const type = typeRot & 0x3f;
    const rotation = (typeRot >>> 6) & 3;
    const straight = [
        edge(x, y, x, y, x, y + 1),
        edge(x, y, x, y + 1, x + 1, y + 1),
        edge(x, y, x + 1, y, x + 1, y + 1),
        edge(x, y, x, y, x + 1, y),
    ];
    if (type === 0) return [straight[rotation]];
    if (type === 2) return [straight[rotation], straight[(rotation + 1) & 3]];
    if (type === 1 || type === 3 || type === 9) {
        return rotation & 1
            ? [edge(x, y, x, y + 1, x + 1, y)]
            : [edge(x, y, x, y, x + 1, y + 1)];
    }
    return [];
};

/** Returns the largest connected cycle which surrounds the roof's extent. */
function closedWallCycle(
    edges: WallEdge[],
    minX: number,
    maxX: number,
    minY: number,
    maxY: number,
): WallEdge[] {
    const unique = new Map<string, WallEdge>();
    for (const candidate of edges) {
        const key =
            candidate.a < candidate.b
                ? `${candidate.a}|${candidate.b}`
                : `${candidate.b}|${candidate.a}`;
        const current = unique.get(key);
        if (!current || candidate.level < current.level) unique.set(key, candidate);
    }
    const candidates = [...unique.values()];
    const byPoint = new Map<string, Set<number>>();
    candidates.forEach((candidate, index) => {
        for (const endpoint of [candidate.a, candidate.b]) {
            const indexes = byPoint.get(endpoint) ?? new Set<number>();
            indexes.add(index);
            byPoint.set(endpoint, indexes);
        }
    });

    // Remove dangling fence branches. What remains is the graph's cycle core.
    const active = new Set(candidates.map((_, index) => index));
    const queue = [...byPoint].filter(([, indexes]) => indexes.size < 2).map(([key]) => key);
    while (queue.length) {
        const endpoint = queue.pop()!;
        for (const index of [...(byPoint.get(endpoint) ?? [])]) {
            if (!active.delete(index)) continue;
            const candidate = candidates[index];
            for (const other of [candidate.a, candidate.b]) {
                const indexes = byPoint.get(other)!;
                indexes.delete(index);
                if (indexes.size === 1) queue.push(other);
            }
        }
    }

    let best: WallEdge[] = [];
    const unseen = new Set(active);
    while (unseen.size) {
        const first = unseen.values().next().value as number;
        const stack = [first];
        const component: WallEdge[] = [];
        unseen.delete(first);
        while (stack.length) {
            const index = stack.pop()!;
            const candidate = candidates[index];
            component.push(candidate);
            for (const endpoint of [candidate.a, candidate.b]) {
                for (const neighbour of byPoint.get(endpoint) ?? []) {
                    if (unseen.delete(neighbour)) stack.push(neighbour);
                }
            }
        }
        if (component.length < 4) continue;
        const points = component.flatMap((candidate) => [tile(candidate.a), tile(candidate.b)]);
        const xs = points.map(({ x }) => x);
        const ys = points.map(({ y }) => y);
        if (
            Math.max(...xs) - Math.min(...xs) >= Math.max(1, maxX - minX - 2) &&
            Math.max(...ys) - Math.min(...ys) >= Math.max(1, maxY - minY - 2) &&
            component.length > best.length
        ) {
            best = component;
        }
    }
    return best;
}

function edgeBordersFootprint(candidate: WallEdge, footprint: Set<string>): boolean {
    const a = tile(candidate.a);
    const b = tile(candidate.b);
    if (a.x === b.x) {
        const y = Math.min(a.y, b.y);
        return footprint.has(point(a.x - 1, y)) !== footprint.has(point(a.x, y));
    }
    if (a.y === b.y) {
        const x = Math.min(a.x, b.x);
        return footprint.has(point(x, a.y - 1)) !== footprint.has(point(x, a.y));
    }
    return footprint.has(point(candidate.x, candidate.y));
}

function rectangleBoundaryCovered(
    edges: WallEdge[],
    minX: number,
    maxX: number,
    minY: number,
    maxY: number,
): boolean {
    const west = new Set<number>();
    const east = new Set<number>();
    const south = new Set<number>();
    const north = new Set<number>();
    for (const candidate of edges) {
        const a = tile(candidate.a);
        const b = tile(candidate.b);
        if (a.x === b.x) {
            const y = Math.min(a.y, b.y);
            if (y < minY || y > maxY) continue;
            if (a.x === minX) west.add(y);
            if (a.x === maxX + 1) east.add(y);
        } else if (a.y === b.y) {
            const x = Math.min(a.x, b.x);
            if (x < minX || x > maxX) continue;
            if (a.y === minY) south.add(x);
            if (a.y === maxY + 1) north.add(x);
        }
    }
    return west.size > 0 && east.size > 0 && south.size > 0 && north.size > 0;
}

/** Detects a connected roof and, for non-rectangles, its closed wall circumference. */
export function detectRectangularBuilding(
    pointerX: number,
    pointerY: number,
    getLocs: (x: number, y: number) => BuildingMapLoc[],
    isDoor: (id: number) => boolean = () => false,
): DetectedBuilding | undefined {
    const cache = new Map<string, BuildingMapLoc[]>();
    const at = (x: number, y: number): BuildingMapLoc[] => {
        const key = point(x, y);
        let locs = cache.get(key);
        if (!locs) {
            locs = getLocs(x, y);
            cache.set(key, locs);
        }
        return locs;
    };
    const blockedBetween = (x: number, y: number, nextX: number, nextY: number): boolean => {
        const [side, opposite] =
            nextX < x
                ? [WEST, EAST]
                : nextX > x
                  ? [EAST, WEST]
                  : nextY > y
                    ? [NORTH, SOUTH]
                    : [SOUTH, NORTH];
        return (
            at(x, y).some((loc) => wallSides(loc) & side) ||
            at(nextX, nextY).some((loc) => wallSides(loc) & opposite)
        );
    };

    for (let radius = 0; radius <= 1; radius++) {
        for (let dx = -radius; dx <= radius; dx++) {
            for (let dy = -radius; dy <= radius; dy++) {
                if (Math.max(Math.abs(dx), Math.abs(dy)) !== radius) continue;
                const seedX = pointerX + dx;
                const seedY = pointerY + dy;
                for (const seed of at(seedX, seedY)) {
                    if (!isRoof(seed)) continue;

                    const roof = new Set<string>();
                    const bridges = new Set<string>();
                    const queue: Array<[number, number]> = [[seedX, seedY]];
                    for (let i = 0; i < queue.length && roof.size <= MAX_ROOF_TILES; i++) {
                        const [x, y] = queue[i];
                        const key = point(x, y);
                        if (roof.has(key) || !at(x, y).some(isRoof)) continue;
                        roof.add(key);
                        for (const [dx, dy] of [[-1, 0], [1, 0], [0, -1], [0, 1]]) {
                            const adjacentX = x + dx;
                            const adjacentY = y + dy;
                            if (at(adjacentX, adjacentY).some(isRoof)) {
                                queue.push([adjacentX, adjacentY]);
                                continue;
                            }
                            if (blockedBetween(x, y, x - dx, y - dy)) continue;
                            let previousX = x;
                            let previousY = y;
                            for (let distance = 1; distance <= MAX_ROOF_GAP + 1; distance++) {
                                const nextX = x + dx * distance;
                                const nextY = y + dy * distance;
                                if (blockedBetween(previousX, previousY, nextX, nextY)) break;
                                if (at(nextX, nextY).some(isRoof)) {
                                    if (blockedBetween(nextX, nextY, nextX + dx, nextY + dy)) break;
                                    queue.push([nextX, nextY]);
                                    for (let gap = 1; gap < distance; gap++) {
                                        bridges.add(point(x + dx * gap, y + dy * gap));
                                    }
                                    break;
                                }
                                previousX = nextX;
                                previousY = nextY;
                            }
                        }
                    }
                    // ponytail: cap pathological connected roofs; lift only if real maps exceed it.
                    if (roof.size < 4 || roof.size > MAX_ROOF_TILES) continue;

                    const footprint = new Set([...roof, ...bridges]);
                    const tiles = [...footprint].map(tile);
                    const xs = tiles.map(({ x }) => x);
                    const ys = tiles.map(({ y }) => y);
                    const minX = Math.min(...xs);
                    const maxX = Math.max(...xs);
                    const minY = Math.min(...ys);
                    const maxY = Math.max(...ys);
                    if (minX === maxX || minY === maxY) continue;
                    const maxPlane = Math.max(
                        ...tiles.flatMap(({ x, y }) =>
                            at(x, y).filter(isRoof).map(({ level }) => level | 0),
                        ),
                    );
                    const width = maxX - minX + 1;
                    const depth = maxY - minY + 1;
                    const rectangular = footprint.size === width * depth;
                    const edges: WallEdge[] = [];
                    for (let x = minX - 1; x <= maxX + 1; x++) {
                        for (let y = minY - 1; y <= maxY + 1; y++) {
                            for (const loc of at(x, y)) {
                                if (loc.level > maxPlane) continue;
                                const door = isDoor(loc.id);
                                const type = (loc.typeRot ?? -1) & 0x3f;
                                for (const [a, b] of wallSegments(x, y, loc.typeRot)) {
                                    edges.push({
                                        a,
                                        b,
                                        x,
                                        y,
                                        id: loc.id | 0,
                                        level: loc.level | 0,
                                        door,
                                        type,
                                    });
                                }
                            }
                        }
                    }

                    const cycle = closedWallCycle(edges, minX, maxX, minY, maxY);
                    const cyclePoints = cycle.flatMap((candidate) => [tile(candidate.a), tile(candidate.b)]);
                    const cycleEnclosesFootprint =
                        cyclePoints.length > 0 &&
                        Math.min(...cyclePoints.map(({ x }) => x)) <= minX &&
                        Math.max(...cyclePoints.map(({ x }) => x)) >= maxX + 1 &&
                        Math.min(...cyclePoints.map(({ y }) => y)) <= minY &&
                        Math.max(...cyclePoints.map(({ y }) => y)) >= maxY + 1;
                    const cornerTiles = new Set(
                        edges
                            .filter(({ type }) => type === 1 || type === 2 || type === 3 || type === 9)
                            .map(({ x, y }) => point(x, y)),
                    );
                    const reachesEverySide =
                        edges.some(({ x }) => x <= minX + 1) &&
                        edges.some(({ x }) => x >= maxX - 1) &&
                        edges.some(({ y }) => y <= minY + 1) &&
                        edges.some(({ y }) => y >= maxY - 1);
                    // Some cache buildings mix wall IDs and corner models which do not form a
                    // mathematically closed edge graph. Four corners around every roof side is
                    // the map-data equivalent, while the roof footprint still excludes fences.
                    if (
                        rectangular &&
                        bridges.size === 0 &&
                        !cycleEnclosesFootprint &&
                        !rectangleBoundaryCovered(edges, minX, maxX, minY, maxY) &&
                        Math.min(width, depth) <= 3 &&
                        Math.max(width, depth) >= Math.min(width, depth) * 3
                    ) {
                        // ponytail: narrow unenclosed roof strips are bridges/ridges; revisit if
                        // cache evidence produces a real long, three-tile-deep building without walls.
                        continue;
                    }
                    if (!rectangular && !cycle.length && (!reachesEverySide || cornerTiles.size < 4)) {
                        continue;
                    }
                    const boundary = cycleEnclosesFootprint ? cycle : edges;
                    const walls = boundary.filter(({ door }) => !door);
                    const wallId = [...new Set(walls.map(({ id }) => id))]
                        .sort(
                            (a, b) =>
                                walls.filter(({ id }) => id === b).length -
                                walls.filter(({ id }) => id === a).length,
                        )[0];
                    if (wallId === undefined) continue;

                    const structure = new Set(footprint);
                    for (const candidate of cycle) {
                        if (edgeBordersFootprint(candidate, footprint)) {
                            structure.add(point(candidate.x, candidate.y));
                        }
                    }
                    const relevantEdges = cycleEnclosesFootprint
                        ? cycle
                        : edges.filter(({ id, x, y }) => id === wallId && footprint.has(point(x, y)));
                    const minPlane = relevantEdges.length
                        ? Math.min(...relevantEdges.map(({ level }) => level))
                        : 0;
                    return {
                        minX,
                        maxX,
                        minY,
                        maxY,
                        minPlane,
                        maxPlane,
                        wallId,
                        shape: rectangular ? "Rectangle" : "Irregular",
                        tiles,
                        structureTiles: [...structure].map(tile),
                    };
                }
            }
        }
    }
    return undefined;
}
