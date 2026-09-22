import assert from "node:assert/strict";

import { WorkerJs5Coordinator } from "../rs/cache/js5/Js5Coordinator";

type MessageHandler = ((event: MessageEvent<unknown>) => void) | null;

class FakeBroadcastChannel {
    static readonly channels = new Map<string, FakeBroadcastChannel[]>();
    onmessage: MessageHandler = null;

    constructor(readonly name: string) {
        const channels = FakeBroadcastChannel.channels.get(name) ?? [];
        channels.push(this);
        FakeBroadcastChannel.channels.set(name, channels);
    }

    postMessage(data: unknown): void {
        for (const channel of FakeBroadcastChannel.channels.get(this.name) ?? []) {
            if (channel !== this) channel.onmessage?.({ data } as MessageEvent<unknown>);
        }
    }

    close(): void {}
}

function createStore() {
    let present = false;
    return {
        store: {
            isGroupPresent: () => present,
            onMiss: undefined,
        } as any,
        setPresent(value: boolean): void {
            present = value;
        },
    };
}

async function testDeduplicatesWorkerRequests(): Promise<void> {
    FakeBroadcastChannel.channels.clear();
    const state = createStore();
    const main = new BroadcastChannel("test-js5-dedupe");
    let requests = 0;
    let lastRequest: any;
    main.onmessage = ({ data }: MessageEvent<any>) => {
        requests++;
        lastRequest = data;
    };

    const coordinator = new WorkerJs5Coordinator(
        state.store,
        "test-js5-dedupe",
        50,
        3,
    );
    try {
        const first = coordinator.requestGroup(7, 123);
        const second = coordinator.requestGroup(7, 123);
        assert.equal(first, second, "duplicate worker misses must share one promise");
        assert.equal(requests, 1, "duplicate worker misses must emit one initial request");

        state.setPresent(true);
        main.postMessage({
            type: "complete",
            indexId: lastRequest.indexId,
            archiveId: lastRequest.archiveId,
        });
        await Promise.all([first, second]);
    } finally {
        coordinator.close();
        main.close();
    }
}

async function testRetriesLostCoordinatorMessage(): Promise<void> {
    FakeBroadcastChannel.channels.clear();
    const state = createStore();
    const main = new BroadcastChannel("test-js5-retry");
    let requests = 0;
    main.onmessage = ({ data }: MessageEvent<any>) => {
        requests++;
        if (requests === 2) {
            state.setPresent(true);
            main.postMessage({
                type: "complete",
                indexId: data.indexId,
                archiveId: data.archiveId,
            });
        }
    };

    const coordinator = new WorkerJs5Coordinator(
        state.store,
        "test-js5-retry",
        5,
        3,
    );
    try {
        await coordinator.requestGroup(8, 456);
        assert.equal(requests, 2, "lost coordinator requests must be re-issued");
    } finally {
        coordinator.close();
        main.close();
    }
}

async function testPropagatesCentralFailure(): Promise<void> {
    FakeBroadcastChannel.channels.clear();
    const state = createStore();
    const main = new BroadcastChannel("test-js5-failure");
    main.onmessage = ({ data }: MessageEvent<any>) => {
        main.postMessage({
            type: "failed",
            indexId: data.indexId,
            archiveId: data.archiveId,
            error: "permanent failure",
        });
    };

    const coordinator = new WorkerJs5Coordinator(
        state.store,
        "test-js5-failure",
        5,
        3,
    );
    try {
        await assert.rejects(
            coordinator.requestGroup(9, 789),
            /permanent failure/,
        );
    } finally {
        coordinator.close();
        main.close();
    }
}

async function testTimesOutUnansweredRequests(): Promise<void> {
    FakeBroadcastChannel.channels.clear();
    const state = createStore();
    const main = new BroadcastChannel("test-js5-timeout");
    let requests = 0;
    main.onmessage = () => {
        requests++;
    };

    const coordinator = new WorkerJs5Coordinator(
        state.store,
        "test-js5-timeout",
        2,
        2,
    );
    try {
        await assert.rejects(
            coordinator.requestGroup(10, 999),
            /timed out.*after 2 attempts/,
        );
        assert.equal(requests, 2);
    } finally {
        coordinator.close();
        main.close();
    }
}

async function main(): Promise<void> {
    const realBroadcastChannel = globalThis.BroadcastChannel;
    (globalThis as { BroadcastChannel: typeof BroadcastChannel }).BroadcastChannel =
        FakeBroadcastChannel as unknown as typeof BroadcastChannel;
    try {
        await testDeduplicatesWorkerRequests();
        await testRetriesLostCoordinatorMessage();
        await testPropagatesCentralFailure();
        await testTimesOutUnansweredRequests();
    } finally {
        (globalThis as { BroadcastChannel: typeof BroadcastChannel }).BroadcastChannel =
            realBroadcastChannel;
        FakeBroadcastChannel.channels.clear();
    }
    console.log("js5 coordinator tests passed");
}

void main();
