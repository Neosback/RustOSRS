import assert from "node:assert/strict";

import { ServerPacketId } from "../common/packets/ServerPacketId";
import { decodeServerPacket } from "../network/packet/ServerBinaryDecoder";

const body = Uint8Array.from([
    0, 0, 3, 204, // clientscript 972
    0, // no script arguments
    0, 0, // no varps
    0, 0, // no varbits
    1, // one inventory
    2, 72, // inventory 584
    0, 50, // capacity 50
    0, 2, // two populated slots
    0, 0, 16, 56, 1, // slot 0: abyssal whip x1
    0, 1, 3, 228, 255, 0, 0, 1, 44, // slot 1: coins x300
]);
const packet = Uint8Array.from([
    ServerPacketId.WIDGET_RUN_SCRIPT,
    body.length >> 8,
    body.length & 0xff,
    ...body,
]);

const decoded = decodeServerPacket(packet);
assert.equal(decoded?.type, "widget");
assert.equal(decoded?.payload.action, "run_script");
assert.equal(decoded?.payload.scriptId, 972);
assert.deepEqual(decoded?.payload.inventories, {
    584: {
        capacity: 50,
        slots: [
            { slot: 0, itemId: 4151, quantity: 1 },
            { slot: 1, itemId: 995, quantity: 300 },
        ],
    },
});

console.log("widget script inventory test passed");
