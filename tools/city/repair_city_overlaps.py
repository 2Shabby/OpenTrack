#!/usr/bin/env python3
"""Repair the game road layout and export disjoint road surface and open junction medians."""
import argparse
import bisect
import csv
import hashlib
import json
import math
from collections import Counter, defaultdict
from pathlib import Path

import numpy as np
from shapely import constrained_delaunay_triangles
from shapely.geometry import GeometryCollection, LineString, Point
from shapely.ops import unary_union
from shapely.strtree import STRtree
import matplotlib.pyplot as plt
from matplotlib.collections import LineCollection, PatchCollection

from align_city_carriageways import FAMILY_PRIORITY, consolidate, matching_position
from audit_city_roads import ROOT, connected_components, parts, road_layer
from review_city_overlaps import JunctionContext, is_bridge, patches


def lines_for(records):
    return [LineString([(x, -z) for x, z in r["points_xz_m"]]) for r in records]


def duplicate_pairs(records, lines, rows, policy):
    accepted = []
    for row in rows:
        a, b = int(row["road_a"]), int(row["road_b"])
        if is_bridge(records[a]) or is_bridge(records[b]) or road_layer(records[a]) != road_layer(records[b]):
            continue
        angle = row["local_axis_angle_deg"]
        if not angle or float(angle) > policy["max_axis_angle_deg"] or lines[a].distance(lines[b]) > policy["max_pair_gap_m"]:
            continue
        short, long = (a, b) if lines[a].length <= lines[b].length else (b, a)
        count = max(4, math.ceil(lines[short].length / policy["sample_step_m"]))
        coverage = sum(matching_position(lines[short], lines[long], lines[short].length * (k + 0.5) / count,
                                         1, 1, policy) is not None for k in range(count)) / count
        if coverage >= policy["min_shorter_road_coverage"]:
            accepted.append((a, b))
    return accepted


def carry_provenance(corridors, inputs):
    for corridor in corridors:
        contributors = [inputs[i] for i in corridor["source_road_indexes"]]
        corridor["input_corridor_ids"] = sorted({r["osm_id"] for r in contributors})
        corridor["source_road_indexes"] = sorted({i for r in contributors for i in r["source_road_indexes"]})
        corridor["source_osm_ids"] = sorted({i for r in contributors for i in r["source_osm_ids"]})
        corridor["names"] = sorted({name for r in contributors for name in r["names"]})
        for key in ("width_m", "lanes_per_direction", "lane_width_m", "median_width_m", "carriageway_width_m",
                    "lane_center_offsets_from_median_m", "offset_convention", "lane_travel_direction", "traffic_side"):
            corridor[key] = contributors[0][key]
        corridor["source_tags_are_driving_rules"] = False


