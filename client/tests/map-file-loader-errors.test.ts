import assert from "node:assert/strict";

import { GroupMissingError } from "../rs/cache/js5/GroupMissingError";
import { MapFileLoader } from "../rs/map/MapFileLoader";

function loaderWithError(error: Error): MapFileLoader {
    const mapIndex = {
        getFile(): never {
            throw error;
        },
    };
    const mapFileIndex = {
        getTerrainArchiveId(): number {
            return 100;
        },
        getTerrainFileId(): number {
            return 0;
        },
        getLocArchiveId(): number {
            return 101;
        },
        getLocFileId(): number {
            return 0;
        },
    };
    return new MapFileLoader(mapIndex as any, mapFileIndex as any);
}

const sparseMiss = loaderWithError(new GroupMissingError(5, 100, 0, 128));
assert.equal(
    sparseMiss.getTerrainData(50, 50),
    undefined,
    "sparse terrain miss should remain a temporary empty result",
);
assert.equal(
    sparseMiss.getLocData(50, 50, new Map()),
    undefined,
    "sparse loc miss should remain a temporary empty result",
);

const decodeFailure = loaderWithError(new Error("corrupt archive"));
assert.throws(
    () => decodeFailure.getTerrainData(50, 50),
    /corrupt archive/,
    "real terrain decode errors must not be hidden as sparse misses",
);
assert.throws(
    () => decodeFailure.getLocData(50, 50, new Map()),
    /corrupt archive/,
    "real loc decode errors must not be hidden as sparse misses",
);

console.log("Map file loader error regression test passed");
