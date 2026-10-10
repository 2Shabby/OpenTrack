"""Geometry checks for carriageway consolidation, independent of the local extract."""
import json
import unittest
from pathlib import Path

from shapely.geometry import LineString

from align_city_carriageways import align_preserving_junctions, consolidate, select_pairs, unconnected_crossings
from audit_city_roads import connected_components


POLICY = json.loads((Path(__file__).parent / "bengaluru_backbone.json").read_text())["dual_carriageway_layout"]


def road(index, name="Test Road", layer="0", oneway="yes"):
    return {"osm_id": str(index), "highway": "primary", "name": name,
            "tags": {"oneway": oneway, "layer": layer}}


class CarriagewayTests(unittest.TestCase):
    def test_different_vertex_sampling_and_attached_junction(self):
        records = [road(0), road(1), road(2, "Branch")]
        lines = [LineString([(0, 0), (4, 0), (10, 0)]),
                 LineString([(10, 0.5), (8, 0.5), (2, 0.5), (0, 0.5)]),
                 LineString([(4, 0), (4, 3)])]
        accepted, _ = select_pairs(records, lines, [(0, 1)], POLICY)
        self.assertEqual(accepted, [(0, 1)])
        corridors, report = consolidate(records, lines, accepted, {**POLICY, "prune_alignment_dead_ends": False})
        self.assertEqual(len(connected_components([set(c["node_ids"]) for c in corridors])), 1)
        self.assertEqual({i for c in corridors for i in c["source_road_indexes"]}, {0, 1, 2})
        self.assertAlmostEqual(sum(c["game_length_m"] for c in corridors), 12.75, places=5)
        self.assertEqual(len(corridors), 3)
        main_points = [point for c in corridors if 0 in c["source_road_indexes"] for point in c["points_xz_m"]]
        self.assertTrue(all(abs(z + 0.25) < 1e-8 for x, z in main_points))
        self.assertEqual(report["source_parts_collapsed_into_junctions"], [])

    def test_structure_and_distinct_names_are_not_merged(self):
        lines = [LineString([(0, 0), (10, 0)]), LineString([(10, 0.5), (0, 0.5)])]
        for other, reason in [(road(1, layer="1"), "different_structure"),
                              (road(1, name="Another Road"), "different_names")]:
            accepted, decisions = select_pairs([road(0), other], lines, [(0, 1)], POLICY)
            self.assertEqual(accepted, [])
            self.assertEqual(decisions[0]["decision"], reason)

    def test_oneway_minus_one_changes_travel_direction(self):
        lines = [LineString([(0, 0), (10, 0)]), LineString([(0, 0.5), (10, 0.5)])]
        accepted, _ = select_pairs([road(0), road(1)], lines, [(0, 1)], POLICY)
        self.assertEqual(accepted, [])
        accepted, _ = select_pairs([road(0), road(1, oneway="-1")], lines, [(0, 1)], POLICY)
        self.assertEqual(accepted, [(0, 1)])

    def test_nearby_junction_is_not_an_aligned_run(self):
        lines = [LineString([(0, 0), (10, 0)]), LineString([(10, 0.5), (10, 10)])]
        accepted, _ = select_pairs([road(0), road(1)], lines, [(0, 1)], POLICY)
        self.assertEqual(accepted, [])

    def test_alignment_pruning_keeps_cycles_and_removes_branches(self):
        records = [road(0), road(1, "Branch")]
        lines = [LineString([(0, 0), (10, 0), (10, 10), (0, 10), (0, 0)]),
                 LineString([(0, 0), (-3, 0)])]
        corridors, report = consolidate(records, lines, [], POLICY)
        self.assertAlmostEqual(sum(c["game_length_m"] for c in corridors), 40)
        self.assertEqual(report["source_parts_removed_by_alignment_pruning"], [1])
        self.assertEqual(report["degree_one_vertices"], 0)

    def test_merge_that_introduces_a_crossing_is_deferred(self):
        records = [road(0), road(1), road(2, "Other Road", oneway="no")]
        lines = [LineString([(0, 0), (4, 0), (10, 0)]),
                 LineString([(10, 1), (4, 1), (0, 1)]),
                 LineString([(4, 0.45), (4, 0.55)])]
        policy = {**POLICY, "prune_alignment_dead_ends": False}
        corridors, _, accepted, deferred = align_preserving_junctions(records, lines, [(0, 1)], policy)
        self.assertEqual(accepted, [])
        self.assertEqual(deferred, {(0, 1)})
        self.assertEqual(unconnected_crossings(corridors), [])

    def test_identical_edges_with_different_classes_share_one_corridor(self):
        records = [road(0), {**road(1), "highway": "secondary"}]
        line = LineString([(0, 0), (10, 0)])
        corridors, _ = consolidate(records, [line, line], [], {**POLICY, "prune_alignment_dead_ends": False})
        self.assertEqual(len(corridors), 1)
        self.assertEqual(corridors[0]["highway"], "primary")
        self.assertEqual(corridors[0]["source_road_indexes"], [0, 1])


if __name__ == "__main__":
    unittest.main()
