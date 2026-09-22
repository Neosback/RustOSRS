import { vec2 } from "gl-matrix";
import PicoGL, { DrawCall, Texture, VertexBuffer } from "picogl";

import { EquipmentSlot } from "../../rs/config/player/Equipment";
import { resolveControlledPlayerEcsIndex } from "./ControlledPlayer";
import { PlayerAppearance } from "../../rs/config/player/PlayerAppearance";
import { getMapIndexFromTile } from "../../rs/map/MapFileIndex";
import { Model } from "../../rs/model/Model";
import { clamp } from "../../common/utils/MathUtil";
import { ActorAnimationClip } from "../../game/actor/ActorAnimation";
import type { PlayerAnimKey } from "../../game/ecs/PlayerEcs";
import { resolveHeightSamplePlaneForLocal } from "../../game/scene/PlaneResolver";
import { DrawRange, NULL_DRAW_RANGE, newDrawRange } from "../DrawRange";
import { WebGLMapSquare } from "../WebGLMapSquare";
import type { WebGLOsrsRenderer } from "../WebGLOsrsRenderer";
import { createVertexBatchBuilderIfReady } from "../rust/RustGeometryPreparation";
import {
    isRustPlayerShadowEnabled,
    isRustPrimaryRendererActive,
    mirrorRustPlayerGeometry,
} from "../rust/RustShadowIntegration";

/**
 * PlayerRenderer encapsulates player-specific render passes and instance data handling.
 * It mirrors the NPC rendering pathway but consumes the player geometry and state
 * assembled by WebGLOsrsClientRenderer (dynamic or pre-baked).
 */
const PLAYER_INTERACT_BASE = 0x8000;
const UNANIMATED_PLAYER_COUNT = 200;

export function shouldUseUnanimatedIdlePlayer(
    activePlayerCount: number,
    lowPriority: boolean,
    moving: boolean,
    actionActive: boolean,
    movementSeqId: number,
    idleSeqId: number,
): boolean {
    return (
        activePlayerCount > UNANIMATED_PLAYER_COUNT &&
        lowPriority &&
        !moving &&
        !actionActive &&
        movementSeqId === idleSeqId
    );
}

export function drawPlayerSlots(
    draw: DrawCall,
    slotBuffer: VertexBuffer,
    slotScratch: Int32Array,
    slots: number[],
    elementCount: number,
    submitPico: boolean = true,
): void {
    if (!submitPico) return;
    if (slots.length === 1) {
        draw.uniform("u_usePlayerSlotAttribute", false).uniform("u_drawIdOverride", slots[0] | 0);
        (draw as any).drawRanges([0, elementCount | 0, 1]);
        draw.draw();
        draw.uniform("u_drawIdOverride", -1);
        return;
    }
    for (let index = 0; index < slots.length; index++) slotScratch[index] = slots[index] | 0;
    slotBuffer.data(slotScratch.subarray(0, slots.length));
    draw.uniform("u_usePlayerSlotAttribute", true).uniform("u_drawIdOverride", -1);
    (draw as any).drawRanges([0, elementCount | 0, slots.length]);
    draw.draw();
    draw.uniform("u_usePlayerSlotAttribute", false);
}

type PlayerGpuPass = {
    vao: any;
    vb: VertexBuffer;
    ib: VertexBuffer;
    drawCall: DrawCall;
    count: number;
    vertexCapacityBytes: number;
    indexCapacityBytes: number;
    vertices: Uint8Array;
    indices: Int32Array;
};

type PlayerGpuGeometry = {
    geometryKey: string;
    opaque?: PlayerGpuPass;
    alpha?: PlayerGpuPass;
};

type PlayerGeometryBuildResult = {
    countOpaque: number;
    countAlpha: number;
    opaqueVertices?: Uint8Array;
    opaqueIndices?: Int32Array;
    alphaVertices?: Uint8Array;
    alphaIndices?: Int32Array;
};

export class PlayerRenderer {
    constructor(private renderer: WebGLOsrsRenderer) {}

    // Reusable buffers to avoid per-frame allocations
    private playerIndicesBuffer: number[] = [];
    private slotsBuffer: number[] = [];
    private playerSlotScratch: Int32Array = new Int32Array(2048);
    private frameRenderSelectionId: number = -1;
    private frameRenderPlayersByMap: Map<number, number[]> = new Map();
    // Per-frame alpha counts captured during opaque pass; used to gate alpha pass work.
    private framePlayerAlphaCounts: Map<number, number> = new Map();
    // PERF: Cached Map for alpha pass batch groups to avoid per-frame allocation
    private alphaBatchGroups: Map<
        string,
        {
            appearance: PlayerAppearance;
            seqId: number;
            frameIdx: number;
            overlaySeqId?: number;
            overlayFrameIdx?: number;
            instances: Array<{ slot: number; pid: number; mode: "idle" | "walk" | "run" }>;
        }
    > = new Map();

    // Batching optimization: track batch groups per frame
    private batchGroups: Map<
        string,
        {
            appearance: PlayerAppearance;
            seqId: number;
            frameIdx: number;
            overlaySeqId?: number;
            overlayFrameIdx?: number;
            instances: Array<{ slot: number; pid: number; mode: "idle" | "walk" | "run" }>;
        }
    > = new Map();

    // PERF (mobile): reuse a SceneBuffer + typed index arrays for the local player.
    private localSceneBuf?: any;
    private localIndexScratch: Int32Array = new Int32Array(0);
    private localIndexScratchAlpha: Int32Array = new Int32Array(0);
    private readonly emptyIndexScratch: Int32Array = new Int32Array(0);
    private readonly emptyVertexScratch: Uint8Array = new Uint8Array(0);
    private lastUploadedOpaqueGeomKey?: string;
    private lastUploadedAlphaGeomKey?: string;

    // Geometry build entry: delegates to renderer's current implementation.
    async initGeometry(): Promise<void> {
        const r: any = this.renderer as any;
        if (typeof r.initPlayerGeometry !== "function") return;
        this.clearPlayerGpuGeometryCache();
        // Build player geometry once using the current animation mode.
        await r.initPlayerGeometry();
        this.lastUploadedOpaqueGeomKey = undefined;
        this.lastUploadedAlphaGeomKey = undefined;
        // Capture active variant meta for quick access; no prebake variants.
        try {
            const active = this.captureCurrentVariant();
            this.drawCall = active.drawCall;
            this.drawCallAlpha = active.drawCallAlpha;
            this.drawRanges = active.frames;
            this.drawRangesAlpha = active.framesAlpha;
            this.frameCount = active.frameCount | 0;
            this.frameLengths = active.frameLengths?.slice();
            this.frameHeightsTiles = active.frameHeightsTiles?.slice();
            this.defaultHeightTiles =
                active.frameHeightsTiles?.[0] ??
                (this.renderer as any).playerDefaultHeightTiles ??
                200 / 128;
            // Get indices count from ECS for controlled player
            const osrsClient = (this.renderer as any).osrsClient;
            if (osrsClient && osrsClient.playerEcs) {
                const ecsIdx = osrsClient.playerEcs.getIndexForServerId(
                    osrsClient.controlledPlayerServerId,
                );
                if (ecsIdx !== undefined) {
                    this.dynamicIndicesCount =
                        osrsClient.playerEcs.getModelIndicesCount(ecsIdx) | 0;
                    this.dynamicIndicesCountAlpha =
                        osrsClient.playerEcs.getModelIndicesCountAlpha(ecsIdx) | 0;
                }
            }
            this.interleavedBuffer = (this.renderer as any).playerInterleavedBuffer;
            this.indexBuffer = (this.renderer as any).playerIndexBuffer;
        } catch {}
    }

    // Remote appearance prebake removed; dynamic-only path.

    // Resolve sequence id for current mode (idle/walk/run); used by dynamic path.
    resolveSeqIdForMode(): number {
        const r: any = this.renderer as any;
        return typeof r._resolvePlayerSeqIdForMode === "function"
            ? r._resolvePlayerSeqIdForMode()
            : -1;
    }

    /**
     * Build animation clip metadata for a sequence ID.
     * Contains frame count, frame lengths, and skeletal animation info.
     */
    buildAnimClipMeta(seqId: number): ActorAnimationClip | undefined {
        if (seqId < 0) return undefined;
        try {
            const client = (this.renderer as any).osrsClient;
            const seqType = client.seqTypeLoader.load(seqId);
            if (!seqType) return undefined;

            const isSkeletal = !!seqType.isSkeletalSeq?.();
            let frameCount = 1;
            let frameLengths: number[] | undefined = undefined;

            if (isSkeletal) {
                frameCount = this.getEffectiveSkeletalDuration(seqType, seqId | 0);
            } else {
                frameCount = Math.max(seqType.frameIds?.length ?? 1, 1);
                frameLengths = new Array(frameCount);
                for (let i = 0; i < frameCount; i++) {
                    frameLengths[i] = seqType.getFrameLength(client.seqFrameLoader, i) | 0;
                }
            }

            return {
                frames: [],
                framesAlpha: undefined,
                isSkeletal,
                frameCount,
                frameLengths,
                frameStep: seqType.frameStep | 0,
                looping: !!seqType.looping,
                maxLoops: seqType.maxLoops | 0,
            };
        } catch {
            return undefined;
        }
    }

    /**
     * Get effective duration for skeletal animations.
     * Caches results for performance.
     */
    private getEffectiveSkeletalDuration(seqType: any, seqId: number): number {
        try {
            const cached = this.skeletalDurationCache.get(seqId | 0);
            if (cached && cached > 0) return cached | 0;

            // cached-seq duration is (end-start).
            let duration = Number(seqType?.getSkeletalDuration?.() ?? 0) | 0;

            if (!(duration > 0)) duration = 1;
            this.skeletalDurationCache.set(seqId | 0, duration | 0);
            return duration | 0;
        } catch {
            return 1;
        }
    }

    // ===== Internal geometry/meta owned by PlayerRenderer =====
    private drawCall?: DrawCall;
    private drawCallAlpha?: DrawCall;
    private drawRanges?: DrawRange[];
    private drawRangesAlpha?: DrawRange[];
    private frameCount: number = 0;
    private frameLengths?: number[];
    private frameHeightsTiles?: number[];
    private defaultHeightTiles: number = 200 / 128;
    private dynamicIndicesCount: number = 0;
    private dynamicIndicesCountAlpha: number = 0;
    private interleavedBuffer?: any;
    private indexBuffer?: any;
    private lastLoggedSeqId: number = -1;
    private lastLoggedFrameIndex: number = -1;
    private playerSoundState: Map<string, { seqId: number; frameIdx: number }> = new Map();
    private skeletalDurationCache: Map<number, number> = new Map();

    private variantCache: Map<
        string,
        {
            drawCall: DrawCall;
            drawCallAlpha?: DrawCall;
            frames: DrawRange[];
            framesAlpha?: DrawRange[];
            frameCount: number;
            frameLengths?: number[];
            frameHeightsTiles?: number[];
            isSkeletal: boolean;
        }
    > = new Map();

    // ===== Per-appearance base model cache (shared) =====
    private appearanceBaseCache: Map<
        string,
        { baseModel: any; baseCenterX: number; baseCenterZ: number; defaultHeightTiles: number }
    > = new Map();
    private lastRenderableAppearance = new Map<number, PlayerAppearance>();

    // Clean up cache entries for a deallocated player appearance
    cleanupAppearanceCache(appearanceHash?: string): void {
        if (appearanceHash) {
            this.appearanceBaseCache.delete(appearanceHash);
            // Also clear any geometry cache entries that contain this appearance
            for (const [key] of this.geomCache) {
                if (key.startsWith(appearanceHash + "|")) {
                    this.geomCache.delete(key);
                }
            }
            for (const [key, geometry] of this.playerGpuGeometryCache) {
                if (key.startsWith(appearanceHash + "|")) {
                    this.deletePlayerGpuGeometry(geometry);
                    this.playerGpuGeometryCache.delete(key);
                }
            }
            return;
        }
        this.appearanceBaseCache.clear();
        this.lastRenderableAppearance.clear();
        this.geomCache.clear();
        this.clearPlayerGpuGeometryCache();
    }

