import type { EditModeEdit, EditModeTile } from "./types";

export type BuildingStyle = "Varrock" | "Classic" | "Barbarian" | "Canifis";
export type BuildingShape = "Rectangle" | "Diagonal";
export const BUILDING_STYLES: readonly BuildingStyle[] = ["Varrock", "Classic", "Barbarian", "Canifis"];
export const BUILDING_SHAPES: readonly BuildingShape[] = ["Rectangle", "Diagonal"];

type Bounds = { minX: number; maxX: number; minY: number; maxY: number; plane: number };
type Style = { wall: number; corner: number; cornerShape: number; floor: number; door: number; roofEdge: number; roofSlope: number; roofFill: number };

const STYLES: Record<BuildingStyle, Style> = {
    Varrock: { wall: 23735, corner: 23735, cornerShape: 1, floor: 22, door: 11775, roofEdge: 15552, roofSlope: 15552, roofFill: 15552 },
    Classic: { wall: 1902, corner: 1631, cornerShape: 3, floor: 108, door: 1535, roofEdge: 1793, roofSlope: 1933, roofFill: 1640 },
    Barbarian: { wall: 11558, corner: -1, cornerShape: 3, floor: 108, door: 1535, roofEdge: 4242, roofSlope: 11586, roofFill: 11586 },
    Canifis: { wall: 24371, corner: 24379, cornerShape: 3, floor: 57, door: 24369, roofEdge: 15552, roofSlope: 15552, roofFill: 15552 },
};
const LADDER_IDS = { ground: 16683, middle: 16684, top: 16679 };

const place = (locId: number, tileX: number, tileY: number, plane: number, shape: number, rotation: number): EditModeEdit =>
    ({ kind: "place", locId, tileX, tileY, plane, shape, rotation });

