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
import { RENDER_CONSTANTS, LocHighlightTarget, NpcHighlightTarget } from "../constants";

export function buildLocModelHighlightTriangles(host: WebGLOsrsRendererHost, 
        target: LocHighlightTarget,
    ): ReadonlyArray<readonly [number, number, number]> | undefined {

        const locModelLoader = host.getInteractLocModelLoader();
        if (!locModelLoader) return undefined;

        let locType = host.osrsClient.locTypeLoader.load(target.locId | 0);
        if (!locType) return undefined;
        let sizeX = Math.max(1, Number(locType.sizeX ?? 1));
        let sizeY = Math.max(1, Number(locType.sizeY ?? 1));
        if (locType.transforms) {
            const transformed = locType.transform(
                host.osrsClient.varManager,
                host.osrsClient.locTypeLoader,
            );
            if (transformed) {
                locType = transformed;
            }
        }

        const rawType =
            typeof target.locModelType === "number" ? (target.locModelType | 0) & 0x3f : undefined;
        const rawRotation =
            typeof target.locRotation === "number" ? (target.locRotation | 0) & 0x3 : undefined;
        if (rawType === undefined || rawRotation === undefined) {
            return undefined;
        }

        let modelType = rawType;
        let modelRotation = rawRotation;
        if (modelType === LocModelType.NORMAL_DIAGIONAL) {
            modelType = LocModelType.NORMAL;
            modelRotation = (rawRotation + 4) & 0x7;
        }

        // Find the current animation frame from the map's animated loc list.
        // LocAnimated x/y are in RS sub-tile units (localTile * 128 + 64).
        let seqId = locType.seqId ?? -1;
        let seqFrame = 0;
        const locMap = host.getPreferredMapForWorldTile(target.tileX, target.tileY);
        if (locMap) {
            const localTile = host.getMapLocalTile(locMap, target.tileX, target.tileY);
            if (localTile) {
                const rsX = (localTile.x * 128 + 64) | 0;
                const rsY = (localTile.y * 128 + 64) | 0;
                for (const anim of locMap.locsAnimated) {
                    if (
                        anim.id === (target.locId | 0) &&
                        anim.x === rsX &&
                        anim.y === rsY &&
                        anim.level === (target.plane | 0)
                    ) {
                        seqId = anim.seqType?.id ?? seqId;
                        seqFrame = anim.frame | 0;
                        break;
                    }
                }
            }
        }
        let model = locModelLoader.getModelAnimated(
            locType,
            modelType as LocModelType,
            modelRotation,
            seqId,
            seqFrame,
        );

        // Invisible interaction volumes (all faces fully transparent) use a
        // visual proxy loc at the same tile for the highlight outline.
        if (model && host.hasNoVisibleFaces(model)) {
            const proxy = host.findVisualProxyModel(
                locModelLoader,
                target,
                modelType,
                modelRotation,
            );
            if (proxy) {
                model = proxy;
                target.overworldProxy = true;
            }
        }

        if (!model || !model.verticesX || !model.verticesY || !model.verticesZ) {
            return undefined;
        }

        if (rawRotation === 1 || rawRotation === 3) {
            const tmp = sizeX;
            sizeX = sizeY;
            sizeY = tmp;
        }
        const entityX = (target.tileX << 7) + (sizeX << 6);
        const entityZ = (target.tileY << 7) + (sizeY << 6);
        // For world entity overlay locs, sample overworld height (plane 0) — the GPU
        // renders at the overlay's source plane height which sits at sea level, not a
        // full UNITS_LEVEL_HEIGHT below.
        const heightMap = host.getPreferredMapForWorldTile(target.tileX, target.tileY);
        const isOverlay = !target.overworldProxy && heightMap && heightMap.interactionPlane >= 0;
        const heightPlane = isOverlay ? 0 : target.plane | 0;
        let baseY = sampleBridgeHeightForWorldTile(
            host.mapManager,
            entityX / 128.0,
            entityZ / 128.0,
            heightPlane,
            BridgePlaneStrategy.RENDER,
        ).height;
        if (isOverlay) {
            baseY += host.getWorldEntityDeckHeight(0, 0) / 128.0;
        }
        return host.buildModelTrianglePoints(model, (i) => ({
            x: (entityX + model.verticesX[i]) / 128.0,
            y: baseY + model.verticesY[i] / 128.0,
            z: (entityZ + model.verticesZ[i]) / 128.0,
        }));
    
}