def separate_parallel_roads(records, fixed_points, policy):
    node_ids = sorted({n for r in records for n in r["node_ids"]})
    indexes = {n: i for i, n in enumerate(node_ids)}
    original = np.zeros((len(node_ids), 2))
    road_nodes = []
    neighbors = defaultdict(set)
    for road in records:
        ids = np.array([indexes[n] for n in road["node_ids"]], dtype=int)
        road_nodes.append(ids)
        for i, point in zip(ids, road["points_xz_m"]):
            original[i] = (point[0], -point[1])
        for a, b in zip(ids, ids[1:]):
            neighbors[a].add(b)
            neighbors[b].add(a)
    coordinates = original.copy()
    fixed = np.array([tuple(round(v, 6) for v in p) in fixed_points for p in original])
    context = JunctionContext(records, policy["junction_radius_m"])
    local_nodes = {}
    max_angle_cos = math.cos(math.radians(policy["parallel_angle_deg"]))
    history = []
    baseline_lengths = np.array([LineString(original[ids]).length for ids in road_nodes])
    for iteration in range(policy["separation_iterations"]):
        lines = [LineString(coordinates[ids]) for ids in road_nodes]
        tree = STRtree(lines)
        cumulative = [np.concatenate(([0.0], np.cumsum(np.linalg.norm(np.diff(coordinates[ids], axis=0), axis=1))))
                      for ids in road_nodes]
        movements, weights = np.zeros_like(coordinates), np.zeros(len(node_ids))
        constraints = 0

        def support(road, distance):
            distances = cumulative[road]
            k = min(len(distances) - 2, max(0, bisect.bisect_right(distances, distance) - 1))
            length = distances[k + 1] - distances[k]
            fraction = (distance - distances[k]) / length if length > 1e-9 else 0
            vector = coordinates[road_nodes[road][k + 1]] - coordinates[road_nodes[road][k]]
            magnitude = np.linalg.norm(vector)
            return [(road_nodes[road][k], 1 - fraction), (road_nodes[road][k + 1], fraction)], vector / magnitude if magnitude > 1e-9 else None

        for a, line in enumerate(lines):
            if is_bridge(records[a]):
                continue
            count = max(1, math.ceil(line.length / policy["sample_step_m"]))
            for sample in range(count):
                distance = line.length * (sample + 0.5) / count
                point = line.interpolate(distance)
                pa = np.array(point.coords[0])
                support_a, axis_a = support(a, distance)
                if axis_a is None:
                    continue
                for b in tree.query(point, predicate="dwithin", distance=policy["road_separation_m"]):
                    b = int(b)
                    if b <= a or is_bridge(records[b]) or road_layer(records[a]) != road_layer(records[b]):
                        continue
                    projected = lines[b].project(point)
                    pb = np.array(lines[b].interpolate(projected).coords[0])
                    gap = np.linalg.norm(pa - pb)
                    if gap < 0.05 or gap >= policy["road_separation_m"]:
                        continue
                    support_b, axis_b = support(b, projected)
                    if axis_b is None or abs(np.dot(axis_a, axis_b)) < max_angle_cos:
                        continue
                    key = (a, b)
                    if key not in local_nodes:
                        common = context.nearby_nodes(a) & context.nearby_nodes(b)
                        common |= set(records[a]["node_ids"]) & set(records[b]["node_ids"])
                        local_nodes[key] = [indexes[n] for n in common]
                    if any(min(np.linalg.norm(pa - coordinates[n]), np.linalg.norm(pb - coordinates[n])) <= policy["junction_radius_m"]
                           for n in local_nodes[key]):
                        continue
                    mobile_a = [(n, w) for n, w in support_a if not fixed[n] and w > 1e-6]
                    mobile_b = [(n, w) for n, w in support_b if not fixed[n] and w > 1e-6]
                    if not mobile_a and not mobile_b:
                        continue
                    constraints += 1
                    normal = (pa - pb) / gap
                    force = normal * min(2.0, (policy["road_separation_m"] - gap) * 0.45)
                    split = 0.5 if mobile_a and mobile_b else 1.0
                    for support_nodes, sign in ((mobile_a, 1), (mobile_b, -1)):
                        for node, weight in support_nodes:
                            movements[node] += sign * force * split * weight
                            weights[node] += weight
        movable = weights > 0
        movements[movable] /= weights[movable, None]
        displacement = coordinates - original
        for node, adjacent in neighbors.items():
            if len(adjacent) == 2 and not fixed[node]:
                movements[node] += 0.08 * (np.mean(displacement[list(adjacent)], axis=0) - displacement[node])
        magnitudes = np.linalg.norm(movements, axis=1)
        large_step = magnitudes > policy["max_step_m"]
        movements[large_step] *= policy["max_step_m"] / magnitudes[large_step, None]
        proposed = coordinates + movements
        offset = proposed - original
        lengths = np.linalg.norm(offset, axis=1)
        excessive = lengths > policy["max_local_shift_m"]
        proposed[excessive] = original[excessive] + offset[excessive] * (policy["max_local_shift_m"] / lengths[excessive, None])
        proposed[fixed] = original[fixed]
        rejected_roads = set()
        for check in range(len(records) + 1):
            invalid = []
            for index, ids in enumerate(road_nodes):
                line = LineString(proposed[ids])
                if not line.is_valid or line.length <= 1e-6 or not line.is_simple or line.length > baseline_lengths[index] + max(2, baseline_lengths[index] * 0.15):
                    if np.any(proposed[ids] != coordinates[ids]):
                        invalid.append(index)
            if not invalid:
                break
            rejected_roads.update(invalid)
            for index in invalid:
                proposed[road_nodes[index]] = coordinates[road_nodes[index]]
        else:
            raise ValueError("Separation geometry guards failed to stabilize")
        max_step = float(np.max(np.linalg.norm(proposed - coordinates, axis=1)))
        coordinates = proposed
        history.append({"iteration": iteration + 1, "parallel_sample_constraints": constraints, "max_step_m": max_step,
                        "guarded_road_movements": len(rejected_roads)})
        print(f"Separation {iteration + 1}: {constraints:,} parallel samples; max step {max_step:.3f}m", flush=True)
        if constraints == 0 or max_step < 0.01:
            break
    for record, ids in zip(records, road_nodes):
        record["points_xz_m"] = [[float(x), float(-y)] for x, y in coordinates[ids]]
        record["game_length_m"] = LineString(coordinates[ids]).length
    return {"iterations": history, "maximum_local_shift_m": float(np.max(np.linalg.norm(coordinates - original, axis=1))),
            "fixed_bridge_vertices": int(np.count_nonzero(fixed))}


