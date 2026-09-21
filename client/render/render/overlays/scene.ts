import Denque from "denque";
import { mat4, vec2, vec3, vec4 } from "gl-matrix";
import { button, folder } from "leva";
import { Schema } from "leva/dist/declarations/src/types";
import {
    DrawCall,
    Framebuffer,
    App as PicoApp,
    PicoGL,
    Program,
    Renderbuffer,
    Texture,
    Timer,
    UniformBuffer,
    VertexArray,
    VertexBuffer,
} from "picogl";

import {
    getClientCycle,
    getCurrentTick,
    getServerTickPhaseNow,
    isServerConnected,
    sendEmote,
    sendInteractFollow,
    sendInteractStop,
    subscribeTick,
} from "../../../network/ServerConnection";
import { sendLogin } from "../../../network/ServerConnection";
import { flushPackets } from "../../../network/packet";
import { createTextureArray } from "../../../picogl/PicoTexture";
import { RS_TO_RADIANS } from "../../../rs/MathConstants";
import { CollisionFlag } from "../../../common/CollisionFlag";
import {
    getWorldLocChanges,
    getWorldLocSpawns,
    getWorldTerrainOverrides,
} from "../../../common/gamemode/GamemodeContentStore";
import { OsrsMenuEntry } from "../../../rs/MenuEntry";
import { MenuTargetType } from "../../../rs/MenuEntry";
import type { OverlayFloorType } from "../../../rs/config/floortype/OverlayFloorType";
import { LocModelLoader } from "../../../rs/config/loctype/LocModelLoader";
import { LocModelType } from "../../../rs/config/loctype/LocModelType";
import { NpcModelLoader } from "../../../rs/config/npctype/NpcModelLoader";
import { NpcDrawPriority, NpcType } from "../../../rs/config/npctype/NpcType";
import { PlayerAppearance } from "../../../rs/config/player/PlayerAppearance";
import { PlayerModelLoader } from "../../../rs/config/player/PlayerModelLoader";
import { decodeInteractionIndex } from "../../../rs/interaction/InteractionIndex";
import { getMapIndexFromTile, getMapPlaneId, getMapSquareId } from "../../../rs/map/MapFileIndex";
import { Model } from "../../../rs/model/Model";
import { ModelData } from "../../../rs/model/ModelData";
import { Scene } from "../../../rs/scene/Scene";
import { getUiScale } from "../../../ui/UiScale";
import { ClickCrossOverlay } from "../../../ui/devoverlay/ClickCrossOverlay";
import { GroundItemOverlay } from "../../../ui/devoverlay/GroundItemOverlay";
import { HealthBarOverlay } from "../../../ui/devoverlay/HealthBarOverlay";
import { HitsplatOverlay } from "../../../ui/devoverlay/HitsplatOverlay";
import {
    InteractHighlightDrawTarget,
    InteractHighlightOverlay,
} from "../../../ui/devoverlay/InteractHighlightOverlay";
import { LoadingMessageOverlay } from "../../../ui/devoverlay/LoadingMessageOverlay";
import { LoginOverlay } from "../../../ui/devoverlay/LoginOverlay";
import { OverheadPrayerOverlay } from "../../../ui/devoverlay/OverheadPrayerOverlay";
import { OverheadTextOverlay } from "../../../ui/devoverlay/OverheadTextOverlay";
import {
    HealthBarEntry,
    HitsplatEntry,
    OverheadPrayerEntry,
    OverheadTextEntry,
    type OverlayUpdateArgs,
    RenderPhase,
} from "../../../ui/devoverlay/Overlay";
import { OverlayManager } from "../../../ui/devoverlay/OverlayManager";
import type { TileMarkerOverlay } from "../../../ui/devoverlay/TileMarkerOverlay";
import { TileTextOverlay } from "../../../ui/devoverlay/TileTextOverlay";
import { WidgetsOverlay } from "../../../ui/devoverlay/WidgetsOverlay";
import { MENU_ACTION_DEPRIORITIZE_OFFSET, MenuAction, menuAction } from "../../../ui/menu/MenuAction";
import { worldEntriesToSimple } from "../../../ui/menu/MenuBridge";
import type { MenuClickContext, SimpleMenuEntry } from "../../../ui/menu/MenuEngine";
import { chooseDefaultMenuEntry, shouldLeftClickOpenMenu } from "../../../ui/menu/MenuEngine";
import { MenuOpcode } from "../../../ui/menu/MenuState";
import { Model2DRenderer } from "../../../ui/model/Model2DRenderer";
import {
    canTargetGroundItem,
    canTargetNpc,
    canTargetObject,
    canTargetPlayer,
} from "../../../widgets/WidgetFlags";
import { WidgetLoader } from "../../../widgets/WidgetLoader";
import { WidgetManager } from "../../../widgets/WidgetManager";
import { layoutWidgets } from "../../../widgets/layout/WidgetLayout";
import { collectWidgetsAtPoint } from "../../../widgets/menu/utils";
import {
    getCanvasCssSize,
    isIos,
    isMobileMode,
    isTouchDevice,
    isWebGL2Supported,
} from "../../../common/utils/DeviceUtil";
import { clamp } from "../../../common/utils/MathUtil";
import { ClientState } from "../../../game/ClientState";
import { GameRenderer } from "../../../game/GameRenderer";
import type { HitsplatEventPayload } from "../../../game/GameRenderer";
import { OsrsRendererType, WEBGL } from "../../../game/GameRenderers";
import { ClickMode, getMousePos } from "../../../game/InputManager";
import { OsrsClient } from "../../../game/OsrsClient";
import { ActorAnimationClip } from "../../../game/actor/ActorAnimation";
import {
    ActorHealthBarsState,
    ActorHitsplatState,
    HealthBarBarState,
    HealthBarDefinitionState,
    HealthBarUpdateState,
    MAX_HITSPLAT_SLOTS,
    createActorHealthBarsState,
    createActorHitsplatState,
} from "../../../game/actor/ActorOverlayState";
import type { ClientGroundItemStack, GroundItemOverlayEntry } from "../../../game/data/ground/GroundItemStore";
import { NpcEcs } from "../../../game/ecs/NpcEcs";
import type { PlayerAnimKey } from "../../../game/ecs/PlayerEcs";
import { GameState, LoginIndex } from "../../../game/login";
import { Ray, rayIntersectsBox } from "../../../game/math/Raycast";
import { isMouseInUIRegion as checkMouseInUIRegion } from "../../../game/menu/WorldMenuBuilder";
import {
    advanceAnimation,
    computeMovementOrientation,
    computeMovementStep,
    interpolateRotation,
    parseInteractionTarget,
} from "../../../game/movement/NpcClientTick";
import type { TileMarkersPluginConfig } from "../../../game/plugins/tilemarkers/types";
import { computeRoofPlaneLimit } from "../../../game/roof/RoofVisibility";
import { sampleBridgeHeightForWorldTile } from "../../../game/scene/BridgeHeightSampler";
import {
    BridgePlaneStrategy,
    resolveBridgePromotedPlane,
    resolveCollisionSamplePlaneForLocal,
    resolveCollisionSamplePlaneForWorldTile,
    resolveGroundItemStackPlane,
    resolveHeightSamplePlaneForLocal,
    resolveInteractionPlaneForLocal,
    resolveInteractionPlaneForWorldTile,
} from "../../../game/scene/PlaneResolver";
import { SceneRaycastHit, SceneRaycaster } from "../../../game/scene/SceneRaycaster";
import {
    TILE_FLAG_BRIDGE,
    getTileRenderFlagAt as lookupTileRenderFlagAt,
} from "../../../game/scene/TileRenderFlags";
import { LoadingRequirement } from "../../../game/state/LoadingTracker";
import type { PlayerSpotAnimationEvent } from "../../../game/sync/PlayerSyncTypes";
import { RAD_TO_RS_UNITS, computeFacingRotation } from "../../../game/utils/rotation";
import { AnimationFrames } from "../../AnimationFrames";
import { ChatheadFactory } from "../../ChatheadFactory";
import { DrawRange, NULL_DRAW_RANGE, newDrawRange } from "../../DrawRange";
import { InteractType } from "../../InteractType";
import { profiler } from "../../PerformanceProfiler";
import { PlayerChatheadFactory } from "../../PlayerChatheadFactory";
import { resolveFogRange } from "../../RenderDistancePolicy";
import { WebGLMapSquare } from "../../WebGLMapSquare";
import { WorldEntityAnimator } from "../../WorldEntityAnimator";
import { SceneBuffer } from "../../buffer/SceneBuffer";
import { getModelFaces, isModelFaceTransparent } from "../../buffer/SceneBuffer";
import { GfxManager } from "../../gfx/GfxManager";
import { GfxRenderer } from "../../gfx/GfxRenderer";
import { buildGroundItemGeometry } from "../../ground/GroundItemMeshBuilder";
import { type MinimapIcon, SdMapData } from "../../loader/SdMapData";
import { SdMapDataLoader } from "../../loader/SdMapDataLoader";
import { SdMapLoaderInput } from "../../loader/SdMapLoaderInput";
import { isDoorLocType } from "../../loc/SceneLocs";
import {
    DynamicNpcAnimLoader,
    DynamicNpcFrameGeometry,
    DynamicNpcSequenceMeta,
} from "../../npc/DynamicNpcAnimLoader";
import { PlayerRenderer } from "../../player/PlayerRenderer";
import { ProjectileManager } from "../../projectiles/ProjectileManager";
import { ProjectileRenderer } from "../../projectiles/ProjectileRenderer";
import {
    FRAME_FXAA_PROGRAM,
    FRAME_PROGRAM,
    createMainProgram,
    createNpcProgram,
    createPlayerProgram,
    createProjectileProgram,
} from "../../shaders/Shaders";
import { KNOWN_WATER_TEXTURE_IDS } from "../../water/WaterTextureIds";
import {
    isRustPrimaryRendererActive,
    mirrorRustSceneOverlay,
} from "../../rust/RustShadowIntegration";
import type { WebGLOsrsRenderer } from "../../WebGLOsrsRenderer";
import type { WebGLOsrsRendererHost } from "../hostInterface";
import { RENDER_CONSTANTS } from "../constants";

