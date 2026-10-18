"""Observed symbolic adjacency from reviewed sparse prefabs; y increases north."""
from collections import Counter
import json
from pathlib import Path


def learn(prefabs, style, role):
    counts, pairs, sources = Counter(), set(), set()
    for prefab in prefabs:
        b = prefab.get('building') or {}
        if not b.get('reviewed'):
            raise ValueError('Review all source prefab memberships before learning')
        sources.add(b['provenance'])
        tiles = {(t['x'], t['y'], t['plane']): t for t in prefab['tiles']}
        floors = {(f['x'], f['y'], f['plane']) for f in b['floors']}
        sockets = {cell: [0, 0, 0, 0] for cell in floors}
        # Client orientation W/N/E/S -> catalog N/E/S/W, including opposite-anchor edges.
        for i, obj in enumerate(prefab['objects']):
            if obj['kind'] not in (0, 2):
                continue
            label = b.get('labels', {}).get(f'inferred:member:{i}:role')
            feature = 2 if label == 'Door' else 1
            arms = [obj['orientation']]
            if obj['kind'] == 2:
                arms.append((obj['orientation'] + 1) % 4)
            for arm in arms:
                dx, dy, direction = [(-1, 0, 3), (0, 1, 0), (1, 0, 1), (0, -1, 2)][arm]
                cell = (obj['x'], obj['y'], obj['plane'])
                neighbor = (obj['x'] + dx, obj['y'] + dy, obj['plane'])
                if cell in sockets:
                    sockets[cell][direction] = feature
                if neighbor in sockets:
                    sockets[neighbor][(direction + 2) % 4] = feature
        patterns = {}
        for cell in sorted(floors):
            t = tiles.get(cell, {})
            overlay = t.get('overlay') or {}
            floor_class = ((t.get('underlay') or 0) << 16) | overlay.get('id', 0)
            if floor_class > 0xffffffff:
                raise ValueError('Floor identifiers exceed symbolic catalog range')
            patterns[cell] = (floor_class, tuple(sockets[cell]))
            counts[patterns[cell]] += 1
        for (x, y, p), pattern in patterns.items():
            for direction, (dx, dy) in enumerate([(0, 1), (1, 0), (0, -1), (-1, 0)]):
                neighbor = patterns.get((x + dx, y + dy, p))
                if neighbor is not None:
                    pairs.add((pattern, direction, neighbor))
    ordered = sorted(counts)
    if not 0 < len(ordered) <= 128:
        raise ValueError(f'Catalog has {len(ordered)} patterns; supported range is 1..128')
    ids = {p: i for i, p in enumerate(ordered)}
    return {'format': 'rustosrs-learned-catalog', 'version': 1, 'style': style, 'role': role,
            'source': '; '.join(sorted(sources)),
            'patterns': [{'floor_class': p[0], 'sockets': p[1]} for p in ordered],
            'weights': [counts[p] for p in ordered],
            'adjacency': sorted((ids[a], d, ids[b]) for a, d, b in pairs)}


def main():
    import argparse
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--prefabs', required=True)
    p.add_argument('--style', required=True)
    p.add_argument('--role', required=True)
    p.add_argument('--out', required=True)
    args = p.parse_args()
    prefabs = [json.loads(f.read_text()) for f in sorted(Path(args.prefabs).glob('*.prefab.json'))]
    prefabs = [v for v in prefabs if (v.get('building') or {}).get('reviewed')]
    result = learn(prefabs, args.style, args.role)
    Path(args.out).write_text(json.dumps(result, sort_keys=True, indent=2) + '\n')
    print(f"{len(result['patterns'])} observed patterns; {len(result['adjacency'])} directed pairs")


if __name__ == '__main__':
    main()
