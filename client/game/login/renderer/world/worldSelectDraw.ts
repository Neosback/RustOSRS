import { WorldFlags } from "../types";
import type { LoginState } from "../../LoginState";
import type { LoginRendererHost, RenderContext } from "../host";
import { drawGradientRect, drawCenteredText, drawText, drawSprite, measureText } from "../render/drawUtils";
import { getGridLayout, getSortedWorlds, findHoveredWorld, getWorldBackgroundType } from "./worldData";
import { drawSortColumn } from "./worldSelectSort";
import { drawMobileWorldSelectList } from "./worldSelectMobile";

export function drawWorldSelect(host: LoginRendererHost, ctx: RenderContext, state: LoginState, width: number, height: number) {

        if (!host.fontBold12 || !host.fontPlain11) return;

        // Mobile: use full-screen list view for touch-friendly world selection
        if (host.layoutConfig.worldSelectListMode) {
            drawMobileWorldSelectList(host, ctx, state, width, height);
            return;
        }

        // Sort worlds based on current sort option
        const sortedWorlds = getSortedWorlds(host);
        const worldCount = sortedWorlds.length;

        // Use cached grid layout (consolidates duplicate calculation)
        const layout = getGridLayout(host, worldCount);
        const {
            cols,
            rows,
            xGap,
            yGap,
            xOffset,
            yOffset,
            rowWidth,
            rowHeight,
            columnsPerPage,
            totalColumns,
        } = layout;

        // Calculate page count
        state.worldSelectPagesCount = Math.max(0, totalColumns - columnsPerPage);

        // Store sorted worlds for click handling
        host.currentSortedWorlds = sortedWorlds;

        // Performance: Check if we can use cached world grid
        const needsRedraw =
            host.worldSelectCache === null ||
            host.worldSelectCachePage !== state.worldSelectPage ||
            host.worldSelectCacheSortOption !== host.worldSortOption ||
            host.worldSelectCacheSortDirection !== host.worldSortDirection ||
            host.worldSelectCacheWidth !== width ||
            host.worldSelectCacheHeight !== height;

        if (needsRedraw) {
            // Create or resize cache canvas
            if (
                !host.worldSelectCache ||
                host.worldSelectCacheWidth !== width ||
                host.worldSelectCacheHeight !== height
            ) {
                host.worldSelectCache = new OffscreenCanvas(width, height);
                host.worldSelectCacheCtx = host.worldSelectCache.getContext("2d");
            }

            if (host.worldSelectCacheCtx) {
                const cacheCtx = host.worldSelectCacheCtx;

                // Clear cache
                cacheCtx.clearRect(0, 0, width, height);

                // Fill background - match parent container size
                cacheCtx.fillStyle = "#000000";
                cacheCtx.fillRect(
                    host.containerX,
                    23,
                    host.containerWidth,
                    host.containerHeight - 23,
                );

                // Draw gradient header bars - match container width
                const headerLeftWidth = 125;
                const headerRightWidth = host.containerWidth - headerLeftWidth;
                drawGradientRect(host, 
                    cacheCtx,
                    host.containerX,
                    0,
                    headerLeftWidth,
                    23,
                    0xbda9a9,
                    0x8b7a88,
                );
                drawGradientRect(host, 
                    cacheCtx,
                    host.containerX + headerLeftWidth,
                    0,
                    headerRightWidth,
                    23,
                    0x4f4f4f,
                    0x292929,
                );

                // Draw "Select a world" title - centered in left header
                drawCenteredText(host, 
                    cacheCtx,
                    host.fontBold12,
                    "Select a world",
                    host.containerX + headerLeftWidth / 2,
                    15,
                    0x000000,
                );

                // Draw members/free legend with stars
                if (host.worldSelectStarSprites && host.worldSelectStarSprites.length >= 2) {
                    drawSprite(host, 
                        cacheCtx,
                        host.worldSelectStarSprites[1],
                        host.xPadding + 140,
                        1,
                    );
                    drawText(host, 
                        cacheCtx,
                        host.fontPlain11,
                        "Members only world",
                        host.xPadding + 152,
                        10,
                        0xffffff,
                    );
                    drawSprite(host, 
                        cacheCtx,
                        host.worldSelectStarSprites[0],
                        host.xPadding + 140,
                        12,
                    );
                    drawText(host, 
                        cacheCtx,
                        host.fontPlain11,
                        "Free world",
                        host.xPadding + 152,
                        21,
                        0xffffff,
                    );
                }

                // Draw sort arrows and column headers
                if (host.worldSelectArrowSprites && host.worldSelectArrowSprites.length >= 4) {
                    drawSortColumn(host, cacheCtx, host.xPadding + 280, "World", 0);
                    drawSortColumn(host, cacheCtx, host.xPadding + 390, "Players", 1);
                    drawSortColumn(host, cacheCtx, host.xPadding + 500, "Location", 2);
                    drawSortColumn(host, cacheCtx, host.xPadding + 610, "Type", 3);
                }

                // Draw cancel button
                cacheCtx.fillStyle = "#000000";
                cacheCtx.fillRect(host.xPadding + 708, 4, 50, 16);
                drawCenteredText(host, 
                    cacheCtx,
                    host.fontPlain11,
                    "Cancel",
                    host.xPadding + 708 + 25,
                    16,
                    0xffffff,
                );

                // Draw pagination arrows
                if (host.worldSelectLeftSprite && state.worldSelectPage > 0) {
                    const arrowY = Math.floor(
                        height / 2 - host.worldSelectLeftSprite.subHeight / 2,
                    );
                    drawSprite(host, cacheCtx, host.worldSelectLeftSprite, 8, arrowY);
                }
                if (
                    host.worldSelectRightSprite &&
                    state.worldSelectPage < state.worldSelectPagesCount
                ) {
                    const arrowX = width - host.worldSelectRightSprite.subWidth - 8;
                    const arrowY = Math.floor(
                        height / 2 - host.worldSelectRightSprite.subHeight / 2,
                    );
                    drawSprite(host, cacheCtx, host.worldSelectRightSprite, arrowX, arrowY);
                }

                // Draw world grid to cache
                let drawY = yOffset + 23;
                let drawX = xOffset + host.xPadding;
                let rowIndex = 0;
                let columnIndex = state.worldSelectPage;

                const startWorldIndex = state.worldSelectPage * rows;
                for (
                    let i = startWorldIndex;
                    i < worldCount && columnIndex - state.worldSelectPage < cols;
                    i++
                ) {
                    const world = sortedWorlds[i];

                    // Determine population text
                    let popText = world.population.toString();
                    if (world.population === -1) {
                        popText = "OFF";
                    } else if (world.population > 1980) {
                        popText = "FULL";
                    }

                    // Determine background type
                    const bgType = getWorldBackgroundType(host, world);

                    // Draw background
                    if (
                        host.worldSelectBackSprites &&
                        bgType < host.worldSelectBackSprites.length
                    ) {
                        drawSprite(host, 
                            cacheCtx,
                            host.worldSelectBackSprites[bgType],
                            drawX,
                            drawY,
                        );
                    }

                    // Draw flag
                    if (host.worldSelectFlagSprites) {
                        const isMember = (world.properties & WorldFlags.MEMBERS) !== 0;
                        const flagIndex = (isMember ? 8 : 0) + world.location;
                        if (flagIndex < host.worldSelectFlagSprites.length) {
                            drawSprite(host, 
                                cacheCtx,
                                host.worldSelectFlagSprites[flagIndex],
                                drawX + 29,
                                drawY,
                            );
                        }
                    }

                    // Draw world ID
                    const worldIdColor =
                        (world.properties & WorldFlags.HIGH_RISK) !== 0 ? 0xff0000 : 0x000000;
                    drawCenteredText(host, 
                        cacheCtx,
                        host.fontBold12,
                        world.id.toString(),
                        drawX + 15,
                        drawY + rowHeight / 2 + 5,
                        worldIdColor,
                    );

                    // Draw population
                    drawCenteredText(host, 
                        cacheCtx,
                        host.fontPlain11,
                        popText,
                        drawX + 60,
                        drawY + rowHeight / 2 + 5,
                        0x0ffffff,
                    );

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

                // Update cache metadata
                host.worldSelectCachePage = state.worldSelectPage;
                host.worldSelectCacheSortOption = host.worldSortOption;
                host.worldSelectCacheSortDirection = host.worldSortDirection;
                host.worldSelectCacheWidth = width;
                host.worldSelectCacheHeight = height;
            }
        }

        // Draw cached world grid to main canvas
        if (host.worldSelectCache) {
            ctx.drawImage(host.worldSelectCache, 0, 0);
        }

        // Use consolidated hover detection (eliminates duplicate loop)
        const hoverResult = findHoveredWorld(host, sortedWorlds, layout, state.worldSelectPage);
        state.hoveredWorldId = hoverResult.world ? hoverResult.world.id : -1;

        // Draw hover highlight overlay (just a simple rectangle)
        if (hoverResult.world) {
            ctx.save();
            ctx.globalAlpha = 0.5;
            ctx.fillStyle = "#ffffff";
            ctx.fillRect(hoverResult.x, hoverResult.y, rowWidth, rowHeight);
            ctx.restore();
        }

        // Draw hover tooltip
        if (hoverResult.world) {
            const activity = hoverResult.world.activity || "-";
            const tooltipWidth = measureText(host, host.fontPlain11, activity) + 6;
            const tooltipHeight = host.fontPlain11.lineHeight + 8;
            let tooltipY = host.mouseY + 25;
            if (tooltipHeight + tooltipY > 480) {
                tooltipY = host.mouseY - 25 - tooltipHeight;
            }
            const tooltipX = host.mouseX - tooltipWidth / 2;

            // Draw tooltip background
            ctx.fillStyle = "#ffff70";
            ctx.fillRect(tooltipX, tooltipY, tooltipWidth, tooltipHeight);
            ctx.strokeStyle = "#000000";
            ctx.lineWidth = 1;
            ctx.strokeRect(tooltipX, tooltipY, tooltipWidth, tooltipHeight);

            // Draw tooltip text
            drawCenteredText(host, 
                ctx,
                host.fontPlain11,
                activity,
                host.mouseX,
                tooltipY + host.fontPlain11.lineHeight + 4,
                0x000000,
            );
        }
    
}
