"""Deterministic learner contracts, including geographic and rotation leakage prevention."""
import unittest
import tempfile
import json
import argparse
import contextlib
import io
from pathlib import Path
from train import (
    best_of_n,
    features,
    fit,
    group_for,
    imitate,
    score_with_critic,
    split_for,
    train_critic_model,
)


class TrainingTests(unittest.TestCase):
    def test_geographic_neighbors_and_rotated_authored_variants_stay_together(self):
        a = {'source': {'from': 'captured', 'tile_x': 3200, 'tile_y': 3200}}
        b = {'source': {'from': 'captured', 'tile_x': 3250, 'tile_y': 3260}}
        self.assertEqual(group_for(a), group_for(b))
        a, b = {'name': 'Tavern'}, {'name': 'Tavern copy rotated'}
        self.assertEqual(group_for(a), group_for(b))
        self.assertEqual(split_for(group_for(a)), split_for(group_for(b)))

    def test_features_match_runtime_tokens_and_definition_channels(self):
        self.assertEqual(features({'name': 'Oak table 2', 'size': [2, 1],
                                   'actions': ['Search'], 'category': 7}, 10),
                         ['token:oak', 'token:table', 'token:2', 'kind:10',
                          'size:2x1', 'action:search', 'category:7'])

    def test_fit_exports_deterministic_versioned_weights_and_reports_empty_holdouts(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            dataset, prefabs = root / 'dataset', root / 'prefabs'
            dataset.mkdir(); prefabs.mkdir()
            (dataset / 'objects.json').write_text(json.dumps({'1': {'name': 'oak bed', 'actions': []}}))
            (dataset / 'manifest.json').write_text(json.dumps({'source': {'cache_fingerprint': 'fixture'}}))
            (dataset / 'buildings.jsonl').write_text('')
            family = next(f'family{i}' for i in range(100) if split_for('family:family' + str(i)) == 'train')
            prefab = {'name': family, 'source': {'from': 'authored'},
                      'objects': [{'id': 1, 'kind': 10}],
                      'building': {'reviewed': True, 'labels': {'human:object:1:10': 'bed'}}}
            (prefabs / 'a.prefab.json').write_text(json.dumps(prefab))
            args = argparse.Namespace(dataset=dataset, prefabs=prefabs, out=root / 'model.json', kit_in=None, kit_out=None)
            with contextlib.redirect_stdout(io.StringIO()):
                fit(args)
                original = args.out.read_bytes()
                fit(args)
            self.assertEqual(args.out.read_bytes(), original)
            model = json.loads(original)
            self.assertEqual(model['sources'], ['fixture'])
            self.assertEqual(model['labels'], ['bed'])
            metrics = json.loads(Path(str(args.out) + '.metrics.json').read_text())
            self.assertIsNone(metrics['test']['accuracy'])
            self.assertIsNone(metrics['validation']['accuracy'])
            self.assertTrue(Path(str(args.out) + '.furniture.json').is_file())
            self.assertTrue(Path(str(args.out) + '.critic.json').is_file())

    def test_imitation_emits_only_tools_and_keeps_occurrences(self):
        wall = {'id': 1, 'kind': 0, 'x': 0, 'y': 0, 'plane': 0, 'orientation': 0}
        other = dict(wall, x=1)
        actions = list(imitate(None, {'current': {'tiles': [], 'heights': [], 'objects': []},
                                    'target': {'tiles': [], 'heights': [], 'objects': [wall, other]}}))
        self.assertEqual(actions, [{'tool': 'place', 'object': wall}, {'tool': 'place', 'object': other}])
        self.assertTrue(all('patch' not in a for a in actions))

    def test_imitation_packs_maximal_rectangles_and_collinear_wall_lines(self):
        tiles = [
            {'x': x, 'y': y, 'plane': 0, 'underlay': 2, 'overlay': {'id': 4, 'shape': 0, 'rotation': 0}, 'flags': 0}
            for x in range(3) for y in range(2)
        ]
        heights = [
            {'x': x, 'y': y, 'plane': 0, 'value': -64}
            for x in range(2) for y in range(2)
        ]
        walls = [
            {'id': 10, 'kind': 0, 'x': x, 'y': 5, 'plane': 0, 'orientation': 1}
            for x in range(4)
        ]
        actions = list(imitate(None, {
            'current': {'tiles': [], 'heights': [], 'objects': []},
            'target': {'tiles': tiles, 'heights': heights, 'objects': walls},
        }))
        self.assertEqual(actions, [
            {'tool': 'paint_rect', 'x': 0, 'y': 0, 'width': 3, 'height': 2, 'plane': 0,
             'overlay': 4, 'underlay': 2, 'shape': 0, 'rotation': 0, 'flags': 0},
            {'tool': 'height_stroke', 'x': 0, 'y': 0, 'width': 2, 'height': 2, 'plane': 0, 'value': -64},
            {'tool': 'wall_line', 'object': {'id': 10, 'kind': 0, 'x': 0, 'y': 5, 'plane': 0, 'orientation': 1},
             'length': 4, 'vertical': False},
        ])

    def test_structural_critic_and_best_of_n_prefer_closed_furnished_roofed_building(self):
        good = {
            'rooms': [{'plane': 0, 'tag': 'living', 'tiles': [[x, y] for x in range(4) for y in range(4)]}],
            'objects': (
                [{'kind': 0, 'role': 'Wall'} for _ in range(14)]
                + [{'kind': 0, 'role': 'Door'}]
                + [{'kind': 10, 'role': 'Furniture'} for _ in range(3)]
                + [{'kind': 12, 'role': 'Roof'} for _ in range(16)]
            ),
        }
        bad = {
            'rooms': [{'plane': 0, 'tag': 'living', 'tiles': [[x, y] for x in range(4) for y in range(4)]}],
            'objects': [{'kind': 10, 'role': 'Furniture'}],
        }
        critic = train_critic_model([good], {'cache_fingerprint': 'fixture'})
        self.assertGreater(score_with_critic(critic, good), score_with_critic(critic, bad))
        self.assertIs(best_of_n(critic, [bad, good]), good)


if __name__ == '__main__':
    unittest.main()
