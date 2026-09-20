import { WorldFlags } from "../types";
import type { LoginState } from "../../LoginState";
import type { LoginRendererHost, RenderContext } from "../host";
import { getSortedWorlds } from "./worldData";
import { drawCenteredText, drawText, drawSprite, measureText } from "../render/drawUtils";

export function drawMobileWorldSelectList(host: LoginRendererHost, ctx: RenderContext, state: LoginState, width: number, height: number) {

        if (!host.fontBold12 || !host.fontPlain11) return;

        const sortedWorlds = getSortedWorlds(host);
        host.currentSortedWorlds = sortedWorlds;

        // Mobile list constants
        const ROW_HEIGHT = 60; // Touch-friendly row height (vs 19px grid)
        const HEADER_HEIGHT = 50;
        const CLOSE_BUTTON_SIZE = 44; // Touch target minimum
        const PADDING = 16;

        // Apply momentum scrolling
        if (Math.abs(state.mobileWorldSelectScrollVelocity) > 0.5) {
            state.mobileWorldSelectScrollOffset += state.mobileWorldSelectScrollVelocity;
            state.mobileWorldSelectScrollVelocity *= 0.92; // Friction
        } else {
            state.mobileWorldSelectScrollVelocity = 0;
        }

        // Clamp scroll bounds
        const maxScroll = Math.max(0, sortedWorlds.length * ROW_HEIGHT - (height - HEADER_HEIGHT));
        state.mobileWorldSelectScrollOffset = Math.max(
            0,
            Math.min(maxScroll, state.mobileWorldSelectScrollOffset),
        );

        // Draw full-screen background
        ctx.fillStyle = "#1a1a1a";
        ctx.fillRect(0, 0, width, height);

        // Draw header bar
        ctx.fillStyle = "#2d2d2d";
        ctx.fillRect(0, 0, width, HEADER_HEIGHT);

        // Draw header title
        drawCenteredText(host, 
            ctx,
            host.fontBold12,
            "Select World",
            width / 2,
            HEADER_HEIGHT / 2 + 5,
            0xffffff,
        );

        // Draw close button (X) in top right
        const closeX = width - CLOSE_BUTTON_SIZE - 4;
        const closeY = (HEADER_HEIGHT - CLOSE_BUTTON_SIZE) / 2;
        ctx.fillStyle = "#444444";
        ctx.fillRect(closeX, closeY, CLOSE_BUTTON_SIZE, CLOSE_BUTTON_SIZE);
        ctx.strokeStyle = "#666666";
        ctx.lineWidth = 1;
        ctx.strokeRect(closeX, closeY, CLOSE_BUTTON_SIZE, CLOSE_BUTTON_SIZE);
        drawCenteredText(host, 
            ctx,
            host.fontBold12,
            "X",
            closeX + CLOSE_BUTTON_SIZE / 2,
            closeY + CLOSE_BUTTON_SIZE / 2 + 5,
            0xffffff,
        );

        // Draw world list
        const listY = HEADER_HEIGHT;
        const listHeight = height - HEADER_HEIGHT;
        const scrollOffset = state.mobileWorldSelectScrollOffset;

        // Calculate visible range for efficient rendering
        const firstVisibleIndex = Math.floor(scrollOffset / ROW_HEIGHT);
        const lastVisibleIndex = Math.min(
            sortedWorlds.length - 1,
            Math.ceil((scrollOffset + listHeight) / ROW_HEIGHT),
        );

        // Clip to list area
        ctx.save();
        ctx.beginPath();
        ctx.rect(0, listY, width, listHeight);
        ctx.clip();

        for (let i = firstVisibleIndex; i <= lastVisibleIndex; i++) {
            const world = sortedWorlds[i];
            const rowY = listY + i * ROW_HEIGHT - scrollOffset;

            // Row background (alternating colors)
            const isMember = (world.properties & WorldFlags.MEMBERS) !== 0;
            if (i % 2 === 0) {
                ctx.fillStyle = isMember ? "#1e2a1e" : "#1a1a1a";
            } else {
                ctx.fillStyle = isMember ? "#253025" : "#222222";
            }
            ctx.fillRect(0, rowY, width, ROW_HEIGHT);

            // Highlight hovered/selected world
            if (world.id === state.hoveredWorldId) {
                ctx.fillStyle = "rgba(255, 255, 255, 0.15)";
                ctx.fillRect(0, rowY, width, ROW_HEIGHT);
            }

            // Draw flag icon
            if (host.worldSelectFlagSprites) {
                const flagIndex = (isMember ? 8 : 0) + world.location;
                if (flagIndex < host.worldSelectFlagSprites.length) {
                    const flagSprite = host.worldSelectFlagSprites[flagIndex];
                    drawSprite(host, ctx, flagSprite, PADDING, rowY + (ROW_HEIGHT - 16) / 2);
                }
            }

            // Draw world ID (larger for touch)
            const worldIdX = PADDING + 40;
            const worldIdColor =
                (world.properties & WorldFlags.HIGH_RISK) !== 0 ? 0xff6666 : 0xffffff;
            drawText(host, 
                ctx,
                host.fontBold12,
                `World ${world.id}`,
                worldIdX,
                rowY + 22,
                worldIdColor,
            );

            // Draw activity text below world ID
            const activityText = world.activity || "-";
            drawText(host, ctx, host.fontPlain11, activityText, worldIdX, rowY + 40, 0xaaaaaa);

            // Draw population on right side
            let popText: string;
            let popColor: number;
            if (world.population === -1) {
                popText = "Offline";
                popColor = 0x888888;
            } else if (world.population > 1980) {
                popText = "Full";
                popColor = 0xff6666;
            } else if (world.population > 1500) {
                popText = `${world.population}`;
                popColor = 0xffaa00;
            } else {
                popText = `${world.population}`;
                popColor = 0x66ff66;
            }
            const popWidth = measureText(host, host.fontBold12, popText);
            drawText(host, 
                ctx,
                host.fontBold12,
                popText,
                width - PADDING - popWidth,
                rowY + ROW_HEIGHT / 2 + 5,
                popColor,
            );

            // Draw population label
            drawText(host, 
                ctx,
                host.fontPlain11,
                "players",
                width - PADDING - popWidth,
                rowY + ROW_HEIGHT / 2 + 18,
                0x666666,
            );

            // Draw separator line
            ctx.strokeStyle = "#333333";
            ctx.lineWidth = 1;
            ctx.beginPath();
            ctx.moveTo(PADDING, rowY + ROW_HEIGHT - 0.5);
            ctx.lineTo(width - PADDING, rowY + ROW_HEIGHT - 0.5);
            ctx.stroke();
        }

        ctx.restore();

        // Draw scroll indicator if content overflows
        if (maxScroll > 0) {
            const scrollBarHeight = Math.max(
                30,
                (listHeight / (maxScroll + listHeight)) * listHeight,
            );
            const scrollBarY = listY + (scrollOffset / maxScroll) * (listHeight - scrollBarHeight);
            ctx.fillStyle = "rgba(255, 255, 255, 0.3)";
            ctx.fillRect(width - 6, scrollBarY, 4, scrollBarHeight);
        }
    
}

export function getMobileWorldIndexAtPosition(host: LoginRendererHost, state: LoginState, x: number, y: number, width: number, height: number) {

        const ROW_HEIGHT = 60;
        const HEADER_HEIGHT = 50;
        const CLOSE_BUTTON_SIZE = 44;

        // Check close button first
        const closeX = width - CLOSE_BUTTON_SIZE - 4;
        const closeY = (HEADER_HEIGHT - CLOSE_BUTTON_SIZE) / 2;
        if (
            x >= closeX &&
            x <= closeX + CLOSE_BUTTON_SIZE &&
            y >= closeY &&
            y <= closeY + CLOSE_BUTTON_SIZE
        ) {
            return -2; // Special value for close button
        }

        // Check if in list area
        if (y < HEADER_HEIGHT) return -1;

        // Calculate which row was tapped
        const listY = y - HEADER_HEIGHT + state.mobileWorldSelectScrollOffset;
        const index = Math.floor(listY / ROW_HEIGHT);

        if (index >= 0 && index < host.currentSortedWorlds.length) {
            return index;
        }

        return -1;
    
}
