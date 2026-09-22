import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const clientDir = path.resolve(scriptDir, "..");

function source(relativePath) {
    return readFileSync(path.join(clientDir, relativePath), "utf8");
}

function assertIncludes(text, expected, label) {
    if (!text.includes(expected)) {
        throw new Error(label + ": missing expected ownership guard");
    }
}

function assertExcludes(text, unexpected, label) {
    if (text.includes(unexpected)) {
        throw new Error(label + ": legacy primary allocation contract regressed");
    }
}

function functionBody(text, signature, nextSignature) {
    const start = text.indexOf(signature);
    if (start < 0) throw new Error("Could not find " + signature);
    const end = nextSignature ? text.indexOf(nextSignature, start + signature.length) : -1;
    return text.slice(start, end >= 0 ? end : text.length);
}

const mapSquare = source("render/WebGLMapSquare.ts");
assertIncludes(
    mapSquare,
    "type LegacyMapTextureState = {",
    "WebGLMapSquare lazy map textures",
);
assertIncludes(
    mapSquare,
    "function ensureLegacyMapTextures(state: LegacyMapTextureState)",
    "WebGLMapSquare lazy map textures",
);
assertIncludes(
    mapSquare,
    "get heightMapTexture(): Texture",
    "WebGLMapSquare legacy fallback texture accessor",
);
assertIncludes(
    mapSquare,
    "deleteLegacyMapTextures(this.legacyMapTextureState);",
    "WebGLMapSquare legacy texture cleanup",
);
assertIncludes(
    mapSquare,
    "releaseLegacySceneGpuResources(): void",
    "WebGLMapSquare primary-resume GPU cleanup",
);
assertIncludes(
    mapSquare,
    "dematerializeDrawCallRange(resources.drawCall);",
    "WebGLMapSquare repeatable fallback draw-call cleanup",
);
assertExcludes(
    mapSquare,
    "readonly heightMapTexture: Texture",
    "WebGLMapSquare eager map texture ownership",
);
assertExcludes(
    mapSquare,
    "const heightMapTexture = app.createTextureArray(",
    "WebGLMapSquare eager map texture allocation",
);

const drawHelpers = source("render/render/draw2.ts");
const actorTextureUpload = functionBody(
    drawHelpers,
    "export function updateActorDataTexture",
    "export function _accumulate",
);
assertIncludes(
    actorTextureUpload,
    "if (rustPrimaryRendererEnabled) {",
    "actor-data primary ownership branch",
);
assertIncludes(
    actorTextureUpload,
    "mirrorRustActorData(host, uploadView, texWidth, texHeight);",
    "actor-data CPU-to-Rust upload",
);
assertIncludes(
    actorTextureUpload,
    "host.actorDataTextures[i]?.delete();",
    "legacy actor texture release on Rust-primary resume",
);

const opaqueActors = source("render/render/frame/render4.ts");
assertIncludes(
    opaqueActors,
    "if (!rustPrimaryRendererEnabled && !actorDataTexture) return;",
    "opaque actor traversal legacy texture gate",
);

const transparentNpcs = source("render/render/frame/render3.ts");
assertIncludes(
    transparentNpcs,
    "(!rustPrimaryRendererEnabled && !npcDataTexture)",
    "transparent NPC traversal legacy texture gate",
);

const transparentActors = source("render/render/frame/render5.ts");
assertIncludes(
    transparentActors,
    "if (rustPrimaryRendererEnabled || playerDataTexture)",
    "transparent effect traversal primary actor-data path",
);

const rustIntegration = source("render/rust/RustShadowIntegration.ts");
const currentActorSync = functionBody(
    rustIntegration,
    "function syncCurrentActorData",
    "export async function initRustRendererShadow",
);
assertExcludes(
    currentActorSync,
    "actorDataTextures",
    "Rust actor recovery must not depend on Pico actor textures",
);
assertIncludes(
    rustIntegration,
    "function releaseLegacySceneGpuResourcesForPrimary(",
    "Rust-primary legacy map GPU reclamation",
);
assertIncludes(
    rustIntegration,
    "map.releaseLegacySceneGpuResources();",
    "Rust-primary resident-map GPU reclamation",
);
const primaryReleaseCalls =
    rustIntegration.match(/releaseLegacySceneGpuResourcesForPrimary\(host, runtime\);/g) ?? [];