    // Bounded geometry cache: (appearance|seqId|frameIdx) -> buffers
    private static readonly GEOM_CACHE_MAX_ENTRIES = 384;
    private geomCache: Map<
        string,
        { verts: Uint8Array; inds: Int32Array; vertsA: Uint8Array; indsA: Int32Array }
    > = new Map();
    private playerGpuGeometryCache: Map<string, PlayerGpuGeometry> = new Map();

    private ensureBaseForAppearance(
        app: PlayerAppearance,
    ):
        | { baseModel: any; baseCenterX: number; baseCenterZ: number; defaultHeightTiles: number }
        | undefined {
        try {
            const mv = this.renderer.osrsClient;
            const key = this.getAppearanceCacheKey(app);
            const existing = this.appearanceBaseCache.get(key);
            if (existing) return existing;

            // Delegate construction to PlayerEcs so renderer doesn't build models
            const rec = mv.playerEcs.ensureBaseForAppearance(app, {
                idkTypeLoader: mv.idkTypeLoader,
                objTypeLoader: mv.objTypeLoader,
                modelLoader: mv.modelLoader,
                textureLoader: mv.textureLoader,
                npcTypeLoader: mv.npcTypeLoader,
                seqTypeLoader: mv.seqTypeLoader,
                seqFrameLoader: mv.seqFrameLoader,
                skeletalSeqLoader: mv.loaderFactory.getSkeletalSeqLoader?.(),
                varManager: mv.varManager,
                basTypeLoader: mv.basTypeLoader,
            });
            if (!rec) return undefined;

            // Ensure textures used by this appearance are uploaded to the texture array
            try {
                const used = new Set<number>();
                if (rec.baseModel?.faceTextures) {
                    const texLoader = mv.textureLoader;
                    for (let i = 0; i < rec.baseModel.faceCount; i++) {
                        const tid = rec.baseModel.faceTextures[i];
                        if (tid !== -1 && texLoader.isSd?.(tid)) used.add(tid);
                    }
                }
                if (used.size > 0) {
                    const toUpload = new Map<number, Int32Array>();
                    for (const tid of used) {
                        if (!(this.renderer as any).loadedTextureIds?.has?.(tid)) {
                            try {
                                const px = mv.textureLoader.getPixelsArgb(tid, 128, true, 1.0);
                                toUpload.set(tid, px);
                            } catch {}
                        }
                    }
                    if (toUpload.size > 0) (this.renderer as any).updateTextureArray?.(toUpload);
                }
            } catch {}

            this.appearanceBaseCache.set(key, rec);
            return rec;
        } catch {
            return undefined;
        }
    }

    private getAppearanceCacheKey(app: PlayerAppearance): string {
        const equipKey =
            app.getEquipKey?.() ??
            (Array.isArray(app.equip) ? app.equip.slice(0, 14).join(",") : "");
        return app.getCacheKey?.() ?? `${app.getHash?.().toString() ?? "0"}|${equipKey}`;
    }

    private resolveRenderableAppearance(
        serverId: number,
        appearance: PlayerAppearance,
    ): PlayerAppearance | undefined {
        if (this.ensureBaseForAppearance(appearance)) {
            this.lastRenderableAppearance.set(serverId, appearance);
            return appearance;
        }
        return this.lastRenderableAppearance.get(serverId);
    }

    private getFirstPersonAppearance(pid: number, appearance: PlayerAppearance): PlayerAppearance {
        if (!this.isFirstPersonArmsPlayer(pid, appearance)) {
            return appearance;
        }
        return new PlayerAppearance(
            appearance.gender,
            appearance.colors,
            appearance.kits,
            appearance.equip,
            appearance.headIcons,
            appearance.npcTransformationId,
            true,
        );
    }

    private isFirstPersonArmsPlayer(pid: number, appearance?: PlayerAppearance): boolean {
        const app = appearance ?? this.renderer.osrsClient.playerEcs.getAppearance(pid);
        return (
            this.renderer.osrsClient.firstPersonArmsVisible === true &&
            this.isControlledPid(pid) &&
            (app?.npcTransformationId ?? -1) < 0
        );
    }

    private getPlayerGpuGeometry(ownerKey: string, geometryKey: string): PlayerGpuGeometry | undefined {
        if (isRustPrimaryRendererActive(this.renderer)) {
            return undefined;
        }
        let geometry = this.playerGpuGeometryCache.get(ownerKey);
        if (geometry?.geometryKey === geometryKey) {
            this.playerGpuGeometryCache.delete(ownerKey);
            this.playerGpuGeometryCache.set(ownerKey, geometry);
            return geometry;
        }

        const cached = this.geomCache.get(geometryKey);
        if (!cached) return undefined;

        geometry ??= { geometryKey };
        geometry.opaque = this.updatePlayerGpuPass(
            geometry.opaque,
            cached.verts,
            cached.inds,
            (this.renderer as any).playerProgramOpaque ?? (this.renderer as any).playerProgram,
        );
        geometry.alpha = this.updatePlayerGpuPass(
            geometry.alpha,
            cached.vertsA,
            cached.indsA,
            (this.renderer as any).playerProgram,
        );
        geometry.geometryKey = geometryKey;
        this.playerGpuGeometryCache.delete(ownerKey);
        this.playerGpuGeometryCache.set(ownerKey, geometry);

        while (this.playerGpuGeometryCache.size > PlayerRenderer.GEOM_CACHE_MAX_ENTRIES) {
            const oldest = this.playerGpuGeometryCache.keys().next().value as string | undefined;
            if (oldest === undefined) break;
            const evicted = this.playerGpuGeometryCache.get(oldest);
            if (evicted) this.deletePlayerGpuGeometry(evicted);
            this.playerGpuGeometryCache.delete(oldest);
        }
        return geometry;
    }

    private updatePlayerGpuPass(
        pass: PlayerGpuPass | undefined,
        vertices: Uint8Array,
        indices: Int32Array,
        program: any,
    ): PlayerGpuPass | undefined {
        if (!program) return undefined;
        if (
            pass &&
            pass.vertexCapacityBytes >= vertices.byteLength &&
            pass.indexCapacityBytes >= indices.byteLength
        ) {
            if (vertices.byteLength > 0) pass.vb.data(vertices);
            if (indices.byteLength > 0) pass.ib.data(indices);
            pass.count = indices.length | 0;
            pass.vertices = vertices;
            pass.indices = indices;
            return pass;
        }
        if (pass) this.deletePlayerGpuPass(pass);
        if (indices.length <= 0) return undefined;

        const r: any = this.renderer as any;
        const vb = r.app.createInterleavedBuffer(12, vertices, PicoGL.DYNAMIC_DRAW);
        const ib = r.app.createIndexBuffer(
            PicoGL.UNSIGNED_INT as number,
            indices,
            PicoGL.DYNAMIC_DRAW,
        );
        const vao = r.app
            .createVertexArray()
            .vertexAttributeBuffer(0, vb, {
                type: PicoGL.UNSIGNED_INT,
                size: 3,
                stride: 12,
                integer: true as any,
            })
            .instanceAttributeBuffer(1, r.playerSlotBuffer, {
                type: PicoGL.INT,
                size: 1,
                integer: true as any,
            })
            .indexBuffer(ib);
        const drawCall = r.app
            .createDrawCall(program, vao)
            .uniformBlock("SceneUniforms", r.sceneUniformBuffer)
            .uniform("u_timeLoaded", -1.0)
            .uniform("u_usePlayerSlotAttribute", false)
            .texture("u_textures", r.textureArray)
            .texture("u_textureMaterials", r.textureMaterials);
        return {
            vao,
            vb,
            ib,
            drawCall,
            count: indices.length | 0,
            vertexCapacityBytes: vertices.byteLength,
            indexCapacityBytes: indices.byteLength,
            vertices,
            indices,
        };
    }

    private deletePlayerGpuPass(pass: PlayerGpuPass): void {
        try {
            pass.vao.delete();
            pass.vb.delete();
            pass.ib.delete();
        } catch {}
    }

    private deletePlayerGpuGeometry(geometry: PlayerGpuGeometry): void {
        if (geometry.opaque) this.deletePlayerGpuPass(geometry.opaque);
        if (geometry.alpha) this.deletePlayerGpuPass(geometry.alpha);
    }

    private clearPlayerGpuGeometryCache(): void {
        for (const geometry of this.playerGpuGeometryCache.values()) {
            this.deletePlayerGpuGeometry(geometry);
        }
        this.playerGpuGeometryCache.clear();
    }

    private getSeqMeta(
        seqId: number,
    ):
        | { isSkeletal: boolean; frameCount: number; frameLengths?: number[]; looping?: boolean }
        | undefined {
        try {
            const mv = this.renderer.osrsClient;
            const seqType = mv.seqTypeLoader.load(seqId | 0);
            if (!seqType) return undefined;
            if (seqType.isSkeletalSeq())
                return {
                    isSkeletal: true,
                    frameCount: this.getEffectiveSkeletalDuration(seqType as any, seqId | 0),
                    looping: !!seqType.looping,
                };
            const fc = Math.max(1, (seqType.frameIds?.length ?? 1) | 0);
            const lens = new Array(fc);
            for (let i = 0; i < fc; i++) lens[i] = seqType.getFrameLength(mv.seqFrameLoader, i) | 0;
            return {
                isSkeletal: false,
                frameCount: fc,
                frameLengths: lens,
                looping: !!seqType.looping,
            };
        } catch {
            return undefined;
        }
    }

    private computeFrameIndex(seqId: number, pid: number): number {
        try {
            const meta = this.getSeqMeta(seqId);
            if (!meta) return 0;
            const pe: any = this.renderer.osrsClient.playerEcs as any;
            const baseTick: number = (pe.getAnimTick?.(pid) ?? 0) | 0;
            const tick: number = baseTick | 0;
            const fc = Math.max(1, meta.frameCount | 0);
            if (meta.isSkeletal) return tick % fc;
            const lens = meta.frameLengths;
            if (lens && lens.length === fc) {
                let total = 0;
                for (let i = 0; i < fc; i++) total += lens[i] | 0;
                const t = total > 0 ? tick % total : tick % fc;
                let acc = 0;
                for (let i = 0; i < fc; i++) {
                    acc += lens[i] | 0;
                    if (t < acc) return i;
                }
            }
            return tick % fc;
        } catch {
            return 0;
        }
    }

    private resolveControlledIdleSeqOverride(
        pid: number,
    ): { seqId: number; frameIdx: number } | undefined {
        const overrideSeqId = this.renderer.playerIdleSeqId | 0;
        if (overrideSeqId < 0 || !this.isControlledPid(pid)) {
            return undefined;
        }
        return {
            seqId: overrideSeqId,
            frameIdx: this.computeFrameIndex(overrideSeqId, pid) | 0,
        };
    }

    private applySingleSequenceToModel(
        model: Model,
        seqType: any,
        seqId: number,
        frameIdx: number,
        mv: any,
    ): boolean {
        if (!seqType) return false;
        if (seqType.isSkeletalSeq?.()) {
            const skeletal = mv.loaderFactory.getSkeletalSeqLoader?.()?.load(seqType.skeletalId);
            if (!skeletal) return false;
            const duration = this.getEffectiveSkeletalDuration(seqType, seqId | 0);
            const local = Math.max(0, frameIdx | 0) % Math.max(1, duration | 0);
            // cached sequences use the local frame index (no start offset at render time).
            model.animateSkeletal(skeletal, local | 0);
            return true;
        }

        if (seqType.frameIds && seqType.frameIds.length > 0) {
            const ids = seqType.frameIds as number[];
            const idx = Math.max(0, frameIdx | 0) % (ids.length | 0);
            const key = ids[idx] | 0;
            const frame0 = mv.seqFrameLoader.load(key);
            if (frame0) {
                model.animate(frame0, undefined, !!seqType.op14);
                return true;
            }
        }
        return false;
    }

