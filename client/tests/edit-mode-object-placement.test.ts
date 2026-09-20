import assert from "node:assert/strict";

import { EditModePlugin } from "../game/plugins/editmode/EditModePlugin";
import { LocPlacementPreviewOverlay } from "../game/plugins/editmode/LocPlacementPreviewOverlay";

const plugin = new EditModePlugin();
plugin.setConfig({ shape: 9, rotation: 3 });
(plugin as any).search = { kind: "loc", query: "", loading: false, results: [] };
plugin.useSearchResult(100);

assert.equal(plugin.getConfig().shape, 10, "fresh cache objects use the normal loc shape");
assert.equal(plugin.getConfig().rotation, 0, "fresh cache objects start unrotated");

const reopened = new EditModePlugin({
    load: () => ({ heightLevel: 1 }),
    save: () => {},
});
assert.equal(reopened.getConfig().heightLevel, 0, "reopened editors start on ground level");

const preview = new LocPlacementPreviewOverlay() as any;
preview.previewVertShader = "ready";
preview.mainFragShader = "ready";
preview.key = "1:10:0";
preview.opaque = { vertexBuffer: {}, indexBuffer: {}, array: {}, program: {}, drawCall: {} };
preview.setPreviews({} as any, {} as any, [{ locId: 2, x: 0, y: 0, plane: 0, shape: 10, rotation: 0 }]);
assert.equal(preview.opaque, undefined, "switching objects never leaves the previous model visible");
assert.equal(preview.key, "2:10:0");

let modelArgs: number[] = [];
const diagonalPreview = new LocPlacementPreviewOverlay() as any;
Object.assign(diagonalPreview, {
    app: {},
    sceneUniforms: {},
    waterMask: {},
    previewVertShader: "ready",
    mainFragShader: "ready",
    previews: [{ locId: 2, x: 0, y: 0, plane: 0, shape: 11, rotation: 3 }],
});
diagonalPreview.rebuild(
    {
        textureLoader: {},
        locTypeLoader: { load: () => ({ transforms: undefined }) },
        varManager: {},
    },
    {
        textureArray: {},
        textureMaterials: {},
        waterTextures: {},
        getInteractLocModelLoader: () => ({
            getModelAnimated: (_type: unknown, shape: number, rotation: number) => {
                modelArgs = [shape, rotation];
                return undefined;
            },
        }),
    },
);
assert.deepEqual(modelArgs, [10, 7], "diagonal duplicates use the cache's normal model variant");

let cleared = 0;
const selectionPlugin = new EditModePlugin();
selectionPlugin.attach({
    getPointerTile: () => undefined,
    clearPlacementPreview: () => cleared++,
} as any);
cleared = 0;
selectionPlugin.setConfig({ tool: "place" });
selectionPlugin.setConfig({ tool: "select" });
assert.ok(cleared > 0, "returning to selection always clears renderer placement state");
console.log("Edit Mode object placement defaults passed");

// The GE booth has wall models, but no normal (shape 10) model.
const { CacheSystem } = require("../rs/cache/CacheSystem");
const { getCacheLoaderFactory } = require("../rs/cache/loader/CacheLoaderFactory");
const { loadCache, loadCacheInfos, loadCacheList } = require("../scripts/cache/load-util");
const { LocModelLoader } = require("../rs/config/loctype/LocModelLoader");
const { getLocPlacementShape } = require("../game/plugins/editmode/LocPlacementPreviewOverlay");
const info = loadCacheList(loadCacheInfos()).latest;
const factory = getCacheLoaderFactory(info, CacheSystem.fromFiles(info, loadCache(info).files));
const booth = factory.getLocTypeLoader().load(10060);
const modelLoader = new LocModelLoader(factory.getLocTypeLoader(), factory.getModelLoader(), factory.getTextureLoader(), factory.getSeqTypeLoader(), factory.getSeqFrameLoader(), factory.getSkeletalSeqLoader());
assert.equal(modelLoader.getModelAnimated(booth, 10, 0, -1, -1), undefined);
const boothShape = getLocPlacementShape(booth);
assert.equal(boothShape, 0);
for (const rotation of [0, 1, 2, 3]) {
    assert(modelLoader.getModelAnimated(booth, boothShape, rotation, -1, -1), "booth preview/placement model exists in each orientation");
}
assert.equal(getLocPlacementShape({ types: [0, 10] }), 10, "normal objects retain their preferred shape");
assert.equal(getLocPlacementShape({ types: [22] }), 22, "floor decorations use their supported shape");
assert.equal(getLocPlacementShape({}), 10);
plugin.attach({ getPointerTile: () => undefined, getLocPlacementShape: () => boothShape } as any);
plugin.useSearchResult(10060);
assert.equal(plugin.getConfig().shape, 0, "preview and placement share the cache-supported shape");
console.log("Cache-backed GE booth placement regression passed");
