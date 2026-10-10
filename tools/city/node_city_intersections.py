#!/usr/bin/env python3
"""Split same-profile road intersections and review branches on the resulting graph."""
import argparse
import hashlib
import json
import math
from collections import Counter, defaultdict
from pathlib import Path

import matplotlib.pyplot as plt
from matplotlib.collections import LineCollection
from shapely.geometry import LineString, Point, Polygon
from shapely.ops import unary_union
from shapely.strtree import STRtree

from audit_city_roads import connected_components, parts, road_layer
from repair_city_overlaps import lines_for
from review_city_overlaps import is_bridge


def coordinate_key(point):
    return tuple(round(v, 8) for v in point)


def node_roads(records):
    originals = lines_for(records)
    profiles, neighbours, positions = defaultdict(set), defaultdict(set), {}
    for record in records:
        layer = road_layer(record)
        for node, (x, z) in zip(record["node_ids"], record["points_xz_m"]):
            xy = (x, -z)
            if node in positions and positions[node] != xy:
                raise ValueError("Shared input node has inconsistent coordinates")
            positions[node] = xy
            profiles[node].add(layer)
        for a, b in zip(record["node_ids"], record["node_ids"][1:]):
            if a != b:
                neighbours[a].add(b)
                neighbours[b].add(a)
    interfaces = {coordinate_key(positions[n]): profiles[n] for n in profiles if len(profiles[n]) > 1}
    forced = {n for n in profiles if len(profiles[n]) > 1 or len(neighbours[n]) != 2}
    grouped, source_indexes = defaultdict(list), defaultdict(list)
    for index, (record, line) in enumerate(zip(records, originals)):
        layer = road_layer(record)
        source_indexes[layer].append(index)
        start = 0
        for end in range(1, len(record["node_ids"])):
            if end == len(record["node_ids"]) - 1 or record["node_ids"][end] in forced:
                piece = LineString(list(line.coords)[start:end + 1])
                if piece.length > 0:
                    grouped[layer].append(piece)
                start = end
    nodes, coordinates, output, skipped = {}, {}, [], []

    def key(layer, point):
        xy = coordinate_key(point)
        return ("interface", xy) if layer in interfaces.get(xy, ()) else (layer, xy)

    # Keep existing bridge and grade-transition positions exact when registering shared endpoints.
    pinned = {}
    for record, line in zip(records, originals):
        for node, xy in zip(record["node_ids"], line.coords):
            if is_bridge(record) or len(profiles[node]) > 1:
                pinned[key(road_layer(record), xy)] = tuple(xy)

    def register(layer, point):
        node_key = key(layer, point)
        if node_key not in nodes:
            nodes[node_key] = len(nodes)
            coordinates[nodes[node_key]] = pinned.get(node_key, tuple(point))
        return nodes[node_key]

    for layer in sorted(grouped):
        indexes = source_indexes[layer]
        source_lines = [originals[i] for i in indexes]
        tree = STRtree(source_lines)
        noded = sorted(parts(unary_union(grouped[layer])), key=lambda g: (g.bounds, tuple(g.coords)))
        for line in noded:
            if line.geom_type != "LineString":
                raise ValueError("Noding produced non-line geometry")
            ids = [register(layer, xy) for xy in line.coords]
            points = [coordinates[n] for n in ids]
            geometry = LineString(points)
            if geometry.length <= 1e-8:
                skipped.append(line.length)
                continue
            contributors = []
            for i in tree.query(line.buffer(1e-7), predicate="intersects"):
                i = int(i)
                if line.difference(source_lines[i].buffer(1e-7)).length <= 1e-7:
                    contributors.append(indexes[i])
            if not contributors:
                raise ValueError("A noded segment lost its input-road provenance")
            contributors.sort()
            seed = records[contributors[0]]
            record = {**seed, "osm_id": f"segment-{len(output):05d}", "node_ids": ids,
                      "points_xz_m": [[x, -y] for x, y in points], "game_length_m": geometry.length,
                      "layout_seed_corridor_ids": [records[i]["osm_id"] for i in contributors]}
            for field in ("names", "source_osm_ids", "source_road_indexes", "input_corridor_ids"):
                record[field] = sorted({value for i in contributors for value in records[i].get(field, [])})
            output.append(record)
    coverage = {}
    for layer, inputs in grouped.items():
        before = unary_union(inputs)
        after = unary_union([line for r, line in zip(output, lines_for(output)) if road_layer(r) == layer])
        distance = before.hausdorff_distance(after)
        length_error = abs(before.length - after.length)
        if distance > 1e-7 or length_error > 1e-5:
            raise ValueError(f"Intersection splitting changed {layer} geometry: {distance}, {length_error}")
        coverage[str(layer)] = {"hausdorff_distance_m": distance, "unique_length_error_m": length_error}
    return output, {"input_corridors": len(records), "output_segments": len(output),
                    "original_grade_interfaces": len(interfaces), "coverage_checks": coverage,
                    "omitted_sub_nanometre_segments": len(skipped), "omitted_length_m": sum(skipped)}


