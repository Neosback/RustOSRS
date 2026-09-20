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
} from "../../network/ServerConnection";
import { sendLogin } from "../../network/ServerConnection";
import { flushPackets } from "../../network/packet";
import { createTextureArray } from "../../picogl/PicoTexture";
import { RS_TO_RADIANS } from "../../rs/MathConstants";
import { CollisionFlag } from "../../common/CollisionFlag";
import {
    getWorldLocChanges,
    getWorldLocSpawns,
    getWorldTerrainOverrides,
} from "../../common/gamemode/GamemodeContentStore";
import { OsrsMenuEntry } from "../../rs/MenuEntry";
import { MenuTargetType } from "../../rs/MenuEntry";
import type { OverlayFloorType } from "../../rs/config/floortype/OverlayFloorType";
import { LocModelLoader } from "../../rs/config/loctype/LocModelLoader";
import { LocModelType } from "../../rs/config/loctype/LocModelType";
import { NpcModelLoader } from "../../rs/config/npctype/NpcModelLoader";
import { NpcDrawPriority, NpcType } from "../../rs/config/npctype/NpcType";
import { PlayerAppearance } from "../../rs/config/player/PlayerAppearance";
import { PlayerModelLoader } from "../../rs/config/player/PlayerModelLoader";
import { decodeInteractionIndex } from "../../rs/interaction/InteractionIndex";
import { getMapIndexFromTile, getMapPlaneId, getMapSquareId } from "../../rs/map/MapFileIndex";
import { Model } from "../../rs/model/Model";
import { ModelData } from "../../rs/model/ModelData";
import { Scene } from "../../rs/scene/Scene";
import { getUiScale } from "../../ui/UiScale";
import { ClickCrossOverlay } from "../../ui/devoverlay/ClickCrossOverlay";
import { GroundItemOverlay } from "../../ui/devoverlay/GroundItemOverlay";
import { HealthBarOverlay } from "../../ui/devoverlay/HealthBarOverlay";
import { HitsplatOverlay } from "../../ui/devoverlay/HitsplatOverlay";
import {
    InteractHighlightDrawTarget,
    InteractHighlightOverlay,
} from "../../ui/devoverlay/InteractHighlightOverlay";
import { LoadingMessageOverlay } from "../../ui/devoverlay/LoadingMessageOverlay";
import { LoginOverlay } from "../../ui/devoverlay/LoginOverlay";
import { OverheadPrayerOverlay } from "../../ui/devoverlay/OverheadPrayerOverlay";
import { OverheadTextOverlay } from "../../ui/devoverlay/OverheadTextOverlay";
import {
    HealthBarEntry,
    HitsplatEntry,
    OverheadPrayerEntry,
    OverheadTextEntry,
    type OverlayUpdateArgs,
    RenderPhase,
} from "../../ui/devoverlay/Overlay";
import { OverlayManager } from "../../ui/devoverlay/OverlayManager";
import type { TileMarkerOverlay } from "../../ui/devoverlay/TileMarkerOverlay";
import { TileTextOverlay } from "../../ui/devoverlay/TileTextOverlay";
import { WidgetsOverlay } from "../../ui/devoverlay/WidgetsOverlay";
import { MENU_ACTION_DEPRIORITIZE_OFFSET, MenuAction, menuAction } from "../../ui/menu/MenuAction";
import { worldEntriesToSimple } from "../../ui/menu/MenuBridge";
import type { MenuClickContext, SimpleMenuEntry } from "../../ui/menu/MenuEngine";
import { chooseDefaultMenuEntry, shouldLeftClickOpenMenu } from "../../ui/menu/MenuEngine";
import { MenuOpcode } from "../../ui/menu/MenuState";
import { Model2DRenderer } from "../../ui/model/Model2DRenderer";
import {
    canTargetGroundItem,
    canTargetNpc,
    canTargetObject,
    canTargetPlayer,
} from "../../widgets/WidgetFlags";
import { WidgetLoader } from "../../widgets/WidgetLoader";
import { WidgetManager } from "../../widgets/WidgetManager";
import { layoutWidgets } from "../../widgets/layout/WidgetLayout";
import { collectWidgetsAtPoint } from "../../widgets/menu/utils";
import {
    getCanvasCssSize,
    isIos,
    isMobileMode,
    isTouchDevice,
    isWebGL2Supported,
} from "../../common/utils/DeviceUtil";
import { clamp } from "../../common/utils/MathUtil";
import { ClientState } from "../../game/ClientState";
import { GameRenderer } from "../../game/GameRenderer";
import type { HitsplatEventPayload } from "../../game/GameRenderer";
import { OsrsRendererType, WEBGL } from "../../game/GameRenderers";
import { ClickMode, getMousePos } from "../../game/InputManager";
import { OsrsClient } from "../../game/OsrsClient";
import { ActorAnimationClip } from "../../game/actor/ActorAnimation";
import {
    ActorHealthBarsState,
    ActorHitsplatState,
    HealthBarBarState,
    HealthBarDefinitionState,
    HealthBarUpdateState,
    MAX_HITSPLAT_SLOTS,
    createActorHealthBarsState,
    createActorHitsplatState,
} from "../../game/actor/ActorOverlayState";
import type { ClientGroundItemStack, GroundItemOverlayEntry } from "../../game/data/ground/GroundItemStore";
import { NpcEcs } from "../../game/ecs/NpcEcs";
import type { PlayerAnimKey } from "../../game/ecs/PlayerEcs";
import { GameState, LoginIndex } from "../../game/login";
import { Ray, rayIntersectsBox } from "../../game/math/Raycast";
import { isMouseInUIRegion as checkMouseInUIRegion } from "../../game/menu/WorldMenuBuilder";
import {
    advanceAnimation,
    computeMovementOrientation,
    computeMovementStep,
    interpolateRotation,
    parseInteractionTarget,
} from "../../game/movement/NpcClientTick";
import type { TileMarkersPluginConfig } from "../../game/plugins/tilemarkers/types";
import { computeRoofPlaneLimit } from "../../game/roof/RoofVisibility";
import { sampleBridgeHeightForWorldTile } from "../../game/scene/BridgeHeightSampler";
import {
    BridgePlaneStrategy,
    resolveBridgePromotedPlane,
    resolveCollisionSamplePlaneForLocal,
    resolveCollisionSamplePlaneForWorldTile,
    resolveGroundItemStackPlane,
    resolveHeightSamplePlaneForLocal,
    resolveInteractionPlaneForLocal,
    resolveInteractionPlaneForWorldTile,
} from "../../game/scene/PlaneResolver";
import { SceneRaycastHit, SceneRaycaster } from "../../game/scene/SceneRaycaster";
import {
    TILE_FLAG_BRIDGE,
    getTileRenderFlagAt as lookupTileRenderFlagAt,
} from "../../game/scene/TileRenderFlags";
import { LoadingRequirement } from "../../game/state/LoadingTracker";
import type { PlayerSpotAnimationEvent } from "../../game/sync/PlayerSyncTypes";
import { RAD_TO_RS_UNITS, computeFacingRotation } from "../../game/utils/rotation";
import { AnimationFrames } from "../AnimationFrames";
import { ChatheadFactory } from "../ChatheadFactory";
import { type DrawBackend, createDrawBackend } from "../DrawBackend";
import { DrawRange, NULL_DRAW_RANGE, newDrawRange } from "../DrawRange";
import { InteractType } from "../InteractType";
import { profiler } from "../PerformanceProfiler";
import { PlayerChatheadFactory } from "../PlayerChatheadFactory";
import { resolveFogRange } from "../RenderDistancePolicy";
import { WebGLMapSquare } from "../WebGLMapSquare";
import { WorldEntityAnimator } from "../WorldEntityAnimator";
import { SceneBuffer } from "../buffer/SceneBuffer";
import { getModelFaces, isModelFaceTransparent } from "../buffer/SceneBuffer";
import { GfxManager } from "../gfx/GfxManager";
import { GfxRenderer } from "../gfx/GfxRenderer";
import { buildGroundItemGeometry } from "../ground/GroundItemMeshBuilder";
import { type MinimapIcon, SdMapData } from "../loader/SdMapData";
import { SdMapDataLoader } from "../loader/SdMapDataLoader";
import { SdMapLoaderInput } from "../loader/SdMapLoaderInput";
import { isDoorLocType } from "../loc/SceneLocs";
import {
    DynamicNpcAnimLoader,
    DynamicNpcFrameGeometry,
    DynamicNpcSequenceMeta,
} from "../npc/DynamicNpcAnimLoader";
import { PlayerRenderer } from "../player/PlayerRenderer";
import { ProjectileManager } from "../projectiles/ProjectileManager";
import { ProjectileRenderer } from "../projectiles/ProjectileRenderer";
import {
    FRAME_FXAA_PROGRAM,
    FRAME_PROGRAM,
    createMainProgram,
    createNpcProgram,
    createPlayerProgram,
    createProjectileProgram,
} from "../shaders/Shaders";
import { KNOWN_WATER_TEXTURE_IDS } from "../water/WaterTextureIds";
import type { AmbientSoundInstance } from "../../game/audio/SoundEffectSystem";
import type { WebGLOsrsRendererHost } from "./hostInterface";
import { RENDER_CONSTANTS } from "./constants";

