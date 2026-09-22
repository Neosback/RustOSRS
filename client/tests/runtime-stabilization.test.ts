import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";

import { resolveControlledPlayerEcsIndex } from "../render/player/ControlledPlayer";
import { resolveRenderWorkerCount } from "../game/worker/RenderWorkerPolicy";

assert.equal(
    resolveControlledPlayerEcsIndex(
        { getIndexForServerId: (serverId) => (serverId === 0 ? 17 : undefined) },
        0,
    ),
    17,
    "controlled player server id 0 must remain valid",
);
assert.equal(
    resolveControlledPlayerEcsIndex(
        { getIndexForServerId: () => 17 },
        -1,
    ),
    undefined,
    "negative controlled player ids are unassigned",
);

assert.equal(
    resolveRenderWorkerCount({
        hardwareConcurrency: 8,
        mobile: false,
        sharedCacheMemoryAvailable: false,
    }),
    1,
    "non-isolated browsers must not clone sparse DAT2 into multiple workers",
);
assert.equal(
    resolveRenderWorkerCount({
        hardwareConcurrency: 8,
        mobile: false,
        sharedCacheMemoryAvailable: true,
    }),
    4,
);
assert.equal(
    resolveRenderWorkerCount({
        hardwareConcurrency: 8,
        mobile: true,
        sharedCacheMemoryAvailable: true,
    }),
    2,
);
assert.equal(
    resolveRenderWorkerCount({
        hardwareConcurrency: 2,
        mobile: false,
        sharedCacheMemoryAvailable: true,
    }),
    1,
);

const craco = fs.readFileSync(path.resolve(__dirname, "../craco.config.js"), "utf8");
assert.ok(
    craco.includes('"Cross-Origin-Opener-Policy": "same-origin"'),
    "development server must enable cross-origin isolation",
);
assert.ok(craco.includes('"Cross-Origin-Embedder-Policy": "require-corp"'));

const vercel = JSON.parse(
    fs.readFileSync(path.resolve(__dirname, "../vercel.json"), "utf8").replace(/^\uFEFF/, ""),
);
const headers = (vercel.headers ?? []).flatMap((entry: any) => entry.headers ?? []);
const headerMap = new Map(headers.map((entry: any) => [entry.key, entry.value]));
assert.equal(headerMap.get("Cross-Origin-Opener-Policy"), "same-origin");
assert.equal(headerMap.get("Cross-Origin-Embedder-Policy"), "require-corp");


const playerRendererSource = fs.readFileSync(
    path.resolve(__dirname, "../render/player/PlayerRenderer.ts"),
    "utf8",
);
assert.ok(
    !playerRendererSource.includes("this.isControlledPid(inst.pid) ? undefined : batchKey"),
    "controlled player should reuse the bounded animation geometry cache",
);


const tick2Source = fs.readFileSync(
    path.resolve(__dirname, "../render/render/tick2.ts"),
    "utf8",
);
assert.ok(
    !tick2Source.includes("controlledServerId > 0"),
    "controlled player server id 0 must remain valid in final player visibility checks",
);
assert.ok(
    tick2Source.includes("resolveInteractionPlaneForWorldTile"),
    "player/NPC tile arbitration must use bridge-aware interaction planes",
);


const overlaysSource = fs.readFileSync(
    path.resolve(__dirname, "../render/render/overlays.ts"),
    "utf8",
);
assert.ok(
    overlaysSource.includes("if (actual >= 0)"),
    "controlled player server id 0 must remain valid for overlay ownership",
);
assert.ok(
    overlaysSource.includes("return -1;"),
    "overlay ownership must use a negative id for the unassigned sentinel",
);

const overlays2Source = fs.readFileSync(
    path.resolve(__dirname, "../render/render/overlays2.ts"),
    "utf8",
);
assert.ok(
    !overlays2Source.includes("controlledId > 0"),
    "controlled player server id 0 must survive hitsplat cleanup",
);

const overlays3Source = fs.readFileSync(
    path.resolve(__dirname, "../render/render/overlays3.ts"),
    "utf8",
);
assert.ok(
    !overlays3Source.includes("controlledId > 0"),
    "controlled player server id 0 must survive health-bar cleanup",
);

const overlays4Source = fs.readFileSync(
    path.resolve(__dirname, "../render/render/overlays4.ts"),
    "utf8",
);
assert.ok(
    !overlays4Source.includes("targetId > 0"),
    "player overlay events must accept server id 0",
);
assert.ok(
    !overlays4Source.includes("controlledId <= 0"),
    "server id 0 must not be treated as an unassigned controlled player",
);

const frameRenderSource = fs.readFileSync(
    path.resolve(__dirname, "../render/render/frame/render.ts"),
    "utf8",
);
assert.ok(
    !frameRenderSource.includes("playerServerId > 0"),
    "server id 0 controlled-player overlays must render",
);


assert.ok(
    !frameRenderSource.includes("if (serverId > 0 && healthBars.length < healthBarMaxEntries)"),
    "player server id 0 must be eligible for health-bar rendering",
);
assert.ok(
    !frameRenderSource.includes("serverId === undefined || (serverId | 0) <= 0"),
    "player server id 0 must be retained in cached server-tile data",
);
assert.ok(
    !overlays3Source.includes("const serverId = event.serverId | 0;\n        if (serverId <= 0) return;"),
    "player health-bar updates must accept server id 0",
);

console.log("Runtime stabilization regression test passed");