    private applySequenceTransformationsToModel(
        model: Model,
        baseType: any,
        baseSeqId: number,
        baseFrameIdx: number,
        overlayType: any,
        overlaySeqId: number,
        overlayFrameIdx: number,
        mv: any,
    ): boolean {
        if (!baseType) return false;
        if (!overlayType) {
            return this.applySingleSequenceToModel(
                model,
                baseType,
                baseSeqId | 0,
                baseFrameIdx | 0,
                mv,
            );
        }

        const baseCached = !!baseType.isSkeletalSeq?.();
        const overlayCached = !!overlayType.isSkeletalSeq?.();

        if (baseCached) {
            const baseSkeletal = mv.loaderFactory
                .getSkeletalSeqLoader?.()
                ?.load(baseType.skeletalId);
            if (!baseSkeletal) return false;

            const baseDuration = this.getEffectiveSkeletalDuration(baseType, baseSeqId | 0);
            const baseLocal = Math.max(0, baseFrameIdx | 0) % Math.max(1, baseDuration | 0);
            const baseAnimFrame = baseLocal | 0;

            if (overlayCached) {
                if (!Array.isArray(baseType?.skeletalMasks)) {
                    model.animateSkeletal(baseSkeletal, baseAnimFrame);
                    return true;
                }

                const overlaySkeletal = mv.loaderFactory
                    .getSkeletalSeqLoader?.()
                    ?.load(overlayType.skeletalId);
                if (!overlaySkeletal) {
                    model.animateSkeletal(baseSkeletal, baseAnimFrame);
                    return false;
                }

                const overlayDuration = this.getEffectiveSkeletalDuration(
                    overlayType,
                    overlaySeqId | 0,
                );
                const overlayLocal =
                    Math.max(0, overlayFrameIdx | 0) % Math.max(1, overlayDuration | 0);
                const overlayAnimFrame = overlayLocal | 0;

                model.animateSkeletalComposite(baseSkeletal, baseAnimFrame, {
                    masks: baseType.skeletalMasks,
                    overlay: { seq: overlaySkeletal, frame: overlayAnimFrame },
                });
                return true;
            }

            // Cached base + frame overlay
            if (Array.isArray(baseType?.skeletalMasks)) {
                model.animateSkeletal(baseSkeletal, baseAnimFrame, {
                    masks: baseType.skeletalMasks,
                    maskMatch: false,
                });
            } else {
                model.animateSkeletal(baseSkeletal, baseAnimFrame);
            }

            if (overlayType.frameIds && overlayType.frameIds.length > 0) {
                const ids = overlayType.frameIds as number[];
                const idx = Math.max(0, overlayFrameIdx | 0) % (ids.length | 0);
                const key = ids[idx] | 0;
                const frame0 = mv.seqFrameLoader.load(key);
                if (!frame0) return false;
                const interleave = Array.isArray(baseType?.masks)
                    ? (baseType.masks as number[])
                    : undefined;
                if (interleave && interleave.length > 0) {
                    model.animateInterleavedFrame(frame0, !!overlayType.op14, interleave, true);
                } else {
                    model.animate(frame0, undefined, !!overlayType.op14);
                }
            }
            return true;
        }

        // Frame base
        if (!baseType.frameIds || baseType.frameIds.length <= 0) return false;
        const baseIds = baseType.frameIds as number[];
        const baseIdx = Math.max(0, baseFrameIdx | 0) % (baseIds.length | 0);
        const baseKey = baseIds[baseIdx] | 0;
        const baseFrame0 = mv.seqFrameLoader.load(baseKey);
        if (!baseFrame0) return false;

        const interleave = Array.isArray(baseType?.masks)
            ? (baseType.masks as number[])
            : undefined;

        if (overlayCached) {
            // OSRS: requires an interleave array; otherwise overlay is ignored.
            if (!interleave || interleave.length === 0) {
                model.animate(baseFrame0, undefined, !!baseType.op14);
                return true;
            }

            const overlaySkeletal = mv.loaderFactory
                .getSkeletalSeqLoader?.()
                ?.load(overlayType.skeletalId);
            if (!overlaySkeletal) {
                model.animate(baseFrame0, undefined, !!baseType.op14);
                return false;
            }

            const overlayDuration = this.getEffectiveSkeletalDuration(
                overlayType,
                overlaySeqId | 0,
            );
            const overlayLocal =
                Math.max(0, overlayFrameIdx | 0) % Math.max(1, overlayDuration | 0);
            const overlayAnimFrame = overlayLocal | 0;

            model.animateSkeletal(overlaySkeletal, overlayAnimFrame, {
                masks: Array.isArray(baseType?.skeletalMasks) ? baseType.skeletalMasks : undefined,
                maskMatch: true,
                applyAlpha: false,
            });
            model.animateInterleavedFrame(baseFrame0, !!baseType.op14, interleave, false);
            return true;
        }

        // Frame base + frame overlay
        if (
            !overlayType.frameIds ||
            overlayType.frameIds.length <= 0 ||
            !interleave ||
            interleave.length === 0
        ) {
            model.animate(baseFrame0, undefined, !!baseType.op14);
            return true;
        }

        const overlayIds = overlayType.frameIds as number[];
        const overlayIdx = Math.max(0, overlayFrameIdx | 0) % (overlayIds.length | 0);
        const overlayKey = overlayIds[overlayIdx] | 0;
        const overlayFrame0 = mv.seqFrameLoader.load(overlayKey);
        if (!overlayFrame0) {
            model.animate(baseFrame0, undefined, !!baseType.op14);
            return false;
        }

        model.animateInterleavedFrames(
            baseFrame0,
            !!baseType.op14,
            overlayFrame0,
            !!overlayType.op14,
            interleave,
        );
        return true;
    }

