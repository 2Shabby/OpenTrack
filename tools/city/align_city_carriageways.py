#!/usr/bin/env python3
"""Consolidate matching carriageways and generate the offline dual-carriageway layout."""
import argparse
import bisect
import csv
import json
import math
from collections import Counter, defaultdict, deque
from pathlib import Path

import matplotlib.pyplot as plt
from matplotlib.collections import LineCollection, PatchCollection
from matplotlib.patches import Polygon as PlotPolygon
from shapely.affinity import scale
from shapely.geometry import LineString, Point
from shapely.strtree import STRtree

from audit_city_roads import ROOT, HIGHWAY_COLORS, audit, connected_components, parts, road_layer
from review_city_backbone import geometry_digest, load_export, other_checks

FAMILY_PRIORITY = {"motorway": 0, "trunk": 1, "primary": 2, "secondary": 3}

def family(record):
    return record["highway"].split("_")[0]


def direction(record):
    value = record["tags"].get("oneway", "").lower()
    return -1 if value == "-1" else 1 if value in {"yes", "1", "true"} else 0


def tangent(line, distance):
    delta = min(0.15, line.length / 4)
    a = line.interpolate(max(0, distance - delta))
    b = line.interpolate(min(line.length, distance + delta))
    dx, dy = b.x - a.x, b.y - a.y
    length = math.hypot(dx, dy)
    return (dx / length, dy / length) if length > 1e-12 else None


def matching_position(a, b, distance, direction_a, direction_b, policy):
    point = a.interpolate(distance)
    target = b.project(point)
    projected = b.interpolate(target)
    if point.distance(projected) > policy["max_pair_gap_m"]:
        return None
    va, vb = tangent(a, distance), tangent(b, target)
    if va is None or vb is None:
        return None
    dot = sum(x * y for x, y in zip(va, vb))
    if policy.get("match_axes_only", False):
        if abs(dot) < math.cos(math.radians(policy["max_axis_angle_deg"])):
            return None
    elif dot * direction_a * direction_b > -math.cos(math.radians(policy["max_axis_angle_deg"])):
        return None
    # Endpoint caps and curved approaches must not pull whole longitudinal runs together.
    if abs(a.project(projected) - distance) > policy["projection_tolerance_m"]:
        return None
    return target


def select_pairs(records, lines, candidates, policy):
    accepted, decisions = [], []
    for i, j in sorted(set(candidates)):
        a, b = records[i], records[j]
        reason, coverage = "accepted", 0.0
        if road_layer(a) != road_layer(b):
            reason = "different_structure"
        elif family(a) != family(b):
            reason = "different_road_family"
        elif not direction(a) or not direction(b):
            reason = "oneway_pair_not_confirmed"
        elif a.get("name") and b.get("name") and a["name"].strip().casefold() != b["name"].strip().casefold():
            reason = "different_names"
        elif lines[i].distance(lines[j]) > policy["max_pair_gap_m"]:
            reason = "gap_exceeds_limit"
        else:
            short, long = (i, j) if lines[i].length <= lines[j].length else (j, i)
            count = max(4, math.ceil(lines[short].length / policy["sample_step_m"]))
            matches = sum(matching_position(lines[short], lines[long], lines[short].length * (k + 0.5) / count,
                                            direction(records[short]), direction(records[long]), policy) is not None
                          for k in range(count))
            coverage = matches / count
            if coverage < policy["min_shorter_road_coverage"]:
                reason = "insufficient_opposing_alignment"
            else:
                accepted.append((i, j))
        decisions.append({"road_a": i, "road_b": j, "osm_id_a": a["osm_id"], "osm_id_b": b["osm_id"],
                          "decision": reason, "shorter_road_matching_fraction": round(coverage, 6)})
    return accepted, decisions