def endpoint_graph(records):
    graph, coordinates = defaultdict(list), {}
    for index, record in enumerate(records):
        a, b = record["node_ids"][0], record["node_ids"][-1]
        graph[a].append(index)
        graph[b].append(index)
        for node, (x, z) in zip(record["node_ids"], record["points_xz_m"]):
            coordinates[node] = (x, -z)
    return graph, coordinates


def other_end(record, node):
    a, b = record["node_ids"][0], record["node_ids"][-1]
    return b if node == a else a


def biconnected_blocks(records, graph):
    discovery, low, parent_edge, stack, blocks = {}, {}, {}, [], []
    handled_loops = set()
    for root in sorted(graph):
        if root in discovery:
            continue
        discovery[root] = low[root] = len(discovery)
        frames = [(root, iter(graph[root]))]
        while frames:
            node, edges = frames[-1]
            edge = next(edges, None)
            if edge is None:
                frames.pop()
                if node in parent_edge:
                    incoming = parent_edge[node]
                    parent = other_end(records[incoming], node)
                    low[parent] = min(low[parent], low[node])
                    if low[node] >= discovery[parent]:
                        block = []
                        while stack:
                            popped = stack.pop()
                            block.append(popped)
                            if popped == incoming:
                                break
                        blocks.append(sorted(block))
                continue
            other = other_end(records[edge], node)
            if other == node:
                if edge not in handled_loops:
                    blocks.append([edge])
                    handled_loops.add(edge)
            elif edge == parent_edge.get(node):
                continue
            elif other not in discovery:
                stack.append(edge)
                parent_edge[other] = edge
                discovery[other] = low[other] = len(discovery)
                frames.append((other, iter(graph[other])))
            elif discovery[other] < discovery[node]:
                stack.append(edge)
                low[node] = min(low[node], discovery[other])
    if sorted(edge for block in blocks for edge in block) != list(range(len(records))):
        raise ValueError("Biconnected review did not account for every segment exactly once")
    return blocks


