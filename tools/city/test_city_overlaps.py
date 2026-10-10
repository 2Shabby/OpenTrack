"""Geometry checks for the non-bridge overlap review."""
import unittest

from shapely.geometry import LineString, box

from review_city_overlaps import JunctionContext, build_regions, classify_pair


def road(coords, nodes, **tags):
    return {"tags": {"layer": "0", "bridge": "no", "tunnel": "no", **tags},
            "node_ids": nodes, "points_xz_m": [[x, -y] for x, y in coords],
            "game_length_m": LineString(coords).length, "names": ["Test Road"]}


class OverlapReviewTests(unittest.TestCase):
    def test_bridge_is_excluded_even_at_a_shared_junction(self):
        records = [road([(0, 0), (20, 0)], [0, 1], bridge="viaduct"),
                   road([(0, 0), (0, 20)], [0, 2])]
        context = JunctionContext(records, 13)
        category, _, _ = classify_pair(0, 1, box(0, 0, 6.5, 6.5), 90, context)
        self.assertEqual(category, "bridge_excluded")

    def test_direct_junction_local_overlap(self):
        records = [road([(0, 0), (20, 0)], [0, 1]), road([(0, 0), (0, 20)], [0, 2])]
        category, spill, _ = classify_pair(0, 1, box(0, 0, 6.5, 6.5), 90, JunctionContext(records, 13))
        self.assertEqual(category, "direct_junction_local")
        self.assertEqual(spill, 0)

    def test_short_link_does_not_make_neighboring_sections_disconnected(self):
        records = [road([(0, 0), (10, 0)], [0, 1]), road([(10, 1), (0, 1)], [2, 3]),
                   road([(10, 0), (10, 1)], [1, 2])]
        category, _, _ = classify_pair(0, 1, box(0, -5.5, 10, 6.5), 0, JunctionContext(records, 13))
        self.assertEqual(category, "connected_nearby_local")

    def test_long_parallel_spill_is_not_hidden_by_its_junction(self):
        records = [road([(0, 0), (100, 0)], [0, 1]), road([(100, 1), (0, 1)], [2, 3]),
                   road([(100, 0), (100, 1)], [1, 2])]
        category, spill, _ = classify_pair(0, 1, box(0, -5.5, 100, 6.5), 0, JunctionContext(records, 13))
        self.assertEqual(category, "junction_spill")
        self.assertGreater(spill, 1000)

    def test_nonbridge_level_difference_needs_structure_review(self):
        records = [road([(0, 0), (20, 0)], [0, 1]), road([(0, 1), (20, 1)], [2, 3], layer="-1", tunnel="yes")]
        category, _, _ = classify_pair(0, 1, box(0, 0, 20, 1), 0, JunctionContext(records, 13))
        self.assertEqual(category, "non_bridge_level_change")

    def test_regions_union_pair_areas_without_double_counting(self):
        records = [road([(0, 0), (20, 0)], [0, 1]) for _ in range(3)]
        reports, _ = build_regions([(0, 1, box(0, 0, 10, 10)), (1, 2, box(5, 0, 15, 10))], records)
        self.assertEqual(len(reports), 1)
        self.assertAlmostEqual(reports[0]["unique_overlap_area_m2"], 150)
        self.assertEqual(reports[0]["road_indexes"], [0, 1, 2])


if __name__ == "__main__":
    unittest.main()
