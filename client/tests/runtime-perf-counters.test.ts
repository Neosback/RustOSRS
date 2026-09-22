import assert from "node:assert/strict";

import { runtimePerfCounters } from "../common/debug/RuntimePerfCounters";

runtimePerfCounters.reset();
runtimePerfCounters.setWorkerPolicy(4, true);
runtimePerfCounters.setCacheAdvertisedBytes(256 * 1024 * 1024);
runtimePerfCounters.recordJs5GroupRequest(true, false);
runtimePerfCounters.recordJs5GroupRequest(false, true);
runtimePerfCounters.recordJs5HttpBatch(4096);
runtimePerfCounters.recordJs5ChainFetch();
runtimePerfCounters.recordRustUpload("dynamicNpc", 1200);
runtimePerfCounters.recordRustUpload("residentPlayer", 2400);
runtimePerfCounters.recordResidentPlayerMiss();
runtimePerfCounters.recordResidentPlayerHit();
runtimePerfCounters.recordResidentPlayerEviction();
runtimePerfCounters.setResidentPlayerEntries(12);

const snapshot = runtimePerfCounters.snapshot();

assert.equal(snapshot.workerCount, 4);
assert.equal(snapshot.sharedSparseCache, true);
assert.equal(snapshot.cacheAdvertisedBytes, 256 * 1024 * 1024);
assert.equal(snapshot.js5GroupRequests, 2);
assert.equal(snapshot.js5StoreHits, 1);
assert.equal(snapshot.js5PendingDedupHits, 1);
assert.equal(snapshot.js5HttpBatches, 1);
assert.equal(snapshot.js5HttpBytes, 4096);
assert.equal(snapshot.js5ChainFetches, 1);
assert.equal(snapshot.rustUploadCalls.dynamicNpc, 1);
assert.equal(snapshot.rustUploadBytes.dynamicNpc, 1200);
assert.equal(snapshot.rustUploadCalls.residentPlayer, 1);
assert.equal(snapshot.rustUploadBytes.residentPlayer, 2400);
assert.equal(snapshot.residentPlayerMisses, 1);
assert.equal(snapshot.residentPlayerHits, 1);
assert.equal(snapshot.residentPlayerEvictions, 1);
assert.equal(snapshot.residentPlayerEntries, 12);

console.log("Runtime performance counters regression test passed");
