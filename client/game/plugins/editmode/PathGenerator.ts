/**
 * Ported from the elvarg-web-client map editor. Pure geometry: given the tiles
 * a path covers, return the exact tiles that need painting.
 */

export interface PathTile {
    x: number;
    y: number;
}

export interface PathOverlayTile extends PathTile {
    overlayShape: number;
    overlayRotation: number;
}

const DIRECTIONS: ReadonlyArray<PathTile> = [
    { x: -1, y: 0 },
    { x: 0, y: 1 },
    { x: 1, y: 0 },
    { x: 0, y: -1 },
];

const key = (x: number, y: number): string => `${x}:${y}`;

const cornerRotation = ([west, north, east]: boolean[]): number =>
    west ? (north ? 1 : 0) : north && east ? 2 : 3;

/** Bresenham line restricted to orthogonal steps, so paths never cut corners. */
export function createPathTiles(start: PathTile, end: PathTile): PathTile[] {
    const tiles: PathTile[] = [];
    let x = start.x;
    let y = start.y;
    const deltaX = Math.abs(end.x - x);
    const stepX = x < end.x ? 1 : -1;
    const deltaY = -Math.abs(end.y - y);
    const stepY = y < end.y ? 1 : -1;
    let error = deltaX + deltaY;

    // Bounded by the line length; a malformed input still terminates.
    for (;;) {
        tiles.push({ x, y });
        if (x === end.x && y === end.y) return tiles;

        const previousX = x;
        const previousY = y;
        const doubledError = error * 2;
        if (doubledError >= deltaY) {
            error += deltaY;
            x += stepX;
        }
        if (doubledError <= deltaX) {
            error += deltaX;
            y += stepY;
        }
        if (x !== previousX && y !== previousY) {
            tiles.push({ x, y: previousY });
        }
    }
}

/** Rounds bends without adding caps beyond either clicked endpoint. */
export function buildPathCorners(
    path: ReadonlySet<string>,
    editable: ReadonlySet<string>,
): PathOverlayTile[] {
    const result = new Map<string, PathOverlayTile>();

    for (const tileKey of editable) {
        const [x, y] = tileKey.split(":").map(Number);
        const neighbours = DIRECTIONS.map((direction) => path.has(key(x + direction.x, y + direction.y)));
        const connected = neighbours.flatMap((present, index) => (present ? [index] : []));
        const tile: PathOverlayTile = { x, y, overlayShape: 0, overlayRotation: 0 };
        if (connected.length === 2 && connected[0] % 2 !== connected[1] % 2) {
            const first = DIRECTIONS[connected[0]];
            const second = DIRECTIONS[connected[1]];
            const staircase =
                path.has(key(x + first.x - second.x, y + first.y - second.y)) ||
                path.has(key(x + second.x - first.x, y + second.y - first.y));
            if (!staircase) {
                tile.overlayShape = 5;
                tile.overlayRotation = cornerRotation(neighbours);
            }
        }
        result.set(tileKey, tile);
    }

    for (const tileKey of editable) {
        const [x, y] = tileKey.split(":").map(Number);
        for (const direction of DIRECTIONS) {
            const cornerX = x + direction.x;
            const cornerY = y + direction.y;
            const cornerKey = key(cornerX, cornerY);
            if (path.has(cornerKey) || result.has(cornerKey)) continue;
            const neighbours = DIRECTIONS.map((neighbour) => path.has(key(cornerX + neighbour.x, cornerY + neighbour.y)));
            if (neighbours.filter(Boolean).length !== 2 || (neighbours[0] && neighbours[2]) || (neighbours[1] && neighbours[3])) continue;
            result.set(cornerKey, {
                x: cornerX,
                y: cornerY,
                overlayShape: 1,
                overlayRotation: cornerRotation(neighbours),
            });
        }
    }

    return [...result.values()];
}
