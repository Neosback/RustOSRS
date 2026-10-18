"""Deterministic learner contracts, including geographic and rotation leakage prevention."""
import unittest
import tempfile
import json
import argparse
import contextlib
import io
from pathlib import Path
from train import features, group_for, imitate, split_for, fit


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

    def test_imitation_emits_only_tools_and_keeps_occurrences(self):
        wall = {'id': 1, 'kind': 0, 'x': 0, 'y': 0, 'plane': 0, 'orientation': 0}
        other = dict(wall, x=1)
        actions = list(imitate(None, {'current': {'tiles': [], 'heights': [], 'objects': []},
                                    'target': {'tiles': [], 'heights': [], 'objects': [wall, other]}}))
        self.assertEqual(actions, [{'tool': 'place', 'object': wall}, {'tool': 'place', 'object': other}])
        self.assertTrue(all('patch' not in a for a in actions))


if __name__ == '__main__':
    unittest.main()
