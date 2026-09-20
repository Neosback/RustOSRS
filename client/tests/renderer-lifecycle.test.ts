import assert from "node:assert/strict";

let scheduled = 0;
(globalThis as any).document = {
    createElement: () => ({ style: {} }),
    addEventListener() {},
    removeEventListener() {},
    hidden: false,
};
(globalThis as any).requestAnimationFrame = () => ++scheduled;
(globalThis as any).cancelAnimationFrame = () => {};

async function main(): Promise<void> {
    const { Renderer } = await import("../game/render/Renderer");

    class TestRenderer extends Renderer {
        initCalls = 0;

        async init() {
            this.initCalls++;
        }
        cleanUp() {}
        render() {}
    }

    const renderer = new TestRenderer();
    await Promise.all([renderer.initOnce(), renderer.initOnce()]);
    assert.equal(renderer.initCalls, 1, "init must run only once");
    renderer.start();
    renderer.start();
    assert.equal(scheduled, 1, "start must create only one render loop");
    renderer.stop();
    console.log("Renderer lifecycle regression test passed");
}

void main();
