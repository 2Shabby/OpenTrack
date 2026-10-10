#!/usr/bin/env python3
"""Simulate removing marked physical arms and mark the resulting pruning cascade."""
import argparse
import hashlib
import json
import math
from collections import defaultdict
from pathlib import Path

import matplotlib.pyplot as plt
from matplotlib.collections import LineCollection, PatchCollection
from shapely.geometry import LineString, Point
from shapely.ops import unary_union
from shapely.strtree import STRtree

from audit_city_roads import road_layer
from node_city_intersections import coordinate_key, dangling_branches
from repair_city_overlaps import lines_for, read_polygon
from review_city_overlaps import patches
from review_city_surface_branches import rasterize, skeleton_records


GROUND = ("0", "no", "no")


def make_network(surface_records, roads, cell, floor=None):
    coordinates, edges, point_nodes, references = {}, [], {}, {}
    ground_edges = {}

    def ground_node(point):
        key = coordinate_key(point)
        if key not in point_nodes:
            index = len(coordinates)
            point_nodes[key] = index
            coordinates[index] = tuple(point)
        return point_nodes[key]

    def edge(a, b, role, profile, names=(), source=None):
        if a == b:
            return
        index = len(edges)
        edges.append({"a": a, "b": b, "role": role, "profile": tuple(profile), "names": tuple(names), "source": source})
        if role == "ground":
            ground_edges[tuple(sorted((coordinate_key(coordinates[a]), coordinate_key(coordinates[b]))))] = index

    for record, line in zip(surface_records, lines_for(surface_records)):
        ids = [ground_node(xy) for xy in line.coords]
        for a, b in zip(ids, ids[1:]):
            edge(a, b, "ground", GROUND)
    pixels = sorted(coordinates)
    tree = STRtree([Point(coordinates[n]) for n in pixels])
    ground_source_nodes = {n for r in roads if road_layer(r) == GROUND for n in r["node_ids"]}
    matched, unmatched, wide_junction_matches = {}, [], []
    padded_floor = floor.buffer(cell * math.sqrt(2)) if floor is not None else None
    grade_positions = {n: (x, -z) for r in roads if road_layer(r) != GROUND
                       for n, (x, z) in zip(r["node_ids"], r["points_xz_m"])}
    for node, xy in sorted(grade_positions.items()):
        point = Point(xy)
        pixel = pixels[int(tree.nearest(point))] if node in ground_source_nodes else None
        distance = point.distance(Point(coordinates[pixel])) if pixel is not None else None
        close = distance is not None and distance <= 6.5 + cell * math.sqrt(2)
        within_junction = (not close and distance is not None and padded_floor is not None and distance <= 52
                           and padded_floor.covers(LineString([xy, coordinates[pixel]])))
        if close or within_junction:
            matched[node] = (pixel, distance)
            if not close:
                wide_junction_matches.append({"source_node": node, "distance_m": distance})
            if distance < 1e-8:
                references[node] = pixel
                continue
        elif distance is not None:
            unmatched.append({"source_node": node, "point_xz_m": [xy[0], -xy[1]], "nearest_skeleton_distance_m": distance})
        references[node] = len(coordinates)
        coordinates[references[node]] = xy
    for index, record in enumerate(roads):
        profile = road_layer(record)
        if profile == GROUND:
            continue
        for a, b in zip(record["node_ids"], record["node_ids"][1:]):
            edge(references[a], references[b], "grade", profile, record["names"], index)
    for source, (pixel, _) in matched.items():
        edge(references[source], pixel, "attachment", GROUND)
    return coordinates, edges, ground_edges, {"matched_grade_attachments": len(matched), "unmatched_grade_attachments": unmatched,
                                              "attachments_within_wide_junctions": wide_junction_matches}


def components(edges, active):
    incident = defaultdict(list)
    for i in active:
        incident[edges[i]["a"]].append(i)
        incident[edges[i]["b"]].append(i)
    pending, groups = set(incident), []
    while pending:
        root = min(pending)
        pending.remove(root)
        queue, found = [root], set()
        for node in queue:
            for i in incident[node]:
                found.add(i)
                other = edges[i]["b"] if edges[i]["a"] == node else edges[i]["a"]
                if other in pending:
                    pending.remove(other)
                    queue.append(other)
        groups.append(found)
    return groups


