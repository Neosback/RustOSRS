export const NPC_INTERACTIONS_MESSAGE = "elvarg:npc-interactions";
export const NPC_INTERACTIONS_REQUEST_MESSAGE = "elvarg:npc-interactions-request";

export type NpcInteractionsMessage = { type: typeof NPC_INTERACTIONS_MESSAGE; contents: string };
