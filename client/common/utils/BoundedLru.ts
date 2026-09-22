export function lruGet<K, V>(map: Map<K, V>, key: K): V | undefined {
    if (!map.has(key)) return undefined;
    const value = map.get(key) as V;
    map.delete(key);
    map.set(key, value);
    return value;
}

export function lruSet<K, V>(
    map: Map<K, V>,
    key: K,
    value: V,
    maxEntries: number,
): void {
    const limit = Math.max(0, maxEntries | 0);
    if (limit === 0) {
        map.clear();
        return;
    }

    if (map.has(key)) {
        map.delete(key);
    }
    map.set(key, value);

    while (map.size > limit) {
        const oldest = map.keys().next();
        if (oldest.done) break;
        map.delete(oldest.value);
    }
}
