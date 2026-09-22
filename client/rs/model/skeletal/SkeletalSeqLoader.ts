import { lruGet, lruSet } from "../../../common/utils/BoundedLru";
import { Archive } from "../../cache/Archive";
import { CacheIndex } from "../../cache/CacheIndex";
import { isGroupMissingError } from "../../cache/js5/GroupMissingError";
import { SeqBaseLoader } from "../seq/SeqBaseLoader";
import { SkeletalSeq } from "./SkeletalSeq";

export interface SkeletalSeqLoader {
    load(id: number): SkeletalSeq | undefined;

    clearCache(): void;
}

const SKELETAL_SEQ_CACHE_MAX = 128;
const SKELETAL_ARCHIVE_CACHE_MAX = 64;

export class IndexSkeletalSeqLoader implements SkeletalSeqLoader {
    seqs: Map<number, SkeletalSeq> = new Map();

    archiveCache: Map<number, Archive> = new Map();

    constructor(
        readonly animIndex: CacheIndex,
        readonly baseLoader: SeqBaseLoader,
    ) {}

    load(id: number): SkeletalSeq | undefined {
        const cached = lruGet(this.seqs, id);
        if (cached) {
            return cached;
        }

        const archiveId = id >> 16;
        const fileId = id & 0xffff;

        let archive = lruGet(this.archiveCache, archiveId);
        if (!archive) {
            try {
                archive = this.animIndex.getArchive(archiveId);
            } catch (e) {
                // Group not downloaded yet (fetch already queued); render a
                // static pose until it arrives.
                if (!isGroupMissingError(e)) {
                    throw e;
                }
                return undefined;
            }
            lruSet(this.archiveCache, archiveId, archive, SKELETAL_ARCHIVE_CACHE_MAX);
        }

        const file = archive.getFile(fileId);
        if (!file) {
            return undefined;
        }

        const skeletalSeq = SkeletalSeq.load(this.baseLoader, id, file.data);
        lruSet(this.seqs, id, skeletalSeq, SKELETAL_SEQ_CACHE_MAX);
        return skeletalSeq;
    }

    clearCache(): void {
        this.seqs.clear();
        this.archiveCache.clear();
    }
}
