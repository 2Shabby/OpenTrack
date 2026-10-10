#!/usr/bin/env python3
"""Review complete physical road arms using a sampled road-surface skeleton."""
import argparse
import hashlib
import json
import math
from collections import Counter
from pathlib import Path

import matplotlib.pyplot as plt
from matplotlib.collections import LineCollection, PatchCollection
import numpy as np
from shapely import contains_xy, prepare
from shapely.geometry import Point
from shapely.ops import unary_union
from shapely.strtree import STRtree

from audit_city_roads import road_layer
from node_city_intersections import dangling_branches
from repair_city_overlaps import lines_for, read_polygon
from review_city_overlaps import patches


def thin(mask):
    """Zhang-Suen thinning preserves connected strokes and loops on an 8-neighbour grid."""
    image = np.pad(mask.astype(bool), 1)
    while True:
        changed = False
        for phase in (0, 1):
            centre = image[1:-1, 1:-1]
            neighbours = (image[:-2, 1:-1], image[:-2, 2:], image[1:-1, 2:], image[2:, 2:],
                          image[2:, 1:-1], image[2:, :-2], image[1:-1, :-2], image[:-2, :-2])
            count = np.zeros(centre.shape, dtype=np.uint8)
            transitions = np.zeros(centre.shape, dtype=np.uint8)
            for i, neighbour in enumerate(neighbours):
                count += neighbour
                transitions += (~neighbour & neighbours[(i + 1) % 8])
            north, east, south, west = (neighbours[i] for i in (0, 2, 4, 6))
            remove = centre & (count >= 2) & (count <= 6) & (transitions == 1)
            if phase == 0:
                remove &= ~(north & east & south) & ~(east & south & west)
            else:
                remove &= ~(north & east & west) & ~(north & south & west)
            if np.any(remove):
                centre[remove] = False
                changed = True
        if not changed:
            return image[1:-1, 1:-1]


def skeleton_records(mask, minx, miny, cell):
    pixels = {tuple(int(v) for v in p) for p in np.argwhere(thin(mask))}
    neighbours = {}
    for row, col in sorted(pixels):
        adjacent = []
        for dr in (-1, 0, 1):
            for dc in (-1, 0, 1):
                other = (row + dr, col + dc)
                if (not dr and not dc) or other not in pixels:
                    continue
                # A diagonal shortcut around an occupied corner would create a false triangle cycle.
                if dr and dc and ((row + dr, col) in pixels or (row, col + dc) in pixels):
                    continue
                adjacent.append(other)
        neighbours[(row, col)] = adjacent
    node_ids = {p: i for i, p in enumerate(sorted(pixels))}
    seen, records = set(), []
    roots = [p for p in sorted(pixels) if len(neighbours[p]) != 2]
    roots.extend(p for p in sorted(pixels) if len(neighbours[p]) == 2)
    for start in roots:
        for other in neighbours[start]:
            if tuple(sorted((start, other))) in seen:
                continue
            points, previous, node = [start], start, other
            while True:
                seen.add(tuple(sorted((previous, node))))
                points.append(node)
                if node == start or len(neighbours[node]) != 2:
                    break
                following = next(p for p in neighbours[node] if p != previous)
                previous, node = node, following
            xy = [(minx + (col + 0.5) * cell, miny + (row + 0.5) * cell) for row, col in points]
            records.append({"osm_id": f"surface-arm-{len(records):05d}", "node_ids": [node_ids[p] for p in points],
                            "points_xz_m": [[x, -y] for x, y in xy], "names": [], "width_m": 13,
                            "tags": {"layer": "0", "bridge": "no", "tunnel": "no"},
                            "game_length_m": sum(math.dist(a, b) for a, b in zip(xy, xy[1:]))})
    return records, len(pixels)


def rasterize(floor, cell, max_cells=30_000_000):
    minx, miny, maxx, maxy = floor.bounds
    minx, miny = math.floor(minx / cell) * cell - cell, math.floor(miny / cell) * cell - cell
    columns, rows = math.ceil((maxx - minx) / cell) + 1, math.ceil((maxy - miny) / cell) + 1
    if columns * rows > max_cells:
        raise ValueError("Surface review exceeds its cell budget; increase --cell-size")
    mask = np.empty((rows, columns), dtype=bool)
    prepare(floor)
    xs = minx + (np.arange(columns) + 0.5) * cell
    for start in range(0, rows, 256):
        ys = miny + (np.arange(start, min(rows, start + 256)) + 0.5) * cell
        mask[start:start + len(ys)] = contains_xy(floor, xs[None, :], ys[:, None])
    return mask, minx, miny


