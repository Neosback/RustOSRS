import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";

import { lruGet, lruSet } from "../common/utils/BoundedLru";

const cache = new Map<number, string>();
lruSet(cache, 1, "one", 2);
lruSet(cache, 2, "two", 2);
assert.equal(lruGet(cache, 1), "one");
lruSet(cache, 3, "three", 2);

assert.equal(cache.has(1), true, "recently touched entry must remain resident");
assert.equal(cache.has(2), false, "least-recently-used entry must be evicted");
assert.equal(cache.has(3), true);

lruSet(cache, 4, "four", 0);
assert.equal(cache.size, 0, "zero-sized LRU must retain nothing");

const mapLoaderSource = fs.readFileSync(
    path.resolve(__dirname, "../render/loader/SdMapDataLoader.ts"),
    "utf8",
);
assert.ok(
    mapLoaderSource.includes("shouldClearWorkerCacheAfterLoad(): boolean"),
    "map loader must explicitly own worker-cache retention policy",
);
assert.ok(
    /shouldClearWorkerCacheAfterLoad\(\): boolean \{\s*return false;\s*\}/m.test(
        mapLoaderSource,
    ),
    "normal map loads must retain bounded worker decode caches",
);

const workerSource = fs.readFileSync(
    path.resolve(__dirname, "../game/worker/RenderDataWorker.ts"),
    "utf8",
);
const npcLoadBlock = workerSource.slice(
    workerSource.indexOf("async loadNpcGeometry("),
    workerSource.indexOf("async loadTexture("),
);
assert.ok(
    !npcLoadBlock.includes("clearCache(workerState)"),
    "NPC geometry jobs must not flush bounded worker decode caches after every load",
);
assert.ok(
    workerSource.includes("async resetDataLoader"),
    "loader reset must remain an explicit decode-cache invalidation point",
);
assert.ok(
    workerSource.includes("clearCache(workerState);"),
    "worker must retain an explicit full cache invalidation path",
);

console.log("Worker bounded-cache regression test passed");
