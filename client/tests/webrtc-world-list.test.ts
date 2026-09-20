import assert from "node:assert/strict";

import { getBrowserHostWorldConfig, getCacheBaseUrl, getServerListUrl, getWebRtcRelayConfig } from "../config/clientEnv";
import { handleServerListClick } from "../game/login/renderer/input/mouseClick";
import { forumProfileUrl, relayWorldEntries, replaceRelayWorlds } from "../game/login/renderer/serverList";
import { setServerUrl } from "../network/serverConnection/outgoing/connectionInfo";
import { state as connectionState } from "../network/serverConnection/state";

assert.deepEqual(getWebRtcRelayConfig(), {
    signalUrl: "wss://worlds.rsps.app",
    iceServers: [{ urls: "stun:stun.rsps.app:3478" }],
});

const previousPublicUrl = process.env.PUBLIC_URL;
process.env.PUBLIC_URL = "/play";
assert.equal(getServerListUrl(), "/play/servers.json");
assert.equal(getCacheBaseUrl(), "/play/caches/");
if (previousPublicUrl === undefined) delete process.env.PUBLIC_URL;
else process.env.PUBLIC_URL = previousPublicUrl;

const previousSignalUrl = process.env.REACT_APP_WEBRTC_SIGNAL_URL;
process.env.REACT_APP_WEBRTC_SIGNAL_URL = "ws://127.0.0.1:8787";
(globalThis as any).window = { location: { search: "?browser-host-client=1&browser-host-world=browser-test" } };
assert.equal(getWebRtcRelayConfig()?.signalUrl, "wss://worlds.rsps.app");
assert.deepEqual(getBrowserHostWorldConfig(), {
    signalUrl: "wss://worlds.rsps.app",
    iceServers: [{ urls: "stun:stun.rsps.app:3478" }],
    worldId: "browser-test",
});
setServerUrl("ws://127.0.0.1:43594");
assert.deepEqual(connectionState.webRtcConfig, getBrowserHostWorldConfig());
delete (globalThis as any).window;
if (previousSignalUrl === undefined) delete process.env.REACT_APP_WEBRTC_SIGNAL_URL;
else process.env.REACT_APP_WEBRTC_SIGNAL_URL = previousSignalUrl;

const discovered = relayWorldEntries("ws://127.0.0.1:8787", [], {
    worlds: [
        { worldId: "toby", name: "TobyScape", ownerUsername: "toby", playerCount: 12 },
        { worldId: "alice" },
        { worldId: "invalid world", name: "Ignored" },
    ],
});
assert.deepEqual(discovered.map((world) => world.worldId), ["toby", "alice"]);
assert.equal(discovered[0].transport, "webrtc");
assert.equal(discovered[0].playerCount, 12);
assert.equal(discovered[0].name, "TobyScape");
assert.equal(discovered[0].ownerUsername, "toby");
assert.equal(forumProfileUrl("toby"), "https://rsps.app/public/u/toby");

const opened: string[] = [];
(globalThis as any).window = { open: (url: string) => opened.push(url) };
const clickHost = {
    probed: true,
    serverList: [{ ownerUsername: "toby" }],
    canvasWidth: 800,
    canvasHeight: 600,
    contentScale: 1,
    layoutConfig: { isTouch: false, minTouchTarget: 44 },
    fontPlain12: { measure: (text: string) => text.length * 6 },
} as any;
assert.equal(handleServerListClick(clickHost, {} as any, 365, 310), undefined);
assert.deepEqual(opened, ["https://rsps.app/public/u/toby"]);
assert.deepEqual(handleServerListClick(clickHost, {} as any, 400, 310), { type: "select_server", index: 0 });
delete (globalThis as any).window;

const configured = {
    ...discovered[0],
    name: "Toby's World",
    relayDiscovered: false,
};
const refreshed = replaceRelayWorlds([configured, { ...discovered[1], worldId: "stale" }], discovered);
assert.deepEqual(refreshed.map((world) => world.worldId), ["toby", "alice"]);
assert.equal(refreshed[0].name, "Toby's World");

console.log("WebRTC relay world discovery test passed");
