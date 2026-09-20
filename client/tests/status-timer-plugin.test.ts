import { strict as assert } from "node:assert";

import { StatusTimerPlugin } from "../game/plugins/statustimer/StatusTimerPlugin";

const plugin = new StatusTimerPlugin();
plugin.sync(5, 319, 1_000);
assert.equal(plugin.getRemainingSeconds(1_000), 5);
plugin.sync(4, 319, 2_000);
assert.equal(plugin.getRemainingSeconds(2_000), 4, "server countdown updates must not restart the timer");
plugin.sync(-1, 2, 3_000);
assert.equal(plugin.getState().endsAt, null, "venom does not expire on its own");
plugin.sync(0, 0, 4_000);
assert.equal(plugin.getState().active, false);
console.log("status timer plugin smoke test passed");
