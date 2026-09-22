import { existsSync, readFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const clientDir = path.resolve(scriptDir, "..");

function commandVersion(command, args = ["--version"]) {
    const result = spawnSync(command, args, {
        cwd: clientDir,
        encoding: "utf8",
        env: process.env,
    });
    if (result.error || result.status !== 0) return undefined;
    return `${result.stdout ?? ""} ${result.stderr ?? ""}`.trim();
}

function parseSimpleEnv(filePath) {
    if (!existsSync(filePath)) return {};
    const result = {};
    for (const rawLine of readFileSync(filePath, "utf8").split(/\r?\n/)) {
        const line = rawLine.trim();
        if (!line || line.startsWith("#")) continue;
        const index = line.indexOf("=");
        if (index <= 0) continue;
        const key = line.slice(0, index).trim();
        let value = line.slice(index + 1).trim();
        if (
            (value.startsWith('"') && value.endsWith('"')) ||
            (value.startsWith("'") && value.endsWith("'"))
        ) {
            value = value.slice(1, -1);
        }
        result[key] = value;
    }
    return result;
}

function nodeSupported() {
    const [major, minor] = process.versions.node.split(".").map(Number);
    return major === 22 && minor >= 5;
}

const env = {
    ...parseSimpleEnv(path.join(clientDir, ".env")),
    ...parseSimpleEnv(path.join(clientDir, ".env.local")),
    ...process.env,
};

const checks = [];
const warnings = [];

checks.push({
    ok: nodeSupported(),
    label: `Node ${process.versions.node}`,
    help: "Use Node 22.5+ (but below Node 23). From the repo root, 'nvm install && nvm use' uses .nvmrc.",
});

const corepack = commandVersion("corepack");
checks.push({
    ok: !!corepack,
    label: corepack ? `Corepack ${corepack}` : "Corepack",
    help: "Install/use the supported Node 22 release, then run 'corepack enable'.",
});

const cargo = commandVersion("cargo");
checks.push({
    ok: !!cargo,
    label: cargo ? cargo : "Cargo",
    help: "Install Rust with rustup from https://rustup.rs/ and reopen your shell.",
});

const rustup = commandVersion("rustup");
checks.push({
    ok: !!rustup,
    label: rustup ? rustup : "rustup",
    help: "Install Rust with rustup from https://rustup.rs/ and reopen your shell.",
});

if (rustup) {
    const installedTargets = commandVersion("rustup", ["target", "list", "--installed"]) ?? "";
    checks.push({
        ok: installedTargets.split(/\s+/).includes("wasm32-unknown-unknown"),
        label: "Rust target wasm32-unknown-unknown",
        help: "Run 'rustup target add wasm32-unknown-unknown'.",
    });
}

const linker = commandVersion("cc", ["--version"]);
if (!linker) {
    warnings.push(
        process.platform === "darwin"
            ? "No C compiler detected. If Rust native tools fail to link, run 'xcode-select --install'."
            : "No C compiler detected. Install your distribution's build-essential/base-devel toolchain if Rust native tools fail to link.",
    );
}

const dependenciesInstalled =
    existsSync(path.join(clientDir, "node_modules")) ||
    existsSync(path.join(clientDir, ".pnp.cjs")) ||
    existsSync(path.join(clientDir, ".pnp.loader.mjs")) ||
    existsSync(path.join(clientDir, ".yarn", "install-state.gz"));
checks.push({
    ok: dependenciesInstalled,
    label: "JavaScript dependencies",
    help: "Run 'node scripts/bootstrap.mjs'.",
});

const wasmPackage = existsSync(
    path.join(clientDir, "public", "rust-renderer", "rustosrs_renderer_bg.wasm"),
);
checks.push({
    ok: wasmPackage,
    label: "Rust/WASM renderer package",
    help: "Run 'yarn build:rust-renderer' or 'node scripts/bootstrap.mjs'.",
});

const configuredCacheDir = env.RUSTOSRS_CACHE_DIR
    ? path.resolve(clientDir, env.RUSTOSRS_CACHE_DIR)
    : undefined;
const cacheCandidates = [
    configuredCacheDir,
    path.join(clientDir, "caches"),
    path.resolve(clientDir, "../server/caches"),
].filter(Boolean);
const localCache = cacheCandidates.find(
    (candidate) => existsSync(path.join(candidate, "caches.json")),
);
const remoteCache = String(env.REACT_APP_CACHE_BASE_URL ?? "").trim();

if (localCache) {
    console.log(`[ok] Cache source: ${localCache}`);
} else if (remoteCache) {
    console.log(`[ok] Cache source: ${remoteCache}`);
} else {
    warnings.push(
        "No cache source is configured. Put cache data under client/caches/, set RUSTOSRS_CACHE_DIR, or set REACT_APP_CACHE_BASE_URL in .env.local.",
    );
}

const wsUrl = String(env.REACT_APP_DEFAULT_WS_URL ?? "ws://localhost:43594").trim();
console.log(`[info] Game server: ${wsUrl}`);
if (!env.REACT_APP_DEFAULT_WS_URL) {
    warnings.push(
        "Using the default game server ws://localhost:43594. A compatible server is not bundled with this repository.",
    );
}

let failures = 0;
for (const check of checks) {
    if (check.ok) {
        console.log(`[ok] ${check.label}`);
    } else {
        failures++;
        console.error(`[missing] ${check.label}`);
        console.error(`          ${check.help}`);
    }
}
for (const warning of warnings) {
    console.warn(`[warning] ${warning}`);
}

if (failures > 0) {
    console.error(`\nEnvironment check failed with ${failures} required item(s) missing.`);
    process.exitCode = 1;
} else {
    console.log("\nEnvironment check passed.");
}