    private dynamicUpdateBuffersFor(
        baseModel: any,
        baseCenterX: number,
        baseCenterZ: number,
        seqId: number,
        frameIdx: number,
        cacheKey?: string,
        pid: number = 0,
        modeHint?: "idle" | "walk" | "run",
        overlaySeqId?: number,
        overlayFrameIdx?: number,
        uploadTarget: "both" | "opaqueOnly" | "alphaOnly" | "cacheOnly" = "both",
    ): PlayerGeometryBuildResult {
        const r: any = this.renderer as any;
        const rustPrimaryRendererEnabled = isRustPrimaryRendererActive(this.renderer);
        if (
            !rustPrimaryRendererEnabled &&
            (!r.playerInterleavedBuffer || !r.playerIndexBuffer)
        ) {
            return { countOpaque: 0, countAlpha: 0 };
        }
        const captureRustGeometry = isRustPlayerShadowEnabled();
        const controlled = this.isControlledPid(pid);
        const uploadOpaque =
            !rustPrimaryRendererEnabled &&
            (uploadTarget === "both" || uploadTarget === "opaqueOnly");
        const uploadAlpha =
            !rustPrimaryRendererEnabled &&
            (uploadTarget === "both" || uploadTarget === "alphaOnly");
        const opaqueUploadKey = cacheKey && uploadOpaque ? `opaque:${cacheKey}` : undefined;
        const alphaUploadKey = cacheKey && uploadAlpha ? `alpha:${cacheKey}` : undefined;
        // Hit cache
        if (cacheKey) {
            const c = this.geomCache.get(cacheKey);
            if (c) {
                // Keep cache access LRU-ordered (Map preserves insertion order).
                this.geomCache.delete(cacheKey);
                this.geomCache.set(cacheKey, c);
                const cachedOpaqueCount = c.inds.length | 0;
                const cachedAlphaCount = c.indsA.length | 0;
                if (uploadOpaque) {
                    this.ensurePlayerGpuCapacity(c.verts, c.inds);
                    if (this.lastUploadedOpaqueGeomKey !== opaqueUploadKey) {
                        r.playerInterleavedBuffer.data(c.verts);
                        r.playerIndexBuffer.data(c.inds);
                        this.lastUploadedOpaqueGeomKey = opaqueUploadKey;
                    }
                }
                // Update ECS with new counts
                const osrsClient = (r as any).osrsClient;
                if (osrsClient && osrsClient.playerEcs) {
                    const ecsIdx = osrsClient.playerEcs.getIndexForServerId(
                        osrsClient.controlledPlayerServerId,
                    );
                    if (ecsIdx !== undefined) {
                        osrsClient.playerEcs.setModelIndicesCount(ecsIdx, cachedOpaqueCount);
                        osrsClient.playerEcs.setModelIndicesCountAlpha(ecsIdx, cachedAlphaCount);
                        this.dynamicIndicesCount = cachedOpaqueCount;
                        this.dynamicIndicesCountAlpha = cachedAlphaCount;
                    }
                }
                if (uploadAlpha) {
                    this.ensurePlayerGpuCapacityAlpha(c.vertsA, c.indsA);
                    if (this.lastUploadedAlphaGeomKey !== alphaUploadKey) {
                        r.playerInterleavedBufferAlpha?.data(c.vertsA);
                        r.playerIndexBufferAlpha?.data(c.indsA);
                        this.lastUploadedAlphaGeomKey = alphaUploadKey;
                    }
                }
                return {
                    countOpaque: cachedOpaqueCount,
                    countAlpha: cachedAlphaCount,
                    opaqueVertices: captureRustGeometry ? c.verts : undefined,
                    opaqueIndices: captureRustGeometry ? c.inds : undefined,
                    alphaVertices: captureRustGeometry ? c.vertsA : undefined,
                    alphaIndices: captureRustGeometry ? c.indsA : undefined,
                };
            }
        }
        const ModelMod = require("../../rs/model/Model").Model;
        // Do not shallow-copy face alpha: sequences (frame-based or skeletal) can mutate faceAlphas via ALPHA transforms.
        // Sharing would leak those mutations back into the cached base model and cause visual artifacts (e.g., "blur"/ghosting).
        let model = ModelMod.copyAnimated(baseModel, false, true);
        let animationApplied = false;
        try {
            const mv = this.renderer.osrsClient as any;
            const seqType = mv.seqTypeLoader.load(seqId | 0);
            const overlayId =
                typeof overlaySeqId === "number" && Number.isFinite(overlaySeqId)
                    ? overlaySeqId | 0
                    : -1;
            const overlayFrame =
                typeof overlayFrameIdx === "number" && Number.isFinite(overlayFrameIdx)
                    ? overlayFrameIdx | 0
                    : -1;

            if (seqType) {
                const overlayType =
                    overlayId >= 0 && overlayFrame >= 0
                        ? mv.seqTypeLoader.load(overlayId | 0)
                        : undefined;
                animationApplied = this.applySequenceTransformationsToModel(
                    model,
                    seqType,
                    seqId | 0,
                    frameIdx | 0,
                    overlayType,
                    overlayId | 0,
                    overlayFrame | 0,
                    mv,
                );
            }
        } catch {}
        try {
            model.calculateBounds();
            const dx = ((baseCenterX | 0) - ((model as any).xMid | 0)) | 0;
            const dz = ((baseCenterZ | 0) - ((model as any).zMid | 0)) | 0;
            if ((dx | dz) !== 0) model.translate(dx, 0, dz);
        } catch {}
        const textureLoader = this.renderer.osrsClient.textureLoader;
        const textureIdIndexMap =
            (this.renderer as any).textureIdIndexMap ?? new Map<number, number>();
        const SceneBufferMod = require("../buffer/SceneBuffer");
        const SceneBufferCls = SceneBufferMod.SceneBuffer;

        const resetSceneBuf = (sb: any) => {
            if (!sb) return;
            try {
                if (typeof sb.vertexBuf.reset === "function") {
                    sb.vertexBuf.reset();
                } else {
                    sb.vertexBuf.offset = 0;
                    sb.vertexBuf.vertexIndices?.clear?.();
                }
            } catch {}
            try {
                sb.indices.length = 0;
            } catch {}
            try {
                sb.usedTextureIds?.clear?.();
            } catch {}
        };

        const fillScratch = (src: number[], alpha: boolean): Int32Array => {
            const len = src.length | 0;
            if (len <= 0) return this.emptyIndexScratch;
            let scratch = alpha ? this.localIndexScratchAlpha : this.localIndexScratch;
            if (!scratch || scratch.length < len) {
                const next = new Int32Array(Math.max(len, (scratch?.length ?? 0) * 2, 256));
                scratch = next;
                if (alpha) this.localIndexScratchAlpha = scratch;
                else this.localIndexScratch = scratch;
            }
            for (let i = 0; i < len; i++) scratch[i] = src[i] | 0;
            return scratch.subarray(0, len);
        };

        // Build opaque geometry. For local player, reuse SceneBuffer + typed index arrays.
        let vertices: Uint8Array;
        let indices: Int32Array;
        if (controlled) {
            if (!this.localSceneBuf) {
                this.localSceneBuf = new SceneBufferCls(
                    textureLoader,
                    textureIdIndexMap,
                    0,
                    createVertexBatchBuilderIfReady(),
                );
            }
            resetSceneBuf(this.localSceneBuf);
            this.localSceneBuf.addModelFiltered(model, false);
            vertices = this.localSceneBuf.vertexBuf.byteArray();
            indices = fillScratch(this.localSceneBuf.indices, false);
        } else {
            const sceneBuf = new SceneBufferCls(
                textureLoader,
                textureIdIndexMap,
                model.verticesCount + 16,
                createVertexBatchBuilderIfReady(),
            );
            sceneBuf.addModelFiltered(model, false);
            vertices = sceneBuf.vertexBuf.byteArray();
            indices = new Int32Array(sceneBuf.indices);
        }

        const rustOpaqueVertices =
            captureRustGeometry ? new Uint8Array(vertices) : undefined;
        const rustOpaqueIndices =
            captureRustGeometry ? new Int32Array(indices) : undefined;

        // Ensure GPU buffers have enough capacity. Recreate and rebind VAO if needed.
        if (uploadOpaque) {
            this.ensurePlayerGpuCapacity(vertices, indices);
            if (vertices.byteLength > 0) {
                r.playerInterleavedBuffer.data(vertices);
            }
            if (indices.length > 0) {
                r.playerIndexBuffer.data(indices);
            }
            this.lastUploadedOpaqueGeomKey = opaqueUploadKey;
        }
        r.playerDrawRanges = [newDrawRange(0, indices.length | 0, 1)];
        r.playerDrawRangesAlpha = [newDrawRange(0, 0, 1)];
        r.playerDrawRanges = [newDrawRange(0, indices.length | 0, 1)];
        r.playerDrawRangesAlpha = [newDrawRange(0, 0, 1)];
        // Snapshot opaque geometry before local scratch buffers are potentially reused
        // for alpha geometry generation later in this function.
        const cacheOpaqueVerts =
            cacheKey !== undefined ? new Uint8Array(vertices) : this.emptyVertexScratch;
        const cacheOpaqueInds =
            cacheKey !== undefined ? new Int32Array(indices) : this.emptyIndexScratch;
        // Update ECS with new counts (opaque now, alpha updated after potential alpha build)
        const osrsClient = (r as any).osrsClient;
        if (osrsClient && osrsClient.playerEcs) {
            const ecsIdx = osrsClient.playerEcs.getIndexForServerId(
                osrsClient.controlledPlayerServerId,
            );
            if (ecsIdx !== undefined) {
                const idxCount = Math.max(0, indices.length | 0);
                osrsClient.playerEcs.setModelIndicesCount(ecsIdx, idxCount);
                this.dynamicIndicesCount = idxCount;
                // Alpha count will be set below if we actually built alpha geometry
            }
        }

        // Transparent faces (alpha): build whenever present so wearable details
        // (e.g., wing fins on Primordial/Pegasian boots) render in the player alpha pass.
        let verticesAlpha = this.emptyVertexScratch;
        let indicesAlpha = this.emptyIndexScratch;
        const alphaFaceCount = controlled
            ? (() => {
                if (!this.localSceneBuf) {
                    this.localSceneBuf = new SceneBufferCls(
                        textureLoader,
                        textureIdIndexMap,
                        0,
                        createVertexBatchBuilderIfReady(),
                    );
                }
                return this.localSceneBuf.getModelFaceCount(model, true);
            })()
            : (() => {
                const counter = new SceneBufferCls(
                    textureLoader,
                    textureIdIndexMap,
                    Math.max(16, model.verticesCount + 16),
                    createVertexBatchBuilderIfReady(),
                );
                return counter.getModelFaceCount(model, true);
            })();
        if (alphaFaceCount > 0) {
            if (controlled) {
                if (!this.localSceneBuf) {
                    this.localSceneBuf = new SceneBufferCls(
                        textureLoader,
                        textureIdIndexMap,
                        0,
                        createVertexBatchBuilderIfReady(),
                    );
                }
                resetSceneBuf(this.localSceneBuf);
                this.localSceneBuf.addModelFiltered(model, true);
                verticesAlpha = this.localSceneBuf.vertexBuf.byteArray();
                indicesAlpha = fillScratch(this.localSceneBuf.indices, true);
            } else {
                const sceneBufA = new SceneBufferCls(
                    textureLoader,
                    textureIdIndexMap,
                    model.verticesCount + 16,
                    createVertexBatchBuilderIfReady(),
                );
                sceneBufA.addModelFiltered(model, true);
                verticesAlpha = sceneBufA.vertexBuf.byteArray();
                indicesAlpha = new Int32Array(sceneBufA.indices);
            }
            if (uploadAlpha) {
                this.ensurePlayerGpuCapacityAlpha(verticesAlpha, indicesAlpha);
                r.playerInterleavedBufferAlpha?.data(verticesAlpha);
                r.playerIndexBufferAlpha?.data(indicesAlpha);
                this.lastUploadedAlphaGeomKey = alphaUploadKey;
            }
            // Update ECS and local counters for alpha counts
            try {
                const mvAny: any = (this.renderer as any).osrsClient;
                const ecsIdx = mvAny?.playerEcs?.getIndexForServerId?.(
                    mvAny?.controlledPlayerServerId,
                );
                if (ecsIdx !== undefined) {
                    mvAny.playerEcs.setModelIndicesCountAlpha(ecsIdx, indicesAlpha.length | 0);
                }
            } catch {}
            this.dynamicIndicesCountAlpha = indicesAlpha.length | 0;
        } else {
            if (uploadAlpha) {
                if (this.lastUploadedAlphaGeomKey !== alphaUploadKey) {
                    r.playerInterleavedBufferAlpha?.data(verticesAlpha);
                    r.playerIndexBufferAlpha?.data(indicesAlpha);
                    this.lastUploadedAlphaGeomKey = alphaUploadKey;
                }
            }
            this.dynamicIndicesCountAlpha = 0;
        }

        const result: PlayerGeometryBuildResult = {
            countOpaque: indices.length | 0,
            countAlpha: indicesAlpha.length | 0,
            opaqueVertices: rustOpaqueVertices,
            opaqueIndices: rustOpaqueIndices,
            alphaVertices:
                captureRustGeometry
                    ? new Uint8Array(verticesAlpha)
                    : undefined,
            alphaIndices:
                captureRustGeometry
                    ? new Int32Array(indicesAlpha)
                    : undefined,
        };
        try {
            if (cacheKey && (animationApplied || seqId < 0)) {
                const cacheAlphaVerts = new Uint8Array(verticesAlpha);
                const cacheAlphaInds = new Int32Array(indicesAlpha);
                this.geomCache.set(cacheKey, {
                    verts: cacheOpaqueVerts,
                    inds: cacheOpaqueInds,
                    vertsA: cacheAlphaVerts,
                    indsA: cacheAlphaInds,
                });
                while (this.geomCache.size > PlayerRenderer.GEOM_CACHE_MAX_ENTRIES) {
                    const oldest = this.geomCache.keys().next().value as string | undefined;
                    if (oldest === undefined) break;
                    this.geomCache.delete(oldest);
                }
            }
        } catch {}
        return result;
    }

    private ensurePlayerGpuCapacity(vertexData: Uint8Array, indexData: Int32Array): void {
        if (isRustPrimaryRendererActive(this.renderer)) return;
        const r: any = this.renderer as any;
        const app = r.app;
        const vao = r.playerVertexArray;
        if (!app || !vao) return;

        // Track current capacities on renderer instance
        const vbCap = Math.max(0, r.playerInterleavedBuffer?.byteLength ?? 0);
        const ibCap = Math.max(0, Math.floor((r.playerIndexBuffer?.byteLength ?? 0) / 4));

        const vertexBytes = vertexData.byteLength | 0;
        const indexCount = indexData.length | 0;
        const needGrowVB = vertexBytes > vbCap;
        const needGrowIB = indexCount > ibCap;

        // Growth policy: next power-of-two for headroom to reduce churn
        const nextPow2 = (v: number) => {
            let n = 1;
            while (n < Math.max(1, v)) n <<= 1;
            return n;
        };

        const strideBytes = 12;

        if (needGrowVB) {
            const oldBuf = r.playerInterleavedBuffer;
            const requiredBytes = Math.max(strideBytes, vertexBytes);
            const pow2Bytes = nextPow2(requiredBytes);
            const capBytes = Math.ceil(pow2Bytes / strideBytes) * strideBytes;
            const newBuf = app.createInterleavedBuffer(strideBytes, capBytes);
            // Rebind VAO attribute 0 to new buffer
            r.playerVertexArray = vao.vertexAttributeBuffer(0, newBuf, {
                type: PicoGL.UNSIGNED_INT,
                size: 3,
                stride: strideBytes,
                integer: true as any,
            });
            r.playerInterleavedBuffer = newBuf;
            oldBuf?.delete();
            this.interleavedBuffer = newBuf;
            this.lastUploadedOpaqueGeomKey = undefined;
            // Recreate opaque draw call to ensure it references updated VAO (defensive)
            if (r.playerProgramOpaque || r.playerProgram) {
                r.playerDrawCall = app
                    .createDrawCall(r.playerProgramOpaque ?? r.playerProgram, r.playerVertexArray)
                    .uniformBlock("SceneUniforms", r.sceneUniformBuffer)
                    .uniform("u_timeLoaded", -1.0)
                    .uniform("u_usePlayerSlotAttribute", false)
                    .texture("u_textures", r.textureArray)
                    .texture("u_textureMaterials", r.textureMaterials);
                // Keep alpha draw call bound to the dedicated alpha VAO; see ensurePlayerGpuCapacityAlpha().
                this.drawCall = r.playerDrawCall;
            }
        }

        if (needGrowIB) {
            const oldBuf = r.playerIndexBuffer;
            const newElems = nextPow2(indexCount);
            const initData = new Int32Array(newElems);
            const newBuf = app.createIndexBuffer(PicoGL.UNSIGNED_INT as number, initData);
            r.playerVertexArray = r.playerVertexArray.indexBuffer(newBuf);
            r.playerIndexBuffer = newBuf;
            oldBuf?.delete();
            this.indexBuffer = newBuf;
            this.lastUploadedOpaqueGeomKey = undefined;
            // Recreate opaque draw call to ensure it references updated VAO (defensive)
            if (r.playerProgramOpaque || r.playerProgram) {
                r.playerDrawCall = app
                    .createDrawCall(r.playerProgramOpaque ?? r.playerProgram, r.playerVertexArray)
                    .uniformBlock("SceneUniforms", r.sceneUniformBuffer)
                    .uniform("u_timeLoaded", -1.0)
                    .uniform("u_usePlayerSlotAttribute", false)
                    .texture("u_textures", r.textureArray)
                    .texture("u_textureMaterials", r.textureMaterials);
                // Keep alpha draw call bound to the dedicated alpha VAO; see ensurePlayerGpuCapacityAlpha().
                this.drawCall = r.playerDrawCall;
            }
        }
    }