export function ensureOverlayUpdateArgs(host: WebGLOsrsRendererHost, scenePass: boolean): OverlayUpdateArgs {

        const key = scenePass ? "cachedSceneOverlayUpdateArgs" : "cachedOverlayUpdateArgs";
        let args = host[key];
        if (!args) {
            args = {
                time: 0,
                delta: 0,
                resolution: { width: 0, height: 0 },
                state: {
                    hoverEnabled: false,
                    hoverTile: { x: 0, y: 0 },
                    playerLevel: 0,
                    playerRawLevel: 0,
                    destTile: undefined,
                    currentTile: undefined,
                    tileHighlights: undefined,
                    clientTickPhase: 0,
                    playerWorldX: undefined,
                    playerWorldZ: undefined,
                    actorServerTiles: undefined,
                },
                helpers: host.getOverlayHelpers(),
            };
            host[key] = args;
        }
        return args;
    
}

export function syncTileMarkerOverlayConfig(host: WebGLOsrsRendererHost, tileMarkersConfig: TileMarkersPluginConfig): void {

        if (!host.tileMarkerOverlay) {
            return;
        }
        host.tileMarkerOverlay.setDestinationColor(tileMarkersConfig.destinationTileColor);
        host.tileMarkerOverlay.setCurrentTileColor(tileMarkersConfig.currentTileColor);
    
}

