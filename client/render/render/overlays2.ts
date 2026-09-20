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
import type { WebGLOsrsRendererHost } from "./hostInterface";
import { RENDER_CONSTANTS } from "./constants";

export function addHitSplatOsrs(host: WebGLOsrsRendererHost, 
        state: ActorHitsplatState,
        type: number,
        value: number,
        type2: number,
        value2: number,
        currentCycle: number,
        delayCycles: number,
    ): void {

        // Mirror Actor.addHitSplat exactly
        let allExpired = true; // var7
        let allActive = true; // var8
        for (let i = 0; i < 4; i++) {
            if ((state.hitSplatCycles[i] | 0) > (currentCycle | 0)) {
                allExpired = false;
            } else {
                allActive = false;
            }
        }

        let slot = -1; // var9
        let compareType = -1;
        let displayCycles = 0;
        if ((type | 0) >= 0) {
            const def = host.hitsplatOverlay?.getDefinition?.(type | 0);
            if (def) {
                compareType = (def.compareType ?? -1) | 0;
                displayCycles = (def.displayCycles ?? 70) | 0;
            } else {
                compareType = -1;
                displayCycles = 70;
            }
        }

        if (allActive) {
            // All 4 slots are active, need to replace one based on compareType
            if ((compareType | 0) === -1) {
                return; // No replacement priority defined, skip this hitsplat
            }
            slot = 0;
            let best = 0;
            // compareType 0 = replace oldest (lowest cycle), 1 = replace lowest damage
            if ((compareType | 0) === 0) best = state.hitSplatCycles[0] | 0;
            else if ((compareType | 0) === 1) best = state.hitSplatValues[0] | 0;
            for (let i = 1; i < 4; i++) {
                if ((compareType | 0) === 0) {
                    const v = state.hitSplatCycles[i] | 0;
                    if (v < best) {
                        slot = i;
                        best = v;
                    }
                } else if ((compareType | 0) === 1) {
                    const v = state.hitSplatValues[i] | 0;
                    if (v < best) {
                        slot = i;
                        best = v;
                    }
                }
            }
            // If compareType=1 and new value is <= existing lowest, don't replace
            if ((compareType | 0) === 1 && (best | 0) >= (value | 0)) {
                return;
            }
        } else {
            // At least one slot is expired, find an empty one
            if (allExpired) {
                state.hitSplatCount = 0;
            }
            for (let i = 0; i < 4; i++) {
                const idx = state.hitSplatCount & 3;
                state.hitSplatCount = (state.hitSplatCount + 1) & 3;
                if ((state.hitSplatCycles[idx] | 0) <= (currentCycle | 0)) {
                    slot = idx;
                    break;
                }
            }
        }

        if (slot >= 0) {
            state.hitSplatTypes[slot] = type | 0;
            state.hitSplatValues[slot] = value | 0;
            state.hitSplatTypes2[slot] = type2 | 0;
            state.hitSplatValues2[slot] = value2 | 0;
            // OSRS: hitSplatCycles[slot] = currentCycle + displayCycles + delayCycles
            // This stores the END cycle (when hitsplat expires)
            // Start visibility = hitSplatCycles - displayCycles (calculated at render time)
            state.hitSplatCycles[slot] = (currentCycle + displayCycles + delayCycles) | 0;
        }
    
}

export function getHitsplatVisibility(host: WebGLOsrsRendererHost, 
        state: ActorHitsplatState,
        slot: number,
        clientCycle: number,
    ): number | undefined {

        const endCycle = state.hitSplatCycles[slot] | 0;
        const type = state.hitSplatTypes[slot] | 0;

        // Type < 0 means unused slot
        if (type < 0) return undefined;

        // Check if expired: hitSplatCycles <= currentCycle
        if (endCycle <= clientCycle) return undefined;

        // Get displayCycles from definition
        const def = host.hitsplatOverlay?.getDefinition?.(type);
        const displayCycles = (def?.displayCycles ?? 70) | 0;

        // Calculate start cycle: endCycle - displayCycles
        const startCycle = endCycle - displayCycles;

        // Check if not yet visible: startCycle > currentCycle
        if (startCycle > clientCycle) return undefined;

        // Calculate animation progress (0 = just started, 1 = about to expire)
        // remainingCycles = endCycle - currentCycle
        // elapsedCycles = displayCycles - remainingCycles = currentCycle - startCycle
        // animProgress = elapsedCycles / displayCycles
        const remainingCycles = endCycle - clientCycle;
        const elapsedCycles = displayCycles - remainingCycles;
        const animProgress = Math.max(0, Math.min(1, elapsedCycles / displayCycles));

        return animProgress;
    
}

export function trimHitsplats(host: WebGLOsrsRendererHost, tick: number): void {

        const playerEcs = host.osrsClient.playerEcs;
        const controlledId = host.getEffectiveControlledPlayerId();
        for (const [playerId, state] of host.playerHitsplats) {
            let active = false;
            for (let i = 0; i < 4; i++) {
                if ((state.hitSplatCycles[i] | 0) > (tick | 0)) {
                    active = true;
                    break;
                }
            }
            const isControlledPlayer = controlledId > 0 && (playerId | 0) === controlledId;
            const missingEcsEntry = playerEcs.getIndexForServerId(playerId) === undefined;
            if (!active || (missingEcsEntry && !isControlledPlayer)) {
                host.playerHitsplats.delete(playerId);
            }
        }
        const npcEcs = host.osrsClient.npcEcs;
        for (const [serverId, state] of host.npcHitsplats) {
            let active = false;
            for (let i = 0; i < 4; i++) {
                if ((state.hitSplatCycles[i] | 0) > (tick | 0)) {
                    active = true;
                    break;
                }
            }
            const ecsId = npcEcs.getEcsIdForServer(serverId);
            if (!active || ecsId === undefined || !npcEcs.isActive(ecsId)) {
                host.npcHitsplats.delete(serverId);
            }
        }
    
}