    // Ensure alpha buffers/VAO/drawcall have enough capacity when we render
    // dynamic player geometry via the transparent pass (rare; e.g., fishing skillcape emote).
    private ensurePlayerGpuCapacityAlpha(vertexData: Uint8Array, indexData: Int32Array): void {
        if (isRustPrimaryRendererActive(this.renderer)) return;
        const r: any = this.renderer as any;
        const app = r.app;
        const vao = r.playerVertexArrayAlpha;
        if (!app || !vao) return;

        const vbCap = Math.max(0, r.playerInterleavedBufferAlpha?.byteLength ?? 0);
        const ibCap = Math.max(0, Math.floor((r.playerIndexBufferAlpha?.byteLength ?? 0) / 4));

        const vertexBytes = vertexData.byteLength | 0;
        const indexCount = indexData.length | 0;
        const needGrowVB = vertexBytes > vbCap;
        const needGrowIB = indexCount > ibCap;

        const nextPow2 = (v: number) => {
            let n = 1;
            while (n < Math.max(1, v)) n <<= 1;
            return n;
        };
        const strideBytes = 12;

        if (needGrowVB) {
            const oldBuf = r.playerInterleavedBufferAlpha;
            const requiredBytes = Math.max(strideBytes, vertexBytes);
            const pow2Bytes = nextPow2(requiredBytes);
            const capBytes = Math.ceil(pow2Bytes / strideBytes) * strideBytes;
            const newBuf = app.createInterleavedBuffer(strideBytes, capBytes);
            r.playerVertexArrayAlpha = vao.vertexAttributeBuffer(0, newBuf, {
                type: PicoGL.UNSIGNED_INT,
                size: 3,
                stride: strideBytes,
                integer: true as any,
            });
            r.playerInterleavedBufferAlpha = newBuf;
            oldBuf?.delete();
            this.lastUploadedAlphaGeomKey = undefined;
            if (r.playerProgram) {
                r.playerDrawCallAlpha = app
                    .createDrawCall(r.playerProgram, r.playerVertexArrayAlpha)
                    .uniformBlock("SceneUniforms", r.sceneUniformBuffer)
                    .uniform("u_timeLoaded", -1.0)
                    .uniform("u_usePlayerSlotAttribute", false)
                    .texture("u_textures", r.textureArray)
                    .texture("u_textureMaterials", r.textureMaterials);
                this.drawCallAlpha = r.playerDrawCallAlpha;
            }
        }

        if (needGrowIB) {
            const oldBuf = r.playerIndexBufferAlpha;
            const newElems = nextPow2(indexCount);
            const initData = new Int32Array(newElems);
            const newBuf = app.createIndexBuffer(PicoGL.UNSIGNED_INT as number, initData);
            r.playerVertexArrayAlpha = r.playerVertexArrayAlpha.indexBuffer(newBuf);
            r.playerIndexBufferAlpha = newBuf;
            oldBuf?.delete();
            this.lastUploadedAlphaGeomKey = undefined;
            if (r.playerProgram) {
                r.playerDrawCallAlpha = app
                    .createDrawCall(r.playerProgram, r.playerVertexArrayAlpha)
                    .uniformBlock("SceneUniforms", r.sceneUniformBuffer)
                    .uniform("u_timeLoaded", -1.0)
                    .uniform("u_usePlayerSlotAttribute", false)
                    .texture("u_textures", r.textureArray)
                    .texture("u_textureMaterials", r.textureMaterials);
                this.drawCallAlpha = r.playerDrawCallAlpha;
            }
        }
    }

    private captureCurrentVariant(): {
        drawCall: DrawCall;
        drawCallAlpha?: DrawCall;
        frames: DrawRange[];
        framesAlpha?: DrawRange[];
        frameCount: number;
        frameLengths?: number[];
        frameHeightsTiles?: number[];
        isSkeletal: boolean;
    } {
        const r: any = this.renderer as any;
        const frameCount = r.playerFrameCount | 0 || r.playerDynamicFrameCount | 0 || 1;
        const frameLengths =
            r.playerFrameLengths?.slice?.() || r.playerDynamicFrameLengths?.slice?.();
        const frameHeightsTiles = undefined; // Removed from renderer
        const isSkeletal = !!(r.playerIsSkeletal || r.playerDynamicIsSkeletal);
        return {
            drawCall: r.playerDrawCall,
            drawCallAlpha: r.playerDrawCallAlpha,
            frames: r.playerDrawRanges || [],
            framesAlpha: r.playerDrawRangesAlpha,
            frameCount,
            frameLengths,
            frameHeightsTiles,
            isSkeletal,
        };
    }

    private getVariantFor(mode: "idle" | "walk" | "run", playerId: number) {
        // Universal variant lookup based on player ID
        const key = `player:${playerId}:${mode}`;
        return (
            this.variantCache.get(key) ||
            this.variantCache.get(`player:${playerId}:walk`) ||
            this.variantCache.get(`player:${playerId}:idle`) ||
            // Fallback to generic mode variants
            this.variantCache.get(mode) ||
            this.variantCache.get("walk") ||
            this.variantCache.get("idle")
        );
    }

    private getVariantForBot(mode: "idle" | "walk" | "run", playerId: number) {
        return (
            this.variantCache.get(`bot:${playerId}:${mode}`) ||
            this.variantCache.get(`bot:${playerId}:walk`) ||
            this.variantCache.get(`bot:${playerId}:idle`) ||
            this.getVariantFor(mode, playerId)
        );
    }

    private getVariantForAppearance(
        mode: "idle" | "walk" | "run",
        playerId: number,
        app?: PlayerAppearance,
    ) {
        if (!app) return this.getVariantForBot(mode, playerId);
        try {
            const cacheKey =
                app.getCacheKey?.() ??
                `${app.getHash?.().toString() ?? "0"}|${
                    app.getEquipKey?.() ?? app.equip.slice(0, 14).join(",")
                }`;
            const vKey = `player:${playerId}:${cacheKey}:${mode}`;
            return (
                this.variantCache.get(vKey) ||
                // fallbacks within same appearance
                this.variantCache.get(`player:${playerId}:${cacheKey}:walk`) ||
                this.variantCache.get(`player:${playerId}:${cacheKey}:idle`) ||
                // fall back to bot variant if appearance not cached
                this.getVariantForBot(mode, playerId)
            );
        } catch {
            return this.getVariantForBot(mode, playerId);
        }
    }

    getFrameCount(): number {
        return this.frameCount | 0;
    }
    getFrameLengths(): number[] | undefined {
        return this.frameLengths;
    }
    getFrameHeights(): number[] | undefined {
        return this.frameHeightsTiles;
    }
    getDefaultHeightTiles(): number {
        return this.defaultHeightTiles;
    }
    getDynamicIndicesCount(): number {
        return this.dynamicIndicesCount | 0;
    }
    getDynamicIndicesCountAlpha(): number {
        return this.dynamicIndicesCountAlpha | 0;
    }
    getInterleavedBuffer(): any {
        return this.interleavedBuffer;
    }
    getIndexBuffer(): any {
        return this.indexBuffer;
    }

    /**
     * Append player instance data for a given map into the unified actor buffer
     * and write the map's data texture offset for this frame's ring slot.
     */
    addPlayerRenderData(map: WebGLMapSquare): void {
        const r = this.renderer;
        const playerEcs = r.osrsClient.playerEcs;
        const n = playerEcs.size?.() ?? (playerEcs as any).size?.() ?? 0;
        if (!n) return;

        // Always use slot 0 for double-buffered actor data
        const sampleIdx = 0;

        const baseOffset = r.actorRenderCount;
        // Write offset for the ring slot we will sample during this frame's draw
        map.playerDataTextureOffsets[sampleIdx] = baseOffset;

        const mapBaseTileX = map.getRenderBaseTileX();
        const mapBaseTileY = map.getRenderBaseTileY();
        const mapTileSpan = map.getLocalTileSpan();

        // Append only players selected for this map for this frame (matches draw path exactly).
        const renderPlayers = this.getRenderPlayersForMap(map);
        let indexInMap = 0;
        for (let k = 0; k < renderPlayers.length; k++) {
            const i = renderPlayers[k] | 0;
            const px = playerEcs.getX(i) | 0;
            const py = playerEcs.getY(i) | 0;
            const tileX = (px / 128) | 0;
            const tileY = (py / 128) | 0;

            // Ensure capacity (8 uint16 per actor = 2 texels)
            if (r.unifiedActorData) {
                const newCount = r.actorRenderCount + 1;
                if (r.actorRenderData.length / 8 < newCount) {
                    const newData = new Uint16Array(Math.ceil((newCount * 2) / 16) * 16 * 8);
                    newData.set(r.actorRenderData);
                    r.actorRenderData = newData;
                }
            }

            const localTileX = tileX - mapBaseTileX;
            const localTileY = tileY - mapBaseTileY;

            const tx = clamp(localTileX, 0, Math.max(0, mapTileSpan - 1));
            const ty = clamp(localTileY, 0, Math.max(0, mapTileSpan - 1));
            const renderPlane = resolveHeightSamplePlaneForLocal(
                map,
                playerEcs.getLevel(i) | 0,
                tx,
                ty,
            );

            // actor positions advance on client cycles; do not render-time interpolate.
            const localX = (px - mapBaseTileX * 128) | 0;
            const localY = (py - mapBaseTileY * 128) | 0;
            // Apply yaw bias so model forward aligns with OSRS orientation
            const rot =
                (playerEcs.getRotation(i) + ((r as any).playerRotationBiasUnits ?? 0)) & 2047;

            if (r.unifiedActorData) {
                const offset = r.actorRenderCount * 8;
                // Texel 0: position, plane|rotation, interactionId
                r.actorRenderData[offset + 0] = localX;
                r.actorRenderData[offset + 1] = localY;
                r.actorRenderData[offset + 2] = renderPlane | (rot << 2);
                r.actorRenderData[offset + 3] = PLAYER_INTERACT_BASE + (indexInMap & 0x7fff);
                // Texel 1: per-actor HSL override
                // Pack: R = hue(7) | sat(7) << 7, G = lum(7) | amount(8) << 7
                const override = playerEcs.getColorOverride(i);
                const clientCycle = (r.osrsClient as any).clientCycle | 0;
                if (
                    override.amount !== 0 &&
                    clientCycle >= override.startCycle &&
                    clientCycle < override.endCycle
                ) {
                    r.actorRenderData[offset + 4] =
                        (override.hue & 0x7f) | ((override.sat & 0x7f) << 7);
                    r.actorRenderData[offset + 5] =
                        (override.lum & 0x7f) | ((override.amount & 0xff) << 7);
                } else {
                    r.actorRenderData[offset + 4] = 0;
                    r.actorRenderData[offset + 5] = 0;
                }
                r.actorRenderData[offset + 6] = 0;
                r.actorRenderData[offset + 7] = 0;
                r.actorRenderCount++;
            }
            indexInMap++;
        }
    }

