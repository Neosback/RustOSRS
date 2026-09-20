import { WorldFlags } from "../types";
import type { LoginState } from "../../LoginState";
import type { LoginRendererHost, RenderContext } from "../host";
import { drawGradientRect, drawCenteredText, drawText, drawSprite } from "../render/drawUtils";
import { getGridLayout, getSortedWorlds, getWorldBackgroundType } from "./worldData";
import { drawSortColumn } from "./worldSelectSort";

export function drawWorldSelectGridOnly(host: LoginRendererHost, ctx: RenderContext, state: LoginState, width: number, height: number) {

        if (!host.fontBold12 || !host.fontPlain11) return;

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

        state.worldSelectPagesCount = Math.max(0, totalColumns - columnsPerPage);
        host.currentSortedWorlds = sortedWorlds;

        // Draw directly to provided context (for title cache)
        ctx.fillStyle = "#000000";
        ctx.fillRect(host.containerX, 23, host.containerWidth, host.containerHeight - 23);

        const headerLeftWidth = 125;
        const headerRightWidth = host.containerWidth - headerLeftWidth;
        drawGradientRect(host, ctx, host.containerX, 0, headerLeftWidth, 23, 0xbda9a9, 0x8b7a88);
        drawGradientRect(host, 
            ctx,
            host.containerX + headerLeftWidth,
            0,
            headerRightWidth,
            23,
            0x4f4f4f,
            0x292929,
        );

        drawCenteredText(host, 
            ctx,
            host.fontBold12,
            "Select a world",
            host.containerX + headerLeftWidth / 2,
            15,
            0x000000,
        );

        if (host.worldSelectStarSprites && host.worldSelectStarSprites.length >= 2) {
            drawSprite(host, ctx, host.worldSelectStarSprites[1], host.xPadding + 140, 1);
            drawText(host, 
                ctx,
                host.fontPlain11,
                "Members only world",
                host.xPadding + 152,
                10,
                0xffffff,
            );
            drawSprite(host, ctx, host.worldSelectStarSprites[0], host.xPadding + 140, 12);
            drawText(host, ctx, host.fontPlain11, "Free world", host.xPadding + 152, 21, 0xffffff);
        }

        if (host.worldSelectArrowSprites && host.worldSelectArrowSprites.length >= 4) {
            drawSortColumn(host, ctx, host.xPadding + 280, "World", 0);
            drawSortColumn(host, ctx, host.xPadding + 390, "Players", 1);
            drawSortColumn(host, ctx, host.xPadding + 500, "Location", 2);
            drawSortColumn(host, ctx, host.xPadding + 610, "Type", 3);
        }

        ctx.fillStyle = "#000000";
        ctx.fillRect(host.xPadding + 708, 4, 50, 16);
        drawCenteredText(host, 
            ctx,
            host.fontPlain11,
            "Cancel",
            host.xPadding + 708 + 25,
            16,
            0xffffff,
        );

        if (host.worldSelectLeftSprite && state.worldSelectPage > 0) {
            const arrowY = Math.floor(height / 2 - host.worldSelectLeftSprite.subHeight / 2);
            drawSprite(host, ctx, host.worldSelectLeftSprite, 8, arrowY);
        }
        if (host.worldSelectRightSprite && state.worldSelectPage < state.worldSelectPagesCount) {
            const arrowX = width - host.worldSelectRightSprite.subWidth - 8;
            const arrowY = Math.floor(height / 2 - host.worldSelectRightSprite.subHeight / 2);
            drawSprite(host, ctx, host.worldSelectRightSprite, arrowX, arrowY);
        }

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

            let popText = world.population.toString();
            if (world.population === -1) {
                popText = "OFF";
            } else if (world.population > 1980) {
                popText = "FULL";
            }

            const bgType = getWorldBackgroundType(host, world);

            if (host.worldSelectBackSprites && bgType < host.worldSelectBackSprites.length) {
                drawSprite(host, ctx, host.worldSelectBackSprites[bgType], drawX, drawY);
            }

            if (host.worldSelectFlagSprites) {
                const isMember = (world.properties & WorldFlags.MEMBERS) !== 0;
                const flagIndex = (isMember ? 8 : 0) + world.location;
                if (flagIndex < host.worldSelectFlagSprites.length) {
                    drawSprite(host, ctx, host.worldSelectFlagSprites[flagIndex], drawX + 29, drawY);
                }
            }

            const worldIdColor =
                (world.properties & WorldFlags.HIGH_RISK) !== 0 ? 0xff0000 : 0x000000;
            drawCenteredText(host, 
                ctx,
                host.fontBold12,
                world.id.toString(),
                drawX + 15,
                drawY + rowHeight / 2 + 5,
                worldIdColor,
            );
            drawCenteredText(host, 
                ctx,
                host.fontPlain11,
                popText,
                drawX + 60,
                drawY + rowHeight / 2 + 5,
                0x0ffffff,
            );

            drawY += rowHeight + yGap;
            rowIndex++;
            if (rowIndex >= rows) {
                drawY = yOffset + 23;
                drawX += xGap + rowWidth;
                rowIndex = 0;
                columnIndex++;
            }
        }
    
}
