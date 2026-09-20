export const OBJECT_SPAWNS_REQUEST_MESSAGE = "elvarg:object-spawns-request";
export const OBJECT_SPAWNS_MESSAGE = "elvarg:object-spawns";
export type ObjectSpawn = {
    id: number;
    position: { x: number; y: number; z: number };
    type?: number;
    face?: number;
    remove?: boolean;
};
export type ObjectSpawnsRequestMessage = { type: typeof OBJECT_SPAWNS_REQUEST_MESSAGE };
export type ObjectSpawnsMessage = { type: typeof OBJECT_SPAWNS_MESSAGE; contents: string };

export function parseObjectSpawns(contents: string): ObjectSpawn[] {
    if (typeof contents !== "string" || contents.length > 2_000_000) throw new Error("Invalid object-spawns.json");
    const spawns = JSON.parse(contents);
    const integer = (value: unknown, max: number) => Number.isInteger(value) && (value as number) >= 0 && (value as number) <= max;
    if (!Array.isArray(spawns) || spawns.some((spawn) =>
        !spawn || !integer(spawn.id, 0x7fffffff) || !integer(spawn.position?.x, 0x3fff)
        || !integer(spawn.position?.y, 0x3fff) || !integer(spawn.position?.z, 3)
        || (spawn.type !== undefined && !integer(spawn.type, 22))
        || (spawn.face !== undefined && !integer(spawn.face, 3))
        || (spawn.remove !== undefined && typeof spawn.remove !== "boolean")
    )) throw new Error("Invalid object-spawns.json");
    return spawns;
}

export function formatObjectSpawns(spawns: readonly ObjectSpawn[]): string {
    return spawns.length ? "[\n" + spawns.map((spawn) => "  " + formatJsonLine(spawn)).join(",\n") + "\n]\n" : "[]\n";
}

/** Space JSON punctuation without changing punctuation inside strings. */
export function formatJsonLine(value: unknown): string {
    return JSON.stringify(value).replace(/"(?:\\.|[^"\\])*"|\{\}|[{},:]/g, (token) => {
        if (token === "{") return "{ ";
        if (token === "}") return " }";
        if (token === "," || token === ":") return token + " ";
        return token;
    });
}