def compress_network(coordinates, edges, active):
    incident = defaultdict(list)
    for i in sorted(active):
        incident[edges[i]["a"]].append(i)
        incident[edges[i]["b"]].append(i)

    def signature(i):
        return edges[i]["role"], edges[i]["profile"]

    def stop(node):
        return len(incident[node]) != 2 or signature(incident[node][0]) != signature(incident[node][1])

    starts = [n for n in sorted(incident) if stop(n)] + [n for n in sorted(incident) if not stop(n)]
    seen, records = set(), []
    for start in starts:
        for first in incident[start]:
            if first in seen:
                continue
            node, path, ids, picked = start, [coordinates[start]], [start], []
            current = first
            while True:
                seen.add(current)
                picked.append(current)
                node = edges[current]["b"] if edges[current]["a"] == node else edges[current]["a"]
                path.append(coordinates[node])
                ids.append(node)
                if node == start or stop(node):
                    break
                current = next(i for i in incident[node] if i != current)
            profile = edges[first]["profile"]
            records.append({"osm_id": f"simulation-{len(records):05d}", "node_ids": ids,
                            "points_xz_m": [[x, -y] for x, y in path],
                            "game_length_m": sum(math.dist(a, b) for a, b in zip(path, path[1:])),
                            "width_m": 13, "tags": dict(zip(("layer", "bridge", "tunnel"), profile)),
                            "names": sorted({name for i in picked for name in edges[i]["names"]}),
                            "analysis_edges": picked})
    if seen != active:
        raise ValueError("Simulation compression lost active edges")
    return records


def selected_edges(branches, ground_edges):
    selected = set()
    for branch in branches:
        for path in branch["paths_xz_m"]:
            xy = [(x, -z) for x, z in path]
            for a, b in zip(xy, xy[1:]):
                key = tuple(sorted((coordinate_key(a), coordinate_key(b))))
                if key not in ground_edges:
                    raise ValueError("Marked arm differs from the reconstructed surface graph")
                selected.add(ground_edges[key])
    return selected