    /**
     * Render opaque player geometry for a single map using unified actor texture data.
     */
    renderOpaqueForMap(
        map: WebGLMapSquare,
        actorDataTextureIndex: number,
        actorDataTexture: Texture | undefined,
    ): void {
        const r = this.renderer;
        const rustPrimaryRendererEnabled = isRustPrimaryRendererActive(r);
        if (!rustPrimaryRendererEnabled && !actorDataTexture) return;
        if ((!rustPrimaryRendererEnabled && !this.drawCall) || !this.drawRanges) return;

        const baseOffsetPlayer = map.playerDataTextureOffsets[actorDataTextureIndex];
        if (baseOffsetPlayer === -1) return;

        // Determine players selected for this map this frame (same set used by actor data upload).
        const pe = r.osrsClient.playerEcs;
        const playerIndices = this.getRenderPlayersForMap(map);
        if (playerIndices.length === 0) return;
        const activePlayerCount = pe.getActiveCount();
        const combatTargetPid = r.getCombatTargetPlayerEcsIndex();

        // Clear batch groups for this frame
        this.batchGroups.clear();

        // Reuse the slot buffer.
        const slots = this.slotsBuffer;
        if (slots.length < playerIndices.length) {
            slots.length = playerIndices.length;
        }
        // Compute slot indices (order within this map) while building the player list
        let slotCounter = 0;
        for (let j = 0; j < playerIndices.length; j++) {
            slots[j] = slotCounter++;
        }

        // Group players by appearance and animation state for batching
        for (let j = 0; j < playerIndices.length; j++) {
            const pid = playerIndices[j] | 0;
            const slot = slots[j] | 0;
            const peInst = r.osrsClient.playerEcs;
            const px = peInst.getX(pid) | 0;
            const py = peInst.getY(pid) | 0;
            const plane = peInst.getLevel(pid) | 0;
            const moving = !!(peInst as any).isMoving?.(pid);
            const wantsRun = !!(
                (peInst as any).isRunVisual?.(pid) || (peInst as any).isRunning?.(pid)
            );
            const mode: "idle" | "walk" | "run" = moving ? (wantsRun ? "run" : "walk") : "idle";
            // Action `sequence` (server-authored). Movement uses `movementSequence` separately.
            const actionSeqId = peInst.getAnimSeqId(pid) | 0;
            const movementSeqId = peInst.getAnimMovementSeqId(pid) | 0;
            const idleSeqId = peInst.getAnimSeq(pid, "idle") | 0;

            const actionDelay = (peInst.getAnimSeqDelay?.(pid) ?? 0) | 0;
            const actionActive = actionSeqId >= 0 && actionDelay === 0;
            const forcedSeq = this.resolveControlledIdleSeqOverride(pid | 0);
            const useActionSequence = forcedSeq === undefined && actionActive;
            const unanimatedIdle = shouldUseUnanimatedIdlePlayer(
                activePlayerCount,
                this.canBatchPlayer(pid, peInst) && (pid | 0) !== (combatTargetPid ?? -1),
                moving,
                actionActive,
                movementSeqId,
                idleSeqId,
            );

            let movementFrameIdx = 0;
            let actionFrameIdx = 0;
            if (!unanimatedIdle) {
                const controller = this.renderer.osrsClient.playerAnimController;
                const serverId = this.renderer.osrsClient.playerEcs.getServerIdForIndex(pid);
                if (controller && serverId !== undefined) {
                    movementFrameIdx =
                        (controller.getMovementSequenceState(serverId)?.frame ?? 0) | 0;
                    actionFrameIdx = (controller.getSequenceState(serverId)?.frame ?? 0) | 0;
                }
            }

            if (!forcedSeq && !actionActive && (movementSeqId | 0) < 0) continue;

            let seqId =
                unanimatedIdle
                    ? -1
                    : forcedSeq !== undefined
                    ? forcedSeq.seqId | 0
                    : useActionSequence
                      ? actionSeqId | 0
                      : movementSeqId | 0;
            let frameIdx =
                unanimatedIdle
                    ? 0
                    : forcedSeq !== undefined
                    ? forcedSeq.frameIdx | 0
                    : useActionSequence
                      ? actionFrameIdx | 0
                      : movementFrameIdx | 0;
            let overlaySeqId: number | undefined;
            let overlayFrameIdx: number | undefined;
            if (useActionSequence) {
                let canLayer = false;
                try {
                    const st = this.renderer.osrsClient.seqTypeLoader.load(actionSeqId | 0) as any;
                    if (st?.isSkeletalSeq?.()) canLayer = Array.isArray(st.skeletalMasks);
                    else canLayer = Array.isArray(st?.masks) && st.masks.length > 0;
                } catch {}
                if (
                    canLayer &&
                    (movementSeqId | 0) >= 0 &&
                    (movementSeqId | 0) !== (idleSeqId | 0)
                ) {
                    overlaySeqId = movementSeqId | 0;
                    overlayFrameIdx = movementFrameIdx | 0;
                }
            }

            // Frame sounds: mirror server-driven movement/action unless a local debug override is active.
            if (!forcedSeq && !unanimatedIdle && (movementSeqId | 0) >= 0) {
                this.emitPlayerFrameSound(
                    pid | 0,
                    movementSeqId | 0,
                    movementFrameIdx | 0,
                    px | 0,
                    py | 0,
                    plane | 0,
                    this.isControlledPid(pid),
                    "movement",
                );
            }
            if (useActionSequence) {
                this.emitPlayerFrameSound(
                    pid | 0,
                    actionSeqId | 0,
                    actionFrameIdx | 0,
                    px | 0,
                    py | 0,
                    plane | 0,
                    this.isControlledPid(pid),
                    "action",
                );
            }

            // Debug logging moved to tickPass to avoid duplicate logs during rendering

            // Appearance is not required for logging, but is required for batching/rendering
            const app = this.renderer.osrsClient.playerEcs.getAppearance(pid);
            if (!app) continue;

            let effectiveApp = app;
            if (useActionSequence && (app.npcTransformationId ?? -1) < 0) {
                try {
                    const seqType = this.renderer.osrsClient.seqTypeLoader.load(actionSeqId | 0);
                    if (seqType && (seqType.leftHandItem >= 0 || seqType.rightHandItem >= 0)) {
                        const newEquip = app.equip.slice();
                        // OSRS cache SeqType stores item IDs with 512 offset (0x200) for equipment overrides.
                        // We must strip this offset to get the actual Item ID for our loader.
                        let shield = seqType.leftHandItem;
                        let weapon = seqType.rightHandItem;
                        if (shield >= 512) shield -= 512;
                        if (weapon >= 512) weapon -= 512;

                        if (shield >= 0) newEquip[EquipmentSlot.SHIELD] = shield;
                        if (weapon >= 0) newEquip[EquipmentSlot.WEAPON] = weapon;
                        effectiveApp = new PlayerAppearance(
                            app.gender,
                            app.colors,
                            app.kits,
                            newEquip,
                            app.headIcons,
                        );
                    }
                } catch {}
            }
            const serverId = peInst.getServerIdForIndex(pid);
            if (serverId === undefined) continue;
            effectiveApp = this.getFirstPersonAppearance(pid, effectiveApp);
            const renderableAppearance = this.resolveRenderableAppearance(serverId, effectiveApp);
            if (!renderableAppearance) continue;
            effectiveApp = renderableAppearance;

            // Create batch key from appearance hash, sequence ID, and frame index
            const appKey =
                effectiveApp.getCacheKey?.() ??
                `${effectiveApp.getHash?.().toString() ?? "0"}|${
                    effectiveApp.getEquipKey?.() ?? effectiveApp.equip.slice(0, 14).join(",")
                }`;
            const overlayKey =
                typeof overlaySeqId === "number" && typeof overlayFrameIdx === "number"
                    ? `|${overlaySeqId | 0}|${overlayFrameIdx | 0}`
                    : "";
            const batchKey = `${appKey}|${seqId}|${frameIdx}${overlayKey}`;

            // Add player to batch group
            let group = this.batchGroups.get(batchKey);
            if (!group) {
                group = {
                    appearance: effectiveApp,
                    seqId: seqId | 0,
                    frameIdx: frameIdx | 0,
                    overlaySeqId: overlaySeqId,
                    overlayFrameIdx: overlayFrameIdx,
                    instances: [],
                };
                this.batchGroups.set(batchKey, group);
            }
            group.instances.push({ slot, pid, mode });
        }

        // Batched rendering: primary keeps only CPU geometry/state; legacy modes
        // retain the Pico draw call for fallback/parity.
        const draw = rustPrimaryRendererEnabled
            ? undefined
            : r.configureDrawCall(this.drawCall as any as DrawCall);
        const playerEcs = r.osrsClient?.playerEcs;
        const playerDeckH = r.getWorldEntityDeckHeight(0, 0);
        const playerMapPos = vec2.fromValues(map.renderPosX, map.renderPosY);
        if (draw) {
            draw.uniform("u_mapPos", playerMapPos)
                .uniform("u_npcDataOffset", baseOffsetPlayer)
                .uniform("u_modelYOffset", r.playerYOffset)
                .uniform("u_worldEntityTransform", WebGLMapSquare.IDENTITY_MAT4)
                .texture("u_npcDataTexture", actorDataTexture as Texture)
                .texture("u_heightMap", map.heightMapTexture)
                .uniform("u_sceneBorderSize", map.borderSize);

            // Player models use the same winding as terrain/NPC geometry.
            if (r.cullBackFace) r.app.enable(PicoGL.CULL_FACE);
            else r.app.disable(PicoGL.CULL_FACE);
        }

        // Process each batch group
        for (const [batchKey, group] of this.batchGroups) {
            // The first-person camera can look at the reverse side of an arm,
            // weapon, or shield face. Those pieces must be double-sided; normal
            // player models retain back-face culling to avoid visible internals.
            const rustCullBackFace =
                !group.appearance.firstPersonArmsOnly && !!r.cullBackFace;
            if (draw) {
                if (rustCullBackFace) r.app.enable(PicoGL.CULL_FACE);
                else r.app.disable(PicoGL.CULL_FACE);
            }
            if (group.instances.length === 0) continue;

            const baseRec = this.ensureBaseForAppearance(group.appearance);
            if (!baseRec) continue;

            // Players in the same group share appearance and animation geometry. Upload it once,
            // while retaining one instance per actor slot for exact placement.
            slots.length = 0;
            let batchSource: (typeof group.instances)[number] | undefined;
            for (const inst of group.instances) {
                if (this.canBatchPlayer(inst.pid, playerEcs)) {
                    batchSource ??= inst;
                    slots.push(inst.slot | 0);
                }
            }

            if (batchSource) {
                const gpuOwnerKey = `${this.getAppearanceCacheKey(group.appearance)}|seq:${
                    group.seqId | 0
                }|overlay:${group.overlaySeqId ?? -1}`;
                let gpuGeometry = this.getPlayerGpuGeometry(gpuOwnerKey, batchKey);
                if (!gpuGeometry) {
                    this.dynamicUpdateBuffersFor(
                        baseRec.baseModel,
                        baseRec.baseCenterX,
                        baseRec.baseCenterZ,
                        group.seqId,
                        group.frameIdx,
                        batchKey,
                        batchSource.pid,
                        batchSource.mode,
                        group.overlaySeqId,
                        group.overlayFrameIdx,
                        "cacheOnly",
                    );
                    gpuGeometry = this.getPlayerGpuGeometry(gpuOwnerKey, batchKey);
                }
                const counts: PlayerGeometryBuildResult = gpuGeometry
                    ? {
                          countOpaque: gpuGeometry.opaque?.count ?? 0,
                          countAlpha: gpuGeometry.alpha?.count ?? 0,
                          opaqueVertices: gpuGeometry.opaque?.vertices,
                          opaqueIndices: gpuGeometry.opaque?.indices,
                          alphaVertices: gpuGeometry.alpha?.vertices,
                          alphaIndices: gpuGeometry.alpha?.indices,
                      }
                    : this.dynamicUpdateBuffersFor(
                          baseRec.baseModel,
                          baseRec.baseCenterX,
                          baseRec.baseCenterZ,
                          group.seqId,
                          group.frameIdx,
                          batchKey,
                          batchSource.pid,
                          batchSource.mode,
                          group.overlaySeqId,
                          group.overlayFrameIdx,
                          "opaqueOnly",
                      );
                for (const inst of group.instances) {
                    if (this.canBatchPlayer(inst.pid, playerEcs)) {
                        this.framePlayerAlphaCounts.set(inst.pid | 0, counts.countAlpha | 0);
                    }
                }
                if ((counts.countOpaque | 0) > 0) {
                    const playerDraw = gpuGeometry?.opaque
                        ? r
                              .configureDrawCall(gpuGeometry.opaque.drawCall)
                              .uniform("u_mapPos", playerMapPos)
                              .uniform("u_npcDataOffset", baseOffsetPlayer)
                              .uniform("u_modelYOffset", r.playerYOffset)
                              .uniform("u_worldEntityTransform", WebGLMapSquare.IDENTITY_MAT4)
                              .texture("u_npcDataTexture", actorDataTexture as Texture)
                              .texture("u_heightMap", map.heightMapTexture)
                              .uniform("u_sceneBorderSize", map.borderSize)
                        : draw;
                    if (playerDraw && r.playerSlotBuffer) {
                        drawPlayerSlots(
                            playerDraw,
                            r.playerSlotBuffer,
                            this.playerSlotScratch,
                            slots,
                            counts.countOpaque | 0,
                            true,
                        );
                    }
                    if (counts.opaqueVertices && counts.opaqueIndices) {
                        mirrorRustPlayerGeometry(
                            r,
                            map,
                            counts.opaqueVertices,
                            counts.opaqueIndices,
                            baseOffsetPlayer,
                            slots,
                            r.playerYOffset,
                            WebGLMapSquare.IDENTITY_MAT4,
                            false,
                            rustCullBackFace,
                            !!r.cullBackFace,
                        );
                    }
                }
            }

            for (const inst of group.instances) {
                if (batchSource && this.canBatchPlayer(inst.pid, playerEcs)) {
                    continue;
                }
                const counts = this.dynamicUpdateBuffersFor(
                    baseRec.baseModel,
                    baseRec.baseCenterX,
                    baseRec.baseCenterZ,
                    group.seqId,
                    group.frameIdx,
                    batchKey,
                    inst.pid,
                    inst.mode,
                    group.overlaySeqId,
                    group.overlayFrameIdx,
                    "opaqueOnly",
                );
                this.framePlayerAlphaCounts.set(inst.pid | 0, counts.countAlpha | 0);

                // Per-player WorldView: apply deck height + bobbing transform
                // inst.pid is the ECS index directly (from playerIndices)
                const wvId = playerEcs?.getWorldViewId?.(inst.pid) ?? -1;
                let playerModelYOffset = r.playerYOffset;
                let playerWorldEntityTransform = WebGLMapSquare.IDENTITY_MAT4;
                if (wvId >= 0) {
                    playerModelYOffset = r.playerYOffset + playerDeckH;
                    playerWorldEntityTransform =
                        r.worldEntityAnimator?.getTransform(wvId)
                        ?? WebGLMapSquare.IDENTITY_MAT4;
                    if (draw) {
                        draw.uniform("u_modelYOffset", playerModelYOffset).uniform(
                            "u_worldEntityTransform",
                            playerWorldEntityTransform,
                        );
                    }
                }

                // Use drawIdOverride since gl_DrawID will be 0 for single-range legacy draws.
                if (draw) {
                    draw.uniform("u_drawIdOverride", inst.slot | 0);
                    (draw as any).drawRanges([0, counts.countOpaque | 0, 1]);
                    draw.draw();
                }
                if (counts.opaqueVertices && counts.opaqueIndices) {
                    mirrorRustPlayerGeometry(
                        r,
                        map,
                        counts.opaqueVertices,
                        counts.opaqueIndices,
                        baseOffsetPlayer,
                        [inst.slot | 0],
                        playerModelYOffset,
                        playerWorldEntityTransform,
                        false,
                        rustCullBackFace,
                        !!r.cullBackFace,
                    );
                }

                // Restore overworld uniforms after WE player draw
                if (wvId >= 0 && draw) {
                    draw.uniform("u_modelYOffset", r.playerYOffset).uniform(
                        "u_worldEntityTransform",
                        WebGLMapSquare.IDENTITY_MAT4,
                    );
                }
            }
            if (draw) draw.uniform("u_drawIdOverride", -1); // Reset
        }
        if (draw && r.cullBackFace) r.app.enable(PicoGL.CULL_FACE);
    }

