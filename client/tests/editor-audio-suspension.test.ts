import assert from "node:assert/strict";
import { registerManagedAudioContext, unregisterManagedAudioContext, setAudioSuspended, resumeAudioContextIfNeeded } from "../game/audio/audioContext";

function context() {
    return {
        state: "running",
        suspend() { this.state = "suspended"; return Promise.resolve(); },
        resume() { this.state = "running"; return Promise.resolve(); },
    };
}
const existing = context();
const late = context();
registerManagedAudioContext(existing as AudioContext);
setAudioSuspended(true);
registerManagedAudioContext(late as AudioContext);
for (const ctx of [existing, late]) {
    resumeAudioContextIfNeeded(ctx as AudioContext);
    assert.equal(ctx.state, "suspended", "editor gestures must not resume existing or newly created audio");
}
setAudioSuspended(false);
for (const ctx of [existing, late]) {
    assert.equal(ctx.state, "running", "leaving the editor restores audio");
    unregisterManagedAudioContext(ctx as AudioContext);
}