class Nodes:
    def __init__(self, max_span, fixed_points=()):
        self.parent, self.sums, self.counts, self.bounds = [], [], [], []
        self.max_span = max_span
        self.fixed_points = set(fixed_points)
        self.fixed = {}

    def add(self, coord):
        index = len(self.parent)
        self.parent.append(index)
        self.sums.append(list(coord))
        self.counts.append(1)
        self.bounds.append([*coord, *coord])
        if tuple(round(v, 6) for v in coord) in self.fixed_points:
            self.fixed[index] = tuple(coord)
        return index

    def root(self, index):
        while self.parent[index] != index:
            self.parent[index] = self.parent[self.parent[index]]
            index = self.parent[index]
        return index

    def join(self, a, b):
        a, b = self.root(a), self.root(b)
        if a == b:
            return True
        if a in self.fixed and b in self.fixed and self.fixed[a] != self.fixed[b]:
            return False
        low_a, low_b = self.bounds[a], self.bounds[b]
        bounds = [min(low_a[0], low_b[0]), min(low_a[1], low_b[1]),
                  max(low_a[2], low_b[2]), max(low_a[3], low_b[3])]
        if math.hypot(bounds[2] - bounds[0], bounds[3] - bounds[1]) > self.max_span:
            return False
        if self.counts[a] < self.counts[b]:
            a, b = b, a
        self.parent[b] = a
        self.sums[a] = [x + y for x, y in zip(self.sums[a], self.sums[b])]
        self.counts[a] += self.counts[b]
        self.bounds[a] = bounds
        if b in self.fixed:
            self.fixed[a] = self.fixed[b]
        return True

    def coord(self, index):
        root = self.root(index)
        if root in self.fixed:
            return self.fixed[root]
        return tuple(value / self.counts[root] for value in self.sums[root])


def consolidate(records, lines, accepted, policy, fixed_points=()):
    nodes = Nodes(policy["max_cluster_span_m"], fixed_points)
    originals, cuts, original_coords = {}, [], {}
    for line in lines:
        distances, ids, distance = [], [], 0.0
        previous = None
        for point in line.coords:
            if previous is not None:
                distance += math.dist(previous, point)
            key = tuple(round(value, 6) for value in point)
            if key not in originals:
                originals[key] = nodes.add(point)
                original_coords[originals[key]] = point
            distances.append(distance)
            ids.append(originals[key])
            previous = point
        cuts.append([distances, ids])

    def cut(road, distance):
        distances, ids = cuts[road]
        pos = bisect.bisect_left(distances, distance)
        near = [k for k in (pos - 1, pos) if 0 <= k < len(ids)]
        nearest = min(near, key=lambda k: abs(distances[k] - distance))
        if abs(distances[nearest] - distance) <= policy["cut_snap_m"]:
            return ids[nearest]
        point = lines[road].interpolate(distance)
        node = nodes.add((point.x, point.y))
        distances.insert(pos, distance)
        ids.insert(pos, node)
        return node

    joins, rejected, join_dependencies = 0, 0, []
    for pair_index, (i, j) in enumerate(accepted):
        # Split both carriageways at each other's vertices, accommodating different OSM segmentation.
        for a, b in ((i, j), (j, i)):
            for distance, node in list(zip(*cuts[a])):
                target = matching_position(lines[a], lines[b], distance,
                                           direction(records[a]), direction(records[b]), policy)
                if target is None:
                    continue
                other = cut(b, target)
                if nodes.join(node, other):
                    joins += 1
                    join_dependencies.append((node, (i, j)))
                else:
                    rejected += 1
        if (pair_index + 1) % 1000 == 0:
            print(f"Aligned {pair_index + 1:,}/{len(accepted):,} candidate pairs", flush=True)

    edges, collapsed = {}, 0
    for index, (_, ids) in enumerate(cuts):
        layer = road_layer(records[index])
        for a, b in zip(ids, ids[1:]):
            a, b = nodes.root(a), nodes.root(b)
            if a == b:
                collapsed += 1
                continue
            key = (min(a, b), max(a, b), layer)
            if key not in edges:
                edges[key] = {"a": a, "b": b, "layer": layer, "family": family(records[index]), "sources": set()}
            edges[key]["sources"].add(index)
            if FAMILY_PRIORITY[family(records[index])] < FAMILY_PRIORITY[edges[key]["family"]]:
                edges[key]["family"] = family(records[index])
    represented_before_pruning = {i for e in edges.values() for i in e["sources"]}
    collapsed_sources = sorted(set(range(len(records))) - represented_before_pruning)
    edges = list(edges.values())
    pruned_edges = set()
    if policy.get("prune_alignment_dead_ends", False):
        incident = defaultdict(set)
        for index, edge in enumerate(edges):
            incident[edge["a"]].add(index)
            incident[edge["b"]].add(index)
        queue = deque(node for node, ids in incident.items() if len(ids) == 1)
        while queue:
            node = queue.popleft()
            if len(incident[node]) != 1:
                continue
            index = next(iter(incident[node]))
            pruned_edges.add(index)
            edge = edges[index]
            for endpoint in (edge["a"], edge["b"]):
                incident[endpoint].discard(index)
                if len(incident[endpoint]) == 1:
                    queue.append(endpoint)
    pruned_length = sum(math.dist(nodes.coord(edges[i]["a"]), nodes.coord(edges[i]["b"])) for i in pruned_edges)
    edges = [edge for i, edge in enumerate(edges) if i not in pruned_edges]
    if not edges:
        raise ValueError("Carriageway consolidation leaves no cycle-containing road network")
    adjacency = defaultdict(list)
    for index, edge in enumerate(edges):
        adjacency[edge["a"]].append(index)
        adjacency[edge["b"]].append(index)
    visited, corridors = set(), []

    def compatible(edge, other):
        return edge["layer"] == other["layer"] and edge["family"] == other["family"]

    def boundary(node, edge):
        return len(adjacency[node]) != 2 or not all(compatible(edge, edges[e]) for e in adjacency[node])

    def walk(first, start):
        path, sources, index, node = [start], set(), first, start
        while index not in visited:
            visited.add(index)
            edge = edges[index]
            sources.update(edge["sources"])
            node = edge["b"] if node == edge["a"] else edge["a"]
            path.append(node)
            if boundary(node, edge):
                break
            choices = [e for e in adjacency[node] if e not in visited]
            if not choices:
                break
            index = choices[0]
        coords = [nodes.coord(n) for n in path]
        line = LineString(coords)
        if line.length <= 0 or not line.is_valid:
            raise ValueError("Alignment generated an invalid corridor")
        edge = edges[first]
        layer, bridge, tunnel = edge["layer"]
        corridors.append({"osm_id": f"corridor-{len(corridors):05d}", "highway": edge["family"],
                          "name": None, "names": sorted({records[i]["name"] for i in sources if records[i].get("name")}),
                          "tags": {"layer": layer, "bridge": bridge, "tunnel": tunnel},
                          "source_road_indexes": sorted(sources),
                          "source_osm_ids": sorted({records[i]["osm_id"] for i in sources}),
                          "points_xz_m": [[x, -y] for x, y in coords], "node_ids": path,
                          "game_length_m": line.length})

    for index, edge in enumerate(edges):
        if index in visited:
            continue
        for node in (edge["a"], edge["b"]):
            if boundary(node, edge):
                walk(index, node)
                break
    for index, edge in enumerate(edges):
        if index not in visited:
            walk(index, edge["a"])
    represented = {i for c in corridors for i in c["source_road_indexes"]}
    pruned_sources = sorted(represented_before_pruning - represented)
    displacement = [math.dist(coord, nodes.coord(n)) for n, coord in original_coords.items()]
    dependencies = defaultdict(set)
    for node, pair in join_dependencies:
        dependencies[nodes.root(node)].add(pair)
    return corridors, {
        "_vertex_pair_dependencies": dependencies,
        "successful_vertex_matches": joins, "cluster_span_rejections": rejected,
        "collapsed_zero_length_segments": collapsed,
        "source_parts_collapsed_into_junctions": collapsed_sources,
        "source_parts_removed_by_alignment_pruning": pruned_sources,
        "alignment_pruned_graph_edges": len(pruned_edges), "alignment_pruned_length_m": pruned_length,
        "unique_graph_edges": len(edges), "graph_vertices": len(adjacency),
        "degree_one_vertices": sum(len(edges_at_node) == 1 for edges_at_node in adjacency.values()),
        "maximum_original_vertex_displacement_m": max(displacement, default=0),
        "mean_original_vertex_displacement_m": sum(displacement) / max(1, len(displacement)),
    }


