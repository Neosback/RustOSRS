import fs from "fs";
import path from "path";

import { CacheList, LoadedCache, XteaMap } from "../../game/Caches";
import { CacheFiles } from "../../rs/cache/CacheFiles";
import { CacheInfo, getLatestCache } from "../../rs/cache/CacheInfo";
import { detectCacheType } from "../../rs/cache/CacheType";

const SERVER_CACHES = path.resolve(__dirname, "../../../server/caches");

export function loadCacheInfos(): CacheInfo[] {
    const json = fs.readFileSync(path.join(SERVER_CACHES, "caches.json"), "utf8");
    return JSON.parse(json);
}

export function loadCacheList(caches: CacheInfo[]): CacheList {
    const latest = getLatestCache(caches);
    if (!latest) {
        throw new Error("No latest cache");
    }
    return {
        caches,
        latest,
    };
}

export function loadCacheFiles(cache: CacheInfo): CacheFiles {
    const cachePath = path.join(SERVER_CACHES, cache.name) + path.sep;

    const files = new Map<string, ArrayBuffer>();

    fs.readdirSync(cachePath).forEach((fileName: string) => {
        const buffer = fs.readFileSync(cachePath + fileName);
        const arrayBuffer = buffer.buffer.slice(
            buffer.byteOffset,
            buffer.byteOffset + buffer.byteLength,
        );
        files.set(fileName, arrayBuffer);
    });

    return new CacheFiles(files);
}

export function loadCache(info: CacheInfo): LoadedCache {
    const files = loadCacheFiles(info);
    const xteas = loadXteas(info);
    return {
        info,
        type: detectCacheType(info),
        files,
        xteas,
    };
}

export function loadXteas(cache: CacheInfo): XteaMap {
    const cachePath = path.join(SERVER_CACHES, cache.name) + path.sep;
    const json = fs.readFileSync(cachePath + "keys.json", "utf8");
    const data: Record<string, number[]> = JSON.parse(json);
    return new Map(Object.keys(data).map((key) => [parseInt(key), data[key]]));
}
