import unittest
from catalog import learn


class CatalogTests(unittest.TestCase):
    def test_opposite_outside_anchor_and_north_direction(self):
        p = {'tiles': [], 'objects': [{'x': 0, 'y': 0, 'plane': 0, 'kind': 0, 'orientation': 2}],
             'building': {'reviewed': True, 'provenance': 'fixture',
                          'floors': [{'x': 1, 'y': 0, 'plane': 0}, {'x': 1, 'y': 1, 'plane': 0}]}}
        learned = learn([p], 'town', 'house')
        self.assertEqual(learned['patterns'][0]['sockets'], (0, 4, 4, 5))
        self.assertIn((0, 0, 1), learned['adjacency'])
        self.assertIn((1, 2, 0), learned['adjacency'])
        self.assertNotIn((0, 1, 1), learned['adjacency'])

    def test_sparse_holes_never_become_observed_neighbors(self):
        p = {'tiles': [], 'objects': [], 'building': {'reviewed': True, 'provenance': 'fixture',
             'floors': [{'x': 0, 'y': 0, 'plane': 0}, {'x': 2, 'y': 0, 'plane': 0}]}}
        self.assertEqual(learn([p], '', '')['adjacency'], [])
        self.assertEqual(learn([p], '', '')['weights'], [2])
        p['building']['reviewed'] = False
        with self.assertRaises(ValueError):
            learn([p], '', '')


if __name__ == '__main__':
    unittest.main()