def dangling_branches(records):
    graph, coordinates = endpoint_graph(records)
    lines = lines_for(records)
    results, seen = [], set()

    def add(kind, edges, attachment, tip=None):
        edge_key = tuple(sorted(edges))
        if edge_key in seen:
            return
        seen.add(edge_key)
        geometry = unary_union([lines[i] for i in edges])
        root = Point(coordinates[attachment])
        furthest = max((Point(x, y).distance(root), (x, y)) for line in parts(geometry) for x, y in line.coords)
        profiles = sorted({road_layer(records[i]) for i in edges})
        structures = any(is_bridge(records[i]) or road_layer(records[i]) != ("0", "no", "no") for i in edges)
        results.append({"kind": kind, "segment_indexes": sorted(edges), "attachment_node": attachment,
                        "attachment_xz_m": [root.x, -root.y], "tip_node": tip,
                        "furthest_xz_m": [furthest[1][0], -furthest[1][1]], "reach_m": furthest[0],
                        "centerline_length_m": sum(records[i]["game_length_m"] for i in edges),
                        "structure_profiles": [list(p) for p in profiles], "has_structure_transition": structures,
                        "road_names": sorted({name for i in edges for name in records[i]["names"]})})

    for tip in sorted(graph):
        if len(graph[tip]) != 1:
            continue
        node, edges = tip, []
        while True:
            outgoing = [i for i in graph[node] if i not in edges]
            if not outgoing:
                break
            edge = outgoing[0]
            edges.append(edge)
            node = other_end(records[edge], node)
            if len(graph[node]) != 2:
                break
        if edges:
            add("degree_one_tail", edges, node, tip)

    blocks = biconnected_blocks(records, graph)
    membership = defaultdict(set)
    for index, edges in enumerate(blocks):
        for edge in edges:
            for node in (records[edge]["node_ids"][0], records[edge]["node_ids"][-1]):
                membership[node].add(index)
    for edges in blocks:
        vertices = {n for i in edges for n in (records[i]["node_ids"][0], records[i]["node_ids"][-1])}
        attachments = [n for n in vertices if len(membership[n]) > 1]
        if len(edges) < len(vertices) or len(attachments) != 1:
            continue
        attachment, edge_set = attachments[0], set(edges)
        # Follow the stem back to the network instead of stopping at the loop's return point.
        while True:
            outside = [i for i in graph[attachment] if i not in edge_set]
            if len(outside) != 1:
                break
            edge = outside[0]
            edge_set.add(edge)
            attachment = other_end(records[edge], attachment)
            if len(graph[attachment]) != 2:
                break
        footprint = unary_union([lines[i].buffer(records[i]["width_m"] / 2, cap_style="flat", join_style="mitre", mitre_limit=2) for i in edge_set])
        clear_loop_area = sum(abs(Polygon(ring).area) for p in parts(footprint) if p.geom_type == "Polygon" for ring in p.interiors)
        kind = "collapsed_return_tail" if clear_loop_area <= 1.0 else "single_entry_loop"
        add(kind, sorted(edge_set), attachment)
        if results and tuple(results[-1]["segment_indexes"]) == tuple(sorted(edge_set)):
            results[-1]["clear_loop_interior_area_m2"] = clear_loop_area
    # A terminal arm may contain several return loops. Inspect whole subtrees of
    # the block-cut forest so an internal return point cannot hide its full extent.
    tree = defaultdict(set)
    for node, owners in membership.items():
        if len(owners) > 1:
            articulation = ("a", node)
            for block in owners:
                tree[articulation].add(("b", block))
                tree[("b", block)].add(articulation)
    pending = set(tree)
    complete = []
    buffered = {}
    while pending:
        root = max(pending, key=lambda n: (len(tree[n]), len(blocks[n[1]]) if n[0] == "b" else 0, n))
        parent, order = {root: None}, [root]
        for node in order:
            for other in sorted(tree[node]):
                if other not in parent:
                    parent[other] = node
                    order.append(other)
        pending.difference_update(parent)
        descendants, cyclic = {}, {}
        for node in reversed(order):
            own = set(blocks[node[1]]) if node[0] == "b" else set()
            vertices = {v for i in own for v in (records[i]["node_ids"][0], records[i]["node_ids"][-1])}
            has_cycle = bool(own) and len(own) >= len(vertices)
            for child in tree[node]:
                if parent.get(child) == node:
                    own.update(descendants[child])
                    has_cycle |= cyclic[child]
            descendants[node], cyclic[node] = own, has_cycle
            upstream = parent[node]
            if node[0] != "b" or upstream is None or upstream[0] != "a" or not has_cycle:
                continue
            for index in own:
                if index not in buffered:
                    buffered[index] = lines[index].buffer(records[index]["width_m"] / 2, cap_style="flat", join_style="mitre", mitre_limit=2)
            footprint = unary_union([buffered[i] for i in sorted(own)])
            clear_area = sum(Polygon(ring).area for p in parts(footprint) if p.geom_type == "Polygon" for ring in p.interiors)
            if clear_area <= 1.0:
                complete.append((own, upstream[1], clear_area))
    selected = []
    for edges, attachment, clear_area in sorted(complete, key=lambda value: (-len(value[0]), sorted(value[0]))):
        if any(edges <= earlier for earlier in selected):
            continue
        selected.append(edges)
        results = [r for r in results if not set(r["segment_indexes"]) <= edges]
        seen.discard(tuple(sorted(edges)))
        add("collapsed_return_tail", sorted(edges), attachment)
        results[-1]["clear_loop_interior_area_m2"] = clear_area
    results.sort(key=lambda r: (-r["reach_m"], r["segment_indexes"]))
    for index, result in enumerate(results, 1):
        result["branch"] = index
    return results, graph, coordinates


