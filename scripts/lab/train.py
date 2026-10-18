#!/usr/bin/env python3
"""Small offline learners and tool-based reconstruction baselines. Python 3.11+, no ML runtime."""
import argparse
from collections import Counter, defaultdict
import hashlib
import json
import math
from pathlib import Path
import random
import re
import time
from client import LabClient


def write_json(path, value):
    Path(path).write_text(json.dumps(value, indent=2, sort_keys=True) + '\n')


def object_key(value):
    return json.dumps(value, sort_keys=True)


def _pack_rectangles(cells_by_key, max_span=16):
    """Greedy maximal rectangle decomposition per (plane, value_tuple) key."""
    rects = []
    for (plane, payload), coords in sorted(cells_by_key.items(), key=lambda kv: (kv[0][0], str(kv[0][1]))):
        remaining = set(coords)
        while remaining:
            x0, y0 = min(remaining, key=lambda c: (c[1], c[0]))
            w = 1
            while w < max_span and (x0 + w, y0) in remaining:
                w += 1
            h = 1
            while h < max_span and all((x0 + dx, y0 + h) in remaining for dx in range(w)):
                h += 1
            for dx in range(w):
                for dy in range(h):
                    remaining.remove((x0 + dx, y0 + dy))
            rects.append((plane, x0, y0, w, h, payload))
    rects.sort(key=lambda r: (r[0], r[1], r[2]))
    return rects


def imitate(client, observation):
    """Visible-target curriculum only: emit ordinary tool gestures, never raw patches."""
    current, target = observation['current'], observation['target']
    have = {object_key(o) for o in current['objects']}
    want = {object_key(o) for o in target['objects']}
    for item in current['objects']:
        if object_key(item) not in want:
            yield {'tool': 'remove', 'object': item}

    old = {(t['x'], t['y'], t['plane']): t for t in current['tiles']}
    new = {(t['x'], t['y'], t['plane']): t for t in target['tiles']}
    tile_groups = defaultdict(set)
    for x, y, plane in sorted(old.keys() | new.keys()):
        tile = new.get((x, y, plane), {'underlay': None, 'overlay': None, 'flags': 0})
        if old.get((x, y, plane)) == tile:
            continue
        overlay = tile['overlay']
        payload = (
            overlay['id'] if overlay else None,
            tile['underlay'],
            overlay['shape'] if overlay else 0,
            overlay['rotation'] if overlay else 0,
            tile['flags'],
        )
        tile_groups[(plane, payload)].add((x, y))
    for plane, x, y, w, h, (ov_id, un_id, shape, rot, flags) in _pack_rectangles(tile_groups):
        yield {'tool': 'paint_rect', 'x': x, 'y': y, 'width': w, 'height': h,
               'plane': plane, 'overlay': ov_id, 'underlay': un_id,
               'shape': shape, 'rotation': rot, 'flags': flags}

    old_heights = {(h['x'], h['y'], h['plane']): h['value'] for h in current['heights']}
    height_groups = defaultdict(set)
    for height in target['heights']:
        coord = (height['x'], height['y'], height['plane'])
        if old_heights.get(coord) != height['value']:
            height_groups[(height['plane'], height['value'])].add((height['x'], height['y']))
    for plane, x, y, w, h, value in _pack_rectangles(height_groups):
        yield {'tool': 'height_stroke', 'x': x, 'y': y, 'width': w, 'height': h,
               'plane': plane, 'value': value}

    todo = [
        item for item in sorted(
            target['objects'],
            key=lambda o: (o['kind'], o['plane'], o['y'], o['x'], o['id']),
        )
        if object_key(item) not in have
    ]
    used_indices = set()
    # Emit collinear straight wall runs (kind == 0, length >= 3 along the wall's own axis) as WallLine.
    by_wall_pos = {
        (o['plane'], o['x'], o['y'], o['id'], o['orientation']): idx
        for idx, o in enumerate(todo)
        if o['kind'] == 0
    }
    for idx, o in enumerate(todo):
        if idx in used_indices or o['kind'] != 0:
            continue
        plane, x0, y0, wid, orient = o['plane'], o['x'], o['y'], o['id'], o['orientation']
        step = (1, 0) if orient in (1, 3) else (0, 1)
        run = [idx]
        cx, cy = x0 + step[0], y0 + step[1]
        while (plane, cx, cy, wid, orient) in by_wall_pos:
            nxt = by_wall_pos[(plane, cx, cy, wid, orient)]
            if nxt in used_indices:
                break
            run.append(nxt)
            cx += step[0]
            cy += step[1]
        if len(run) >= 3:
            used_indices.update(run)
            yield {
                'tool': 'wall_line',
                'object': o,
                'length': len(run),
                'vertical': orient in (0, 2),
            }

    for idx, item in enumerate(todo):
        if idx not in used_indices:
            yield {'tool': 'place', 'object': item}