export function populateTileMarkerOverlayState(host: WebGLOsrsRendererHost, 
        state: OverlayUpdateArgs["state"],
        tileMarkersConfig: TileMarkersPluginConfig,
        playerLevel: number,
        playerRawLevel: number,
    ): void {
        state.hoverEnabled = !!host.osrsClient.hoverOverlayEnabled;
        if (host.hoverTileX !== -1 && host.hoverTileY !== -1) {
            if (!state.hoverTile) {
                state.hoverTile = { x: 0, y: 0 };
            }
            state.hoverTile.x = host.hoverTileX | 0;
            state.hoverTile.y = host.hoverTileY | 0;
            const picked = host.osrsClient.hoveredTile;
            state.hoverTile.plane =
                picked &&
                (picked.tileX | 0) === state.hoverTile.x &&
                (picked.tileY | 0) === state.hoverTile.y
                    ? picked.plane
                    : undefined;
        } else {
            state.hoverTile = undefined;
        }

        state.playerLevel = playerLevel;
        state.playerRawLevel = playerRawLevel;
        state.destTile = undefined;
        state.currentTile = undefined;

        const destWorldX = ClientState.destinationWorldX | 0;
        const destWorldY = ClientState.destinationWorldY | 0;
        let activeDestX = destWorldX;
        let activeDestY = destWorldY;
        if (activeDestX === 0 && activeDestY === 0) {
            const destLocalX = ClientState.destinationX | 0;
            const destLocalY = ClientState.destinationY | 0;
            if (destLocalX !== 0 || destLocalY !== 0) {
                activeDestX = ClientState.localToWorldX(destLocalX) | 0;
                activeDestY = ClientState.localToWorldY(destLocalY) | 0;
            }
        }
        const hasActiveDestination = activeDestX !== 0 || activeDestY !== 0;
        const nativeTileHighlights = host.osrsClient.tileHighlightManager.getRenderEntries();
        const shouldOwnDestinationTile =
            tileMarkersConfig.enabled &&
            tileMarkersConfig.showDestinationTile &&
            hasActiveDestination;
        const destinationColor = tileMarkersConfig.destinationTileColor & 0xffffff;
        const defaultNativeDestinationColor = 0xa9a753;
        const visibleTileHighlights = shouldOwnDestinationTile
            ? nativeTileHighlights.filter((highlight) => {
                if ((highlight.slot | 0) === 4) {
                    return false;
                }
                const color = highlight.colorRgb & 0xffffff;
                return color !== destinationColor && color !== defaultNativeDestinationColor;
            })
            : nativeTileHighlights;
        state.tileHighlights = visibleTileHighlights.length > 0 ? visibleTileHighlights : undefined;

        if (!tileMarkersConfig.enabled) {
            return;
        }

        const nativeHasCurrentTile = visibleTileHighlights.some(
            (highlight) => (highlight.slot | 0) === 3,
        );
        if (tileMarkersConfig.showDestinationTile && hasActiveDestination) {
            if (!state.destTile) {
                state.destTile = { x: 0, y: 0 };
            }
            state.destTile.x = activeDestX;
            state.destTile.y = activeDestY;
        }

        if (!tileMarkersConfig.showCurrentTile || nativeHasCurrentTile) {
            return;
        }

        const controlledServerId = host.osrsClient.controlledPlayerServerId | 0;
        if (controlledServerId <= 0) {
            return;
        }

        const movementState = host.osrsClient.playerMovementSync?.getState?.(controlledServerId);
        if (!movementState) {
            return;
        }

        const ecsIndex = movementState.ecsIndex | 0;
        const isMoving = ecsIndex >= 0 && host.osrsClient.playerEcs.isMoving(ecsIndex);
        if (!isMoving) {
            return;
        }

        if (!state.currentTile) {
            state.currentTile = { x: 0, y: 0, plane: 0 };
        }
        state.currentTile.x = movementState.tileX | 0;
        state.currentTile.y = movementState.tileY | 0;
        state.currentTile.plane = host.getHeightSamplePlaneForTile(
            movementState.tileX | 0,
            movementState.tileY | 0,
            host.getPlayerRawPlane() | 0,
        );
    }

