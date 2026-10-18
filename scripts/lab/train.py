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
    for x, y, plane in sorted(old.keys() | new.keys()):
        tile = new.get((x, y, plane), {'underlay': None, 'overlay': None, 'flags': 0})
        if old.get((x, y, plane)) == tile:
            continue
        overlay = tile['overlay']
        yield {'tool': 'paint_rect', 'x': x, 'y': y, 'width': 1, 'height': 1,
               'plane': plane, 'overlay': overlay['id'] if overlay else None,
               'underlay': tile['underlay'], 'shape': overlay['shape'] if overlay else 0,
               'rotation': overlay['rotation'] if overlay else 0, 'flags': tile['flags']}
    old_heights = {(h['x'], h['y'], h['plane']): h['value'] for h in current['heights']}
    for height in target['heights']:
        if old_heights.get((height['x'], height['y'], height['plane'])) != height['value']:
            yield {'tool': 'height_stroke', 'x': height['x'], 'y': height['y'],
                   'width': 1, 'height': 1, 'plane': height['plane'], 'value': height['value']}
    for item in sorted(target['objects'], key=lambda o: (o['kind'], o['plane'], o['y'], o['x'], o['id'])):
        if object_key(item) not in have:
            yield {'tool': 'place', 'object': item}


def reconstruct(args):
    started = time.monotonic()
    actions = []
    with LabClient(args.binary) as client:
        reset = client.reset(args.task, args.seed, visible=args.method == 'imitate')
        observation = reset['observation']
        score = reset.get('score', 0.0)
        if args.method == 'imitate':
            for action in imitate(client, observation):
                result = client.step(action)
                actions.append(action)
                score = result['result']['score']
        else:
            # This learner reads current state, legal actions and scalar scores only. It never
            # opens the task artifact or receives the target snapshot.
            rng = random.Random(args.seed)
            legal = observation['legal']
            for _ in range(args.rounds):
                if legal['remaining'] < args.candidates * 2 + 1:
                    break
                best, best_score = None, score
                for _ in range(args.candidates):
                    entry = rng.choice(legal['palette'])
                    action = {'tool': 'place', 'object': {'id': entry['id'], 'kind': entry['kind'],
                              'x': rng.randrange(legal['width']), 'y': rng.randrange(legal['height']),
                              'plane': rng.randrange(legal['planes']), 'orientation': rng.randrange(4)}}
                    try:
                        candidate = client.step(action)['result']['score']
                    except RuntimeError:
                        actions.append(action)  # Failed attempts consume budget and replay deterministically.
                        continue
                    actions.append(action)
                    client.step({'tool': 'undo'})
                    actions.append({'tool': 'undo'})
                    if candidate > best_score:
                        best, best_score = action, candidate
                if best is not None:
                    result = client.step(best)
                    actions.append(best)
                    score = result['result']['score']
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
    with (dataset / 'buildings.jsonl').open() as records:
        for line in records:
            building = json.loads(line)
            for room in building.get('rooms', []):
                tiles = {tuple(t) for t in room['tiles']}
                for obj in building.get('objects', []):
                    if obj['plane'] == room['plane'] and tuple(obj['tile']) in tiles:
                        placements[(room['tag'], obj['object'], obj['kind'], obj['orientation'])] += 1
    write_json(str(args.out) + '.placements.json', {'version': 1, 'source': manifest['source'],
               'room_labels': 'weak census labels; not an accuracy evaluation',
               'counts': [{'room': r, 'object': o, 'kind': k, 'orientation': turn, 'count': n}
                          for (r, o, k, turn), n in sorted(placements.items())]})
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
