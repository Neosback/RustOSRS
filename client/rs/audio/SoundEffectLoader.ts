import { CacheIndex } from "../cache/CacheIndex";
import { CacheInfo } from "../cache/CacheInfo";
import { CacheSystem } from "../cache/CacheSystem";
import { detectCacheType } from "../cache/CacheType";
import { IndexType } from "../cache/IndexType";
import { isGroupMissingError } from "../cache/js5/GroupMissingError";
import { retryOnMissingGroup } from "../cache/js5/retryOnMissingGroup";
import { RawSoundData, SoundEffect } from "./legacy/SoundEffect";

export class SoundEffectLoader {
    private readonly index?: CacheIndex;

    constructor(cacheInfo: CacheInfo, cacheSystem: CacheSystem) {
        const type = detectCacheType(cacheInfo);

        let indexId: number | undefined;
        if (type === "dat") {
            if (cacheSystem.indexExists(IndexType.DAT.sounds)) {
                indexId = IndexType.DAT.sounds;
            }
        } else if (type === "dat2") {
            const soundEffectsIdx = IndexType.DAT2.soundEffects;

            if (cacheSystem.indexExists(soundEffectsIdx)) {
                indexId = soundEffectsIdx;
            } else if (cacheSystem.indexExists(4)) {
                // Try index 4 directly
                indexId = 4;
            }
        }
        if (indexId !== undefined) {
            this.index = cacheSystem.getIndex(indexId);
        }
    }

    available(): boolean {
        return !!this.index;
    }

    private tryDecode(id: number, trimOnset = true): RawSoundData | undefined {
        if (!this.index) return undefined;
        const file = this.index.getFileSmart(id);
        if (!file) return undefined;
        const buffer = file.getDataAsBuffer();
        const effect = SoundEffect.decode(buffer);
        const delayCycles = trimOnset ? effect.calculateDelay() : 0;
        const raw = effect.toRawSound();
        if (!raw || !raw.samples || raw.samples.length <= 0) return undefined;
        raw.delayCycles = delayCycles;
        return raw;
    }

    load(soundId: number, trimOnset = true): RawSoundData | undefined {
        if (!this.index) return undefined;
        try {
            return this.tryDecode(soundId, trimOnset);
        } catch (err) {
            // Missing groups are queued for on-demand fetch; the caller falls
            // back to loadWithRetry to pick the sound up once it lands.
            if (!isGroupMissingError(err)) {
                console.log("[SoundEffectLoader] failed to load sound", soundId, err);
            }
            return undefined;
        }
    }

    /** Like load(), but waits out an on-demand fetch instead of skipping it. */
    async loadWithRetry(soundId: number, trimOnset = true): Promise<RawSoundData | undefined> {
        if (!this.index) return undefined;
        try {
            return await retryOnMissingGroup(() => this.tryDecode(soundId, trimOnset));
        } catch (err) {
            console.log("[SoundEffectLoader] failed to load sound", soundId, err);
            return undefined;
        }
    }
}