export function resolveHealthBarDefinition(host: WebGLOsrsRendererHost, defId: number): HealthBarDefinitionState {

        const def = host.healthBarOverlay?.getDefinition?.(defId | 0);
        return {
            defId: defId | 0,
            int1: (def?.int1 ?? 255) | 0,
            int2: (def?.int2 ?? 255) | 0,
            int3: (def?.int3 ?? -1) | 0,
            stepIncrement: (def?.stepIncrement ?? 1) | 0,
            int5: (def?.int5 ?? 70) | 0,
            width: Math.max(1, Math.min(255, def?.width ?? 30)) | 0,
            widthPadding: Math.max(0, def?.widthPadding ?? 0) | 0,
        };
    
}

export function ensureActorHealthBars(host: WebGLOsrsRendererHost, 
        map: Map<number, ActorHealthBarsState>,
        serverId: number,
    ): ActorHealthBarsState {

        let state = map.get(serverId);
        if (state) return state;
        state = createActorHealthBarsState();
        map.set(serverId, state);
        return state;
    
}

export function healthBarPut(host: WebGLOsrsRendererHost, bar: HealthBarBarState, update: HealthBarUpdateState): void {

        const cycle = update.cycle | 0;
        // Update existing entry at the same cycle.
        for (let i = 0; i < bar.updates.length; i++) {
            if ((bar.updates[i].cycle | 0) === cycle) {
                bar.updates[i] = update;
                return;
            }
        }
        // Insert to keep ascending order by cycle (oldest first).
        let insert = bar.updates.length;
        for (let i = 0; i < bar.updates.length; i++) {
            if ((bar.updates[i].cycle | 0) > cycle) {
                insert = i;
                break;
            }
        }
        bar.updates.splice(insert, 0, update);
        // keep at most 4 updates; drop the oldest.
        if (bar.updates.length > 4) bar.updates.shift();
    
}

export function healthBarGet(host: WebGLOsrsRendererHost, 
        bar: HealthBarBarState,
        clientCycle: number,
    ): HealthBarUpdateState | undefined {

        const now = clientCycle | 0;
        if (bar.updates.length === 0) return undefined;
        if ((bar.updates[0].cycle | 0) > now) return undefined;
        // Promote to the newest update with cycle <= now by removing older entries.
        while (bar.updates.length > 1 && (bar.updates[1].cycle | 0) <= now) {
            bar.updates.shift();
        }
        const current = bar.updates[0];
        const def = bar.def;
        // HealthBarDefinition timings are defined in client cycles (20ms).
        if ((def.int5 | 0) + (current.cycleOffset | 0) + (current.cycle | 0) <= now) {
            bar.updates.shift();
            return undefined;
        }
        return current;
    
}

export function actorAddHealthBar(host: WebGLOsrsRendererHost, 
        state: ActorHealthBarsState,
        defId: number,
        update: HealthBarUpdateState,
    ): void {

        const bars = state.bars;
        // Existing bar -> update its timeline.
        for (const b of bars) {
            if ((b.def.defId | 0) === (defId | 0)) {
                host.healthBarPut(b, update);
                return;
            }
        }

        const def = host.resolveHealthBarDefinition(defId);
        const existingCount = bars.length | 0;
        // only add a 5th bar if we can evict an existing bar with int2 > new.int2
        // (Actor.addHealthBar).
        let removable: HealthBarBarState | undefined = undefined;
        let maxInt2 = def.int2 | 0;
        for (const b of bars) {
            const int2 = b.def.int2 | 0;
            if (int2 > maxInt2) {
                maxInt2 = int2;
                removable = b;
            }
        }
        if (existingCount >= 4 && !removable) return;

        const newBar: HealthBarBarState = { def, updates: [] };
        // Keep bars sorted by definition.int1 descending (Actor.addHealthBar).
        let insertIndex = bars.length;
        for (let i = 0; i < bars.length; i++) {
            if ((bars[i].def.int1 | 0) <= (def.int1 | 0)) {
                insertIndex = i;
                break;
            }
        }
        bars.splice(insertIndex, 0, newBar);

        // If we exceeded the cap, remove the bar with the highest int2.
        if (existingCount >= 4 && removable) {
            const idx = bars.indexOf(removable);
            if (idx >= 0) bars.splice(idx, 1);
        }

        host.healthBarPut(newBar, update);
    
}

export function actorRemoveHealthBar(host: WebGLOsrsRendererHost, state: ActorHealthBarsState, defId: number): void {

        const bars = state.bars;
        for (let i = 0; i < bars.length; i++) {
            if ((bars[i].def.defId | 0) === (defId | 0)) {
                bars.splice(i, 1);
                return;
            }
        }
    
}
