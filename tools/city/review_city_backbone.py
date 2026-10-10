#!/usr/bin/env python3
"""Keep the approved backbone's main component and review game-scale geometry."""
import argparse
import csv
import hashlib
import json
import math
from collections import Counter, defaultdict
from pathlib import Path

from pyproj import Transformer
from shapely.affinity import scale
from shapely.geometry import LineString
from shapely.ops import unary_union

from audit_city_roads import ROOT, ROAD_CLASSES, audit, connected_components, connectivity, preview


def geometry_digest(records):
    keys = ("source_row_id", "osm_id", "part", "pruned_fragment", "highway", "points_xz_m")
    geometry = [{key: record[key] for key in keys} for record in records]
    return hashlib.sha256(json.dumps(geometry, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def load_export(folder, recipe):
    data = json.loads((folder / "roads.json").read_text())
    metadata = data["metadata"]
    if metadata["compression"] != recipe["compression"] or metadata["source_bounds_lon_lat"] != recipe["bounds_lon_lat"]:
        raise ValueError("Export bounds/scale differ from the locked recipe")
    if set(metadata["selected_highway_classes"]) != set(recipe["highway_classes"]):
        raise ValueError("Export road classes differ from the locked recipe")
    if metadata["straggler_filter"]["mode"] != "recursive_dead_end_pruning":
        raise ValueError("Backbone must be pruned before locking its main component")
    inverse = Transformer.from_crs(metadata["projected_crs"], 4326, always_xy=True)
    origin_x, origin_y = metadata["projected_origin_m"]
    lines, nodes = [], []
    for record in data["roads"]:
        if record["highway"] not in recipe["highway_classes"]:
            raise ValueError(f"Unexpected road class on {record['osm_id']}")
        coords = record["points_xz_m"]
        if len(coords) < 2 or any(len(point) != 2 or not all(math.isfinite(v) for v in point) for point in coords):
            raise ValueError(f"Invalid centerline on {record['osm_id']}")
        line = LineString([(x, -z) for x, z in coords])
        if line.length <= 0 or not line.is_valid or not math.isclose(line.length, record["game_length_m"], abs_tol=1e-7):
            raise ValueError(f"Invalid road length on {record['osm_id']}")
        if not math.isclose(line.length * recipe["compression"], record["source_length_m"], abs_tol=1e-6):
            raise ValueError(f"Incorrect compression on {record['osm_id']}")
        lons, lats = inverse.transform(
            [x * recipe["compression"] + origin_x for x, z in coords],
            [-z * recipe["compression"] + origin_y for x, z in coords],
        )
        lines.append(line)
        nodes.append({(round(lon, 8), round(lat, 8)) for lon, lat in zip(lons, lats)})
    return data, lines, nodes, inverse


def group_report(records, lines, nodes, groups, metadata, inverse, context_path):
    main = unary_union([lines[index] for index in groups[0]])
    reports, context_lookup = [], {}
    origin_x, origin_y = metadata["projected_origin_m"]
    for number, group in enumerate(groups[1:], 1):
        geometry = unary_union([lines[index] for index in group])
        lon, lat = inverse.transform(geometry.centroid.x * metadata["compression"] + origin_x,
                                     geometry.centroid.y * metadata["compression"] + origin_y)
        reports.append({
            "group": number, "road_parts": len(group),
            "names": sorted({records[i]["name"] for i in group if records[i]["name"]}),
            "highway_classes": dict(Counter(records[i]["highway"] for i in group)),
            "game_centerline_length_m": sum(lines[i].length for i in group),
            "centroid_lon_lat": [lon, lat], "bounds_east_north_m": geometry.bounds,
            "nearest_main_centerline_gap_m": geometry.distance(main),
            "roads": [{"osm_id": records[i]["osm_id"], "name": records[i]["name"],
                       "highway": records[i]["highway"]} for i in group],
            "omitted_class_roads_touching_group": [],
        })
        for index in group:
            for point in records[index]["points_xz_m"]:
                context_lookup[tuple(round(value, 6) for value in point)] = number
    if context_path is not None:
        context = json.loads(context_path.read_text())
        for field in ("compression", "projected_crs", "projected_origin_m", "source_bounds_lon_lat"):
            if context["metadata"][field] != metadata[field]:
                raise ValueError(f"Context export differs in {field}")
        context_nodes = [{tuple(round(value, 6) for value in point) for point in record["points_xz_m"]}
                         for record in context["roads"]]
        context_groups = connected_components(context_nodes)
        for context_group in context_groups:
            touched = {context_lookup[node] for index in context_group for node in context_nodes[index]
                       if node in context_lookup}
            for number in touched:
                reports[number - 1]["full_road_component_parts"] = len(context_group)
                reports[number - 1]["belongs_to_full_road_main_component"] = context_group is context_groups[0]
        for record in context["roads"]:
            if record["highway"] in metadata["selected_highway_classes"]:
                continue
            found = {context_lookup[key] for point in record["points_xz_m"]
                     if (key := tuple(round(value, 6) for value in point)) in context_lookup}
            for number in found:
                reports[number - 1]["omitted_class_roads_touching_group"].append({
                    "osm_id": record["osm_id"], "name": record["name"], "highway": record["highway"],
                })
    return {"removed_groups": reports, "context_method": "shared compressed vertices rounded to 1 micrometre; source memberships decide component selection"}


def width_for_audit(record, recipe):
    if record["width_m"] is not None:
        width, source = record["width_m"], "explicit_width"
    elif record["lanes"] is not None:
        width, source = record["lanes"] * recipe["audit_lane_width_m"], "lanes_times_lane_width"
    else:
        width, source = recipe["audit_unknown_road_width_m"], "unknown_width_fallback"
    if not math.isfinite(width) or width <= 0:
        raise ValueError(f"Invalid audit width on {record['osm_id']}")
    return width, source


def other_checks(records, lines, folder):
    duplicates = defaultdict(list)
    nonsimple, short, tight, tight_vertices = [], [], set(), 0
    columns = ["road_index", "osm_id", "issue", "game_length_m", "vertex", "heading_change_deg", "radius_proxy_m"]
    with (folder / "geometry_issues.csv").open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=columns)
        writer.writeheader()
        for index, (record, line) in enumerate(zip(records, lines)):
            duplicates[line.normalize().wkb].append(index)
            base = {"road_index": index, "osm_id": record["osm_id"], "game_length_m": round(line.length, 6)}
            if not line.is_simple:
                nonsimple.append(index)
                writer.writerow({**base, "issue": "self_intersecting_centerline"})
            if line.length < 2:
                short.append(index)
                writer.writerow({**base, "issue": "road_part_shorter_than_2m"})
            coords = list(line.coords)
            for vertex in range(1, len(coords) - 1):
                a, b, c = coords[vertex - 1:vertex + 2]
                ux, uy, vx, vy = b[0] - a[0], b[1] - a[1], c[0] - b[0], c[1] - b[1]
                ab, bc = math.hypot(ux, uy), math.hypot(vx, vy)
                cross = abs(ux * vy - uy * vx)
                if min(ab, bc) < 1e-9:
                    continue
                angle = math.degrees(math.acos(max(-1.0, min(1.0, (ux * vx + uy * vy) / (ab * bc)))))
                radius = ab * bc * math.dist(a, c) / (2 * cross) if cross > 1e-12 else 0 if angle > 170 else math.inf
                if angle >= 10 and radius < 5:
                    tight.add(index)
                    tight_vertices += 1
                    writer.writerow({**base, "issue": "tight_bend_candidate", "vertex": vertex,
                                     "heading_change_deg": round(angle, 3), "radius_proxy_m": round(radius, 6)})
    duplicate_groups = [indices for indices in duplicates.values() if len(indices) > 1]
    return {
        "exact_duplicate_centerline_groups": [[records[index]["osm_id"] for index in group] for group in duplicate_groups],
        "self_intersecting_centerline_road_parts": len(nonsimple),
        "road_parts_shorter_than_2m": len(short),
        "tight_bend_candidate_road_parts": len(tight), "tight_bend_candidate_vertices": tight_vertices,
        "tight_bend_method": "three-point circumcircle radius below 5m and heading change at least 10 degrees; diagnostic only, not a vehicle feasibility test",
        "notes": ["Short parts may be normal junction/link subdivisions and are not automatically invalid.",
                  "Surface folds within a single road part and directional reachability are outside this audit."],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("folder", type=Path)
    parser.add_argument("--recipe", type=Path, default=ROOT / "tools/city/bengaluru_backbone.json")
    parser.add_argument("--context-roads", type=Path)
    args = parser.parse_args()
    recipe = json.loads(args.recipe.read_text())
    if not set(recipe["highway_classes"]) <= ROAD_CLASSES or recipe["require_lane_count"] or not recipe["prune_dead_ends"] or not recipe["keep_largest_component_after_pruning"]:
        raise ValueError("Invalid locked backbone selection policy")
    for field in ("compression", "audit_lane_width_m", "audit_unknown_road_width_m"):
        if not math.isfinite(recipe[field]) or recipe[field] <= 0:
            raise ValueError(f"Invalid recipe {field}")
    folder = args.folder.resolve(strict=True)
    data, lines, nodes, inverse = load_export(folder, recipe)
    groups = connected_components(nodes)
    if not groups:
        raise ValueError("Empty backbone export")
    keep = groups[0]
    records = [data["roads"][index] for index in keep]
    if len(records) != recipe["expected_road_parts"] or geometry_digest(records) != recipe["geometry_sha256"]:
        raise ValueError("Main component differs from the locked geometry; review the recipe before changing it")
    removed_path = folder / "discarded_groups.json"
    removed = group_report(data["roads"], lines, nodes, groups, data["metadata"], inverse, args.context_roads) if len(groups) > 1 else (
        json.loads(removed_path.read_text()) if removed_path.exists() else {"removed_groups": []}
    )
    lines = [lines[index] for index in keep]
    nodes = [nodes[index] for index in keep]
    unscaled = [scale(line, recipe["compression"], recipe["compression"], origin=(0, 0)) for line in lines]
    audit_records, width_counts = [], Counter()
    with (folder / "audit_widths.csv").open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=["road_index", "osm_id", "audit_width_m", "width_source"])
        writer.writeheader()
        for index, record in enumerate(records):
            width, source = width_for_audit(record, recipe)
            width_counts[source] += 1
            audit_records.append({**record, "width_m": width})
            writer.writerow({"road_index": index, "osm_id": record["osm_id"], "audit_width_m": width, "width_source": source})
    print(f"Locked main component: {len(records):,} parts; auditing game widths", flush=True)
    _, highlights, overlaps = audit(audit_records, unscaled, lines, nodes, folder, diagnostics=True)
    metadata = dict(data["metadata"])
    metadata.update({
        "road_parts": len(records), "retained_source_ways": len({r["osm_id"] for r in records}),
        "retained_highway_classes": dict(Counter(r["highway"] for r in records)),
        "retained_road_parts_with_lane_count": sum(r["lanes"] is not None for r in records),
        "retained_road_parts_without_lane_count": sum(r["lanes"] is None for r in records),
        "source_centerline_length_km": sum(line.length for line in unscaled) / 1000,
        "game_centerline_length_km": sum(line.length for line in lines) / 1000,
        "connectivity": connectivity(nodes), "locked_geometry_sha256": recipe["geometry_sha256"],
        "recipe": str(args.recipe.resolve()), "overlaps": overlaps,
        "overlap_width_policy": {"lane_width_m": recipe["audit_lane_width_m"],
                                 "unknown_road_width_m": recipe["audit_unknown_road_width_m"],
                                 "parts_by_width_source": dict(width_counts),
                                 "source_metadata_modified": False},
        "geometry_checks": other_checks(records, lines, folder),
        "component_filter": {"mode": "largest_component_after_pruning",
                             "removed_components": len(removed["removed_groups"]),
                             "removed_road_parts": sum(group["road_parts"] for group in removed["removed_groups"]),
                             "removed_game_centerline_length_m": sum(group["game_centerline_length_m"] for group in removed["removed_groups"])},
    })
    if "only the largest shared-source-vertex component retained" not in metadata["filter"]:
        metadata["filter"] += "; only the largest shared-source-vertex component retained after pruning"
    with (folder / "roads.json").open("w") as handle:
        json.dump({"metadata": metadata, "roads": records}, handle, separators=(",", ":"))
    (folder / "report.json").write_text(json.dumps(metadata, indent=2) + "\n")
    removed_path.write_text(json.dumps(removed, indent=2, ensure_ascii=False) + "\n")
    bounds = metadata["game_rectangle_bounds_east_north_m"]
    preview(lines, records, [], bounds, folder, recipe["compression"], True, False,
            road_classes=recipe["highway_classes"])
    preview(lines, records, highlights, bounds, folder, recipe["compression"], True, True,
            "overlap_overview.png", recipe["highway_classes"])
    print(json.dumps({"road_parts": len(records), "game_length_km": metadata["game_centerline_length_km"],
                      "removed_groups": [{key: group[key] for key in ("names", "road_parts", "game_centerline_length_m")}
                                         for group in removed["removed_groups"]], "overlaps": overlaps,
                      "geometry_checks": metadata["geometry_checks"]}, indent=2, ensure_ascii=False))


if __name__ == "__main__":
    main()