    /**
     * Transparent player pass (alpha faces). Disabled when using dynamic player anim.
     */
    renderTransparentPlayerPass(
        playerDataTextureIndex: number,
        playerDataTexture: Texture | undefined,
    ): void {
        const r = this.renderer;
        const rustPrimaryRendererEnabled = isRustPrimaryRendererActive(r);
        if (
            (!rustPrimaryRendererEnabled && !playerDataTexture) ||
            (!rustPrimaryRendererEnabled && !this.drawCallAlpha) ||
            !this.drawRangesAlpha
        ) {
            return;
        }
        const drawCallAlpha = this.drawCallAlpha as DrawCall | undefined;
        const tex = playerDataTexture as Texture | undefined;

        // Use dynamic alpha geometry when enabled, otherwise cycle pre-baked alpha ranges
        const frameId = 0; // unused in variant path

        for (let i = 0; i < r.mapManager.visibleMapCount; i++) {
            const map = r.mapManager.visibleMaps[i];
            const baseOffset = map.playerDataTextureOffsets[playerDataTextureIndex];
            if (baseOffset === -1) continue;

            const pe = r.osrsClient.playerEcs;
            const playerIndices = this.getRenderPlayersForMap(map);
            if (playerIndices.length === 0) continue;
            const activePlayerCount = pe.getActiveCount();
            const combatTargetPid = r.getCombatTargetPlayerEcsIndex();

            // PERF: Clear and reuse cached batch groups Map to avoid per-frame allocation
            this.alphaBatchGroups.clear();
            const alphaBatchGroups = this.alphaBatchGroups;

            // PERF: Reuse slots buffer instead of creating new array
            const slots = this.slotsBuffer;
            slots.length = playerIndices.length;
            let slotCounter = 0;
            for (let j = 0; j < playerIndices.length; j++) slots[j] = slotCounter++;

            // Group players by appearance and animation for alpha pass
            for (let j = 0; j < playerIndices.length; j++) {
                const pid = playerIndices[j] | 0;
                const cachedAlphaCount = this.framePlayerAlphaCounts.get(pid);
                if (cachedAlphaCount !== undefined && (cachedAlphaCount | 0) <= 0) {
                    continue;
                }
                const slot = slots[j] | 0;
                const moving = !!(r.osrsClient.playerEcs as any).isMoving?.(pid);
                const wantsRun = !!(
                    (r.osrsClient.playerEcs as any).isRunVisual?.(pid) ||
                    (r.osrsClient.playerEcs as any).isRunning?.(pid)
                );
                const mode: "idle" | "walk" | "run" = moving ? (wantsRun ? "run" : "walk") : "idle";
                const app = this.renderer.osrsClient.playerEcs.getAppearance(pid);
                if (!app) continue;

                const peInst = r.osrsClient.playerEcs;
                const actionSeqId = peInst.getAnimSeqId(pid) | 0;
                const movementSeqId = peInst.getAnimMovementSeqId(pid) | 0;
                const idleSeqId = peInst.getAnimSeq(pid, "idle") | 0;

                const actionDelay = (pe.getAnimSeqDelay?.(pid) ?? 0) | 0;
                const actionActive = actionSeqId >= 0 && actionDelay === 0;
                const forcedSeq = this.resolveControlledIdleSeqOverride(pid | 0);
                const useActionSequence = forcedSeq === undefined && actionActive;
                const unanimatedIdle = shouldUseUnanimatedIdlePlayer(
                    activePlayerCount,
                    this.canBatchPlayer(pid, peInst) && (pid | 0) !== (combatTargetPid ?? -1),
                    moving,
                    actionActive,
                    movementSeqId,
                    idleSeqId,
                );

                let movementFrameIdx = 0;
                let actionFrameIdx = 0;
                if (!unanimatedIdle) {
                    const controller = this.renderer.osrsClient.playerAnimController;
                    const serverId = this.renderer.osrsClient.playerEcs.getServerIdForIndex(pid);
                    if (controller && serverId !== undefined) {
                        movementFrameIdx =
                            (controller.getMovementSequenceState(serverId)?.frame ?? 0) | 0;
                        actionFrameIdx = (controller.getSequenceState(serverId)?.frame ?? 0) | 0;
                    }
                }

                if (!forcedSeq && !actionActive && (movementSeqId | 0) < 0) continue;

                let seqId =
                    unanimatedIdle
                        ? -1
                        : forcedSeq !== undefined
                        ? forcedSeq.seqId | 0
                        : useActionSequence
                          ? actionSeqId | 0
                          : movementSeqId | 0;
                let frameIdx =
                    unanimatedIdle
                        ? 0
                        : forcedSeq !== undefined
                        ? forcedSeq.frameIdx | 0
                        : useActionSequence
                          ? actionFrameIdx | 0
                          : movementFrameIdx | 0;
                let overlaySeqId: number | undefined;
                let overlayFrameIdx: number | undefined;
                if (useActionSequence) {
                    let canLayer = false;
                    try {
                        const st = this.renderer.osrsClient.seqTypeLoader.load(
                            actionSeqId | 0,
                        ) as any;
                        if (st?.isSkeletalSeq?.()) canLayer = Array.isArray(st.skeletalMasks);
                        else canLayer = Array.isArray(st?.masks) && st.masks.length > 0;
                    } catch {}
                    if (
                        canLayer &&
                        (movementSeqId | 0) >= 0 &&
                        (movementSeqId | 0) !== (idleSeqId | 0)
                    ) {
                        overlaySeqId = movementSeqId | 0;
                        overlayFrameIdx = movementFrameIdx | 0;
                    }
                }

                let effectiveApp = app;
                if (useActionSequence && (app.npcTransformationId ?? -1) < 0) {
                    try {
                        const seqType = this.renderer.osrsClient.seqTypeLoader.load(
                            actionSeqId | 0,
                        );
                        if (seqType && (seqType.leftHandItem >= 0 || seqType.rightHandItem >= 0)) {
                            const newEquip = app.equip.slice();
                            // OSRS cache SeqType stores item IDs with 512 offset (0x200) for equipment overrides.
                            // We must strip this offset to get the actual Item ID for our loader.
                            let shield = seqType.leftHandItem;
                            let weapon = seqType.rightHandItem;
                            if (shield >= 512) shield -= 512;
                            if (weapon >= 512) weapon -= 512;

                            if (shield >= 0) newEquip[EquipmentSlot.SHIELD] = shield;
                            if (weapon >= 0) newEquip[EquipmentSlot.WEAPON] = weapon;
                            effectiveApp = new PlayerAppearance(
                                app.gender,
                                app.colors,
                                app.kits,
                                newEquip,
                                app.headIcons,
                            );
                        }
                    } catch {}
                }
                const serverId = peInst.getServerIdForIndex(pid);
                if (serverId === undefined) continue;
                effectiveApp = this.getFirstPersonAppearance(pid, effectiveApp);
                const renderableAppearance = this.resolveRenderableAppearance(serverId, effectiveApp);
                if (!renderableAppearance) continue;
                effectiveApp = renderableAppearance;

                // Create batch key
                const appKey =
                    effectiveApp?.getCacheKey?.() ??
                    `${effectiveApp?.getHash?.().toString() ?? "0"}|${
                        effectiveApp?.getEquipKey?.() ??
                        effectiveApp?.equip?.slice?.(0, 14)?.join(",") ??
                        ""
                    }`;
                const overlayKey =
                    typeof overlaySeqId === "number" && typeof overlayFrameIdx === "number"
                        ? `|${overlaySeqId | 0}|${overlayFrameIdx | 0}`
                        : "";
                const batchKey = `${appKey}|${seqId}|${frameIdx}${overlayKey}`;

                // Add to alpha batch group
                let group = alphaBatchGroups.get(batchKey);
                if (!group) {
                    group = {
                        appearance: effectiveApp,
                        seqId: seqId | 0,
                        frameIdx: frameIdx | 0,
                        overlaySeqId: overlaySeqId,
                        overlayFrameIdx: overlayFrameIdx,
                        instances: [],
                    };
                    alphaBatchGroups.set(batchKey, group);
                }
                group.instances.push({ slot, pid, mode });
            }
            if (alphaBatchGroups.size === 0) continue;

            // Render batched alpha groups. Rust-primary keeps this CPU-only.
            const draw =
                !rustPrimaryRendererEnabled && drawCallAlpha
                    ? r.configureDrawCall(drawCallAlpha)
                    : undefined;
            const playerEcsAlpha = r.osrsClient?.playerEcs;
            const alphaDeckH = r.getWorldEntityDeckHeight(0, 0);
            const alphaMapPos = vec2.fromValues(map.renderPosX, map.renderPosY);
            if (draw) {
                draw.uniform("u_mapPos", alphaMapPos)
                    .uniform("u_npcDataOffset", baseOffset)
                    .uniform("u_modelYOffset", r.playerYOffset)
                    .uniform("u_worldEntityTransform", WebGLMapSquare.IDENTITY_MAT4)
                    .texture("u_npcDataTexture", playerDataTexture as Texture)
                    .texture("u_heightMap", map.heightMapTexture)
                    .uniform("u_sceneBorderSize", map.borderSize);

                r.app.disable(PicoGL.CULL_FACE);
            }

            for (const [batchKey, group] of alphaBatchGroups) {
                if (group.instances.length === 0) continue;

                const baseRec = this.ensureBaseForAppearance(group.appearance);
                if (!baseRec) continue;

                slots.length = 0;
                let batchSource: (typeof group.instances)[number] | undefined;
                for (const inst of group.instances) {
                    if (this.canBatchPlayer(inst.pid, playerEcsAlpha)) {
                        batchSource ??= inst;
                        slots.push(inst.slot | 0);
                    }
                }

                if (batchSource) {
                    const gpuOwnerKey = `${this.getAppearanceCacheKey(group.appearance)}|seq:${
                        group.seqId | 0
                    }|overlay:${group.overlaySeqId ?? -1}`;
                    let gpuGeometry = this.getPlayerGpuGeometry(gpuOwnerKey, batchKey);
                    if (!gpuGeometry) {
                        this.dynamicUpdateBuffersFor(
                            baseRec.baseModel,
                            baseRec.baseCenterX,
                            baseRec.baseCenterZ,
                            group.seqId,
                            group.frameIdx,
                            batchKey,
                            batchSource.pid,
                            batchSource.mode,
                            group.overlaySeqId,
                            group.overlayFrameIdx,
                            "cacheOnly",
                        );
                        gpuGeometry = this.getPlayerGpuGeometry(gpuOwnerKey, batchKey);
                    }
                    const counts: PlayerGeometryBuildResult = gpuGeometry
                        ? {
                              countOpaque: gpuGeometry.opaque?.count ?? 0,
                              countAlpha: gpuGeometry.alpha?.count ?? 0,
                              opaqueVertices: gpuGeometry.opaque?.vertices,
                              opaqueIndices: gpuGeometry.opaque?.indices,
                              alphaVertices: gpuGeometry.alpha?.vertices,
                              alphaIndices: gpuGeometry.alpha?.indices,
                          }
                        : this.dynamicUpdateBuffersFor(
                              baseRec.baseModel,
                              baseRec.baseCenterX,
                              baseRec.baseCenterZ,
                              group.seqId,
                              group.frameIdx,
                              batchKey,
                              batchSource.pid,
                              batchSource.mode,
                              group.overlaySeqId,
                              group.overlayFrameIdx,
                              "alphaOnly",
                          );
                    if ((counts.countAlpha | 0) > 0) {
                        const playerDraw = gpuGeometry?.alpha
                            ? r
                                  .configureDrawCall(gpuGeometry.alpha.drawCall)
                                  .uniform("u_mapPos", alphaMapPos)
                                  .uniform("u_npcDataOffset", baseOffset)
                                  .uniform("u_modelYOffset", r.playerYOffset)
                                  .uniform("u_worldEntityTransform", WebGLMapSquare.IDENTITY_MAT4)
                                  .texture("u_npcDataTexture", playerDataTexture as Texture)
                                  .texture("u_heightMap", map.heightMapTexture)
                                  .uniform("u_sceneBorderSize", map.borderSize)
                            : draw;
                        if (playerDraw && r.playerSlotBuffer) {
                            drawPlayerSlots(
                                playerDraw,
                                r.playerSlotBuffer,
                                this.playerSlotScratch,
                                slots,
                                counts.countAlpha | 0,
                                true,
                            );
                        }
                        if (counts.alphaVertices && counts.alphaIndices) {
                            mirrorRustPlayerGeometry(
                                r,
                                map,
                                counts.alphaVertices,
                                counts.alphaIndices,
                                baseOffset,
                                slots,
                                r.playerYOffset,
                                WebGLMapSquare.IDENTITY_MAT4,
                                true,
                                false,
                                !!r.cullBackFace,
                            );
                        }
                    }
                }

                for (const inst of group.instances) {
                    if (batchSource && this.canBatchPlayer(inst.pid, playerEcsAlpha)) {
                        continue;
                    }
                    const counts = this.dynamicUpdateBuffersFor(
                        baseRec.baseModel,
                        baseRec.baseCenterX,
                        baseRec.baseCenterZ,
                        group.seqId,
                        group.frameIdx,
                        batchKey,
                        inst.pid,
                        inst.mode,
                        group.overlaySeqId,
                        group.overlayFrameIdx,
                        "alphaOnly",
                    );

                    if ((counts.countAlpha | 0) <= 0) continue;

                    // Per-player WorldView: apply deck height + bobbing transform
                    const wvIdAlpha = playerEcsAlpha?.getWorldViewId?.(inst.pid) ?? -1;
                    let playerModelYOffset = r.playerYOffset;
                    let playerWorldEntityTransform = WebGLMapSquare.IDENTITY_MAT4;
                    if (wvIdAlpha >= 0) {
                        playerModelYOffset = r.playerYOffset + alphaDeckH;
                        playerWorldEntityTransform =
                            r.worldEntityAnimator?.getTransform(wvIdAlpha)
                            ?? WebGLMapSquare.IDENTITY_MAT4;
                        if (draw) {
                            draw.uniform("u_modelYOffset", playerModelYOffset).uniform(
                                "u_worldEntityTransform",
                                playerWorldEntityTransform,
                            );
                        }
                    }

                    // Use drawIdOverride since gl_DrawID will be 0 for single-range legacy draws.
                    if (draw) {
                        draw.uniform("u_drawIdOverride", inst.slot | 0);
                        (draw as any).drawRanges([0, counts.countAlpha | 0, 1]);
                        draw.draw();
                    }
                    if (counts.alphaVertices && counts.alphaIndices) {
                        mirrorRustPlayerGeometry(
                            r,
                            map,
                            counts.alphaVertices,
                            counts.alphaIndices,
                            baseOffset,
                            [inst.slot | 0],
                            playerModelYOffset,
                            playerWorldEntityTransform,
                            true,
                            false,
                            !!r.cullBackFace,
                        );
                    }

                    // Restore overworld uniforms after WE player draw
                    if (wvIdAlpha >= 0 && draw) {
                        draw.uniform("u_modelYOffset", r.playerYOffset).uniform(
                            "u_worldEntityTransform",
                            WebGLMapSquare.IDENTITY_MAT4,
                        );
                    }
                }
                if (draw) draw.uniform("u_drawIdOverride", -1); // Reset
            }
            if (draw && r.cullBackFace) r.app.enable(PicoGL.CULL_FACE);
        }
    }