export function addAmbientSoundInstance(host: WebGLOsrsRendererHost, 
        locId: number,
        soundType: any,
        x: number,
        y: number,
        z: number,
        orientation: number,
        sizeX: number,
        sizeY: number,
    ): void {

        const idx = host.ambientSoundBufferIndex;
        const buffer = host.ambientSoundBuffer;

        // Reuse existing object or create new one
        let inst = buffer[idx];
        if (!inst) {
            inst = {} as AmbientSoundInstance;
            buffer[idx] = inst;
        }

        // Update all properties
        inst.locId = locId;
        inst.soundId = soundType ? soundType.ambientSoundId : -1;
        inst.x = x;
        inst.y = y;
        inst.z = z;
        inst.maxDistance = soundType ? soundType.soundMaxDistance : 0;
        inst.minDistance = soundType ? soundType.soundMinDistance : 0;
        inst.changeTicksMin = soundType ? soundType.ambientSoundChangeTicksMin : 0;
        inst.changeTicksMax = soundType ? soundType.ambientSoundChangeTicksMax : 0;
        inst.soundIds = soundType ? soundType.ambientSoundIds : undefined;
        inst.sizeX = sizeX;
        inst.sizeY = sizeY;
        inst.orientation = orientation;
        inst.fadeInDurationMs = (soundType && soundType.soundFadeInDuration) || undefined;
        inst.fadeOutDurationMs = (soundType && soundType.soundFadeOutDuration) || undefined;
        inst.fadeInCurve = (soundType && soundType.soundFadeInCurve) || undefined;
        inst.fadeOutCurve = (soundType && soundType.soundFadeOutCurve) || undefined;
        inst.distanceFadeCurve = (soundType && soundType.soundDistanceFadeCurve) || undefined;
        inst.distanceOverride = soundType
            ? (soundType.soundAreaRadiusOverride ?? undefined)
            : undefined;
        inst.loopSequentially = soundType ? soundType.loopMultiSoundSequentially : false;
        inst.deferSwap = soundType ? soundType.deferredAmbientSwap : false;
        inst.exactPosition = soundType ? soundType.useExactSoundPosition : false;
        inst.resetOnLoop = soundType ? soundType.resetAmbientOnLoopRestart : false;

        host.ambientSoundBufferIndex = idx + 1;
    
}

