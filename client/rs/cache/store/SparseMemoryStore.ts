import { GroupMissingError } from "../js5/GroupMissingError";
import { PresenceBitset } from "../js5/PresenceBitset";
import { CacheFiles } from "../CacheFiles";
import { MemoryStore } from "./MemoryStore";
import { Sector } from "./Sector";
import { SectorCluster } from "./SectorCluster";

export type GroupSpan = {
    indexId: number;
    archiveId: number;
    startSector: number;
    sectorCount: number;
    startByte: number;
    byteLength: number;
    /** Size in bytes of the group payload (excluding sector headers). */
    dataSize: number;
};

export type GroupMissListener = (span: GroupSpan) => void;

function groupKey(indexId: number, archiveId: number): string {
    return indexId + ":" + archiveId;
}

function readIdxEntry(
    indexFile: ArrayBuffer,
    archiveId: number,
): { size: number; sector: number } | undefined {
    const offset = archiveId * SectorCluster.SIZE;
    if (offset < 0 || offset + SectorCluster.SIZE > indexFile.byteLength) {
        return undefined;
    }
    const u8 = new Uint8Array(indexFile, offset, SectorCluster.SIZE);
    const size = (u8[0] << 16) | (u8[1] << 8) | u8[2];
    const sector = (u8[3] << 16) | (u8[4] << 8) | u8[5];
    return { size, sector };
}

/**
 * The byte region of the dat2 that an index's group payloads occupy.
 * Groups of an index are packed contiguously, so [min, max) over its idx
 * entries is exact; even for a sparse layout it is a safe superset.
 */
export function computeIndexRegion(
    indexFile: ArrayBuffer,
): { startSector: number; endSector: number } | undefined {
    const entryCount = Math.floor(indexFile.byteLength / SectorCluster.SIZE);
    let min = Number.MAX_SAFE_INTEGER;
    let max = 0;
    for (let archiveId = 0; archiveId < entryCount; archiveId++) {
        const entry = readIdxEntry(indexFile, archiveId);
        if (!entry || entry.sector <= 0 || entry.size <= 0) {
            continue;
        }
        const dataPerSector = archiveId > 0xffff ? Sector.EXTENDED_DATA_SIZE : Sector.DATA_SIZE;
        const end = entry.sector + Math.ceil(entry.size / dataPerSector);
        if (entry.sector < min) {
            min = entry.sector;
        }
        if (end > max) {
            max = end;
        }
    }
    if (max === 0) {
        return undefined;
    }
    return { startSector: min, endSector: max };
}

/**
 * A MemoryStore over a sparsely-populated dat2 buffer. The buffer is allocated
 * at full size but only downloaded regions contain data; a presence bitset
 * tracks which sectors are valid. Reading a group whose sectors are absent
 * notifies the miss listener (which queues a network fetch) and throws
 * GroupMissingError, mirroring the original client's "return null, request,
 * retry" pattern.
 */
export class SparseMemoryStore extends MemoryStore {
    onMiss?: GroupMissListener;

    /** Total misses observed; lets callers detect misses across a whole task. */
    missCount = 0;

    /** Assembled data for (rare) fragmented groups that can't live in the sparse buffer. */
    private readonly overrides = new Map<string, Int8Array>();

    static fromSparseFiles(
        cacheFiles: CacheFiles,
        presence: PresenceBitset,
        indicesToLoad: number[] = [],
    ): SparseMemoryStore {
        const files = cacheFiles.files;

        const indexFiles: ArrayBuffer[] = [];
        const indicesSet = new Set(indicesToLoad);
        for (const [name, data] of files.entries()) {
            if (
                name !== CacheFiles.META_FILE_NAME &&
                name.startsWith(CacheFiles.INDEX_FILE_PREFIX)
            ) {
                const indexId = parseInt(name.slice(CacheFiles.INDEX_FILE_PREFIX.length));
                if (indicesSet.size === 0 || indicesSet.has(indexId)) {
                    indexFiles[indexId] = data;
                }
            }
        }

        const dataFile = files.get(CacheFiles.DAT2_FILE_NAME);
        if (!dataFile) {
            throw new Error("main_file_cache data file not found");
        }
        const metaFile = files.get(CacheFiles.META_FILE_NAME);
        return new SparseMemoryStore(dataFile, indexFiles, presence, metaFile);
    }

    constructor(
        dataFile: ArrayBuffer,
        indexFiles: (ArrayBuffer | undefined)[],
        readonly presence: PresenceBitset,
        metaFile?: ArrayBuffer,
    ) {
        super(dataFile, indexFiles, metaFile);
    }

    getGroupSpan(indexId: number, archiveId: number): GroupSpan | undefined {
        const indexFile = this.getIndexFile(indexId);
        if (!indexFile) {
            return undefined;
        }
        const entry = readIdxEntry(indexFile, archiveId);
        if (!entry || entry.sector <= 0 || entry.size <= 0) {
            return undefined;
        }
        const dataPerSector = archiveId > 0xffff ? Sector.EXTENDED_DATA_SIZE : Sector.DATA_SIZE;
        const sectorCount = Math.ceil(entry.size / dataPerSector);
        return {
            indexId,
            archiveId,
            startSector: entry.sector,
            sectorCount,
            startByte: entry.sector * Sector.SIZE,
            byteLength: sectorCount * Sector.SIZE,
            dataSize: entry.size,
        };
    }

    isGroupPresent(indexId: number, archiveId: number): boolean {
        if (this.overrides.has(groupKey(indexId, archiveId))) {
            return true;
        }
        const span = this.getGroupSpan(indexId, archiveId);
        if (!span) {
            // Nothing to fetch; let the base read path produce its natural error.
            return true;
        }
        return this.presence.hasSectors(span.startSector, span.sectorCount);
    }

    setOverride(indexId: number, archiveId: number, data: Int8Array): void {
        this.overrides.set(groupKey(indexId, archiveId), data);
    }

    /** Write fetched bytes into the sparse buffer and mark their sectors present. */
    applyRange(byteOffset: number, bytes: Uint8Array): void {
        if (byteOffset % Sector.SIZE !== 0) {
            console.warn(`[js5] Ignoring unaligned range at ${byteOffset}`);
            return;
        }
        const end = Math.min(byteOffset + bytes.byteLength, this.dataFile.byteLength);
        if (end <= byteOffset) {
            return;
        }
        new Uint8Array(this.dataFile).set(bytes.subarray(0, end - byteOffset), byteOffset);
        // The final sector of the file may be truncated; treat reaching EOF as
        // completing that sector.
        const sectorCount =
            end >= this.dataFile.byteLength
                ? Math.ceil((end - byteOffset) / Sector.SIZE)
                : Math.floor((end - byteOffset) / Sector.SIZE);
        this.presence.markSectors(byteOffset / Sector.SIZE, sectorCount);
    }

    override read(indexId: number, archiveId: number): Int8Array {
        const override = this.overrides.get(groupKey(indexId, archiveId));
        if (override) {
            return override;
        }
        const span = this.getGroupSpan(indexId, archiveId);
        if (span && !this.presence.hasSectors(span.startSector, span.sectorCount)) {
            this.missCount++;
            this.onMiss?.(span);
            throw new GroupMissingError(indexId, archiveId, span.startByte, span.byteLength);
        }
        return super.read(indexId, archiveId);
    }
}
