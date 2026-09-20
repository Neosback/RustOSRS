import assert from "node:assert/strict";

import { PendingInterfaceUpdates } from "../widgets/custom/PendingInterfaceUpdates";

const uid = (groupId: number, component: number) => (groupId << 16) | component;
const pending = new PendingInterfaceUpdates();

// Nothing is being fetched: everything applies immediately.
assert.equal(pending.defer({ action: "set_text", uid: uid(30003, 5) }), false);

// While a definition is loading, that group's updates are held - and only that group's.
pending.open(30003);
assert.equal(pending.defer({ action: "set_text", uid: uid(30003, 5), text: "Attack: 99" }), true);
assert.equal(pending.defer({ action: "set_item", uid: uid(30003, 100), itemId: 4151 }), true);
assert.equal(pending.defer({ action: "set_text", uid: uid(161, 16) }), false, "other groups pass");
assert.equal(pending.defer({ action: "open_sub", groupId: 30003 }), false, "only uid updates");

// Flushing replays them in arrival order, once.
const applied: any[] = [];
pending.flush(30003, (payload) => applied.push(payload));
assert.deepEqual(
    applied.map((payload) => payload.uid),
    [uid(30003, 5), uid(30003, 100)],
);
assert.equal(pending.size, 0);
assert.equal(pending.defer({ action: "set_text", uid: uid(30003, 5) }), false, "no longer held");

const afterFlush: any[] = [];
pending.flush(30003, (payload) => afterFlush.push(payload));
assert.deepEqual(afterFlush, [], "a second flush replays nothing");

// A group that never mounts drops what it held.
pending.open(30004);
assert.equal(pending.defer({ action: "set_text", uid: uid(30004, 1) }), true);
pending.cancel(30004);
assert.equal(pending.size, 0);
const afterCancel: any[] = [];
pending.flush(30004, (payload) => afterCancel.push(payload));
assert.deepEqual(afterCancel, []);

console.log("pending-interface-updates.test.ts: all tests passed");