const rustTileOverlayLineVertices = new Float32Array(15);
const rustTileOverlayFillVertices = new Float32Array(12);
const rustTileOverlayColor = new Float32Array(4);

function setRustTileOverlayColor(colorRgb: number, alpha: number): void {
    const rgb = colorRgb >>> 0;
    rustTileOverlayColor[0] = ((rgb >> 16) & 0xff) / 255;
    rustTileOverlayColor[1] = ((rgb >> 8) & 0xff) / 255;
    rustTileOverlayColor[2] = (rgb & 0xff) / 255;
    rustTileOverlayColor[3] = Math.max(0, Math.min(1, alpha));
}

function mirrorRustDepthAwareTile(
    host: WebGLOsrsRendererHost,
    args: OverlayUpdateArgs,
    tileX: number,
    tileY: number,
    plane: number,
    colorRgb: number,
    fillAlpha: number,
    drawFill: boolean,
): void {
    const sampleHeight = args.helpers.getTileHeightAtPlane;
    const corners = [
        [tileX, sampleHeight(tileX, tileY, plane), tileY],
        [tileX + 1, sampleHeight(tileX + 1, tileY, plane), tileY],
        [tileX + 1, sampleHeight(tileX + 1, tileY + 1, plane), tileY + 1],
        [tileX, sampleHeight(tileX, tileY + 1, plane), tileY + 1],
    ] as const;

    if (drawFill && fillAlpha > 0) {
        for (let i = 0; i < 4; i++) {
            const corner = corners[i];
            const offset = i * 3;
            rustTileOverlayFillVertices[offset] = corner[0];
            rustTileOverlayFillVertices[offset + 1] = corner[1] - 0.015;
            rustTileOverlayFillVertices[offset + 2] = corner[2];
        }
        setRustTileOverlayColor(colorRgb, fillAlpha);
        mirrorRustSceneOverlay(
            host,
            rustTileOverlayFillVertices,
            rustTileOverlayColor,
            true,
        );
    }

    for (let i = 0; i < 4; i++) {
        const corner = corners[i];
        const offset = i * 3;
        rustTileOverlayLineVertices[offset] = corner[0];
        rustTileOverlayLineVertices[offset + 1] = corner[1] - 0.02;
        rustTileOverlayLineVertices[offset + 2] = corner[2];
    }
    rustTileOverlayLineVertices[12] = corners[0][0];
    rustTileOverlayLineVertices[13] = corners[0][1] - 0.02;
    rustTileOverlayLineVertices[14] = corners[0][2];
    setRustTileOverlayColor(colorRgb, 1);
    mirrorRustSceneOverlay(
        host,
        rustTileOverlayLineVertices,
        rustTileOverlayColor,
        false,
    );
}