def simulate(coordinates, edges, initial, max_rounds=100):
    active = set(range(len(edges)))
    coalesced = 0
    for _ in range(50):
        records = compress_network(coordinates, edges, active)
        branches, _, _ = dangling_branches(records)
        details = set()
        for branch in branches:
            if branch["reach_m"] >= 13 or branch["centerline_length_m"] >= 26:
                continue
            values = {i for r in branch["segment_indexes"] for i in records[r]["analysis_edges"]}
            if all(edges[i]["role"] == "ground" for i in values):
                details.update(values)
        if not details or details == active:
            break
        active.difference_update(details)
        coalesced += len(details)
    else:
        raise ValueError("Baseline short-detail coalescing did not settle")
    original_groups = components(edges, active)
    owner = {i: group for group, values in enumerate(original_groups) for i in values}
    baseline_records = compress_network(coordinates, edges, active)
    baseline_branches, _, _ = dangling_branches(baseline_records)
    prior_branches = [(branch["kind"], {i for r in branch["segment_indexes"] for i in baseline_records[r]["analysis_edges"]})
                      for branch in baseline_branches]
    events, rounds, seen_structures = [], [], set()
    active.difference_update(initial)

    def length(values):
        return sum(math.dist(coordinates[edges[i]["a"]], coordinates[edges[i]["b"]]) for i in values)

    def add(kind, values, round_number, newly_exposed=True, branch=None):
        event = {"mark": len(events) + 1, "kind": kind, "round": round_number, "analysis_edges": sorted(values),
                 "length_m": length(values), "newly_exposed": newly_exposed,
                 "contains_grade_roads": any(edges[i]["role"] == "grade" for i in values)}
        if branch is not None:
            event.update({k: branch[k] for k in ("attachment_xz_m", "furthest_xz_m", "reach_m")})
        events.append(event)

    for round_number in range(1, max_rounds + 1):
        roles = defaultdict(set)
        for i in active:
            roles[edges[i]["a"]].add(edges[i]["role"])
            roles[edges[i]["b"]].add(edges[i]["role"])
        orphans = {i for i in active if edges[i]["role"] == "attachment"
                   and ("grade" not in roles[edges[i]["a"]] or "ground" not in roles[edges[i]["b"]])}
        active.difference_update(orphans)
        changed, split_groups = set(), defaultdict(list)
        for values in components(edges, active):
            split_groups[owner[next(iter(values))]].append(values)
        disconnected = []
        for groups in split_groups.values():
            groups.sort(key=lambda values: (-length(values), min(values)))
            for values in groups[1:]:
                add("disconnected_group", values, round_number)
                changed.update(values)
                disconnected.append(values)
        # Exclude the marked islands in the simulation before checking its surviving main groups.
        surviving = active - changed
        records = compress_network(coordinates, edges, surviving)
        branches, _, _ = dangling_branches(records)
        new_tails, small_tails, structures = 0, 0, 0
        for branch in branches:
            values = {i for r in branch["segment_indexes"] for i in records[r]["analysis_edges"]}
            newly_exposed = not any(branch["kind"] == kind and values <= previous for kind, previous in prior_branches)
            grade = any(edges[i]["role"] != "ground" for i in values)
            short = branch["reach_m"] < 13 and branch["centerline_length_m"] < 26
            if branch["kind"] == "single_entry_loop":
                key = tuple(sorted(values))
                if newly_exposed and key not in seen_structures:
                    seen_structures.add(key)
                    add("new_single_entry_loop", values, round_number, True, branch)
                continue
            if grade:
                key = tuple(sorted(values))
                if newly_exposed and key not in seen_structures:
                    seen_structures.add(key)
                    add("dangling_with_structure", values, round_number, True, branch)
                    structures += 1
                continue
            if short:
                if newly_exposed:
                    key = tuple(sorted(values))
                    if key not in seen_structures:
                        seen_structures.add(key)
                        add("short_dangling_detail", values, round_number, True, branch)
                        small_tails += 1
                changed.update(values)
                continue
            add("additional_dangling_arm", values, round_number, newly_exposed, branch)
            changed.update(values)
            new_tails += 1
        rounds.append({"round": round_number, "disconnected_groups": len(disconnected), "additional_arms": new_tails,
                       "new_structure_bearing_branches": structures, "new_short_details": small_tails,
                       "orphan_analysis_attachments_dropped": len(orphans),
                       "simulated_removed_edges": len(changed), "remaining_edges": len(active - changed)})
        if not changed:
            return events, rounds, active, {"baseline_components": len(original_groups), "remaining_components": len(components(edges, active)),
                                            "baseline_short_ground_edges_coalesced": coalesced}
        active.difference_update(changed)
        if not active:
            raise ValueError("Pruning simulation would remove the entire network")
    raise ValueError("Pruning simulation did not settle within its round limit")


def paths_for(values, coordinates, edges):
    records = compress_network(coordinates, edges, set(values))
    return [record["points_xz_m"] for record in records]


