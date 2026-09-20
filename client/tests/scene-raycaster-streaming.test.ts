import assert from "node:assert/strict";

// PicoGL expects a browser global when SceneRaycaster imports WebGLMapSquare.
(globalThis as any).self = globalThis;
const { SceneRaycaster } = require("../game/scene/SceneRaycaster");

const modelLoader = { missCount: 0 };
const raycaster = new SceneRaycaster({}, { modelLoader });
let attempts = 0;
raycaster.getInteractLocModelLoader = () => ({
    getModelAnimated: () => {
        if (++attempts === 1) {
            return undefined;
        }
        return {
            verticesCount: 3,
            verticesX: new Int32Array([0, 128, 0]),
            verticesY: new Int32Array([0, 0, -128]),
            verticesZ: new Int32Array(3),
            faceCount: 1,
            indices1: new Int32Array([0]),
            indices2: new Int32Array([1]),
            indices3: new Int32Array([2]),
        };
    },
});

assert.equal(raycaster.getLocModelMesh({ id: 405 }, 10, 1), undefined, "do not cache a pending model as absent");
const mesh = raycaster.getLocModelMesh({ id: 405 }, 10, 1);
assert.equal(mesh?.faceCount, 1, "retry the object mesh after its model downloads");
assert.equal(raycaster.getLocModelMesh({ id: 405 }, 10, 1), mesh);
assert.equal(attempts, 2, "reuse successfully loaded geometry");

let typeAttempts = 0;
raycaster.osrsClient.locTypeLoader = {
    load: () => {
        if (++typeAttempts === 1) throw new Error("pending");
        return { id: 405 };
    },
};
assert.equal(raycaster.getResolvedLocType(405), undefined, "do not cache a pending definition as absent");
assert.equal(raycaster.getResolvedLocType(405)?.id, 405);
console.log("Scene raycaster streaming regression passed");

// Perdu's spawn overlaps the chair at 3090, 3494. An absent NPC model must
// leave the chair selectable, while a visible NPC still receives normal hits.
let npcType: any = { name: "null" };
raycaster.osrsClient.npcTypeLoader = { load: () => npcType };
raycaster.osrsClient.npcEcs = {
    queryByMap: () => [1], isActive: () => true, isLinked: () => true,
    getMapId: () => 12342, getWorldX: () => 3090 * 128,
    getWorldY: () => 3494 * 128, getSize: () => 1,
    getNpcTypeId: () => 7458, getLevel: () => 0, getServerId: () => 60001,
};
raycaster.sampleHeightAt = () => 0;
const pickNpc = () => {
    const hits: any[] = [];
    raycaster.collectNpcHitsForMap(
        { id: 12342, mapX: 48, mapY: 54 },
        { origin: [3090, -1, 3490], direction: [0, 0, 1] },
        10, hits, 0,
    );
    return hits;
};
assert.equal(pickNpc().length, 0, "model-less NPC does not intercept the chair");
npcType = { modelIds: [123] };
assert.equal(pickNpc()[0]?.interactId, 7458, "visible NPC remains selectable");
npcType = { transforms: [-1], transform: () => undefined };
assert.equal(pickNpc().length, 0, "hidden NPC transform does not intercept scenery");
npcType = { transforms: [1], transform: () => ({ modelIds: [123] }) };
assert.equal(pickNpc().length, 1, "visible transformed NPC remains selectable");
console.log("Scene raycaster invisible NPC regression passed");