def unconnected_crossings(corridors):
    lines = [LineString([(x, -z) for x, z in c["points_xz_m"]]) for c in corridors]
    tree, crossings = STRtree(lines), []
    for i, line in enumerate(lines):
        for j in tree.query(line, predicate="intersects"):
            j = int(j)
            if j <= i or road_layer(corridors[i]) != road_layer(corridors[j]):
                continue
            if set(corridors[i]["node_ids"]) & set(corridors[j]["node_ids"]):
                continue
            crossings.append((i, j, line.intersection(lines[j])))
    return crossings


def align_preserving_junctions(records, lines, accepted, policy):
    crossing_rejections = set()
    for attempt in range(10):
        corridors, alignment = consolidate(records, lines, accepted, policy)
        crossings = unconnected_crossings(corridors)
        if not crossings:
            alignment.pop("_vertex_pair_dependencies")
            return corridors, alignment, accepted, crossing_rejections
        local_rejections = set()
        for a, b, crossing in crossings:
            nearby = {i for c in (corridors[a], corridors[b]) for i in c["source_road_indexes"]
                      if lines[i].distance(crossing) <= policy["max_cluster_span_m"]}
            local_rejections.update((i, j) for i, j in accepted if i in nearby or j in nearby)
            for corridor in (corridors[a], corridors[b]):
                for node, (x, z) in zip(corridor["node_ids"], corridor["points_xz_m"]):
                    if Point(x, -z).distance(crossing) <= policy["max_cluster_span_m"]:
                        local_rejections.update(alignment["_vertex_pair_dependencies"].get(node, ()))
        if not local_rejections:
            raise ValueError("Unconnected crossing cannot be traced to a carriageway alignment")
        crossing_rejections.update(local_rejections)
        accepted = [(i, j) for i, j in accepted if (i, j) not in local_rejections]
        print(f"Junction check {attempt + 1}: deferring {len(local_rejections)} local pairs to avoid {len(crossings)} crossings", flush=True)
    raise ValueError("Junction alignment did not converge; no layout published")


