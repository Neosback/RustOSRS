import { lruGet, lruSet } from "../../../common/utils/BoundedLru";
import { CacheIndex } from "../../cache/CacheIndex";
import { CacheInfo } from "../../cache/CacheInfo";
import { Dat2SeqBase, SeqBase } from "./SeqBase";

export interface SeqBaseLoader {
    load(id: number): SeqBase | undefined;

    clearCache(): void;
}

const SEQ_BASE_CACHE_MAX = 256;

export class IndexSeqBaseLoader implements SeqBaseLoader {
    bases: Map<number, SeqBase> = new Map();

    constructor(
        readonly cacheInfo: CacheInfo,
        readonly index: CacheIndex,
    ) {}

    load(id: number): SeqBase | undefined {
        const cached = lruGet(this.bases, id);
        if (cached) {
            return cached;
        }

        const file = this.index.getFile(id, 0);
        if (!file) {
            return undefined;
        }
        const base = Dat2SeqBase.load(this.cacheInfo, id, file.data);
        lruSet(this.bases, id, base, SEQ_BASE_CACHE_MAX);
        return base;
    }

    clearCache(): void {
        this.bases.clear();
    }
}
