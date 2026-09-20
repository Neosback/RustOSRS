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
import { RENDER_CONSTANTS, MATERIAL_TEXTURE_ROWS, WATER_FLAG_HAS_FOAM, WATER_FLAG_NORMAL_MAP_2, materialByte } from "../constants";

export function initMaterialsTexture(host: WebGLOsrsRendererHost, ): void {

        if (host.textureMaterials) {
            host.textureMaterials.delete();
            host.textureMaterials = undefined;
        }

        const textureCount = host.textureLayerCount || 1;
        const waterTextureIds = host.collectWaterTextureIds();

        // Row 0: animU, animV, alphaCutOff, frameCount
        // Row 1: animSpeed, material flags, water flags, (unused)
        // Row 2: water surface RGB, base opacity
        // Row 3: water depth RGB, fresnel amount
        // Row 4: normal strength, specular strength, specular gloss, duration
        // Row 5: water foam RGB, (unused)
        const data = new Int8Array(textureCount * MATERIAL_TEXTURE_ROWS * 4);
        data[3] = 1; // frameCount for fallback layer 0

        for (let i = 0; i < host.textureIds.length; i++) {
            const id = host.textureIds[i];
            try {
                const material = host.osrsClient.textureLoader.getMaterial(id);
                const frameCount = host.textureFrameCounts.get(id) ?? material.frameCount ?? 1;
                const baseLayer = host.textureIdIndexMap.get(id) ?? 0;

                for (
                    let frame = 0;
                    frame < frameCount && baseLayer + frame < textureCount;
                    frame++
                ) {
                    const layerIndex = baseLayer + frame;
                    const row0 = layerIndex * 4;
                    const row1 = (textureCount + layerIndex) * 4;
                    const row2 = (textureCount * 2 + layerIndex) * 4;
                    const row3 = (textureCount * 3 + layerIndex) * 4;
                    const row4 = (textureCount * 4 + layerIndex) * 4;
                    const row5 = (textureCount * 5 + layerIndex) * 4;
                    const isWater = waterTextureIds.has(id);

                    data[row0] = material.animU;
                    data[row0 + 1] = material.animV;
                    data[row0 + 2] = materialByte(material.alphaCutOff * 255);
                    data[row0 + 3] = materialByte(frameCount);

                    data[row1] = material.animSpeed;
                    data[row1 + 1] = isWater ? 1 : 0;

                    if (isWater) {
                        const water = host.getWaterMaterialParams(id);
                        data[row1 + 2] =
                            (water.hasFoam ? WATER_FLAG_HAS_FOAM : 0) |
                            (water.useNormalMap2 ? WATER_FLAG_NORMAL_MAP_2 : 0);

                        data[row2] = materialByte(water.surfaceColor[0] * 255);
                        data[row2 + 1] = materialByte(water.surfaceColor[1] * 255);
                        data[row2 + 2] = materialByte(water.surfaceColor[2] * 255);
                        data[row2 + 3] = materialByte(water.baseOpacity * 255);

                        data[row3] = materialByte(water.depthColor[0] * 255);
                        data[row3 + 1] = materialByte(water.depthColor[1] * 255);
                        data[row3 + 2] = materialByte(water.depthColor[2] * 255);
                        data[row3 + 3] = materialByte(water.fresnelAmount * 255);

                        data[row4] = materialByte((water.normalStrength / 0.5) * 255);
                        data[row4 + 1] = materialByte(water.specularStrength * 255);
                        data[row4 + 2] = materialByte((water.specularGloss / 500) * 255);
                        data[row4 + 3] = materialByte((water.duration / 4) * 255);

                        data[row5] = materialByte(water.foamColor[0] * 255);
                        data[row5 + 1] = materialByte(water.foamColor[1] * 255);
                        data[row5 + 2] = materialByte(water.foamColor[2] * 255);
                    }
                }
            } catch (e) {
                console.error("Failed loading texture", id, e);
            }
        }

        host.textureMaterials = host.app.createTexture2D(
            data,
            textureCount,
            MATERIAL_TEXTURE_ROWS,
            {
                minFilter: PicoGL.NEAREST,
                magFilter: PicoGL.NEAREST,
                internalFormat: PicoGL.RGBA8I,
            },
        );
    
}
