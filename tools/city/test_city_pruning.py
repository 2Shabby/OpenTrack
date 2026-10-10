"""Checks that removal exposes new tails and islands without touching source data."""
import copy
import unittest

from review_city_pruning import GROUND, components, make_network, simulate


def network(points, pairs):
    coordinates = dict(enumerate(points))
    edges = []
    for pair in pairs:
        a, b, *role = pair
        kind = role[0] if role else "ground"
        edges.append({"a": a, "b": b, "role": kind,
                      "profile": ("1", "yes", "no") if kind == "grade" else GROUND,
                      "names": (), "source": None})
    return coordinates, edges


def road(points, nodes, grade=False):
    return {"points_xz_m": points, "node_ids": nodes, "names": [],
            "tags": dict(zip(("layer", "bridge", "tunnel"), ("1", "yes", "no") if grade else GROUND))}


class PruningTests(unittest.TestCase):
    def test_two_removed_arms_expose_their_stem(self):
        coords, edges = network([(0, 0), (100, 0), (100, 100), (0, 100), (-50, 0), (-80, 30), (-80, -30)],
                               [(0, 1), (1, 2), (2, 3), (3, 0), (0, 4), (4, 5), (4, 6)])
        original = copy.deepcopy((coords, edges))
        events, rounds, remaining, _ = simulate(coords, edges, {5, 6})
        arms = [e for e in events if e["kind"] == "additional_dangling_arm"]
        self.assertEqual(len(arms), 1)
        self.assertEqual(arms[0]["analysis_edges"], [4])
        self.assertTrue(arms[0]["newly_exposed"])
        self.assertEqual(remaining, {0, 1, 2, 3})
        self.assertEqual(rounds[-1]["simulated_removed_edges"], 0)
        self.assertEqual((coords, edges), original)

    def test_disconnected_grade_loop_is_marked(self):
        coords, edges = network([(0, 0), (200, 0), (200, 200), (0, 200), (-50, 0), (-70, 0), (-70, 20), (-50, 20)],
                               [(0, 1), (1, 2), (2, 3), (3, 0), (4, 5, "grade"), (5, 6, "grade"),
                                (6, 7, "grade"), (7, 4, "grade"), (0, 4)])
        events, _, remaining, stats = simulate(coords, edges, {8})
        islands = [e for e in events if e["kind"] == "disconnected_group"]
        self.assertEqual(len(islands), 1)
        self.assertEqual(islands[0]["analysis_edges"], [4, 5, 6, 7])
        self.assertTrue(islands[0]["contains_grade_roads"])
        self.assertEqual(remaining, {0, 1, 2, 3})
        self.assertEqual(stats["remaining_components"], 1)

    def test_existing_isolated_component_is_not_a_new_disconnection(self):
        coords, edges = network([(0, 0), (200, 0), (200, 200), (0, 200), (500, 0), (550, 0), (550, 50), (500, 50)],
                               [(0, 1), (1, 2), (2, 3), (3, 0), (4, 5), (5, 6), (6, 7), (7, 4)])
        events, _, remaining, stats = simulate(coords, edges, set())
        self.assertEqual(events, [])
        self.assertEqual(len(remaining), 8)
        self.assertEqual(stats["baseline_components"], 2)

    def test_new_structure_bearing_tail_is_marked_and_retained(self):
        coords, edges = network([(0, 0), (100, 0), (100, 100), (0, 100), (-50, 0), (-80, 30), (-80, -30)],
                               [(0, 1), (1, 2), (2, 3), (3, 0), (0, 4), (4, 5), (4, 6, "grade")])
        events, _, remaining, _ = simulate(coords, edges, {5})
        marked = [e for e in events if e["kind"] == "dangling_with_structure"]
        self.assertEqual(len(marked), 1)
        self.assertEqual(marked[0]["analysis_edges"], [4, 6])
        self.assertTrue({4, 6} <= remaining)

    def test_grade_crossing_requires_an_explicit_source_attachment(self):
        surface = [road([[-100, 0], [0, 0], [100, 0]], [0, 1, 2])]
        crossing = [road([[-100, 0], [0, 0], [100, 0]], [10, 11, 12]),
                    road([[0, -100], [0, 100]], [20, 21], True)]
        _, edges, _, mapping = make_network(surface, crossing, 1)
        self.assertEqual(len(components(edges, set(range(len(edges))))), 2)
        self.assertEqual(mapping["matched_grade_attachments"], 0)
        attached = [crossing[0], road([[0, 0], [0, 100]], [11, 21], True)]
        _, edges, _, mapping = make_network(surface, attached, 1)
        self.assertEqual(len(components(edges, set(range(len(edges))))), 1)
        self.assertEqual(mapping["matched_grade_attachments"], 1)

    def test_orphan_analysis_connector_does_not_leave_a_fake_road(self):
        coords, edges = network([(0, 0), (100, 0), (100, 100), (0, 100), (-50, 0),
                                 (-51, 0), (-61, 0), (-61, 10), (-51, 10)],
                               [(0, 1), (1, 2), (2, 3), (3, 0), (0, 4),
                                (5, 6, "grade"), (6, 7, "grade"), (7, 8, "grade"), (8, 5, "grade"), (5, 4, "attachment")])
        events, rounds, remaining, _ = simulate(coords, edges, {4})
        self.assertEqual(rounds[0]["orphan_analysis_attachments_dropped"], 1)
        islands = [e for e in events if e["kind"] == "disconnected_group"]
        self.assertEqual(len(islands), 1)
        self.assertEqual(islands[0]["analysis_edges"], [5, 6, 7, 8])
        self.assertEqual(remaining, {0, 1, 2, 3})


if __name__ == "__main__":
    unittest.main()
