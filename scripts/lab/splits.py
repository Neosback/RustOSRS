"""Source-authoritative split: geography v1, FNV-1a authored families. Split before augmentation."""
def group_for(prefab):
    source = prefab.get('source', {})
    if source.get('from') == 'captured':
        return f"geography:{source['tile_x'] // 512}:{source['tile_y'] // 512}"
    labels = (prefab.get('building') or {}).get('labels', {})
    return labels.get('human:family') or 'family:' + prefab.get('name', '').split(' copy')[0]


def split_for(group):
    parts = group.split(':')
    if len(parts) == 3 and parts[0] == 'geography':
        slot = (int(parts[1]) * 31 + int(parts[2])) % 5
    else:
        value = 0xcbf29ce484222325
        for byte in group.encode():
            value = ((value ^ byte) * 0x100000001b3) & ((1 << 64) - 1)
        slot = value % 5
    return 'test' if slot == 0 else 'validation' if slot == 1 else 'train'