def validate_intersections(records):
    lines = lines_for(records)
    tree, checked = STRtree(lines), 0
    for a, line in enumerate(lines):
        for b in tree.query(line, predicate="intersects"):
            b = int(b)
            if b <= a or road_layer(records[a]) != road_layer(records[b]):
                continue
            intersection = line.intersection(lines[b])
            if intersection.length > 1e-7:
                raise ValueError("Noded segments still share a collinear interval")
            for point in intersection.geoms if hasattr(intersection, "geoms") else [intersection]:
                if point.geom_type != "Point":
                    continue
                for index in (a, b):
                    ends = [Point(lines[index].coords[j]) for j in (0, -1)]
                    if min(point.distance(p) for p in ends) > 1e-7:
                        raise ValueError("An intersection still lies inside a road segment")
            checked += 1
    return {"same_profile_intersecting_pairs_checked": checked, "unsplit_intersections": 0}


def draw_review(records, branches, graph, coordinates, bounds, output):
    lines = lines_for(records)
    colours = {"degree_one_tail": "#df4e42", "collapsed_return_tail": "#df4e42", "single_entry_loop": "#d49b36"}
    fig, ax = plt.subplots(figsize=(14, 11), dpi=160)
    ax.add_collection(LineCollection([list(line.coords) for line in lines], colors="#9ba8b2", linewidths=0.7))
    for branch in branches:
        colour = "#946abd" if branch["has_structure_transition"] else colours[branch["kind"]]
        ax.add_collection(LineCollection([list(lines[i].coords) for i in branch["segment_indexes"]], colors=colour, linewidths=2.2))
        if branch["reach_m"] >= 30:
            x, z = branch["furthest_xz_m"]
            ax.annotate(str(branch["branch"]), (x, -z), fontsize=8, xytext=(3, 3), textcoords="offset points")
    ax.set_xlim(bounds[0], bounds[2])
    ax.set_ylim(bounds[1], bounds[3])
    ax.set_aspect("equal")
    ax.grid(alpha=0.15)
    ax.set_title("Dangling review after splitting every same-level intersection\nRed: tips/collapsed returns · gold: single-entry loops · purple: structure-bearing branches")
    fig.tight_layout()
    fig.savefig(output / "dangling_overview.png")
    plt.close(fig)
    fig, ax = plt.subplots(figsize=(14, 11), dpi=160)
    ax.add_collection(LineCollection([list(line.coords) for line in lines], colors="#9ba8b2", linewidths=0.7))
    junctions = [coordinates[n] for n in graph if len(graph[n]) >= 3]
    if junctions:
        ax.scatter(*zip(*junctions), s=4, c="#326da8")
    ax.set_xlim(bounds[0], bounds[2])
    ax.set_ylim(bounds[1], bounds[3])
    ax.set_aspect("equal")
    ax.set_title("Current segments · blue: explicit junction endpoints")
    fig.tight_layout()
    fig.savefig(output / "junctions.png")
    plt.close(fig)
    visible = [r for r in branches if r["reach_m"] >= 5][:6]
    if not visible:
        return
    fig, axes = plt.subplots(2, 3, figsize=(18, 12), dpi=150, layout="constrained")
    for ax, branch in zip(axes.flat, visible):
        branch_lines = [lines[i] for i in branch["segment_indexes"]]
        minx, miny, maxx, maxy = unary_union(branch_lines).bounds
        span = max(maxx - minx, maxy - miny) + 60
        cx, cy = (minx + maxx) / 2, (miny + maxy) / 2
        nearby = [line for line in lines if line.bounds[0] <= cx + span / 2 and line.bounds[2] >= cx - span / 2
                  and line.bounds[1] <= cy + span / 2 and line.bounds[3] >= cy - span / 2]
        ax.add_collection(LineCollection([list(g.coords) for g in nearby], colors="#9ba8b2", linewidths=2))
        ax.add_collection(LineCollection([list(g.coords) for g in branch_lines], colors=colours[branch["kind"]], linewidths=3))
        x, z = branch["attachment_xz_m"]
        ax.plot(x, -z, "o", color="#235a7d", markersize=6)
        ax.set_xlim(cx - span / 2, cx + span / 2)
        ax.set_ylim(cy - span / 2, cy + span / 2)
        ax.set_aspect("equal")
        ax.set_title(f"Branch {branch['branch']} · {branch['reach_m']:.0f} m reach · {branch['kind']}\n" + ", ".join(branch["road_names"][:2]), fontsize=10)
        ax.grid(alpha=0.15)
    for ax in list(axes.flat)[len(visible):]:
        ax.set_visible(False)
    fig.suptitle("Branches after junction splitting · blue: attachment to network")
    fig.savefig(output / "dangling_details.png")
    plt.close(fig)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input", type=Path, help="Repaired layout seed JSON")
    parser.add_argument("--output", type=Path, required=True, help="Current layout directory")
    parser.add_argument("--review", type=Path, required=True)
    args = parser.parse_args()
    source = args.input.read_bytes()
    data = json.loads(source)
    if data["metadata"]["format"] != "repaired-city-layout-v1":
        raise ValueError("Expected a repaired layout seed")
    records, checks = node_roads(data["roads"])
    checks.update(validate_intersections(records))
    groups = connected_components([{r["node_ids"][0], r["node_ids"][-1]} for r in records])
    if len(groups) != 1:
        raise ValueError("Intersection splitting disconnected the layout")
    checks["connected_components"] = len(groups)
    branches, graph, coordinates = dangling_branches(records)
    checks.update({"junction_nodes": sum(len(edges) >= 3 for edges in graph.values()),
                   "degree_one_nodes": sum(len(edges) == 1 for edges in graph.values()),
                   "branch_counts": dict(Counter(r["kind"] for r in branches)),
                   "branches_with_structures": sum(r["has_structure_transition"] for r in branches)})
    metadata = {**data["metadata"], "format": "junction-city-layout-v1", "corridors": len(records),
                "layout_centerline_length_km": sum(r["game_length_m"] for r in records) / 1000,
                "intersection_splitting": {"input_sha256": hashlib.sha256(source).hexdigest(), **checks},
                "notes": ["Every same-profile centerline intersection is an explicit segment endpoint; collinear intervals are stored once.",
                          "Existing shared bridge/grade-transition vertices remain connected; crossings between different structure profiles are not joined.",
                          "Noding preserves unique centerline coverage and the existing physical road surface export.",
                          "Dangling review includes every graph tip, without a minimum length, and single-entry return loops. No branches are removed."]}
    for output in (args.output, args.review):
        output.mkdir(parents=True, exist_ok=True)
    (args.output / "roads.json").write_text(json.dumps({"metadata": metadata, "roads": records}, separators=(",", ":")))
    (args.output / "report.json").write_text(json.dumps(metadata, indent=2) + "\n")
    (args.review / "dangling.json").write_text(json.dumps({"axes": "X east, Z south", "branches": branches}, separators=(",", ":")))
    (args.review / "junctions.json").write_text(json.dumps({"junctions": [{"node": n, "point_xz_m": [coordinates[n][0], -coordinates[n][1]], "segment_indexes": edges}
                                                                           for n, edges in sorted(graph.items()) if len(edges) >= 3]}, separators=(",", ":")))
    draw_review(records, branches, graph, coordinates, metadata["game_rectangle_bounds_east_north_m"], args.review)
    print(json.dumps(checks, indent=2))


if __name__ == "__main__":
    main()
