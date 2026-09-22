export type PlayerEcsServerIdLookup = {
    getIndexForServerId(serverId: number): number | undefined;
};

/**
 * Controlled player server id 0 is valid. Only negative ids mean that the
 * controlled player has not been assigned yet.
 */
export function resolveControlledPlayerEcsIndex(
    playerEcs: PlayerEcsServerIdLookup,
    controlledPlayerServerId: number,
): number | undefined {
    const serverId = controlledPlayerServerId | 0;
    return serverId >= 0 ? playerEcs.getIndexForServerId(serverId) : undefined;
}
