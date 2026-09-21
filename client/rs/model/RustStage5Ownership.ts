export type RustStage5Path =
    | "skeletal"
    | "legacy"
    | "contour"
    | "basic"
    | "mirror"
    | "uv"
    | "normals"
    | "lighting"
    | "vertexBuilder"
    | "facePreparation"
    | "modelInfo"
    | "modelHash"
    | "drawList";

export type RustStage5PathStats = {
    attempts: number;
    successes: number;
    fallbacks: number;
    failures: number;
};

export type RustStage5OwnershipStats = Record<RustStage5Path, RustStage5PathStats>;

const PATHS: RustStage5Path[] = [
    "skeletal",
    "legacy",
    "contour",
    "basic",
    "mirror",
    "uv",
    "normals",
    "lighting",
    "vertexBuilder",
    "facePreparation",
    "modelInfo",
    "modelHash",
    "drawList",
];

let strictMode = false;
let forceTypeScript =
    typeof window !== "undefined"
    && new URLSearchParams(window.location.search).get("rust-renderer") === "off";

function emptyPathStats(): RustStage5PathStats {
    return {
        attempts: 0,
        successes: 0,
        fallbacks: 0,
        failures: 0,
    };
}

const stats = Object.fromEntries(
    PATHS.map((path) => [path, emptyPathStats()]),
) as RustStage5OwnershipStats;

export function setRustStage5StrictMode(enabled: boolean): void {
    strictMode = enabled;
}

export function isRustStage5StrictMode(): boolean {
    return strictMode;
}

export function setRustStage5ForceTypeScript(enabled: boolean): void {
    forceTypeScript = enabled;
}

export function isRustStage5ForceTypeScript(): boolean {
    return forceTypeScript;
}

export function resetRustStage5OwnershipStats(): void {
    for (const path of PATHS) {
        stats[path] = emptyPathStats();
    }
}

export function getRustStage5OwnershipStats(): RustStage5OwnershipStats {
    return Object.fromEntries(
        PATHS.map((path) => [path, { ...stats[path] }]),
    ) as RustStage5OwnershipStats;
}

export function recordRustStage5Attempt(path: RustStage5Path): void {
    stats[path].attempts++;
}

export function recordRustStage5Success(path: RustStage5Path): void {
    stats[path].successes++;
}

export function recordRustStage5Fallback(
    path: RustStage5Path,
    reason: string,
    failed: boolean = false,
): void {
    stats[path].fallbacks++;
    if (failed) {
        stats[path].failures++;
    }
    if (strictMode) {
        throw new Error(`[RustStage5] ${path} fallback: ${reason}`);
    }
}