function mirrorRustDepthAwareTileOverlays(
    host: WebGLOsrsRendererHost,
    args: OverlayUpdateArgs,
    tileMarkersConfig: TileMarkersPluginConfig,
): void {
    const state = args.state;
    const getHeightSamplePlaneForTile = (
        args.helpers as OverlayUpdateArgs["helpers"] & {
            getHeightSamplePlaneForTile?: (
                tileX: number,
                tileY: number,
                basePlane: number,
            ) => number;
        }
    ).getHeightSamplePlaneForTile;
    const resolvePlane = (
        tile: { x: number; y: number; plane?: number },
    ): number => {
        if (typeof tile.plane === "number" && Number.isFinite(tile.plane)) {
            return tile.plane | 0;
        }
        const basePlane =
            (state.playerRawLevel ?? state.playerLevel) | 0;
        return getHeightSamplePlaneForTile
            ? getHeightSamplePlaneForTile(tile.x | 0, tile.y | 0, basePlane)
            : args.helpers.getEffectivePlaneForTile(
                tile.x | 0,
                tile.y | 0,
                basePlane,
            );
    };

    if (state.hoverEnabled && state.hoverTile) {
        mirrorRustDepthAwareTile(
            host,
            args,
            state.hoverTile.x | 0,
            state.hoverTile.y | 0,
            resolvePlane(state.hoverTile),
            0xffff00,
            0.25,
            true,
        );
    }
    if (state.destTile) {
        mirrorRustDepthAwareTile(
            host,
            args,
            state.destTile.x | 0,
            state.destTile.y | 0,
            resolvePlane(state.destTile),
            tileMarkersConfig.destinationTileColor,
            0.2,
            true,
        );
    }
    if (state.currentTile) {
        mirrorRustDepthAwareTile(
            host,
            args,
            state.currentTile.x | 0,
            state.currentTile.y | 0,
            resolvePlane(state.currentTile),
            tileMarkersConfig.currentTileColor,
            0,
            false,
        );
    }

    const highlights = state.tileHighlights;
    if (!highlights) return;
    for (const highlight of highlights) {
        if (!highlight || highlight.alwaysOnTop) continue;
        const plane = args.helpers.getEffectivePlaneForTile(
            highlight.x,
            highlight.y,
            highlight.plane,
        );
        mirrorRustDepthAwareTile(
            host,
            args,
            highlight.x,
            highlight.y,
            plane,
            highlight.colorRgb,
            highlight.fillAlpha,
            highlight.fillAlpha > 0,
        );
    }
}

