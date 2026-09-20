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

async function main(): Promise<void> {
    const realBroadcastChannel = globalThis.BroadcastChannel;
    (globalThis as { BroadcastChannel: typeof BroadcastChannel }).BroadcastChannel =
        FakeBroadcastChannel as unknown as typeof BroadcastChannel;
    try {
        let present = false;
        const store = { isGroupPresent: () => present, onMiss: undefined } as any;
        const main = new BroadcastChannel("test-js5");
        main.onmessage = ({ data }: MessageEvent<any>) => {
            assert.equal(data.type, "request");
            present = true;
            main.postMessage({ type: "complete", indexId: data.indexId, archiveId: data.archiveId });
        };
        const coordinator = new WorkerJs5Coordinator(store, "test-js5");
        await coordinator.requestGroup(7, 123);
        assert.equal(present, true);
    } finally {
        (globalThis as { BroadcastChannel: typeof BroadcastChannel }).BroadcastChannel = realBroadcastChannel;
    }
    console.log("js5 coordinator tests passed");
}

void main();