def _propose_search_candidate(rng, legal, stage):
    w, h, planes = legal['width'], legal['height'], legal['planes']
    overlays = legal.get('overlays', [])
    underlays = legal.get('underlays', [])
    palette = legal.get('palette', [])

    if stage == 'terrain' and (overlays or underlays):
        if overlays and rng.random() < 0.45:
            pts = [
                [round(rng.uniform(1.5, max(2.0, w - 1.5)), 1),
                 round(rng.uniform(1.5, max(2.0, h - 1.5)), 1)]
                for _ in range(rng.choice([2, 3]))
            ]
            return {
                'tool': 'path_lay',
                'plane': 0,
                'points': pts,
                'width': float(rng.choice([2.0, 3.0, 4.0])),
                'edge_weight': 1.5,
                'smooth_curve': len(pts) >= 3,
                'overlay_id': rng.choice(overlays),
                'companion_underlay': rng.choice(underlays) if underlays and rng.random() < 0.7 else None,
                'smooth_heights': {'passes': 2, 'blend': 1.5} if rng.random() < 0.5 else None,
            }
        rw = rng.randint(1, min(8, w))
        rh = rng.randint(1, min(8, h))
        return {
            'tool': 'paint_rect',
            'x': rng.randrange(max(1, w - rw + 1)),
            'y': rng.randrange(max(1, h - rh + 1)),
            'width': rw,
            'height': rh,
            'plane': 0,
            'overlay': rng.choice(overlays) if overlays and rng.random() < 0.7 else None,
            'underlay': rng.choice(underlays) if underlays else None,
            'shape': 0,
            'rotation': 0,
            'flags': 0,
        }

    if stage == 'macro' and palette:
        walls = [p for p in palette if p['kind'] == 0]
        corners = [p for p in palette if p['kind'] == 2]
        slopes = [p for p in palette if p['kind'] == 12]
        ridges = [p for p in palette if p['kind'] == 13]
        if walls and w >= 4 and h >= 4 and rng.random() < 0.5:
            x0 = rng.randrange(0, max(1, w - 3))
            y0 = rng.randrange(0, max(1, h - 3))
            rw = rng.randint(3, min(10, w - x0))
            rh = rng.randint(3, min(10, h - y0))
            return {
                'tool': 'room_rect',
                'object': {
                    'id': rng.choice(walls)['id'],
                    'kind': 0,
                    'x': x0,
                    'y': y0,
                    'plane': 0,
                    'orientation': 0,
                },
                'width': rw,
                'height': rh,
                'corner_id': rng.choice(corners)['id'] if corners else None,
            }
        if slopes and w >= 4 and h >= 4 and rng.random() < 0.5:
            x0 = rng.randrange(0, max(1, w - 3))
            y0 = rng.randrange(0, max(1, h - 3))
            x1 = rng.randrange(x0 + 2, min(w, x0 + 10))
            y1 = rng.randrange(y0 + 2, min(h, y0 + 10))
            return {
                'tool': 'hip_roof',
                'plane': min(1, planes - 1),
                'x0': x0,
                'y0': y0,
                'x1': x1,
                'y1': y1,
                'slope_id': rng.choice(slopes)['id'],
                'ridge_id': rng.choice(ridges)['id'] if ridges else None,
            }

    if palette:
        entry = rng.choice(palette)
        return {
            'tool': 'place',
            'object': {
                'id': entry['id'],
                'kind': entry['kind'],
                'x': rng.randrange(w),
                'y': rng.randrange(h),
                'plane': rng.randrange(planes),
                'orientation': rng.randrange(4),
            },
        }
    return None