    private emitPlayerFrameSound(
        pid: number,
        seqId: number,
        frameIdx: number,
        worldX: number,
        worldY: number,
        plane: number,
        isLocalPlayer: boolean,
        channel: "movement" | "action",
    ): void {
        const key = `${pid | 0}|${channel}`;
        const last = this.playerSoundState.get(key);
        if (last && last.seqId === seqId && last.frameIdx === frameIdx) {
            return;
        }
        this.playerSoundState.set(key, { seqId, frameIdx });
        try {
            const mv = this.renderer.osrsClient;
            const seqType = mv.seqTypeLoader.load(seqId);
            // Check for both modern frameSounds and legacy soundEffects formats
            if (!seqType || (!seqType.frameSounds?.size && !seqType.soundEffects?.length)) return;
            mv.handleSeqFrameSounds(seqType, frameIdx, {
                position: { x: worldX, y: worldY, z: plane * 128 },
                isLocalPlayer,
            });
        } catch {}
    }

    private isControlledPid(pid: number): boolean {
        try {
            const mv = this.renderer.osrsClient;
            const idx = resolveControlledPlayerEcsIndex(
                mv.playerEcs,
                mv.controlledPlayerServerId,
            );
            return idx !== undefined && (idx | 0) === (pid | 0);
        } catch {
            return false;
        }
    }

    private canBatchPlayer(pid: number, playerEcs: any): boolean {
        return (
            !!this.renderer.playerSlotBuffer &&
            !this.isControlledPid(pid) &&
            (playerEcs?.getWorldViewId?.(pid) ?? -1) < 0
        );
    }

    private resetRenderSelectionFrameIfNeeded(): void {
        const frameId = (this.renderer.stats?.frameCount ?? 0) | 0;
        if (frameId === this.frameRenderSelectionId) return;
        this.frameRenderSelectionId = frameId;
        this.frameRenderPlayersByMap.clear();
        this.framePlayerAlphaCounts.clear();
    }

    getRenderPlayersForMap(map: WebGLMapSquare): number[] {
        this.resetRenderSelectionFrameIfNeeded();

        const key = ((map.mapX & 0xffff) << 16) | (map.mapY & 0xffff);
        const cached = this.frameRenderPlayersByMap.get(key);
        if (cached) {
            return cached;
        }

        const out: number[] = [];
        const pe = this.renderer.osrsClient.playerEcs;
        const overlayView = this.renderer.osrsClient.worldViewManager.getWorldViewByOverlayMapId(
            map.id,
        );
        const renderSelf = this.renderer.osrsClient.renderSelf !== false;

        for (const activePid of pe.getAllActiveIndices()) {
            const pid = activePid | 0;
            const isFirstPersonPlayer = this.isFirstPersonArmsPlayer(pid);
            if (!renderSelf && !isFirstPersonPlayer && this.isControlledPid(pid)) {
                continue;
            }

            const px = pe.getX(pid) | 0;
            const py = pe.getY(pid) | 0;
            const tileX = (px >> 7) | 0;
            const tileY = (py >> 7) | 0;
            const worldViewId = pe.getWorldViewId(pid) | 0;

            if (overlayView) {
                if ((worldViewId | 0) !== (overlayView.id | 0)) {
                    continue;
                }
                if (!overlayView.containsTile(tileX, tileY)) {
                    continue;
                }
            } else {
                if (worldViewId >= 0) {
                    continue;
                }
                if (
                    getMapIndexFromTile(tileX) !== map.mapX ||
                    getMapIndexFromTile(tileY) !== map.mapY
                ) {
                    continue;
                }
            }
            if (!isFirstPersonPlayer && !this.renderer.shouldRenderPlayerIndex(pid)) {
                continue;
            }

            out.push(pid | 0);
        }

        this.frameRenderPlayersByMap.set(key, out);
        return out;
    }
}
