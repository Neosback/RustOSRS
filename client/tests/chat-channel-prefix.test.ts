import assert from "node:assert/strict";

import { decodeClientPacket } from "../../server/src/main/typescript/elvarg/net/protocol/ClientProtocol";
import { state } from "../network/serverConnection/state";
import { Opcodes } from "../rs/cs2/Opcodes";
import { registerChatOps } from "../rs/cs2/handlers/ChatOps";
import type { HandlerMap } from "../rs/cs2/handlers/HandlerTypes";

let sentPacket: Uint8Array | undefined;
(globalThis as any).WebSocket = { OPEN: 1 };
state.socket = {
    readyState: 1,
    send: (packet: Uint8Array) => {
        sentPacket = packet;
    },
} as any;

const handlers: HandlerMap = new Map();
registerChatOps(handlers);
const sendPublic = handlers.get(Opcodes.CHAT_SENDPUBLIC);
const sendClan = handlers.get(Opcodes.CHAT_SENDCLAN);
const sendPrivate = handlers.get(Opcodes.CHAT_SENDPRIVATE);
const setFilter = handlers.get(Opcodes.CHAT_SETFILTER);
assert.ok(sendPublic);
assert.ok(sendClan);
assert.ok(sendPrivate);
assert.ok(setFilter);

const clearedVarcs: Array<[number, string]> = [];
sendPublic(
    {
        stringStack: ["/hello channel"],
        stringStackSize: 1,
        intStack: Int32Array.from([2]),
        intStackSize: 1,
        varManager: {
            setVarcString: (id: number, value: string) => clearedVarcs.push([id, value]),
        },
    } as any,
    0,
);

assert.ok(sentPacket);
assert.deepEqual(decodeClientPacket(Buffer.from(sentPacket)), {
    type: "chat",
    text: "hello channel",
    messageType: "friends_chat",
});
assert.deepEqual(clearedVarcs, [[335, ""]]);

sentPacket = undefined;
sendPublic(
    {
        stringStack: ["hello public"],
        stringStackSize: 1,
        intStack: Int32Array.from([0]),
        intStackSize: 1,
        varManager: { setVarcString: () => {} },
    } as any,
    0,
);
assert.ok(sentPacket);
assert.deepEqual(decodeClientPacket(Buffer.from(sentPacket)), {
    type: "chat",
    text: "hello public",
    messageType: "public",
});

sentPacket = undefined;
sendClan(
    {
        stringStack: ["legacy channel must not use this opcode"],
        stringStackSize: 1,
        intStack: Int32Array.from([2, -1]),
        intStackSize: 2,
        varManager: { setVarcString: () => {} },
    } as any,
    0,
);
assert.equal(sentPacket, undefined);

sendPrivate(
    {
        stringStack: ["Alice", "Meet me in Lumbridge."],
        stringStackSize: 2,
    } as any,
    0,
);
assert.ok(sentPacket);
assert.deepEqual(decodeClientPacket(Buffer.from(sentPacket)), {
    type: "private_message",
    recipient: "Alice",
    text: "Meet me in Lumbridge.",
});

setFilter(
    {
        intStack: Int32Array.from([1, 2, 0]),
        intStackSize: 3,
    } as any,
    0,
);
assert.ok(sentPacket);
assert.deepEqual(decodeClientPacket(Buffer.from(sentPacket)), {
    type: "chat_filter",
    publicMode: 1,
    privateMode: 2,
    tradeMode: 0,
});

state.socket = null;
console.log("chat-channel-prefix.test.ts: all tests passed");