def polygon_record(geometry):
    return {"exterior_xz_m": [[x, -y] for x, y in geometry.exterior.coords],
            "holes_xz_m": [[[x, -y] for x, y in ring.coords] for ring in geometry.interiors], "area_m2": geometry.area}


def surface_partition(records, policy):
    lines = lines_for(records)
    by_layer = defaultdict(list)
    for index, record in enumerate(records):
        if not is_bridge(record):
            by_layer[road_layer(record)].append(index)
    surfaces, junctions, medians, stats = [], [], [], Counter()
    for layer, indexes in sorted(by_layer.items()):
        footprints = [lines[i].buffer(records[i]["width_m"] / 2, cap_style="flat", join_style="mitre", mitre_limit=2) for i in indexes]
        if policy.get("ensure_centerline_coverage", False):
            for k, i in enumerate(indexes):
                if lines[i].difference(footprints[k]).length > 1e-7:
                    xy = list(lines[i].coords)
                    atomic = [LineString([a, b]).buffer(records[i]["width_m"] / 2, cap_style="flat")
                              for a, b in zip(xy, xy[1:]) if a != b]
                    footprints[k] = footprints[k].union(unary_union(atomic))
        tree = STRtree(footprints)
        overlaps, involved = [], set()
        for a, footprint in enumerate(footprints):
            for b in tree.query(footprint, predicate="intersects"):
                b = int(b)
                if b <= a:
                    continue
                overlap = footprint.intersection(footprints[b])
                if overlap.area > 1e-6:
                    overlaps.append(overlap)
                    involved.update((indexes[a], indexes[b]))
        road_surface = unary_union(footprints)
        overlap_zone = unary_union(overlaps) if overlaps else GeometryCollection()
        opening = overlap_zone.buffer(policy["median_opening_margin_m"], join_style="round").intersection(road_surface)
        # The shared area is one physical junction, rather than stacked per-road meshes.
        raw_medians = unary_union([lines[i].buffer(records[i]["median_width_m"] / 2, cap_style="flat", join_style="mitre", mitre_limit=2)
                                  for i in indexes])
        median = raw_medians.difference(opening).intersection(road_surface)
        bodies = road_surface.difference(opening.union(median))
        for polygon in parts(bodies):
            if polygon.geom_type == "Polygon" and polygon.area > 1e-6:
                surfaces.append({"structure_profile": list(layer), **polygon_record(polygon)})
        for polygon in parts(opening):
            if polygon.geom_type == "Polygon" and polygon.area > 1e-6:
                junctions.append({"structure_profile": list(layer), **polygon_record(polygon)})
        for polygon in parts(median):
            if polygon.geom_type == "Polygon" and polygon.area > 1e-6:
                medians.append({"structure_profile": list(layer), **polygon_record(polygon)})
        if bodies.intersection(opening).area > 1e-6 or median.intersection(opening).area > 1e-6 or bodies.intersection(median).area > 1e-6:
            raise ValueError("Road body/junction partition or median openings overlap")
        if road_surface.symmetric_difference(unary_union([bodies, opening, median])).area > 1e-5:
            raise ValueError("Surface repair loses road surface coverage")
        stats["same_profile_overlap_pairs_absorbed_into_shared_surface"] += len(overlaps)
        stats["unique_road_surface_area_m2"] += road_surface.area
        stats["shared_junction_area_m2"] += opening.area
        stats["median_area_m2"] += median.area
    return surfaces, junctions, medians, dict(stats)


