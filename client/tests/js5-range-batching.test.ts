import assert from "node:assert/strict";

import { Js5RangeClient } from "../rs/cache/js5/Js5RangeClient";
import { PresenceBitset } from "../rs/cache/js5/PresenceBitset";
import { SparseMemoryStore } from "../rs/cache/store/SparseMemoryStore";

function createStore(): SparseMemoryStore {
    const index = new ArrayBuffer(18);
    const idx = new DataView(index);
    for (const [id, sector] of [[0, 1], [1, 400], [2, 401]]) {
        idx.setUint8(id * 6 + 2, 1);
        idx.setUint8(id * 6 + 4, sector >> 8);
        idx.setUint8(id * 6 + 5, sector & 255);
    }
    return new SparseMemoryStore(
        new ArrayBuffer(600 * 520),
        [index],
        PresenceBitset.forSectorCount(600, false),
    );
}

function rangeResponse(store: SparseMemoryStore, options?: RequestInit): Response {
    const range = (options!.headers as Record<string, string>).Range;
    const [, start, end] = /bytes=(\d+)-(\d+)/.exec(range)!;
    return new Response(new Uint8Array(Number(end) - Number(start) + 1), {
        status: 206,
        headers: {
            "Content-Range": `bytes ${start}-${end}/${store.dataFile.byteLength}`,
        },
    });
}

async function testBatchingAndWarmBlock(): Promise<void> {
    const store = createStore();
    const originalFetch = globalThis.fetch;
    let requests = 0;
    globalThis.fetch = async (_url, options) => {
        requests++;
        return rangeResponse(store, options);
    };
    try {
        const client = new Js5RangeClient("https://example.test/cache", store);
        await Promise.all([client.requestGroup(0, 0), client.requestGroup(0, 1)]);
        assert.equal(requests, 1, "groups in one fetch block share a single HTTP request");
        assert.deepEqual(client.getProgress(), {
            pending: 0,
            active: 0,
            downloadedBytes: 512 * 520,
        });
        await client.requestGroup(0, 1);
        assert.equal(requests, 1, "the warmed block prevents a later request");
    } finally {
        globalThis.fetch = originalFetch;
    }
}

async function testTransientServerFailureRetries(): Promise<void> {
    const store = createStore();
    const originalFetch = globalThis.fetch;
    let requests = 0;
    globalThis.fetch = async (_url, options) => {
        requests++;
        if (requests === 1) {
            return new Response("temporary", { status: 503 });
        }
        return rangeResponse(store, options);
    };
    try {
        const client = new Js5RangeClient("https://example.test/cache", store, 1);
        await client.requestGroup(0, 0);
        assert.equal(requests, 2, "transient server failures retry the same queued group");
        assert.equal(client.getProgress().pending, 0);
    } finally {
        globalThis.fetch = originalFetch;
    }
}

async function testPermanentClientFailureDoesNotRetry(): Promise<void> {
    const store = createStore();
    const originalFetch = globalThis.fetch;
    let requests = 0;
    globalThis.fetch = async () => {
        requests++;
        return new Response("missing", { status: 404 });
    };
    try {
        const client = new Js5RangeClient("https://example.test/cache", store, 1);
        await assert.rejects(client.requestGroup(0, 0), /404/);
        assert.equal(requests, 1, "non-transient client failures are not retried");
        assert.equal(client.getProgress().pending, 0);
    } finally {
        globalThis.fetch = originalFetch;
    }
}

async function main(): Promise<void> {
    await testBatchingAndWarmBlock();
    await testTransientServerFailureRetries();
    await testPermanentClientFailureDoesNotRetry();
    console.log("JS5 range batching tests passed");
}

void main();
