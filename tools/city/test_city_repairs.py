"""Checks for physical road surface partitions, median openings and protected bridges."""
import math
import tempfile
import unittest
from pathlib import Path

from shapely.geometry import Polygon
from shapely.ops import unary_union

from align_city_carriageways import Nodes
from repair_city_overlaps import read_polygon, separate_parallel_roads, surface_partition, write_obj


def road(points, nodes):
    return {"points_xz_m": points, "node_ids": nodes, "tags": {"layer": "0", "bridge": "no", "tunnel": "no"},
            "width_m": 13, "median_width_m": 1,
            "game_length_m": sum(math.dist(a, b) for a, b in zip(points, points[1:]))}


class CityRepairTests(unittest.TestCase):
    def test_junction_surfaces_are_disjoint_and_medians_are_open(self):
        records = [road([[-30, 0], [0, 0], [30, 0]], [0, 1, 2]),
                   road([[0, -30], [0, 0], [0, 30]], [3, 1, 4])]
        bodies, junctions, medians, stats = surface_partition(records, {"median_opening_margin_m": 2})
        body = unary_union([read_polygon(r) for r in bodies])
        junction = unary_union([read_polygon(r) for r in junctions])
        median = unary_union([read_polygon(r) for r in medians])
        self.assertAlmostEqual(body.intersection(junction).area, 0)
        self.assertAlmostEqual(median.intersection(junction).area, 0)
        self.assertAlmostEqual(body.intersection(median).area, 0)
        self.assertAlmostEqual(unary_union([body, junction, median]).area, 60 * 13 * 2 - 13 * 13)
        self.assertGreater(median.area, 0)

    def test_bridge_pin_cannot_move_or_merge_into_another_pin(self):
        nodes = Nodes(5, fixed_points={(0, 0), (2, 0)})
        a, b, c = nodes.add((0, 0)), nodes.add((1, 0)), nodes.add((2, 0))
        self.assertTrue(nodes.join(a, b))
        self.assertEqual(nodes.coord(b), (0, 0))
        self.assertFalse(nodes.join(a, c))
        self.assertEqual(nodes.coord(c), (2, 0))

    def test_different_structure_profiles_are_not_welded(self):
        a = road([[-30, 0], [30, 0]], [0, 1])
        b = {**road([[0, -30], [0, 30]], [2, 3]), "tags": {"layer": "-1", "bridge": "no", "tunnel": "yes"}}
        bodies, junctions, medians, stats = surface_partition([a, b], {"median_opening_margin_m": 2})
        self.assertEqual(junctions, [])
        self.assertEqual(len(bodies), 4)
        self.assertEqual({tuple(r["structure_profile"]) for r in bodies}, {("0", "no", "no"), ("-1", "no", "yes")})
        self.assertAlmostEqual(stats["unique_road_surface_area_m2"], 60 * 13 * 2)

    def test_mesh_triangulation_preserves_holes_and_upward_normals(self):
        polygon = Polygon([(0, 0), (10, 0), (10, 10), (0, 10)], [[(2, 2), (8, 2), (8, 8), (2, 8)]])
        record = {"structure_profile": ["0", "no", "no"],
                  "exterior_xz_m": [[x, -y] for x, y in polygon.exterior.coords],
                  "holes_xz_m": [[[x, -y] for x, y in ring.coords] for ring in polygon.interiors]}
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "surface.obj"
            count = write_obj([record], path)
            lines = path.read_text().splitlines()
        vertices = [tuple(map(float, line.split()[1:])) for line in lines if line.startswith("v ")]
        self.assertEqual(len(vertices), count * 3)
        total_area = 0
        for k in range(0, len(vertices), 3):
            a, b, c = vertices[k:k + 3]
            normal_y = (b[2] - a[2]) * (c[0] - a[0]) - (b[0] - a[0]) * (c[2] - a[2])
            self.assertGreater(normal_y, 0)
            total_area += normal_y / 2
        self.assertAlmostEqual(total_area, 64)

    def test_parallel_separation_keeps_protected_nodes_fixed(self):
        records = [road([[0, 0], [40, 0]], [0, 1]), road([[0, -6], [40, -6]], [2, 3])]
        policy = {"junction_radius_m": 13, "parallel_angle_deg": 15, "separation_iterations": 40,
                  "sample_step_m": 4, "road_separation_m": 13.5, "max_local_shift_m": 8, "max_step_m": 0.5}
        separate_parallel_roads(records, {(0, 0), (40, 0)}, policy)
        self.assertEqual(records[0]["points_xz_m"], [[0, 0], [40, 0]])
        self.assertGreater(-records[1]["points_xz_m"][0][1], 13.4)
        self.assertGreater(-records[1]["points_xz_m"][1][1], 13.4)


if __name__ == "__main__":
    unittest.main()
