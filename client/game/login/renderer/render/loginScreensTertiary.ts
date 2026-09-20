import type { LoginState } from "../../LoginState";
import type { LoginRendererHost, RenderContext } from "../host";
import { drawButton, drawCenteredText } from "./drawUtils";

export function drawBanned(host: LoginRendererHost, ctx: RenderContext, state: LoginState) {

        if (!host.fontBold12) return;
        drawCenteredText(host, 
            ctx,
            host.fontBold12,
            "Your account has been disabled.",
            host.loginBoxX + 180,
            201,
            0xffff00,
            true,
        );
        drawButton(host, ctx, host.loginBoxX + 180, 276, "Appeal");
        drawButton(host, ctx, host.loginBoxX + 180, 326, "Back");
    
}

export function drawOkMessage(host: LoginRendererHost, ctx: RenderContext, state: LoginState) {

        if (!host.fontBold12) return;
        drawCenteredText(host, 
            ctx,
            host.fontBold12,
            state.response1,
            host.loginBoxX + 180,
            221,
            0xffffff,
            true,
        );
        drawCenteredText(host, 
            ctx,
            host.fontBold12,
            state.response2,
            host.loginBoxX + 180,
            236,
            0xffffff,
            true,
        );
        drawCenteredText(host, 
            ctx,
            host.fontBold12,
            state.response3,
            host.loginBoxX + 180,
            251,
            0xffffff,
            true,
        );
        drawButton(host, ctx, host.loginBoxX + 180, 301, "Ok");
    
}

export function drawDobNotSet(host: LoginRendererHost, ctx: RenderContext, state: LoginState) {

        if (!host.fontBold12) return;
        drawCenteredText(host, 
            ctx,
            host.fontBold12,
            "Your date of birth isn't set.",
            host.loginBoxX + 180,
            216,
            0xffff00,
            true,
        );
        drawButton(host, ctx, host.loginBoxX + 180 - 80, 321, "Set Date of Birth");
        drawButton(host, ctx, host.loginBoxX + 180 + 80, 321, "Back");
    
}

export function drawDownloadLauncher(host: LoginRendererHost, ctx: RenderContext, state: LoginState) {

        if (!host.fontBold12) return;
        drawCenteredText(host, 
            ctx,
            host.fontBold12,
            state.response1,
            host.loginBoxX + 180,
            201,
            0xffff00,
            true,
        );
        drawButton(host, ctx, host.loginBoxX + 180, 276, "Download Launcher");
        drawButton(host, ctx, host.loginBoxX + 180, 326, "Back");
    
}

export function drawWorldHopWarning(host: LoginRendererHost, ctx: RenderContext, state: LoginState) {

        if (!host.fontBold12) return;
        // World hop warning is similar to the WARNING screen
        drawCenteredText(host, 
            ctx,
            host.fontBold12,
            state.response0,
            host.loginBoxX + 180,
            201,
            0xffff00,
            true,
        );
        drawCenteredText(host, 
            ctx,
            host.fontBold12,
            state.response1,
            host.loginBoxX + 180,
            236,
            0xffffff,
            true,
        );
        drawCenteredText(host, 
            ctx,
            host.fontBold12,
            state.response2,
            host.loginBoxX + 180,
            251,
            0xffffff,
            true,
        );
        drawCenteredText(host, 
            ctx,
            host.fontBold12,
            state.response3,
            host.loginBoxX + 180,
            266,
            0xffffff,
            true,
        );
        drawButton(host, ctx, host.loginBoxCenter - 80, 321, "Continue");
        drawButton(host, ctx, host.loginBoxCenter + 80, 321, "Cancel");
    
}
