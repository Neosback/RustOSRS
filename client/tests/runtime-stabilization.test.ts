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

console.log("Runtime stabilization regression test passed");
