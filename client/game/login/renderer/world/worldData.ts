import { MOCK_WORLDS } from "../constants";
import type { LoginRendererHost } from "../host";
import type { World, WorldGridLayout, WorldHoverResult } from "../types";
import { WorldFlags, WorldBackgroundType } from "../types";

export function getWorldBackgroundType(host: LoginRendererHost, world: World) {

        const isMember = (world.properties & WorldFlags.MEMBERS) !== 0;

        if ((world.properties & WorldFlags.BETA) !== 0) {
            return isMember ? WorldBackgroundType.MEMBERS_BETA : WorldBackgroundType.FREE_BETA;
        }
        if ((world.properties & WorldFlags.DEADMAN) !== 0) {
            return isMember
                ? WorldBackgroundType.MEMBERS_DEADMAN
                : WorldBackgroundType.FREE_DEADMAN;
        }
        if ((world.properties & WorldFlags.HIGH_RISK) !== 0) {
            return isMember
                ? WorldBackgroundType.MEMBERS_HIGH_RISK
                : WorldBackgroundType.FREE_HIGH_RISK;
        }
        if ((world.properties & WorldFlags.PVP) !== 0) {
            return isMember ? WorldBackgroundType.MEMBERS_PVP : WorldBackgroundType.FREE_PVP;
        }
        if ((world.properties & WorldFlags.FRESH_START) !== 0) {
            return isMember
                ? WorldBackgroundType.MEMBERS_FRESH_START
                : WorldBackgroundType.FREE_FRESH_START;
        }

        return isMember ? WorldBackgroundType.MEMBERS_NORMAL : WorldBackgroundType.FREE_NORMAL;
    
}

export function getGridLayout(host: LoginRendererHost, worldCount: number) {

        // Return cached layout if world count hasn't changed
        if (host.cachedGridLayout !== null && host.cachedGridWorldCount === worldCount) {
            return host.cachedGridLayout;
        }

        const rowWidth = 88;
        const rowHeight = 19;
        let cols = Math.floor(765 / (rowWidth + 1)) - 1;
        let rows = Math.floor(480 / (rowHeight + 1));

        // Fit the grid to the world count
        do {
            const prevRows = rows;
            const prevCols = cols;
            if (rows * (cols - 1) >= worldCount) cols--;
            if (cols * (rows - 1) >= worldCount) rows--;
            if (cols * (rows - 1) >= worldCount) rows--;
            if (prevRows === rows && prevCols === cols) break;
        } while (true);

        // Calculate spacing
        let xGap = Math.floor((765 - rowWidth * cols) / (cols + 1));
        if (xGap > 5) xGap = 5;
        let yGap = Math.floor((480 - rowHeight * rows) / (rows + 1));
        if (yGap > 5) yGap = 5;

        const xOffset = Math.floor((765 - rowWidth * cols - xGap * (cols - 1)) / 2);
        const yOffset = Math.floor((480 - rows * rowHeight - yGap * (rows - 1)) / 2);

        const columnsPerPage = cols;
        const totalColumns = Math.ceil(worldCount / rows);

        host.cachedGridLayout = {
            cols,
            rows,
            xGap,
            yGap,
            xOffset,
            yOffset,
            rowWidth,
            rowHeight,
            worldCount,
            columnsPerPage,
            totalColumns,
        };
        host.cachedGridWorldCount = worldCount;

        return host.cachedGridLayout;
    
}

export function findHoveredWorld(host: LoginRendererHost, sortedWorlds: World[], layout: WorldGridLayout, page: number) {

        const { cols, rows, xGap, yGap, xOffset, yOffset, rowWidth, rowHeight, worldCount } =
            layout;

        let drawY = yOffset + 23;
        let drawX = xOffset + host.xPadding;
        let rowIndex = 0;
        let columnIndex = page;

        const startWorldIndex = page * rows;
        for (let i = startWorldIndex; i < worldCount && columnIndex - page < cols; i++) {
            const world = sortedWorlds[i];
            const canJoin = world.population !== -1;

            const isHovered =
                host.mouseX >= drawX &&
                host.mouseY >= drawY &&
                host.mouseX < drawX + rowWidth &&
                host.mouseY < drawY + rowHeight &&
                canJoin;

            if (isHovered) {
                return { index: i, world, x: drawX, y: drawY };
            }

            // Move to next position
            drawY += rowHeight + yGap;
            rowIndex++;
            if (rowIndex >= rows) {
                drawY = yOffset + 23;
                drawX += xGap + rowWidth;
                rowIndex = 0;
                columnIndex++;
            }
        }

        return { index: -1, world: null, x: 0, y: 0 };
    
}

export function getSortedWorlds(host: LoginRendererHost) {

        // Performance: return cached result if sort options haven't changed
        if (
            host.cachedSortedWorlds !== null &&
            host.cachedSortOption === host.worldSortOption &&
            host.cachedSortDirection === host.worldSortDirection
        ) {
            return host.cachedSortedWorlds;
        }

        const worlds = [...MOCK_WORLDS];
        const ascending = host.worldSortDirection === 0;

        worlds.sort((a, b) => {
            let result = 0;

            switch (host.worldSortOption) {
                case 0: // World ID
                    result = a.id - b.id;
                    break;
                case 1: // Players (population)
                    // Offline worlds (-1) go to end when ascending, start when descending
                    const popA = a.population === -1 ? (ascending ? 2001 : -1) : a.population;
                    const popB = b.population === -1 ? (ascending ? 2001 : -1) : b.population;
                    result = popA - popB;
                    break;
                case 2: // Location
                    result = a.location - b.location;
                    break;
                case 3: // Type (activity)
                    if (a.activity === "-") {
                        result = ascending ? 1 : -1;
                    } else if (b.activity === "-") {
                        result = ascending ? -1 : 1;
                    } else {
                        result = a.activity.localeCompare(b.activity);
                    }
                    break;
            }

            return ascending ? result : -result;
        });

        // Cache the result
        host.cachedSortedWorlds = worlds;
        host.cachedSortOption = host.worldSortOption;
        host.cachedSortDirection = host.worldSortDirection;

        return worlds;
    
}
