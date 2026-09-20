export const REGION_PACK_MESSAGE = "elvarg:region-pack";
export const REGION_PACK_REQUEST_MESSAGE = "elvarg:region-packs-request";
export type RegionPackMessage = { type: typeof REGION_PACK_MESSAGE; regionId: number; data: Uint8Array };
export type RegionPackRequestMessage = { type: typeof REGION_PACK_REQUEST_MESSAGE };
export type RegionPackReceiver = (message: RegionPackMessage) => void;
export function copyRegionPackBytes(value: unknown): Uint8Array | undefined {
    if (!ArrayBuffer.isView(value)) return undefined;
    return new Uint8Array(value.buffer, value.byteOffset, value.byteLength).slice();
}
