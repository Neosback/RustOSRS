import assert from "node:assert/strict";
import { JSDOM } from "jsdom";
import { EditModePlugin } from "../game/plugins/editmode/EditModePlugin";
import { InputManager } from "../game/InputManager";
const dom = new JSDOM('<input><button>Place object</button><canvas tabindex="0"></canvas>', { pretendToBeVisual: true });
Object.assign(globalThis, { window: dom.window, document: dom.window.document, HTMLElement: dom.window.HTMLElement });
const canvas = document.querySelector("canvas")!;
const input = new InputManager();
input.init(canvas);
const plugin = new EditModePlugin();
plugin.attach({ getCanvas: () => canvas, getPointerTile: () => undefined, isLoggedIn: () => true, setFreeCamera: () => {}, cancelPendingClick: () => {} } as any);
for (let attempt = 0; attempt < 2; attempt++) {
    document.querySelector("button")!.focus();
    plugin.setConfig({ enabled: true, active: true, tool: "place", locId: 10060 });
    assert.equal(document.activeElement, canvas, "arming and rearming placement focuses the canvas");
    for (const [key, code, keyCode] of [["w", "KeyW", 87], ["a", "KeyA", 65], ["s", "KeyS", 83], ["d", "KeyD", 68], ["ArrowUp", "ArrowUp", 38], ["ArrowDown", "ArrowDown", 40], ["ArrowLeft", "ArrowLeft", 37], ["ArrowRight", "ArrowRight", 39]] as const) {
        document.activeElement!.dispatchEvent(new dom.window.KeyboardEvent("keydown", { key, code, keyCode, bubbles: true }));
        assert.equal(input.isKeyDown(code), true, `${code} reaches camera input during placement`);
        document.activeElement!.dispatchEvent(new dom.window.KeyboardEvent("keyup", { key, code, keyCode, bubbles: true }));
        assert.equal(input.isKeyDown(code), false);
    }
}
const search = document.querySelector("input")!;
search.focus();
plugin.setConfig({ locId: 590 });
assert.equal(document.activeElement, search, "ordinary config updates preserve typing focus");
plugin.setConfig({ active: false });
input.cleanUp();
dom.window.close();
console.log("Editor placement keyboard tests passed");