export function drawSceneTileOverlays(host: WebGLOsrsRendererHost, time: number, deltaTime: number): void {

        if (host.uiHidden || !host.overlayManager || !host.tileMarkerOverlay) {
            return;
        }

        const tileMarkersConfig = host.osrsClient.tileMarkersPlugin.getConfig();
        host.syncTileMarkerOverlayConfig(tileMarkersConfig);

        const playerLevel = host.getPlayerBasePlane() | 0;
        const playerRawLevel = host.getPlayerRawPlane() | 0;
        const args = host.ensureOverlayUpdateArgs(true);
        args.time = time;
        args.delta = deltaTime;
        args.resolution.width = host.app.width;
        args.resolution.height = host.app.height;
        host.populateTileMarkerOverlayState(
            args.state,
            tileMarkersConfig,
            playerLevel,
            playerRawLevel,
        );
        args.state.clientTickPhase = host.clientTickPhase;
        args.state.playerWorldX = undefined;
        args.state.playerWorldZ = undefined;
        args.state.actorServerTiles = undefined;
        args.state.hitsplats = undefined;
        args.state.healthBars = undefined;
        args.state.overheadTexts = undefined;
        args.state.overheadPrayers = undefined;
        args.state.groundItems = undefined;
        host.overlayManager.update(args);
        if (!isRustPrimaryRendererActive(host)) {
            host.overlayManager.draw(RenderPhase.ToSceneFramebuffer);
        }
        mirrorRustDepthAwareTileOverlays(host, args, tileMarkersConfig);
    
}

export function getOverlayHelpers(host: WebGLOsrsRendererHost, ): NonNullable<WebGLOsrsRenderer["cachedOverlayHelpers"]> {

        if (!host.cachedOverlayHelpers) {
            host.cachedOverlayHelpers = {
                getTileHeightAtPlane: host.getTileHeightAtPlane.bind(host),
                getMinTileHeightInRadius: host.getMinTileHeightInRadius.bind(host),
                sampleHeightAtExactPlane: host.sampleHeightAtExactPlane.bind(host),
                getHeightSamplePlaneForTile: host.getHeightSamplePlaneForTile.bind(host),
                getEffectivePlaneForTile: host.getEffectivePlaneForTile.bind(host),
                getOccupancyPlaneForTile: host.getOccupancyPlaneForTile.bind(host),
                getTileRenderFlagAt: host.getTileRenderFlagAt.bind(host),
                isBridgeSurfaceTile: host.isBridgeSurfaceTile.bind(host),
                worldToScreen: host.worldToScreen.bind(host),
                getCollisionFlagAt: host.getCollisionFlagAt.bind(host),
            };
        }
        return host.cachedOverlayHelpers;
    
}

export function getTileRenderFlagAt(host: WebGLOsrsRendererHost, level: number, tileX: number, tileY: number): number {

        return lookupTileRenderFlagAt(host.mapManager, level, tileX, tileY);
    
}
