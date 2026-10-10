"""Checks for complete intersection splits, grade separation and dangling review."""
import math
import unittest

from node_city_intersections import dangling_branches, node_roads, validate_intersections


def road(points, nodes, tags=None):
    return {"osm_id": f"road-{nodes}", "points_xz_m": points, "node_ids": nodes,
            "tags": tags or {"layer": "0", "bridge": "no", "tunnel": "no"},
            "names": [], "source_osm_ids": [str(nodes)], "source_road_indexes": [nodes[0]],
            "width_m": 13, "game_length_m": sum(math.dist(a, b) for a, b in zip(points, points[1:]))}


class CityIntersectionTests(unittest.TestCase):
    def test_crossings_and_t_junctions_split_through_roads(self):
        records = [road([[-100, 0], [100, 0]], [0, 1]),
                   road([[0, -100], [0, 100]], [2, 3]),
                   road([[50, 0], [50, 100]], [4, 5])]
        output, _ = node_roads(records)
        self.assertEqual(len(output), 6)
        self.assertEqual(validate_intersections(output)["unsplit_intersections"], 0)
        branches, graph, _ = dangling_branches(output)
        self.assertEqual(sum(len(edges) == 1 for edges in graph.values()), 5)
        self.assertEqual(len(branches), 5)
        self.assertTrue(any(abs(r["centerline_length_m"] - 50) < 1e-7 for r in branches))

    def test_bridge_crossing_is_not_a_junction(self):
        records = [road([[-100, 0], [100, 0]], [0, 1]),
                   road([[0, -100], [0, 100]], [2, 3], {"layer": "1", "bridge": "yes", "tunnel": "no"})]
        output, _ = node_roads(records)
        self.assertEqual(len(output), 2)
        _, graph, _ = dangling_branches(output)
        self.assertEqual(sum(len(edges) == 1 for edges in graph.values()), 4)

    def test_existing_bridge_attachment_inside_road_is_split(self):
        records = [road([[-100, 0], [0, 0], [100, 0]], [0, 1, 2]),
                   road([[0, 0], [0, 100]], [1, 3], {"layer": "1", "bridge": "yes", "tunnel": "no"})]
        output, _ = node_roads(records)
        self.assertEqual(len(output), 3)
        _, graph, _ = dangling_branches(output)
        self.assertEqual(sorted(map(len, graph.values())), [1, 1, 1, 3])

    def test_duplicate_intervals_do_not_hide_a_tip(self):
        records = [road([[-100, 0], [100, 0]], [0, 1]), road([[0, 0], [100, 0]], [2, 1])]
        output, _ = node_roads(records)
        self.assertEqual(len(output), 2)
        self.assertAlmostEqual(sum(r["game_length_m"] for r in output), 200)
        branches, _, _ = dangling_branches(output)
        self.assertEqual(len(branches), 1)
        self.assertAlmostEqual(branches[0]["centerline_length_m"], 200)

    def test_closed_return_arm_is_reported_despite_no_tip(self):
        records = [road([[-100, 0], [100, 0]], [0, 1]),
                   road([[0, 0], [-1, 100], [1, 100], [0, 0]], [2, 3, 4, 2])]
        output, _ = node_roads(records)
        branches, _, _ = dangling_branches(output)
        returns = [r for r in branches if r["kind"] == "collapsed_return_tail"]
        self.assertEqual(len(returns), 1)
        self.assertGreaterEqual(returns[0]["reach_m"], 100)

    def test_wide_loop_with_two_connections_is_not_dangling(self):
        records = [road([[-100, 0], [100, 0]], [0, 1]),
                   road([[-50, 0], [-50, 100], [50, 100], [50, 0]], [2, 3, 4, 5])]
        output, _ = node_roads(records)
        branches, _, _ = dangling_branches(output)
        self.assertFalse(any(r["kind"] in {"collapsed_return_tail", "single_entry_loop"} for r in branches))

    def test_nested_returns_are_one_full_tail_from_the_main_junction(self):
        records = [road([[-100, 0], [100, 0]], [0, 1]),
                   road([[0, 0], [-1, 100], [1, 100], [0, 0]], [2, 3, 4, 2]),
                   road([[-1, 100], [-2, 200], [0, 200], [-1, 100]], [3, 5, 6, 3])]
        output, _ = node_roads(records)
        branches, _, _ = dangling_branches(output)
        returns = [r for r in branches if r["kind"] == "collapsed_return_tail"]
        self.assertEqual(len(returns), 1)
        self.assertGreaterEqual(returns[0]["reach_m"], 200)


if __name__ == "__main__":
    unittest.main()
