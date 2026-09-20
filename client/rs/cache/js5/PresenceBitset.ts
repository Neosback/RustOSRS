/**
 * Tracks which 520-byte dat2 sectors have been downloaded.
 * Backed by a SharedArrayBuffer when available so workers and the main thread
 * observe fetches from any context.
 */
export class PresenceBitset {
    private readonly shared: boolean;

    static forSectorCount(sectorCount: number, shared: boolean): PresenceBitset {
        const byteLength = (sectorCount + 7) >> 3;
        const buffer =
            shared && typeof SharedArrayBuffer !== "undefined"
                ? new SharedArrayBuffer(byteLength)
                : new ArrayBuffer(byteLength);
        return new PresenceBitset(new Uint8Array(buffer));
    }

    constructor(readonly bits: Uint8Array) {
        this.shared =
            typeof SharedArrayBuffer !== "undefined" && bits.buffer instanceof SharedArrayBuffer;
    }

    markSectors(startSector: number, count: number): void {
        for (let s = startSector; s < startSector + count; s++) {
            const idx = s >> 3;
            const mask = 1 << (s & 7);
            if (idx < 0 || idx >= this.bits.length) {
                continue;
            }
            if (this.shared) {
                Atomics.or(this.bits, idx, mask);
            } else {
                this.bits[idx] |= mask;
            }
        }
    }

    hasSectors(startSector: number, count: number): boolean {
        const end = startSector + count;
        let s = startSector;
        while (s < end) {
            const idx = s >> 3;
            if (idx < 0 || idx >= this.bits.length) {
                return false;
            }
            const bits = this.shared ? Atomics.load(this.bits, idx) : this.bits[idx];
            // Whole-byte fast path: 8 aligned sectors per load.
            if ((s & 7) === 0 && s + 8 <= end) {
                if (bits !== 0xff) {
                    return false;
                }
                s += 8;
                continue;
            }
            if ((bits & (1 << (s & 7))) === 0) {
                return false;
            }
            s++;
        }
        return true;
    }
}