def draw_before_after(inputs, surfaces, junctions, medians, regions, path):
    fig, axes = plt.subplots(2, 3, figsize=(18, 12), dpi=150, layout="constrained")
    before_lines = lines_for(inputs)
    repaired = [(read_polygon(r), r["structure_profile"]) for r in surfaces + junctions]
    median_polygons = [read_polygon(r) for r in medians]
    for column, region in enumerate(regions[:3]):
        minx, miny, maxx, maxy = region["bounds_east_north_m"]
        span = max(maxx - minx, maxy - miny) + 30
        cx, cy = (minx + maxx) / 2, (miny + maxy) / 2
        def visible(poly):
            a, b, c, d = poly.bounds
            return a <= cx + span / 2 and c >= cx - span / 2 and b <= cy + span / 2 and d >= cy - span / 2
        footprints = [line.buffer(r["width_m"] / 2, cap_style="flat", join_style="mitre", mitre_limit=2)
                      for line, r in zip(before_lines, inputs) if not is_bridge(r) and visible(line)]
        axes[0, column].add_collection(PatchCollection(patches(footprints), facecolors="#b8bec5", edgecolors="none"))
        from shapely.geometry import Polygon
        shape = region["footprint_xz_m"]
        conflict = Polygon([(x, -z) for x, z in shape["exterior"]], [[(x, -z) for x, z in ring] for ring in shape["holes"]])
        axes[0, column].add_collection(PatchCollection(patches([conflict]), facecolors="#d24b4b", edgecolors="none"))
        axes[1, column].add_collection(PatchCollection(patches([poly for poly, layer in repaired if visible(poly)]),
                                                      facecolors="#7a8793", edgecolors="none"))
        axes[1, column].add_collection(PatchCollection(patches([poly for poly in median_polygons if visible(poly)]),
                                                      facecolors="#e8c775", edgecolors="none"))
        for row in range(2):
            ax = axes[row, column]
            ax.set_xlim(cx - span / 2, cx + span / 2)
            ax.set_ylim(cy - span / 2, cy + span / 2)
            ax.set_aspect("equal")
            ax.grid(alpha=0.15)
        names = ", ".join(region["road_names"][:2])
        import textwrap
        axes[0, column].set_title(f"Before · region {region['region']}\n{textwrap.fill(names, 48)}", fontsize=10)
        axes[1, column].set_title("Repaired road surface and median openings", fontsize=10)
    fig.suptitle("Ground overlap repairs · red: original conflicts · gold: repaired median\nBridge references are excluded; structure profiles retain their height-authoring metadata")
    fig.savefig(path)
    plt.close(fig)


def read_polygon(record):
    from shapely.geometry import Polygon
    return Polygon([(x, -z) for x, z in record["exterior_xz_m"]],
                   [[(x, -z) for x, z in ring] for ring in record["holes_xz_m"]])


def write_obj(records, path, audit_stats=None):
    vertex, triangle_count, omitted = 1, 0, 0
    desired_area, emitted_area = 0.0, 0.0
    with path.open("w") as handle:
        handle.write("# OpenStreetMap contributors; BBBike extract. Local X east, Z south, Y=0.\n")
        handle.write("vn 0 1 0\n")
        for record in records:
            layer = record["structure_profile"]
            handle.write(f"g layer_{layer[0]}_bridge_{layer[1]}_tunnel_{layer[2]}\n")
            polygon = read_polygon(record)
            desired_area += polygon.area
            triangles = constrained_delaunay_triangles(polygon)
            if not math.isclose(sum(t.area for t in triangles.geoms), polygon.area, abs_tol=1e-5):
                raise ValueError("Triangulation lost surface area or filled a hole")
            for triangle in triangles.geoms:
                coords = list(triangle.exterior.coords)[:3]
                if not triangle.exterior.is_ccw:
                    coords.reverse()
                # Godot meshes use float32 positions; tiny GEOS slivers can collapse during import.
                coords = np.asarray(coords, dtype=np.float32).astype(float)
                a, b, c = coords
                normal_y = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
                if normal_y <= 1e-12:
                    omitted += 1
                    continue
                emitted_area += normal_y / 2
                for x, y in coords:
                    handle.write(f"v {x:.9f} 0 {-y:.9f}\n")
                handle.write(f"f {vertex}//1 {vertex + 1}//1 {vertex + 2}//1\n")
                vertex += 3
                triangle_count += 1
    error = abs(emitted_area - desired_area)
    if error > 0.05:
        raise ValueError(f"Float32 mesh surface error exceeds 0.05m²: {error}")
    if audit_stats is not None:
        audit_stats.update({"position_precision": "float32", "omitted_degenerate_triangles": omitted,
                            "source_surface_area_m2": desired_area, "exported_triangle_area_m2": emitted_area,
                            "absolute_area_error_m2": error})
    return triangle_count