export function buildNpcModelHighlightTriangles(host: WebGLOsrsRendererHost, 
        target: NpcHighlightTarget,
    ): ReadonlyArray<readonly [number, number, number]> | undefined {

        const npcEcs = host.osrsClient.npcEcs;
        const ecsId = target.ecsId | 0;
        if (!npcEcs.isActive(ecsId) || !npcEcs.isLinked(ecsId)) return undefined;
        if ((npcEcs.getServerId(ecsId) | 0) !== (target.serverId | 0)) return undefined;

        const npcModelLoader = host.getInteractNpcModelLoader();
        if (!npcModelLoader) return undefined;

        const npcTypeId = npcEcs.getNpcTypeId(ecsId) | 0;
        const npcType = host.osrsClient.npcTypeLoader.load(npcTypeId);
        if (!npcType) return undefined;

        const actionSeqId = npcEcs.getSeqId(ecsId) | 0;
        const actionDelay = npcEcs.getSeqDelay?.(ecsId) | 0;
        const { movementSeqId, idleSeqId } = host.resolveNpcMovementSequenceIds(npcEcs, ecsId);
        const actionActive = actionSeqId >= 0 && actionDelay === 0;
        const seqId = actionActive ? actionSeqId : movementSeqId;
        const frame = Math.max(
            0,
            actionActive
                ? npcEcs.getFrameIndex(ecsId) | 0
                : npcEcs.getMovementFrameIndex?.(ecsId) | 0,
        );
        const movementFrame = Math.max(0, npcEcs.getMovementFrameIndex?.(ecsId) | 0);
        const overlaySeqId =
            actionActive &&
            host.shouldLayerNpcMovementSequence(actionSeqId | 0, movementSeqId | 0, idleSeqId | 0)
                ? movementSeqId | 0
                : -1;

        let model =
            seqId >= 0
                ? npcModelLoader.getModel(
                    npcType,
                    seqId | 0,
                    frame | 0,
                    overlaySeqId | 0,
                    (overlaySeqId >= 0 ? movementFrame : -1) | 0,
                )
                : undefined;
        if (!model) {
            model = npcModelLoader.getModel(npcType, -1, -1);
        }
        if (!model || !model.verticesX || !model.verticesY || !model.verticesZ) {
            return undefined;
        }
        const modelForTriangles = model;

        const mapId = npcEcs.getMapId(ecsId) | 0;
        const mapX = (mapId >> 8) & 0xff;
        const mapY = mapId & 0xff;
        const centerSceneX = (mapX << 13) + (npcEcs.getX(ecsId) | 0);
        const centerSceneZ = (mapY << 13) + (npcEcs.getY(ecsId) | 0);
        const plane = npcEcs.getLevel(ecsId) | 0;
        target.plane = plane;
        // Match NPC rendering height: bridge-aware sampling, ground clearance,
        // and deck height for world entities.
        let baseY = sampleBridgeHeightForWorldTile(
            host.mapManager,
            centerSceneX / 128.0,
            centerSceneZ / 128.0,
            plane | 0,
            BridgePlaneStrategy.RENDER,
        ).height;
        baseY += RENDER_CONSTANTS.ACTOR_GROUND_CLEARANCE_MODEL_UNITS / 128.0;
        const wvId = npcEcs.getWorldViewId?.(ecsId) ?? -1;
        if (wvId >= 0) {
            const deckH = host.getWorldEntityDeckHeight(0, 0);
            baseY += deckH / 128.0;
        }
        const angle = (npcEcs.getRotation(ecsId) | 0) * RS_TO_RADIANS;
        const cos = Math.cos(angle);
        const sin = Math.sin(angle);

        return host.buildModelTrianglePoints(modelForTriangles, (i) => {
            const vx = modelForTriangles.verticesX[i] | 0;
            const vz = modelForTriangles.verticesZ[i] | 0;
            // Match npc.vert.glsl exactly:
            // vec4(vertex.pos, 1.0) * rotationY(angle)
            const rx = vx * cos + vz * sin;
            const rz = -vx * sin + vz * cos;
            return {
                x: (centerSceneX + rx) / 128.0,
                y: baseY + modelForTriangles.verticesY[i] / 128.0,
                z: (centerSceneZ + rz) / 128.0,
            };
        });
    
}
