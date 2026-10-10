"""Checks for authored lane widths, crossing heights, joins and collider bounds."""
import json
import tempfile
import unittest
from pathlib import Path

import numpy as np
from shapely.geometry import LineString, Polygon
from shapely.ops import unary_union

from author_city_bridges import layout
from author_city_track import MeshWriter, cross_sections
from repair_city_overlaps import read_polygon, surface_partition


POLICY = json.loads(Path(__file__).with_name('bengaluru_backbone.json').read_text())['game_track']


def road(points, nodes, highway='primary', layer='0', bridge='no', tunnel='no'):
    return {'highway': highway, 'points_xz_m': [[x, -y] for x, y in points], 'node_ids': nodes,
            'tags': {'layer': layer, 'bridge': bridge, 'tunnel': tunnel}, 'names': [],
            'game_length_m': LineString(points).length, 'width_m': 13., 'median_width_m': 1.}


class CityTrackTests(unittest.TestCase):
    def test_motorway_bridge_overrides_three_lane_rule(self):
        rows = cross_sections([road([(0, 0), (100, 0)], [0, 1]),
                               road([(0, 0), (100, 0)], [0, 1], highway='motorway'),
                               road([(0, 0), (100, 0)], [0, 1], highway='motorway', layer='1', bridge='yes')], POLICY)
        self.assertEqual([r['width_m'] for r in rows], [13, 19, 7])
        self.assertEqual([r['lanes_per_direction'] for r in rows], [2, 3, 1])

    def test_short_bridge_keeps_xy_and_meets_crossing_clearance(self):
        records = cross_sections([road([(-2, 0), (2, 0)], [0, 1], layer='1', bridge='yes'),
                                  road([(0, -30), (0, 30)], [2, 3]),
                                  road([(-30, 0), (-2, 0)], [4, 0]),
                                  road([(2, 0), (30, 0)], [1, 5])], POLICY)
        snapshot = json.dumps(records, sort_keys=True)
        floors, fields, profiles, report = layout(records, POLICY)
        self.assertEqual(json.dumps(records, sort_keys=True), snapshot)
        self.assertEqual(report['crossings_checked'], 1)
        self.assertGreaterEqual(report['minimum_clearance_m'], POLICY['bridge_clearance_m'])
        np.testing.assert_allclose(fields[1].sample([(-2, 0), (2, 0)]), 0, atol=1e-6)
        self.assertGreater(float(fields[1].sample([(0, 0)])[0]), 5)
        self.assertTrue(report['steep_ramps'])

    def test_underpass_is_below_ground_and_source_joins_stay_zero(self):
        rows = cross_sections([road([(-40, 0), (40, 0)], [0, 1], layer='-1', tunnel='yes'),
                               road([(0, -20), (0, 20)], [2, 3]),
                               road([(-80, 0), (-40, 0)], [4, 0]),
                               road([(40, 0), (80, 0)], [1, 5])], POLICY)
        _, fields, _, report = layout(rows, POLICY)
        self.assertLess(float(fields[-1].sample([(0, 0)])[0]), -5)
        np.testing.assert_allclose(fields[-1].sample([(-40, 0), (40, 0)]), 0, atol=1e-6)
        self.assertGreaterEqual(report['minimum_clearance_m'], 4.5)

    def test_shared_transition_does_not_hide_a_later_crossing(self):
        rows = cross_sections([road([(0, 0), (20, 0)], [0, 1], layer='1', bridge='yes'),
                               road([(0, 0), (10, -10), (10, 10)], [0, 2, 3])], POLICY)
        _, fields, _, report = layout(rows, POLICY)
        self.assertEqual(report['crossings_checked'], 1)
        self.assertAlmostEqual(float(fields[1].sample([(0, 0)])[0]), 0)
        self.assertGreaterEqual(report['minimum_clearance_m'], 4.5)

    def test_crossing_bridge_layers_have_separate_height_fields(self):
        rows = cross_sections([road([(-40, 0), (40, 0)], [0, 1], layer='1', bridge='yes'),
                               road([(0, -40), (0, 40)], [2, 3], layer='2', bridge='yes')], POLICY)
        _, fields, _, report = layout(rows, POLICY)
        difference = fields[2].sample([(0, 0)])[0] - fields[1].sample([(0, 0)])[0]
        self.assertGreaterEqual(difference - POLICY['deck_thickness_m'], 4.5)

    def test_tight_return_footprint_preserves_centerline(self):
        row = road([(367.67130447564415, 557.8969994344749), (369.37416920807243, 555.0065388903091),
                    (369.5677780360947, 554.5796867250665), (369.72809055415576, 554.2845461787879),
                    (369.85963445711843, 554.193331689364), (369.98250962025566, 554.3385426203271)], list(range(6)))
        b, j, m, _ = surface_partition([row], {'median_opening_margin_m': 2, 'ensure_centerline_coverage': True})
        floor = unary_union([read_polygon(r) for r in b + j + m])
        line = LineString([(x, -z) for x, z in row['points_xz_m']])
        self.assertLess(line.difference(floor.buffer(1e-7)).length, 1e-6)

    def test_chunked_polygons_preserve_holes_and_bound_collision(self):
        polygon = Polygon([(0, 0), (120, 0), (120, 70), (0, 70)], [[(40, 20), (80, 20), (80, 50), (40, 50)]])
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'meshes.bin'
            writer = MeshWriter(path, 32)
            writer.polygons(polygon, 'asphalt')
            writer.close()
            raw = path.read_bytes()
            area = 0
            for row in writer.meshes:
                faces = np.frombuffer(raw[row['offset']:row['offset'] + row['bytes']], dtype='<f4').reshape(-1, 3, 3)
                self.assertLessEqual(np.ptp(faces[:, :, 0]), 32.001)
                self.assertLessEqual(np.ptp(faces[:, :, 2]), 32.001)
                area += np.cross(faces[:, 1] - faces[:, 0], faces[:, 2] - faces[:, 0])[:, 1].sum() / 2
            self.assertAlmostEqual(area, polygon.area, places=3)


if __name__ == '__main__':
    unittest.main()