export function locTypeHasSound(locType: any): boolean{

        return (
            !!locType &&
            (locType.ambientSoundId !== -1 ||
                (locType.ambientSoundIds && locType.ambientSoundIds.length > 0))
        );
    
}

export function locHasSoundPotential(host: WebGLOsrsRendererHost, locId: number): boolean {

        const cached = host.locSoundPotentialCache.get(locId);
        if (cached !== undefined) return cached;
        let result = false;
        const locType = host.osrsClient.locTypeLoader.load(locId);
        if (locType) {
            result = locTypeHasSound(locType);
            if (!result && locType.transforms) {
                for (const transformId of locType.transforms) {
                    if (transformId === -1) continue;
                    const transformed = host.osrsClient.locTypeLoader.load(transformId);
                    if (locTypeHasSound(transformed)) {
                        result = true;
                        break;
                    }
                }
            }
        }
        host.locSoundPotentialCache.set(locId, result);
        return result;
    
}

export function getMapSoundEmitters(host: WebGLOsrsRendererHost, 
        map: WebGLMapSquare,
    ): { locId: number; x: number; y: number; level: number; rot: number }[] {

        if (map.ambientSoundEmitters) return map.ambientSoundEmitters;

        const emitters: { locId: number; x: number; y: number; level: number; rot: number }[] = [];
        const mapBaseX = map.mapX * 64;
        const mapBaseY = map.mapY * 64;
        const offsetsByLevel = map.tileLocOffsetsByLevel;
        const idsByLevel = map.tileLocIdsByLevel;
        const typeRotsByLevel = map.tileLocTypeRotByLevel;

        if (offsetsByLevel && idsByLevel) {
            for (let level = 0; level < offsetsByLevel.length; level++) {
                const offsets = offsetsByLevel[level];
                const ids = idsByLevel[level];
                if (!offsets || !ids || offsets.length < 2) continue;
                const typeRots = typeRotsByLevel?.[level];
                const span = Math.round(Math.sqrt(offsets.length - 1));

                for (let localY = 0; localY < span; localY++) {
                    for (let localX = 0; localX < span; localX++) {
                        const tileIdx = localY * span + localX;
                        const start = offsets[tileIdx] | 0;
                        const end = offsets[tileIdx + 1] | 0;
                        for (let i = start; i < end; i++) {
                            const locId = ids[i] | 0;
                            if (!host.locHasSoundPotential(locId)) continue;
                            const packed = typeRots ? typeRots[i] | 0 : 0;
                            emitters.push({
                                locId,
                                x: (mapBaseX + localX) * 128 + 64,
                                y: (mapBaseY + localY) * 128 + 64,
                                level,
                                rot: (packed >> 6) & 3,
                            });
                        }
                    }
                }
            }
        }

        map.ambientSoundEmitters = emitters;
        return emitters;
    
}

