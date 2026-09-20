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
import { type DrawBackend, createDrawBackend } from "../../DrawBackend";
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
import type { WebGLOsrsRendererHost } from "../hostInterface";
import { RENDER_CONSTANTS, MAX_TEXTURES, WATER_TEXTURE_SIZE, WATER_TEXTURE_ASSETS, DEFAULT_WATER_MATERIAL, SWAMP_WATER_MATERIAL, ICE_WATER_MATERIAL, VANILLA_WATER_SURFACE_COLORS, waterRgb, WaterMaterialParams } from "../constants";

export function initTextures(host: WebGLOsrsRendererHost, ): void {

        const textureLoader = host.osrsClient.textureLoader;
        if (!textureLoader) return;

        const allTextureIds = textureLoader.getTextureIds();

        host.textureIds = allTextureIds
            .filter((id) => textureLoader.isSd(id))
            .slice(0, MAX_TEXTURES - 1);

        host.textureIdIndexMap.clear();
        host.textureFrameCounts.clear();
        for (let i = 0; i < host.textureIds.length; i++) {
            const id = host.textureIds[i];
            host.textureIdIndexMap.set(id, i + 1);
            host.textureFrameCounts.set(id, 1);
        }
        host.textureLayerCount = host.textureIds.length + 1;

        host.initTextureArray();
        host.initMaterialsTexture();

        // console.log("init textures", host.textureIds, allTextureIds.length);
    
}

export async function initWaterTextures(host: WebGLOsrsRendererHost, ): Promise<void> {

        let data: Uint8Array;
        try {
            data = await host.loadWaterTextureData();
            host.waterShadingUnavailable = false;
        } catch (error) {
            console.log(
                "[water] Failed to load water textures; water renders with the vanilla texture path",
                error,
            );
            host.waterShadingUnavailable = true;
            data = new Uint8Array(
                WATER_TEXTURE_SIZE * WATER_TEXTURE_SIZE * 4 * WATER_TEXTURE_ASSETS.length,
            );
        }

        host.waterTextures?.delete();
        host.waterTextures = createTextureArray(
            host.app,
            data,
            WATER_TEXTURE_SIZE,
            WATER_TEXTURE_SIZE,
            WATER_TEXTURE_ASSETS.length,
            {
                internalFormat: PicoGL.RGBA8,
                type: PicoGL.UNSIGNED_BYTE,
                minFilter: PicoGL.LINEAR_MIPMAP_LINEAR,
                magFilter: PicoGL.LINEAR,
                wrapS: PicoGL.REPEAT,
                wrapT: PicoGL.REPEAT,
            },
        );
    
}

export async function loadWaterTextureData(host: WebGLOsrsRendererHost, ): Promise<Uint8Array> {

        const images = await Promise.all(
            WATER_TEXTURE_ASSETS.map((src) => host.loadImageAsset(src)),
        );
        const canvas = document.createElement("canvas");
        canvas.width = WATER_TEXTURE_SIZE;
        canvas.height = WATER_TEXTURE_SIZE;
        const context = canvas.getContext("2d", { willReadFrequently: true });
        if (!context) {
            throw new Error("Could not create canvas context for water texture upload");
        }

        const data = new Uint8Array(
            WATER_TEXTURE_SIZE * WATER_TEXTURE_SIZE * 4 * WATER_TEXTURE_ASSETS.length,
        );
        for (let layer = 0; layer < images.length; layer++) {
            context.clearRect(0, 0, WATER_TEXTURE_SIZE, WATER_TEXTURE_SIZE);
            context.drawImage(images[layer], 0, 0, WATER_TEXTURE_SIZE, WATER_TEXTURE_SIZE);
            const imageData = context.getImageData(0, 0, WATER_TEXTURE_SIZE, WATER_TEXTURE_SIZE);
            data.set(imageData.data, layer * WATER_TEXTURE_SIZE * WATER_TEXTURE_SIZE * 4);
        }
        return data;
    
}

export function loadImageAsset(host: WebGLOsrsRendererHost, src: string): Promise<HTMLImageElement> {

        return new Promise((resolve, reject) => {
            const image = new Image();
            image.onload = () => resolve(image);
            image.onerror = () => reject(new Error(`Failed to load image asset ${src}`));
            image.src = src;
        });
    
}

export function collectWaterTextureIds(host: WebGLOsrsRendererHost, ): Set<number> {

        if (host.waterShadingUnavailable) {
            return new Set();
        }
        host.collectWaterOverlayColors();
        return new Set(KNOWN_WATER_TEXTURE_IDS);
    
}

export function collectWaterOverlayColors(host: WebGLOsrsRendererHost, ): void {

        host.waterOverlayColors.clear();
        const loaderFactory = host.osrsClient.loaderFactory;
        if (!loaderFactory?.getOverlayTypeLoader) {
            return;
        }

        let overlayTypeLoader: ReturnType<typeof loaderFactory.getOverlayTypeLoader>;
        try {
            overlayTypeLoader = loaderFactory.getOverlayTypeLoader();
        } catch {
            return;
        }

        const overlayCount = overlayTypeLoader.getCount();
        for (let overlayId = 0; overlayId < overlayCount; overlayId++) {
            let overlay: OverlayFloorType;
            try {
                overlay = overlayTypeLoader.load(overlayId);
            } catch {
                continue;
            }

            const textureId = overlay?.textureId ?? -1;
            if (
                !KNOWN_WATER_TEXTURE_IDS.has(textureId) ||
                host.waterOverlayColors.has(textureId) ||
                (overlay.primaryRgb & 0xffffff) === 0
            ) {
                continue;
            }
            host.waterOverlayColors.set(textureId, waterRgb(overlay.primaryRgb));
        }
    
}

export function getWaterMaterialParams(host: WebGLOsrsRendererHost, textureId: number): WaterMaterialParams {

        if (textureId === 25) {
            return SWAMP_WATER_MATERIAL;
        }
        if (textureId === 91) {
            return ICE_WATER_MATERIAL;
        }

        const surfaceColor =
            VANILLA_WATER_SURFACE_COLORS.get(textureId) ?? host.waterOverlayColors.get(textureId);
        if (surfaceColor) {
            return {
                ...DEFAULT_WATER_MATERIAL,
                surfaceColor,
            };
        }

        return DEFAULT_WATER_MATERIAL;
    
}