/** The stable rectangular HouseType path from RSPSi, expressed as persistent editor edits. */
export function generateBuildingEdits(
    bounds: Bounds,
    styleName: BuildingStyle,
    requestedFloors: number,
    camera?: EditModeTile,
    sampleHeight?: (tile: EditModeTile) => number | undefined,
    shapeName: BuildingShape = "Rectangle",
): EditModeEdit[] {
    const width = bounds.maxX - bounds.minX + 1;
    const depth = bounds.maxY - bounds.minY + 1;
    if (bounds.plane !== 0 || width <= 2 || depth <= 2) return [];
    const floors = Math.max(1, Math.min(3, Math.floor(requestedFloors)));
    const style = STYLES[styleName];
    const edits: EditModeEdit[] = [];
    const left = bounds.minX - 1, right = bounds.maxX + 1;
    const bottom = bounds.minY - 1, top = bounds.maxY + 1;
    const ladderX = Math.floor((bounds.minX + bounds.maxX) / 2);
    const ladderY = Math.floor((bounds.minY + bounds.maxY) / 2);
    const doorRotation = camera && camera.tileY > bounds.maxY ? 3
        : camera && camera.tileX > bounds.maxX ? 0
        : camera && camera.tileX < bounds.minX ? 2 : 1;
    const doorCoord = doorRotation % 2 === 0
        ? Math.floor((bounds.minY + bounds.maxY) / 2)
        : Math.floor((bounds.minX + bounds.maxX) / 2);

    for (let floor = 0; floor < floors; floor++) {
        for (let y = bounds.minY + (shapeName === "Diagonal" ? 1 : 0); y <= bounds.maxY - (shapeName === "Diagonal" ? 1 : 0); y++) {
            edits.push(place(floor === 0 && doorRotation === 2 && y === doorCoord ? style.door : style.wall, left, y, floor, 0, 2));
            edits.push(place(floor === 0 && doorRotation === 0 && y === doorCoord ? style.door : style.wall, right, y, floor, 0, 0));
        }
        for (let x = bounds.minX + (shapeName === "Diagonal" ? 1 : 0); x <= bounds.maxX - (shapeName === "Diagonal" ? 1 : 0); x++) {
            edits.push(place(floor === 0 && doorRotation === 1 && x === doorCoord ? style.door : style.wall, x, bottom, floor, 0, 1));
            edits.push(place(floor === 0 && doorRotation === 3 && x === doorCoord ? style.door : style.wall, x, top, floor, 0, 3));
        }
        if (shapeName === "Diagonal") {
            for (const [x, y, rotation] of [[left, bounds.minY, 1], [bounds.minX, bottom, 1], [left, bounds.maxY, 2], [bounds.minX, top, 2], [bounds.maxX, bottom, 0], [right, bounds.minY, 0], [bounds.maxX, top, 3], [right, bounds.maxY, 3]]) {
                edits.push(place(style.wall, x, y, floor, 1, rotation));
            }
            for (const [x, y, rotation] of [[bounds.minX, bounds.minY, 1], [bounds.minX, bounds.maxY, 2], [bounds.maxX, bounds.minY, 0], [bounds.maxX, bounds.maxY, 3]]) {
                edits.push(place(style.wall, x, y, floor, 9, rotation));
            }
        } else if (style.corner > 0) {
            edits.push(place(style.corner, left, bottom, floor, style.cornerShape, 1));
            edits.push(place(style.corner, left, top, floor, style.cornerShape, 2));
            edits.push(place(style.corner, right, top, floor, style.cornerShape, 3));
            edits.push(place(style.corner, right, bottom, floor, style.cornerShape, 0));
        }
        for (let x = bounds.minX; x <= bounds.maxX; x++) {
            for (let y = bounds.minY; y <= bounds.maxY; y++) {
                const diagonalCorner = shapeName === "Diagonal" && (x === bounds.minX || x === bounds.maxX) && (y === bounds.minY || y === bounds.maxY);
                const rotation = x === bounds.minX ? (y === bounds.minY ? 2 : 3) : (y === bounds.minY ? 1 : 0);
                const ladderOpening = floors > 1 && floor > 0 && x === ladderX && y === ladderY;
                edits.push({ kind: "terrain", locId: ladderOpening ? 0 : style.floor, tileX: x, tileY: y, plane: floor, shape: diagonalCorner ? 1 : 0, rotation: diagonalCorner ? rotation : 0 });
            }
        }
    }

    if (floors > 1) {
        for (let floor = 0; floor < floors; floor++) {
            const locId = floor === 0 ? LADDER_IDS.ground : floor === floors - 1 ? LADDER_IDS.top : LADDER_IDS.middle;
            edits.push(place(locId, ladderX, ladderY, floor, 10, 0));
        }
    }

    const roof = floors;
    for (let y = bounds.minY + (shapeName === "Diagonal" ? 1 : 0); y <= bounds.maxY - (shapeName === "Diagonal" ? 1 : 0); y++) {
        edits.push(place(style.roofEdge, left, y, roof, 18, 2), place(style.roofEdge, right, y, roof, 18, 0));
        if (y > bounds.minY && y < bounds.maxY) {
            edits.push(place(style.roofSlope, bounds.minX, y, roof, 12, 2), place(style.roofSlope, bounds.maxX, y, roof, 12, 0));
        }
    }
    for (let x = bounds.minX + (shapeName === "Diagonal" ? 1 : 0); x <= bounds.maxX - (shapeName === "Diagonal" ? 1 : 0); x++) {
        edits.push(place(style.roofEdge, x, bottom, roof, 18, 1), place(style.roofEdge, x, top, roof, 18, 3));
        if (x > bounds.minX && x < bounds.maxX) {
            edits.push(place(style.roofSlope, x, bounds.minY, roof, 12, 1), place(style.roofSlope, x, bounds.maxY, roof, 12, 3));
        }
    }
    const outerRoofCorners = shapeName === "Diagonal"
        ? [[left, bounds.minY, 1], [bounds.minX, bottom, 1], [left, bounds.maxY, 2], [bounds.minX, top, 2], [bounds.maxX, bottom, 0], [right, bounds.minY, 0], [bounds.maxX, top, 3], [right, bounds.maxY, 3]]
        : [[left, bottom, 1], [left, top, 2], [right, top, 3], [right, bottom, 0]];
    for (const [x, y, rotation] of outerRoofCorners) edits.push(place(style.roofEdge, x, y, roof, shapeName === "Diagonal" ? 19 : 21, rotation));
    for (const [x, y, rotation] of [[bounds.minX, bounds.minY, 1], [bounds.minX, bounds.maxY, 2], [bounds.maxX, bounds.maxY, 3], [bounds.maxX, bounds.minY, 0]]) {
        edits.push(place(style.roofSlope, x, y, roof, shapeName === "Diagonal" ? 13 : 16, rotation));
    }
    for (let x = bounds.minX + 1; x < bounds.maxX; x++) {
        for (let y = bounds.minY + 1; y < bounds.maxY; y++) edits.push(place(style.roofFill, x, y, roof, 17, 0));
    }

    if (sampleHeight) {
        const samples: number[] = [];
        for (let x = bounds.minX - 2; x <= bounds.maxX + 2; x++) {
            for (let y = bounds.minY - 2; y <= bounds.maxY + 2; y++) {
                if (x !== bounds.minX - 2 && x !== bounds.maxX + 2 && y !== bounds.minY - 2 && y !== bounds.maxY + 2) continue;
                const height = sampleHeight({ tileX: x, tileY: y, plane: 0 });
                if (height !== undefined) samples.push(height);
            }
        }
        // The outer ring can cross an unloaded map edge. The selected tiles are
        // visible by definition, so use them as the stable fallback.
        if (samples.length === 0) {
            for (let x = bounds.minX; x <= bounds.maxX; x++) {
                for (let y = bounds.minY; y <= bounds.maxY; y++) {
                    const height = sampleHeight({ tileX: x, tileY: y, plane: 0 });
                    if (height !== undefined) samples.push(height);
                }
            }
        }
        if (samples.length > 0) {
            samples.sort((a, b) => a - b);
            const target = samples[Math.floor(samples.length / 2)];
            for (let plane = 0; plane <= roof; plane++) {
                for (let x = bounds.minX - 1; x <= bounds.maxX + 2; x++) {
                    for (let y = bounds.minY - 1; y <= bounds.maxY + 2; y++) {
                        const below = plane === 0 ? 0 : sampleHeight({ tileX: x, tileY: y, plane: plane - 1 });
                        const height = plane === 0 ? target : (below ?? target - (plane - 1) * 15 / 8) - 15 / 8;
                        edits.push({ kind: "height", locId: Math.max(0, Math.min(255, Math.round(((below ?? 0) - height) * 16))), tileX: x, tileY: y, plane, shape: 0, rotation: 0 });
                    }
                }
            }
        }
    }
    for (let x = bounds.minX; x <= bounds.maxX; x++) {
        for (let y = bounds.minY; y <= bounds.maxY; y++) {
            edits.push({ kind: "flag", locId: 4, tileX: x, tileY: y, plane: 0, shape: 0, rotation: 0 });
            for (let plane = 1; plane <= roof; plane++) edits.push({ kind: "flag", locId: 1, tileX: x, tileY: y, plane, shape: 0, rotation: 0 });
        }
    }
    return edits;
}
