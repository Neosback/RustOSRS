import assert from "node:assert/strict";

import {
    getClientCycle,
    getClientCycleFloat,
    setClientCycleProvider,
} from "../network/ServerConnection";

setClientCycleProvider(() => 42.75);
assert.equal(getClientCycle(), 42, "game logic must keep integer client-cycle semantics");
assert.equal(getClientCycleFloat(), 42.75, "projectile rendering must retain sub-cycle time");
setClientCycleProvider(undefined);

console.log("Projectile timing regression test passed");