def draw(floor, original, events, bounds, output):
    colours = {"disconnected_group": "#2977b7", "additional_dangling_arm": "#ef8e25",
               "dangling_with_structure": "#946abd", "short_dangling_detail": "#d8b445", "new_single_entry_loop": "#d8b445"}
    fig, ax = plt.subplots(figsize=(14, 11), dpi=160)
    ax.add_collection(PatchCollection(patches([floor]), facecolors="#c0c8ce", edgecolors="none", rasterized=True))
    paths = [[(x, -z) for x, z in path] for branch in original for path in branch["paths_xz_m"]]
    ax.add_collection(LineCollection(paths, colors="#d75b53", linewidths=1.7, alpha=0.5))
    for event in events:
        paths = [[(x, -z) for x, z in path] for path in event["paths_xz_m"]]
        ax.add_collection(LineCollection(paths, colors=colours[event["kind"]], linewidths=2.4))
        xy = paths[0][0]
        if event["kind"] == "disconnected_group":
            ax.scatter(*xy, s=13, color=colours[event["kind"]], zorder=4)
        if event.get("reach_m", event["length_m"]) >= 13:
            ax.annotate(f"{event['round']}.{event['mark']}", xy, fontsize=8, xytext=(3, 3), textcoords="offset points")
    ax.set_xlim(bounds[0], bounds[2])
    ax.set_ylim(bounds[1], bounds[3])
    ax.set_aspect("equal")
    ax.set_title("After simulated removal · labels: round.mark\nFaded red: initial removals · orange: further dangling · blue: disconnected · purple: structure review")
    ax.grid(alpha=0.15)
    fig.tight_layout()
    fig.savefig(output / "pruning_overview.png")
    plt.close(fig)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("folder", type=Path)
    parser.add_argument("--review", type=Path, required=True)
    args = parser.parse_args()
    road_bytes = (args.folder / "roads.json").read_bytes()
    surface_bytes = (args.folder / "surfaces.json").read_bytes()
    data, surfaces = json.loads(road_bytes), json.loads(surface_bytes)
    branch_review = json.loads((args.review / "dangling.json").read_text())
    if hashlib.sha256(surface_bytes).hexdigest() != branch_review["surface_review"]["surface_sha256"]:
        raise ValueError("Branch review does not match the current surface")
    cell = branch_review["surface_review"]["cell_size_m"]
    floor = unary_union([read_polygon(r) for key in ("road_bodies", "junctions", "medians") for r in surfaces[key]
                         if tuple(r["structure_profile"]) == GROUND])
    print("Reconstructing the reviewed physical network and explicit grade connections", flush=True)
    mask, minx, miny = rasterize(floor, cell)
    records, _ = skeleton_records(mask, minx, miny, cell)
    coordinates, edges, ground_edges, mapping = make_network(records, data["roads"], cell, floor)
    original = [r for r in branch_review["surface_branches"] if not r["has_structure_transition"] and r["kind"] != "single_entry_loop"]
    initial = selected_edges(original, ground_edges)
    print(f"Simulating removal of {len(original)} marked arms ({len(initial)} sampled edges)", flush=True)
    events, rounds, surviving, connectivity = simulate(coordinates, edges, initial)
    for event in events:
        event["paths_xz_m"] = paths_for(event["analysis_edges"], coordinates, edges)
        event["current_grade_segment_indexes"] = sorted({edges[i]["source"] for i in event["analysis_edges"] if edges[i]["source"] is not None})
        event["analysis_edge_count"] = len(event.pop("analysis_edges"))
    counts = {kind: sum(r["kind"] == kind for r in events) for kind in ("additional_dangling_arm", "disconnected_group", "dangling_with_structure", "short_dangling_detail", "new_single_entry_loop")}
    report = {"initial_removed_arms": len(original), "initial_removed_sampled_edges": len(initial),
              "initial_surface_arm_ids": [r["branch"] for r in original],
              "roads_geometry_sha256": hashlib.sha256(json.dumps(data["roads"], sort_keys=True).encode()).hexdigest(),
              "surface_sha256": hashlib.sha256(surface_bytes).hexdigest(), "cell_size_m": cell,
              "marks": counts, "rounds": rounds, **connectivity, **mapping,
              "notes": ["Read-only what-if simulation of removing the currently red exposed arms; short details and purple tips are not initial removals.",
                        "Explicit shared source grade vertices connect the sampled ground network to bridge/tunnel/layer roads. Other grade crossings are not joined.",
                        "Each round marks disconnected islands and further exposed ground arms, then excludes them in the simulation and repeats until stable.",
                        "Bridge/grade attachment points are retained while coalescing short ground details. Longer attachment links must stay within the existing ground junction footprint.",
                        "Structure-bearing dangling branches and playable single-entry loops are marked for review and retained; new short ground details are marked before coalescing them in the simulation.",
                        "Existing separate baseline components are not misreported as newly disconnected groups.",
                        "This metre-sampled diagnostic does not modify current roads, surfaces or meshes."]}
    (args.review / "pruning.json").write_text(json.dumps({"report": report, "marks": events}, separators=(",", ":")))
    draw(floor, original, events, data["metadata"]["game_rectangle_bounds_east_north_m"], args.review)
    if hashlib.sha256((args.folder / "roads.json").read_bytes()).digest() != hashlib.sha256(road_bytes).digest() or (args.folder / "surfaces.json").read_bytes() != surface_bytes:
        raise ValueError("Current geometry changed while simulating pruning")
    print(json.dumps({"initial_removed_arms": len(original), "marks": counts, "rounds": rounds, **connectivity,
                      "unmatched_grade_attachments": len(mapping["unmatched_grade_attachments"])}, indent=2))


if __name__ == "__main__":
    main()
