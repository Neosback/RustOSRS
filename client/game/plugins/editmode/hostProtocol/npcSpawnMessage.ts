export const NPC_SPAWN_REQUEST_MESSAGE = "elvarg:npc-spawns-request";
export const NPC_SPAWN_MESSAGE = "elvarg:npc-spawns";
export const CUSTOM_NPC_SPAWNS_MESSAGE = "elvarg:custom-npc-spawns";
export type BrowserHostNpcSpawn = { id: number; tileX: number; tileY: number; plane: number; direction?: number };
export type NpcSpawnRequestMessage = { type: typeof NPC_SPAWN_REQUEST_MESSAGE; regionIds: number[] };
export type NpcSpawnMessage = { type: typeof NPC_SPAWN_MESSAGE; regionIds: number[]; spawns: BrowserHostNpcSpawn[] };
export type CustomNpcSpawnsMessage = { type: typeof CUSTOM_NPC_SPAWNS_MESSAGE; spawns: BrowserHostNpcSpawn[] };
export function regionIdForTile(tileX: number, tileY: number): number {
    return ((tileX >> 6) << 8) | (tileY >> 6);
}

export function formatCustomNpcSpawns(spawns: readonly unknown[]): string {
    return spawns.length ? "[\n" + spawns.map((spawn) => "  " + JSON.stringify(spawn)).join(",\n") + "\n]\n" : "[]\n";
}