if (primaryReleaseCalls.length < 2) {
    throw new Error(
        "Rust-primary legacy map GPU reclamation must run on initial activation and recovery",
    );
}

const npc = source("render/render/anim/npc2.ts");
const npcUpload = functionBody(
    npc,
    "export function uploadDynamicNpcGeometry",
    "export function resolveUnbatchedNpcGeometry",
);
assertIncludes(
    npcUpload,
    "if (isRustPrimaryRendererActive(host)) return 0;",
    "dynamic NPC low-level Pico upload guard",
);

const player = source("render/player/PlayerRenderer.ts");
const playerCapacity = functionBody(
    player,
    "private ensurePlayerGpuCapacity(",
    "private ensurePlayerGpuCapacityAlpha(",
);
assertIncludes(
    playerCapacity,
    "if (isRustPrimaryRendererActive(this.renderer)) return;",
    "opaque player Pico capacity guard",
);
const playerAlphaCapacity = functionBody(
    player,
    "private ensurePlayerGpuCapacityAlpha(",
    "private captureCurrentVariant(",
);
assertIncludes(
    playerAlphaCapacity,
    "if (isRustPrimaryRendererActive(this.renderer)) return;",
    "alpha player Pico capacity guard",
);
const playerGpuGeometry = functionBody(
    player,
    "private getPlayerGpuGeometry(",
    "private updatePlayerGpuPass(",
);
assertIncludes(
    playerGpuGeometry,
    "if (isRustPrimaryRendererActive(this.renderer))",
    "player geometry cache primary guard",
);

const spotCache = source("render/gfx/SpotAnimGpuCache.ts");
const spotGetOrCreate = functionBody(
    spotCache,
    "getOrCreate(",
    "clear(): void",
);
assertIncludes(
    spotGetOrCreate,
    "if (isRustPrimaryRendererActive(this.renderer)) return undefined;",
    "spot-animation Pico cache guard",
);

const gfxRenderer = source("render/gfx/GfxRenderer.ts");
assertIncludes(
    gfxRenderer,
    "if (!rustPrimaryRendererEnabled) {",
    "GFX legacy GPU materialization gate",
);

const projectileRenderer = source("render/projectiles/ProjectileRenderer.ts");
assertIncludes(
    projectileRenderer,
    "if (!rustPrimaryRendererEnabled) {",
    "projectile legacy GPU materialization gate",
);

const mapLoader = source("render/render/map.ts");
assertIncludes(
    mapLoader,
    "!isRustPrimaryRendererEnabled()",
    "map legacy GPU startup eager-materialization gate",
);
assertExcludes(
    mapLoader,
    "!isRustPrimaryRendererActive(host)",
    "map loading must defer Pico resources before the Rust runtime becomes active",
);

const draw3 = source("render/render/draw3.ts");
assertIncludes(
    draw3,
    "!isRustPrimaryRendererEnabled()",
    "ground-item legacy GPU startup eager-materialization gate",
);

const frame = source("render/render/frame/render.ts");
assertIncludes(
    frame,
    "if (rustPrimaryRendererEnabled)",
    "primary frame ownership branch",
);
assertIncludes(
    frame,
    "host.textureFramebuffer = undefined;",
    "primary Pico offscreen texture framebuffer cleanup",
);
const settings = source("render/render/settings.ts");
assertIncludes(
    settings,
    "if (!isRustPrimaryRendererActive(host)) {\n                host.initTextureFramebuffer(width, height);",
    "Rust-primary resize must not allocate the legacy presentation framebuffer",
);

const widgetsOverlay = source("ui/devoverlay/WidgetsOverlay.ts");
assertIncludes(
    widgetsOverlay,
    'this.overlayCanvas.style.zIndex = "2";',
    "widget canvas must remain above the Rust scene and Pico overlay canvases",
);

const emptyFrameCalls =
    rustIntegration.match(/runtime\.bridge\.beginEmptyFrame\(host\.skyColor as Float32Array\);/g) ?? [];
if (emptyFrameCalls.length < 2) {
    throw new Error(
        "Rust-primary empty/streaming frames must clear deterministically for zero-visible and zero-resident cases",
    );
}

assertIncludes(
    frame,
    "(host.osrsClient.widgetManager?.rootInterface ?? -1) === WELCOME_SCREEN_GROUP_ID",
    "Welcome Screen backdrop behavior must remain explicit",
);

console.log("Rust-primary Pico scene ownership contract is stable");
