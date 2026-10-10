"""Checks for full physical arms that source graph loops can disguise."""
import unittest

import numpy as np
from shapely.geometry import LineString, Point
from shapely.ops import unary_union
from shapely.strtree import STRtree

from node_city_intersections import dangling_branches
from review_city_surface_branches import rasterize, skeleton_records, structure_continuations, thin


class SurfaceBranchTests(unittest.TestCase):
    def review(self, floor):
        mask, minx, miny = rasterize(floor, 1)
        records, _ = skeleton_records(mask, minx, miny, 1)
        return dangling_branches(records)[0]

    def test_thinning_keeps_strokes_inside_original_surface(self):
        mask = np.zeros((60, 120), dtype=bool)
        mask[20:33, 10:110] = True
        result = thin(mask)
        self.assertTrue(np.all(~result | mask))
        self.assertLess(np.count_nonzero(result), np.count_nonzero(mask))
        records, _ = skeleton_records(mask, 0, 0, 1)
        branches, _, _ = dangling_branches(records)
        self.assertEqual(len(branches), 1)
        self.assertGreater(branches[0]["reach_m"], 80)

    def test_overlapping_return_is_a_full_post_junction_tail(self):
        floor = unary_union([LineString([(-100, 0), (100, 0)]).buffer(6.5, cap_style="flat"),
                             LineString([(0, 0), (-1, 100), (1, 100), (0, 0)]).buffer(6.5, join_style="mitre")])
        branches = self.review(floor)
        self.assertEqual(sum(r["kind"] == "degree_one_tail" and r["reach_m"] >= 80 for r in branches), 3)
        self.assertTrue(any(r["reach_m"] >= 85 and r["furthest_xz_m"][1] < -85 for r in branches))

    def test_two_junction_connections_do_not_become_a_tail(self):
        floor = unary_union([LineString([(-100, 0), (100, 0)]).buffer(6.5, cap_style="flat"),
                             LineString([(-50, 0), (-50, 100), (50, 100), (50, 0)]).buffer(6.5, join_style="mitre")])
        branches = self.review(floor)
        self.assertEqual(len(branches), 2)
        self.assertTrue(all(r["kind"] == "degree_one_tail" for r in branches))

    def test_raster_cell_budget_fails_loudly(self):
        floor = LineString([(0, 0), (100, 0)]).buffer(6.5)
        with self.assertRaises(ValueError):
            rasterize(floor, 0.01, max_cells=1000)

    def test_bridge_along_arm_does_not_hide_its_exposed_tip(self):
        geometry = LineString([(0, 0), (100, 0)])
        self.assertEqual(structure_continuations(geometry, Point(100, 0), STRtree([Point(50, 0)]), 8), (True, False))
        self.assertEqual(structure_continuations(geometry, Point(100, 0), STRtree([Point(105, 0)]), 8), (True, True))


if __name__ == "__main__":
    unittest.main()