def coalesce_short_side_branches(records, road_width=13):
    removed, rounds = [], 0
    while rounds < 50:
        branches, _, _ = dangling_branches(records)
        short = [r for r in branches if r["reach_m"] < road_width and r["centerline_length_m"] < 2 * road_width]
        if not short:
            return records, removed, rounds
        discard = {i for branch in short for i in branch["segment_indexes"]}
        if len(discard) == len(records):
            return records, removed, rounds
        lines = lines_for(records)
        for branch in short:
            branch["paths_xz_m"] = [[[x, -y] for x, y in lines[i].coords] for i in branch.pop("segment_indexes")]
            branch.pop("branch")
            branch["detail"] = len(removed) + 1
            removed.append(branch)
        records = [record for i, record in enumerate(records) if i not in discard]
        rounds += 1
    raise ValueError("Short-detail coalescing did not settle within 50 rounds")


def structure_continuations(geometry, tip, tree, tolerance):
    along_arm = len(tree.query(geometry.buffer(tolerance), predicate="intersects")) > 0
    at_tip = len(tree.query(tip.buffer(tolerance), predicate="intersects")) > 0
    return along_arm, at_tip


def draw(floor, branches, paths, short_details, bounds, output, cell):
    colours = {"degree_one_tail": "#df4e42", "collapsed_return_tail": "#df4e42", "single_entry_loop": "#d49b36"}
    fig, ax = plt.subplots(figsize=(14, 11), dpi=160)
    ax.add_collection(PatchCollection(patches([floor]), facecolors="#bdc5cc", edgecolors="none", rasterized=True))
    small_paths = [[(x, -z) for x, z in path] for detail in short_details for path in detail["paths_xz_m"]]
    ax.add_collection(LineCollection(small_paths, colors="#d49b36", linewidths=0.8, alpha=0.7))
    for branch in branches:
        colour = "#946abd" if branch["has_structure_transition"] else colours[branch["kind"]]
        ax.add_collection(LineCollection(paths[branch["branch"]], colors=colour, linewidths=2))
        if branch["reach_m"] >= 25:
            x, z = branch["furthest_xz_m"]
            ax.annotate(str(branch["branch"]), (x, -z), fontsize=8, xytext=(3, 3), textcoords="offset points")
    ax.set_xlim(bounds[0], bounds[2])
    ax.set_ylim(bounds[1], bounds[3])
    ax.set_aspect("equal")
    ax.set_title(f"Complete physical arms beyond junctions · sampled at {cell:g} m\nRed: exposed branches · purple: tip continues to bridge/level · gold: small details/single-entry loops")
    ax.grid(alpha=0.15)
    fig.tight_layout()
    fig.savefig(output / "dangling_overview.png")
    plt.close(fig)
    visible = [r for r in branches if not r["has_structure_transition"] and r["reach_m"] >= 5][:6]
    if not visible:
        return
    fig, axes = plt.subplots(2, 3, figsize=(18, 12), dpi=150, layout="constrained")
    for ax, branch in zip(axes.flat, visible):
        points = [p for path in paths[branch["branch"]] for p in path]
        minx, miny = np.min(points, axis=0)
        maxx, maxy = np.max(points, axis=0)
        span = max(maxx - minx, maxy - miny) + 60
        cx, cy = (minx + maxx) / 2, (miny + maxy) / 2
        ax.add_collection(PatchCollection(patches([floor]), facecolors="#bdc5cc", edgecolors="none", rasterized=True))
        ax.add_collection(LineCollection(paths[branch["branch"]], colors=colours[branch["kind"]], linewidths=3))
        x, z = branch["attachment_xz_m"]
        ax.plot(x, -z, "o", color="#235a7d", markersize=5)
        ax.set_xlim(cx - span / 2, cx + span / 2)
        ax.set_ylim(cy - span / 2, cy + span / 2)
        ax.set_aspect("equal")
        ax.set_title(f"Physical arm {branch['branch']} · {branch['reach_m']:.0f} m reach\n" + ", ".join(branch["road_names"][:2]), fontsize=10)
        ax.grid(alpha=0.15)
    for ax in list(axes.flat)[len(visible):]:
        ax.set_visible(False)
    fig.suptitle("Full visible tails · red: surface skeleton beyond junction · blue: attachment")
    fig.savefig(output / "dangling_details.png")
    plt.close(fig)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("folder", type=Path, help="Current layout directory")
    parser.add_argument("--review", type=Path, required=True)
    parser.add_argument("--cell-size", type=float, default=1.0)
    args = parser.parse_args()
    if not math.isfinite(args.cell_size) or args.cell_size <= 0:
        parser.error("Cell size must be finite and positive")
    data = json.loads((args.folder / "roads.json").read_text())
    surface_bytes = (args.folder / "surfaces.json").read_bytes()
    surfaces = json.loads(surface_bytes)
    floor = unary_union([read_polygon(r) for key in ("road_bodies", "junctions", "medians") for r in surfaces[key]
                         if tuple(r["structure_profile"]) == ("0", "no", "no")])
    print("Sampling the welded ground road surface", flush=True)
    mask, minx, miny = rasterize(floor, args.cell_size)
    records, pixels = skeleton_records(mask, minx, miny, args.cell_size)
    records, short_details, coalescing_rounds = coalesce_short_side_branches(records)
    branches, _, _ = dangling_branches(records)
    roads = data["roads"]
    lines = lines_for(roads)
    source_tree = STRtree(lines)
    transitions = [Point(x, -z) for r in roads if road_layer(r) != ("0", "no", "no")
                   for x, z in (r["points_xz_m"][0], r["points_xz_m"][-1])]
    transition_tree = STRtree(transitions)
    skeleton_lines, paths = lines_for(records), {}
    for branch in branches:
        geometries = [skeleton_lines[i] for i in branch["segment_indexes"]]
        geometry = unary_union(geometries)
        nearby = geometry.buffer(6.5 + args.cell_size * math.sqrt(2))
        road_indexes = sorted(int(i) for i in source_tree.query(nearby, predicate="intersects"))
        branch["road_names"] = sorted({name for i in road_indexes for name in roads[i]["names"]})
        branch["nearby_current_segment_indexes"] = road_indexes
        tx, tz = branch["furthest_xz_m"]
        along_arm, at_tip = structure_continuations(geometry, Point(tx, -tz), transition_tree, 6.5 + args.cell_size * math.sqrt(2))
        branch["has_structure_attachment_along_arm"] = along_arm
        branch["has_structure_transition"] = at_tip
        branch["below_road_width"] = branch["reach_m"] < 13
        branch.pop("segment_indexes")
        branch["skeleton_attachment_node"] = branch.pop("attachment_node")
        branch["skeleton_tip_node"] = branch.pop("tip_node")
        paths[branch["branch"]] = [list(line.coords) for line in geometries]
        branch["paths_xz_m"] = [[[x, -y] for x, y in line.coords] for line in geometries]
    report = {"cell_size_m": args.cell_size, "raster_cells": int(mask.size), "skeleton_pixels": pixels,
              "surface_sha256": hashlib.sha256(surface_bytes).hexdigest(),
              "branch_counts": dict(Counter(r["kind"] for r in branches)),
              "short_details_retained_separately": len(short_details), "coalescing_rounds": coalescing_rounds,
              "tips_with_structure_continuation": sum(r["has_structure_transition"] for r in branches),
              "arms_with_structure_attachments": sum(r["has_structure_attachment_along_arm"] for r in branches),
              "exposed_arms_13m_or_longer": sum(not r["has_structure_transition"] and r["reach_m"] >= 13 for r in branches),
              "notes": ["Diagnostic skeleton of the existing welded ground road footprint, including median coverage.",
                        "Sampling joins overlapping return tracks into their full physical arm.",
                        "Side fragments under 13 m reach and 26 m total length are retained in short_surface_details; coalescing them lets main arms trace through local corner and pixel details.",
                        "Only tips near a bridge/grade attachment are coloured as potential structure continuations. Attachments along the arm are recorded separately and do not hide a dangling tip.",
                        "Different structures are not flattened into the ground surface.",
                        "This sampled graph is a geometric diagnostic, not gameplay topology. Authoritative roads remain exactly noded centerlines.",
                        "No road geometry or mesh is changed by this review."]}
    args.review.mkdir(parents=True, exist_ok=True)
    target = args.review / "dangling.json"
    review = json.loads(target.read_text()) if target.exists() else {"axes": "X east, Z south"}
    if "branches" in review:
        review["graph_branches"] = review.pop("branches")
    review.update({"surface_review": report, "surface_branches": branches, "short_surface_details": short_details})
    target.write_text(json.dumps(review, separators=(",", ":")))
    draw(floor, branches, paths, short_details, data["metadata"]["game_rectangle_bounds_east_north_m"], args.review, args.cell_size)
    print(json.dumps({k: v for k, v in report.items() if k != "notes"}, indent=2))


if __name__ == "__main__":
    main()