def draw_layout(corridors, lines, bounds, output, width, median, highlights=None):
    fig, ax = plt.subplots(figsize=(14, 11), dpi=160)
    surfaces = [p for line in lines for p in parts(line.buffer(width / 2, cap_style="flat", join_style="mitre", mitre_limit=2))
                if p.geom_type == "Polygon"]
    ax.add_collection(PatchCollection([PlotPolygon(p.exterior.coords) for p in surfaces],
                                     facecolors="#555f69", edgecolors="none", alpha=0.8, rasterized=True))
    ax.add_collection(LineCollection([list(line.coords) for line in lines],
                                     colors=[HIGHWAY_COLORS[c["highway"]] for c in corridors], linewidths=0.6))
    if highlights:
        ax.add_collection(PatchCollection([PlotPolygon(p.exterior.coords) for p in highlights],
                                         facecolors="#e54646", edgecolors="none", alpha=0.65))
    ax.set_xlim(bounds[0], bounds[2])
    ax.set_ylim(bounds[1], bounds[3])
    ax.set_aspect("equal")
    ax.set_xlabel("Game metres east of area center")
    ax.set_ylabel("Game metres north of area center")
    ax.set_title(f"Bengaluru dual-carriageway layout · 1:20\n"
                 f"{len(corridors):,} corridors · {width:g} m total width · {median:g} m median"
                 + (" · red: unconnected surface conflicts" if highlights is not None else ""))
    ax.grid(alpha=0.15)
    fig.tight_layout()
    fig.savefig(output)
    plt.close(fig)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("folder", type=Path)
    parser.add_argument("--recipe", type=Path, default=ROOT / "tools/city/bengaluru_backbone.json")
    args = parser.parse_args()
    recipe = json.loads(args.recipe.read_text())
    policy = recipe["dual_carriageway_layout"]
    for key in ("lanes_per_direction", "lane_width_m", "median_width_m", "max_pair_gap_m", "max_axis_angle_deg",
                "sample_step_m", "projection_tolerance_m", "cut_snap_m", "max_cluster_span_m"):
        if not math.isfinite(policy[key]) or policy[key] <= 0:
            raise ValueError(f"Invalid dual-carriageway policy: {key}")
    if not isinstance(policy["lanes_per_direction"], int) or not 0 < policy["min_shorter_road_coverage"] <= 1:
        raise ValueError("Invalid lane count or match coverage")
    folder = args.folder.resolve(strict=True)
    data, lines, _, _ = load_export(folder, recipe)
    records = data["roads"]
    if len(records) != recipe["expected_road_parts"] or geometry_digest(records) != recipe["geometry_sha256"]:
        raise ValueError("Input differs from locked source backbone")
    candidates, input_categories = [], Counter()
    with (folder / "overlaps.csv").open() as handle:
        for row in csv.DictReader(handle):
            input_categories[row["category"]] += 1
            if row["overlap_kind"] == "parallel_or_aligned_candidate":
                i, j = int(row["road_a"]), int(row["road_b"])
                if not 0 <= i < j < len(records) or records[i]["osm_id"] != row["osm_id_a"] or records[j]["osm_id"] != row["osm_id_b"]:
                    raise ValueError("Overlap indexes differ from the locked input")
                candidates.append((i, j))
    if input_categories != Counter(data["metadata"]["overlaps"]["pairs_by_category"]):
        raise ValueError("Overlap CSV is incomplete or differs from the locked reference audit")
    accepted, decisions = select_pairs(records, lines, candidates, policy)
    print(f"Matching carriageways: {len(accepted):,}/{len(candidates):,} aligned candidates", flush=True)
    output = folder / "dual-carriageway"
    output.mkdir(exist_ok=True)
    initial_accepted = len(accepted)
    corridors, alignment, accepted, crossing_rejections = align_preserving_junctions(records, lines, accepted, policy)
    for decision in decisions:
        if (decision["road_a"], decision["road_b"]) in crossing_rejections:
            decision["decision"] = "deferred_to_preserve_junction_geometry"
    with (output / "alignment_decisions.csv").open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=list(decisions[0]))
        writer.writeheader()
        writer.writerows(decisions)
    carriageway_width = policy["lanes_per_direction"] * policy["lane_width_m"]
    width = carriageway_width * 2 + policy["median_width_m"]
    lane_offsets = [policy["median_width_m"] / 2 + (lane + 0.5) * policy["lane_width_m"]
                    for lane in range(policy["lanes_per_direction"])]
    for corridor in corridors:
        corridor.update({"width_m": width, "lanes_per_direction": policy["lanes_per_direction"],
                         "lane_width_m": policy["lane_width_m"], "median_width_m": policy["median_width_m"],
                         "carriageway_width_m": carriageway_width,
                         "lane_center_offsets_from_median_m": {"left": lane_offsets, "right": [-v for v in lane_offsets]},
                         "offset_convention": "positive: left of forward polyline in east/north coordinates",
                         "lane_travel_direction": {"left": "forward", "right": "reverse"},
                         "traffic_side": "left", "source_tags_are_driving_rules": False})
    game_lines = [LineString([(x, -z) for x, z in c["points_xz_m"]]) for c in corridors]
    node_sets = [set(c["node_ids"]) for c in corridors]
    components = connected_components(node_sets)
    if len(components) != 1:
        raise ValueError(f"Alignment broke connectivity: {len(components)} components")
    report = {"source_geometry_sha256": recipe["geometry_sha256"], "compression": recipe["compression"],
              "format": "dual-carriageway-layout-v1",
              **{key: data["metadata"][key] for key in ("source_bounds_lon_lat", "projected_crs", "projected_origin_m",
                                                       "game_rectangle_bounds_east_north_m")},
              "source_road_parts": len(records), "corridors": len(corridors), "connected_components": len(components),
              "source_game_centerline_length_km": sum(g.length for g in lines) / 1000,
              "layout_centerline_length_km": sum(g.length for g in game_lines) / 1000,
              "policy": policy, "total_road_width_m": width,
              "initial_matching_pairs": initial_accepted, "final_matching_pairs": len(accepted),
              "candidate_pair_decisions": dict(Counter(d["decision"] for d in decisions)), "alignment": alignment,
              "coordinate_axes": "X east, Z south; no elevations", "attribution": data["metadata"]["attribution"],
              "notes": ["All layout corridors have two-way traffic separated by a median; source one-way tags are provenance only.",
                        "Only confirmed opposing one-way pairs are consolidated. Uncertain parallel roads remain separate.",
                        "Matching carriageways share graph vertices; existing junctions follow those vertices.",
                        "Layer, bridge and tunnel distinctions are retained; elevations and junction meshes are not generated.",
                        "Lane offsets describe a cross section. Tight curves and widened junctions still require mesh design."]}
    baseline = [scale(g, recipe["compression"], recipe["compression"], origin=(0, 0)) for g in game_lines]
    _, highlights, overlaps = audit(corridors, baseline, game_lines, node_sets, output, diagnostics=True,
                                    junction_category="shared_layout_junction")
    overlaps["centerline_crossings_without_shared_layout_vertex_by_category"] = overlaps.pop(
        "centerline_crossings_without_shared_source_vertex_by_category")
    overlaps["junction_semantics"] = "shared_layout_junction means a shared consolidated layout vertex"
    overlaps["baseline_semantics"] = f"layout centerlines expanded {recipe['compression']:g}x with {width:g}m widths, not the untouched source geometry"
    overlaps["notes"] = [note for note in overlaps["notes"] if not note.startswith("No junction repair")]
    report["overlaps"] = overlaps
    report["geometry_checks"] = other_checks(corridors, game_lines, output)
    (output / "roads.json").write_text(json.dumps({"metadata": report, "roads": corridors}, separators=(",", ":")))
    (output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    bounds = data["metadata"]["game_rectangle_bounds_east_north_m"]
    draw_layout(corridors, game_lines, bounds, output / "overview.png", width, policy["median_width_m"])
    draw_layout(corridors, game_lines, bounds, output / "overlap_overview.png", width, policy["median_width_m"], highlights)
    print(json.dumps({key: report[key] for key in ("corridors", "connected_components", "layout_centerline_length_km",
                                                  "final_matching_pairs", "candidate_pair_decisions")}, indent=2))


if __name__ == "__main__":
    main()