def reconstruct(args):
    started = time.monotonic()
    actions = []
    with LabClient(args.binary) as client:
        reset = client.reset(args.task, args.seed, visible=args.method == 'imitate')
        observation = reset['observation']
        score = reset.get('score', 0.0)
        breakdown = reset.get('breakdown') or {}
        if args.method == 'imitate':
            for action in imitate(client, observation):
                result = client.step(action, observe=False)
                actions.append(action)
                score = result['result']['score']
        else:
            # Score-only multi-stage coordinate-ascent search using fast observe=False rollouts.
            rng = random.Random(args.seed)
            legal = observation['legal']
            for step_idx in range(args.rounds):
                if legal['remaining'] < args.candidates * 2 + 1:
                    break
                if (legal.get('overlays') or legal.get('underlays')) and breakdown.get('terrain', 1.0) < 0.99 and step_idx < max(2, args.rounds // 3):
                    stage = 'terrain'
                elif legal.get('palette') and step_idx < max(4, (args.rounds * 2) // 3) and rng.random() < 0.35:
                    stage = 'macro'
                else:
                    stage = 'objects'
                best, best_score, best_breakdown = None, score, breakdown
                for _ in range(args.candidates):
                    action = _propose_search_candidate(rng, legal, stage)
                    if action is None:
                        continue
                    try:
                        res = client.step(action, observe=False)['result']
                        candidate = res['score']
                        cand_breakdown = res.get('breakdown') or {}
                    except RuntimeError:
                        actions.append(action)  # Failed attempts consume budget and replay deterministically.
                        continue
                    actions.append(action)
                    client.step({'tool': 'undo'}, observe=False)
                    actions.append({'tool': 'undo'})
                    if candidate > best_score:
                        best, best_score, best_breakdown = action, candidate, cand_breakdown
                if best is not None:
                    result = client.step(best, observe=False)['result']
                    actions.append(best)
                    score = result['score']
                    breakdown = result.get('breakdown') or best_breakdown
                legal = client.call({'cmd': 'legal'})['legal']
        final = client.call({'cmd': 'observe'})
    write_json(args.out, actions)
    seconds = time.monotonic() - started
    print(json.dumps({'method': args.method, 'seed': args.seed, 'actions': len(actions),
                      'score': score, 'seconds': seconds,
                      'actions_per_second': len(actions) / max(seconds, 1e-9),
                      'target_visible': args.method == 'imitate',
                      'observation_has_target': 'target' in final['observation']}, indent=2))


def features(info, kind):
    words = re.findall(r'[a-z]+|[0-9]+', (info.get('name', '') + ' ' + info.get('symbol', '')).lower())
    result = ['token:' + w for w in words]
    result += ['kind:' + str(kind), 'size:' + 'x'.join(map(str, info.get('size', [1, 1])))]
    result += ['action:' + a.lower() for a in info.get('actions', [])]
    if info.get('category') is not None:
        result.append('category:' + str(info['category']))
    return result


def group_for(prefab):
    source = prefab.get('source', {})
    if source.get('from') == 'captured':
        return f"geography:{source['tile_x'] // 512}:{source['tile_y'] // 512}"
    # Seed/rotation variants from the same authored family stay in one evaluation group.
    return 'family:' + prefab.get('name', '').split(' copy')[0]


def split_for(group):
    slot = int.from_bytes(hashlib.sha256(group.encode()).digest()[:4], 'big') % 5
    return 'test' if slot == 0 else 'validation' if slot == 1 else 'train'


def building_critic_features(record):
    """Extract compact structural quality features from a building census record or snapshot."""
    objs = record.get('objects', [])
    rooms = record.get('rooms', [])
    floor_tiles = set()
    for r in rooms:
        for t in r.get('tiles', []):
            floor_tiles.add((r.get('plane', 0), t[0], t[1]))
    if not floor_tiles:
        for t in record.get('tiles', []):
            floor_tiles.add((t.get('plane', 0), t['x'], t['y']))
    n_floors = max(len(floor_tiles), 1)
    walls = [o for o in objs if o.get('kind') in (0, 2)]
    doors = [o for o in objs if o.get('kind') == 0 and o.get('role') == 'Door']
    roofs = [o for o in objs if o.get('kind', 0) in range(12, 22)]
    furniture = [o for o in objs if o.get('kind') in (10, 11)]
    perimeter_est = max(4.0, 4.0 * math.sqrt(n_floors))
    wall_closure = min(2.0, len(walls) / perimeter_est)
    furniture_density = min(1.0, len(furniture) / n_floors)
    roof_coverage = min(1.5, len(roofs) / n_floors)
    has_doors = 1.0 if (doors or len(walls) >= 4) else 0.0
    return [1.0, wall_closure, furniture_density, roof_coverage, has_doors]


def train_critic_model(buildings, source_meta):
    """Fit a lightweight linear/logistic structural critic separating real buildings from corrupted ones."""
    feature_names = ['bias', 'wall_closure', 'furniture_density', 'roof_coverage', 'has_doors']
    if not buildings:
        return {
            'format': 'rustosrs-critic-model',
            'version': 1,
            'source': source_meta,
            'features': feature_names,
            'weights': [0.0, 1.2, 0.8, 0.6, 0.5],
        }
    positives = [building_critic_features(b) for b in buildings]
    negatives = []
    for b in buildings:
        # Synthetic negative 1: stripped walls
        negatives.append(building_critic_features({
            'rooms': b.get('rooms', []),
            'objects': [o for o in b.get('objects', []) if o.get('kind') not in (0, 2)],
        }))
        # Synthetic negative 2: overcrowded furniture / missing roof
        negatives.append([1.0, 0.2, 1.0, 0.0, 0.0])
    dim = len(feature_names)
    pos_mean = [sum(x[i] for x in positives) / len(positives) for i in range(dim)]
    neg_mean = [sum(x[i] for x in negatives) / len(negatives) for i in range(dim)]
    weights = [round(pos_mean[i] - neg_mean[i], 4) for i in range(dim)]
    weights[0] = round(-0.5 * sum(weights[i] * (pos_mean[i] + neg_mean[i]) for i in range(1, dim)), 4)
    return {
        'format': 'rustosrs-critic-model',
        'version': 1,
        'source': source_meta,
        'features': feature_names,
        'weights': weights,
    }


def score_with_critic(critic, record):
    feats = building_critic_features(record)
    weights = critic.get('weights', [0.0] * len(feats))
    z = sum(w * f for w, f in zip(weights, feats))
    return 1.0 / (1.0 + math.exp(-max(-20.0, min(20.0, z))))


def best_of_n(critic, candidates):
    """Rank candidate building records using the learned structural critic (8 GB CPU inference)."""
    if not candidates:
        return None
    return max(candidates, key=lambda c: score_with_critic(critic, c))


def fit(args):
    dataset = Path(args.dataset)
    objects = json.loads((dataset / 'objects.json').read_text())
    manifest = json.loads((dataset / 'manifest.json').read_text())
    examples = []
    for file in sorted(Path(args.prefabs).glob('*.prefab.json')):
        prefab = json.loads(file.read_text())
        building = prefab.get('building') or {}
        if not building.get('reviewed'):
            continue
        labels = building.get('labels', {})
        for obj in prefab['objects']:
            key = f"human:object:{obj['id']}:{obj['kind']}"
            role = labels.get(key)
            info = objects.get(str(obj['id']))
            if role and info:
                examples.append((group_for(prefab), role, features(info, obj['kind'])))
    training = [e for e in examples if split_for(e[0]) == 'train']
    if not training:
        raise SystemExit('No confirmed training labels. Review object roles in captured prefabs first.')
    labels = sorted({role for _, role, _ in training})
    docs = Counter(role for _, role, _ in training)
    counts = {role: Counter() for role in labels}
    for _, role, tokens in training:
        counts[role].update(tokens)
    vocabulary = sorted({token for c in counts.values() for token in c})
    priors = [round(1024 * math.log(docs[role] / len(training))) for role in labels]
    totals = {role: sum(counts[role].values()) + len(vocabulary) for role in labels}
    weights = {word: [round(1024 * math.log((counts[role][word] + 1) / totals[role]))
                      for role in labels] for word in vocabulary}
    model = {'format': 'rustosrs-role-model', 'version': 1,
             'sources': [manifest['source']['cache_fingerprint']], 'labels': labels,
             'scale': 1024, 'priors': priors, 'feature_weights': weights}
    write_json(args.out, model)
    report = {'training_examples': len(training), 'groups': len({e[0] for e in examples})}
    for split in ['validation', 'test']:
        held = [e for e in examples if split_for(e[0]) == split]
        correct = 0
        for _, role, tokens in held:
            scores = [prior + sum(weights.get(t, [0] * len(labels))[i] for t in tokens)
                      for i, prior in enumerate(priors)]
            predicted = labels[max(range(len(labels)), key=lambda i: (scores[i], -i))]
            correct += predicted == role
        report[split] = {'examples': len(held), 'accuracy': correct / len(held) if held else None}
    write_json(str(args.out) + '.metrics.json', report)
    # Exact observed counts remain separate from inferred room labels and human evaluation.
    placements = Counter()
    furniture_priors = defaultdict(Counter)
    building_records = []
    with (dataset / 'buildings.jsonl').open() as records:
        for line in records:
            if not line.strip():
                continue
            building = json.loads(line)
            building_records.append(building)
            for room in building.get('rooms', []):
                tiles = {tuple(t) for t in room['tiles']}
                if not tiles:
                    continue
                xs = [t[0] for t in tiles]
                ys = [t[1] for t in tiles]
                min_x, max_x, min_y, max_y = min(xs), max(xs), min(ys), max(ys)
                for obj in building.get('objects', []):
                    if obj['plane'] == room['plane'] and tuple(obj['tile']) in tiles:
                        placements[(room['tag'], obj['object'], obj['kind'], obj['orientation'])] += 1
                        tx, ty = obj['tile']
                        wall_dist = min(tx - min_x, max_x - tx, ty - min_y, max_y - ty)
                        zone = 'wall' if wall_dist == 0 else 'interior'
                        furniture_priors[room['tag']][(obj['object'], obj['kind'], zone, obj['orientation'])] += 1
    write_json(str(args.out) + '.placements.json', {'version': 1, 'source': manifest['source'],
               'room_labels': 'weak census labels; not an accuracy evaluation',
               'counts': [{'room': r, 'object': o, 'kind': k, 'orientation': turn, 'count': n}
                          for (r, o, k, turn), n in sorted(placements.items())]})
    write_json(str(args.out) + '.furniture.json', {
        'format': 'rustosrs-furniture-model',
        'version': 1,
        'source': manifest['source'],
        'rooms': {
            room_tag: [
                {'object': o, 'kind': k, 'zone': z, 'orientation': turn, 'weight': n}
                for (o, k, z, turn), n in sorted(counter.items())
            ]
            for room_tag, counter in sorted(furniture_priors.items())
        },
    })
    critic = train_critic_model(building_records, manifest['source'])
    write_json(str(args.out) + '.critic.json', critic)
    if args.kit_in and args.kit_out:
        kit = json.loads(Path(args.kit_in).read_text())
        kit.setdefault('finish', {})['learned_roles'] = model
        write_json(args.kit_out, kit)
    print(json.dumps(report, indent=2))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='command', required=True)
    reconstruction = sub.add_parser('reconstruct')
    reconstruction.add_argument('--binary', default='target/debug/osrs-lab')
    reconstruction.add_argument('--task', required=True)
    reconstruction.add_argument('--method', choices=['imitate', 'search'], default='imitate')
    reconstruction.add_argument('--seed', type=int, default=1)
    reconstruction.add_argument('--rounds', type=int, default=20)
    reconstruction.add_argument('--candidates', type=int, default=8)
    reconstruction.add_argument('--out', required=True)
    model = sub.add_parser('fit')
    model.add_argument('--dataset', required=True)
    model.add_argument('--prefabs', required=True)
    model.add_argument('--out', required=True)
    model.add_argument('--kit-in')
    model.add_argument('--kit-out')
    args = parser.parse_args()
    if args.command == 'reconstruct':
        reconstruct(args)
    else:
        fit(args)


if __name__ == '__main__':
    main()