def draw_repair(records, surfaces, junctions, medians, bounds, path):
    fig, ax = plt.subplots(figsize=(14, 11), dpi=160)
    ax.add_collection(PatchCollection(patches([read_polygon(r) for r in surfaces]), facecolors="#6d7882", edgecolors="none", rasterized=True))
    ax.add_collection(PatchCollection(patches([read_polygon(r) for r in junctions]), facecolors="#7d8c96", edgecolors="none", rasterized=True))
    ax.add_collection(PatchCollection(patches([read_polygon(r) for r in medians]), facecolors="#e7ca78", edgecolors="none", rasterized=True))
    bridges = [list(line.coords) for line, r in zip(lines_for(records), records) if is_bridge(r)]
    ax.add_collection(LineCollection(bridges, colors="#8f6ba9", linewidths=1.2))
    ax.set_xlim(bounds[0], bounds[2])
    ax.set_ylim(bounds[1], bounds[3])
    ax.set_aspect("equal")
    ax.set_xlabel("Game metres east of area center")
    ax.set_ylabel("Game metres north of area center")
    ax.set_title("Bengaluru repaired road surfaces · 1:20 · two lanes each way\nGrey: road surface · gold: median with junction openings · purple: bridge reference")
    ax.grid(alpha=0.15)
    fig.tight_layout()
    fig.savefig(path)
    plt.close(fig)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("folder", type=Path)
    parser.add_argument("--recipe", type=Path, default=ROOT / "tools/city/bengaluru_backbone.json")
    args = parser.parse_args()
    folder = args.folder.resolve(strict=True)
    source_bytes = (folder / "roads.json").read_bytes()
    data = json.loads(source_bytes)
    records = data["roads"]
    if data["metadata"]["format"] != "dual-carriageway-layout-v1":
        raise ValueError("Expected the approved dual-carriageway layout")
    recipe = json.loads(args.recipe.read_text())
    policy = recipe["overlap_repair"]
    merge_policy = {**recipe["dual_carriageway_layout"], "match_axes_only": True,
                    "max_pair_gap_m": policy["duplicate_gap_m"], "max_cluster_span_m": policy["duplicate_gap_m"] + 0.25}
    for key, value in policy.items():
        if isinstance(value, (int, float)) and (not math.isfinite(value) or value <= 0):
            raise ValueError(f"Invalid repair policy {key}")
    with (folder / "overlaps.csv").open() as handle:
        rows = list(csv.DictReader(handle))
    counts = Counter(row["category"] for row in rows)
    if counts != data["metadata"]["overlaps"]["pairs_by_category"]:
        raise ValueError("Incomplete layout audit")
    for row in rows:
        a, b = int(row["road_a"]), int(row["road_b"])
        if not 0 <= a < b < len(records) or records[a]["osm_id"] != row["osm_id_a"] or records[b]["osm_id"] != row["osm_id_b"]:
            raise ValueError("Overlap indexes differ from input layout")
    fixed_points = {tuple(round(v, 6) for v in (x, -z)) for r in records if is_bridge(r) for x, z in r["points_xz_m"]}
    accepted = duplicate_pairs(records, lines_for(records), rows, merge_policy)
    print(f"Repairing {len(accepted):,} additional duplicate corridor pairs", flush=True)
    corridors, consolidation = consolidate(records, lines_for(records), accepted, merge_policy, fixed_points)
    consolidation.pop("_vertex_pair_dependencies")
    carry_provenance(corridors, records)
    separation = separate_parallel_roads(corridors, fixed_points, policy)
    if any(not line.is_simple or not line.is_valid or line.length <= 0 for line in lines_for(corridors)):
        raise ValueError("Repair introduced an invalid or self-crossing centerline")
    groups = connected_components([set(r["node_ids"]) for r in corridors])
    if len(groups) != 1:
        raise ValueError("Repair disconnected the road graph")
    original_bridges = defaultdict(list)
    repaired_bridges = defaultdict(list)
    for r, line in zip(records, lines_for(records)):
        if is_bridge(r):
            original_bridges[road_layer(r)].append(line)
    for r, line in zip(corridors, lines_for(corridors)):
        if is_bridge(r):
            repaired_bridges[road_layer(r)].append(line)
    if set(original_bridges) != set(repaired_bridges):
        raise ValueError("Repair removed a bridge structure profile")
    for layer, lines in original_bridges.items():
        original, repaired = unary_union(lines), unary_union(repaired_bridges[layer])
        if original.hausdorff_distance(repaired) > 1e-7 or abs(original.length - repaired.length) > 1e-6:
            raise ValueError("Repair altered bridge geometry")
    surfaces, junctions, medians, surface_stats = surface_partition(corridors, policy)
    output = folder / "repaired"
    output.mkdir(exist_ok=True)
    metadata = {**data["metadata"], "format": "repaired-city-layout-v1", "corridors": len(corridors),
                "layout_centerline_length_km": sum(r["game_length_m"] for r in corridors) / 1000,
                "input_layout_sha256": hashlib.sha256(source_bytes).hexdigest(), "repair_policy": policy,
                "repair": {"additional_duplicate_pairs": len(accepted), "consolidation": consolidation,
                           "parallel_separation": separation, "surface_partition": surface_stats,
                           "connected_components": len(groups), "bridge_geometry_preserved": True,
                           "same_profile_surface_interior_overlap_m2": 0,
                           "median_intrusion_into_shared_junctions_m2": 0},
                "notes": ["Same-profile road surface is welded and split into disjoint body/junction polygons; intentional centerline footprint intersections remain at shared junctions.",
                          "Medians stop at shared surfaces. Two lanes each way describe the corridor cross section outside junction openings.",
                          "Bridge geometry is unchanged. Different structure profiles stay separate and require elevations during scene baking.",
                          "No lane turning graph or bridge/tunnel heights are authored by this repair."]}
    metadata.pop("overlaps", None)
    metadata.pop("geometry_checks", None)
    (output / "surfaces.json").write_text(json.dumps({"axes": "X east, Z south; no elevations", "road_bodies": surfaces,
                                                    "junctions": junctions, "medians": medians}, separators=(",", ":")))
    # OBJ previews contain only ground-level, non-tunnel road surface; other profiles retain their polygons for later height authoring.
    ground = lambda r: r["structure_profile"] == ["0", "no", "no"]
    metadata["repair"]["mesh_export_checks"] = {}
    for name, values, key in (("ground_road_surface", surfaces + junctions, "ground_road_surface_triangles"),
                              ("ground_medians", medians, "ground_median_triangles")):
        checks = {}
        metadata["repair"][key] = write_obj([r for r in values if ground(r)], output / f"{name}.obj", checks)
        metadata["repair"]["mesh_export_checks"][name] = checks
    (output / "roads.json").write_text(json.dumps({"metadata": metadata, "roads": corridors}, separators=(",", ":")))
    (output / "report.json").write_text(json.dumps(metadata, indent=2) + "\n")
    draw_repair(corridors, surfaces, junctions, medians, metadata["game_rectangle_bounds_east_north_m"], output / "overview.png")
    review_path = folder / "overlap-review/outside_junction_regions.json"
    if review_path.exists():
        draw_before_after(records, surfaces, junctions, medians, json.loads(review_path.read_text())["regions"], output / "before_after.png")
    print(json.dumps({"corridors": len(corridors), "length_km": metadata["layout_centerline_length_km"],
                      "duplicate_pairs": len(accepted), "surfaces": surface_stats,
                      "bridge_geometry_preserved": True, "connected_components": len(groups)}, indent=2))


if __name__ == "__main__":
    main()
