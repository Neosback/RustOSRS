import { strict as assert } from "node:assert";

import {
    VENGEANCE_COOLDOWN_MS,
    VengeanceTimerPlugin,
} from "../game/plugins/vengeancetimer/VengeanceTimerPlugin";
import type { VengeanceTimerPluginConfig } from "../game/plugins/vengeancetimer/types";

let saved: VengeanceTimerPluginConfig | undefined;
const plugin = new VengeanceTimerPlugin({
    load: () => undefined,
    save: (config) => {
        saved = config;
    },
});

assert.equal(plugin.getState().config.enabled, true, "the timer should be enabled by default");

const startedAt = 1_000;
plugin.syncCooldownVarbit(1, startedAt);
assert.equal(plugin.getRemainingSeconds(startedAt), VENGEANCE_COOLDOWN_MS / 1000);
assert.equal(plugin.getRemainingSeconds(startedAt + 15_000), 15);

plugin.syncCooldownVarbit(1, startedAt + 15_000);
assert.equal(plugin.getRemainingSeconds(startedAt + 15_000), 15, "polling must not restart the timer");

plugin.setConfig({ enabled: false });
assert.deepEqual(saved, { enabled: false });
assert.equal(plugin.getRemainingSeconds(startedAt + 20_000), 10, "disabling only hides the timer");

plugin.syncCooldownVarbit(0, startedAt + 30_000);
assert.equal(plugin.getState().cooldownEndsAt, null);
assert.equal(plugin.getRemainingSeconds(startedAt + 30_000), 0);

console.log("vengeance timer plugin smoke test passed");
