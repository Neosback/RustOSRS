/**
 * The cache's chatbox search (clientscript 750) only knows how to search item names and
 * only offers "Select". The server reuses it as a spawner by opening it with one of these
 * titles, which switches the search over to npc types and puts the spawn amounts on each
 * row. The chosen id and op travel back together in the dialogue the search already resumes.
 */
export const ITEM_SEARCH_TITLE = "Item Search";
export const NPC_SEARCH_TITLE = "NPC Search";

/** Op index -> amount is the server's business; the client only sends which op was used. */
export const SPAWN_OPS = ["Spawn 1", "Spawn 5", "Spawn 10", "Spawn X"];

// Cache script the search rows run when one is picked. It identifies a result row.
const SELECT_SCRIPT = 754;

let mode: "item" | "npc" | null = null;
let results = new Set<number>();

export function setSpawnSearch(title: unknown): void {
    mode = title === NPC_SEARCH_TITLE ? "npc" : title === ITEM_SEARCH_TITLE ? "item" : null;
    results = new Set();
}

export function isNpcSearch(): boolean {
    return mode === "npc";
}

export function setNpcSearchResults(ids: number[]): void {
    results = new Set(ids);
}

/** True only for ids the open npc search produced, so item lookups elsewhere stay item lookups. */
export function isNpcSearchResult(id: number): boolean {
    return mode === "npc" && results.has(id);
}

/**
 * Turns a result row's item icon into the npc's portrait. Only npc rows of an open spawn
 * search get here, so ordinary item icons never touch the model path. The renderer draws
 * the chathead, falling back to the body model for the ~5k npcs the cache gives no head.
 */
export function applyNpcRowIcon(widget: any, npcId: number): boolean {
    if (!isNpcSearchResult(npcId)) return false;
    widget.type = 6;
    widget.modelType = 2;
    widget.modelId = npcId;
    widget.isNpcChathead = true;
    widget.isPlayerChathead = false;
    widget.npcTypeId = npcId;
    widget.npcPortraitFit = true;
    widget.itemId = -1;
    return true;
}

function searchRowId(widget: any): number | null {
    // setEventHandler keeps the listener args as [scriptId, id, ...].
    const listener = widget?.onOp;
    if (mode === null || !Array.isArray(listener) || listener[0] !== SELECT_SCRIPT) return null;
    return typeof listener[1] === "number" ? listener[1] : null;
}

/** Replaces the row's single "Select" with the spawn amounts. No-op outside a spawn search. */
export function applySpawnSearchOps(widget: any): void {
    if (searchRowId(widget) === null) return;
    widget.actions = [...SPAWN_OPS];
}

/** The dialogue value for a clicked row: "<id> <op>", or null when the click is not ours. */
export function spawnSearchPick(widget: any, opIndex: number): string | null {
    const id = searchRowId(widget);
    if (id === null || !(opIndex >= 1 && opIndex <= SPAWN_OPS.length)) return null;
    return `${id} ${opIndex}`;
}
