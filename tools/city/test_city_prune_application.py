"""Actual cuts preserve through roads, source layers and rebuild determinism."""
import unittest

from shapely.geometry import LineString
from shapely.strtree import STRtree

from node_city_intersections import node_roads
from prune_city_roads import apply_plan, cut_line, digest
from test_city_intersections import road


class PruneApplicationTests(unittest.TestCase):
    def test_tail_after_junction_is_trimmed_without_losing_through_road(self):
        removed = STRtree([LineString([(0, 1), (0, 100)])])
        retained = STRtree([LineString([(-100, 0), (100, 0)])])
        keep, cuts = cut_line(LineString([(0, -100), (0, 0), (0, 100)]), removed, retained)
        self.assertEqual(len(keep), 1)
        self.assertEqual(len(cuts), 1)
        self.assertLess(keep[0].bounds[3], 1)
        self.assertGreater(keep[0].length, 100)
        through, removed_through = cut_line(LineString([(-100, 0), (100, 0)]), removed, retained)
        self.assertFalse(removed_through)
        self.assertAlmostEqual(through[0].length, 200)

    def test_cut_output_preserves_original_bend_vertices(self):
        removed = STRtree([LineString([(1, 0), (100, 0)])])
        retained = STRtree([LineString([(-100, 0), (0, 0)])])
        keep, cuts = cut_line(LineString([(-100, 0), (-50, 2), (0, 0), (100, 0)]), removed, retained)
        self.assertIn((-50, 2), list(keep[0].coords))
        self.assertGreater(cuts[0].length, 98)

    def test_actual_grade_cut_does_not_cut_ground_at_geometric_crossing(self):
        ground = road([[-50, -50], [50, -50], [50, 50], [-50, 50], [-50, -50]], [0, 1, 2, 3, 0])
        grade = road([[-100, 0], [100, 0]], [4, 5], {'layer': '1', 'bridge': 'yes', 'tunnel': 'no'})
        records, _ = node_roads([ground, grade])
        plan = {'approved_initial_arms': 0, 'approved_follow_up_marks': 1,
                'cuts': [{'structure_profile': ['1', 'yes', 'no'], 'points_xz_m': grade['points_xz_m']}]}
        kept, report = apply_plan(records, plan)
        self.assertAlmostEqual(sum(r['game_length_m'] for r in kept), 400)
        self.assertEqual(report['remaining_components'], 1)
        self.assertEqual(report['intersection_checks']['unsplit_intersections'], 0)

    def test_disconnected_return_loop_is_removed_after_its_stem_is_cut(self):
        main = road([[0, 0], [100, 0], [100, 100], [0, 100], [0, 0]], [0, 1, 2, 3, 0])
        stem = road([[100, 100], [200, 100]], [2, 4])
        island = road([[200, 100], [210, 100], [210, 110], [200, 110], [200, 100]], [4, 5, 6, 7, 4])
        records, _ = node_roads([main, stem, island])
        plan = {'approved_initial_arms': 1, 'approved_follow_up_marks': 1,
                'cuts': [{'structure_profile': ['0', 'no', 'no'], 'points_xz_m': [[125, 100], [175, 100]]}]}
        kept, report = apply_plan(records, plan)
        self.assertAlmostEqual(sum(r['game_length_m'] for r in kept), 400)
        self.assertEqual(report['disconnected_groups_removed'], 1)
        self.assertGreater(report['new_dead_end_length_removed_m'], 49)
        repeated, _ = apply_plan(records, plan)
        self.assertEqual(digest(kept), digest(repeated))


if __name__ == '__main__':
    unittest.main()