export function addAmbientEmitter(host: WebGLOsrsRendererHost, 
        locId: number,
        x: number,
        y: number,
        level: number,
        rot: number,
    ): void {

        const baseType = host.osrsClient.locTypeLoader.load(locId);
        if (!baseType) return;

        // Resolve varbit transforms for the active sound fields; the emitter
        // persists with no sound while the active transform is silent so it
        // can fade between states without losing loop phase or timers.
        let soundType: any = baseType;
        if (baseType.transforms) {
            soundType = baseType.transform(
                host.osrsClient.varManager,
                host.osrsClient.locTypeLoader,
            );
        }
        if (soundType && !locTypeHasSound(soundType)) {
            soundType = undefined;
        }

        host.addAmbientSoundInstance(
            locId,
            soundType,
            x,
            y,
            level * 128,
            rot,
            baseType.sizeX || 1,
            baseType.sizeY || 1,
        );
    
}

export function collectAmbientSounds(host: WebGLOsrsRendererHost, map: WebGLMapSquare): void {

        // Animated locs (sparse, iterate all levels)
        if (map.locsAnimated) {
            for (const loc of map.locsAnimated) {
                if (!host.locHasSoundPotential(loc.id)) continue;
                host.addAmbientEmitter(loc.id, loc.x, loc.y, loc.level, loc.rotation);
            }
        }

        // Static locs from the cached per-map emitter list
        const emitters = host.getMapSoundEmitters(map);
        for (let i = 0; i < emitters.length; i++) {
            const emitter = emitters[i];
            host.addAmbientEmitter(emitter.locId, emitter.x, emitter.y, emitter.level, emitter.rot);
        }
    
}
