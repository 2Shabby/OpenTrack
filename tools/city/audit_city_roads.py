#!/usr/bin/env python3
"""Extract and compress vehicle roads, optionally pruning branches or auditing widths.

Offline only. Requires shapely >= 2.1, pyproj and matplotlib. Source stays read-only.
"""
import argparse
import csv
import json
import math
import re
import sqlite3
import struct
from collections import Counter, defaultdict, deque
from pathlib import Path

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib.collections import LineCollection, PatchCollection
from matplotlib.patches import Polygon as PlotPolygon
from matplotlib.lines import Line2D
import pyproj
import shapely
from pyproj import Transformer
from shapely import from_wkb, transform
from shapely.affinity import scale, translate
from shapely.geometry import LineString, box
from shapely.strtree import STRtree

ROOT = Path(__file__).resolve().parents[2]
ROAD_CLASSES = {
    "motorway", "motorway_link", "trunk", "trunk_link", "primary", "primary_link",
    "secondary", "secondary_link", "tertiary", "tertiary_link", "unclassified",
    "residential", "living_street", "service", "road",
}
HIGHWAY_COLORS = {
    "motorway": "#8551a1", "trunk": "#bf613b", "primary": "#326da8", "secondary": "#258678",
}
TAG_PATTERN = re.compile(r'"((?:\\.|[^"\\])*)"\s*=>\s*"((?:\\.|[^"\\])*)"')
WIDTH_PATTERN = re.compile(r"\s*(\d+(?:\.\d+)?)\s*(m|meters|metres|ft|feet|foot|')?\s*", re.I)


def gpkg_line(blob):
    if len(blob) < 9 or blob[:2] != b"GP" or blob[2] != 0:
        raise ValueError("Expected GeoPackage version-0 geometry header")
    flags = blob[3]
    srs = struct.unpack_from(("<" if flags & 1 else ">") + "i", blob, 4)[0]
    if srs != 4326:
        raise ValueError(f"Expected EPSG:4326 geometry, found {srs}")
    envelope = (flags >> 1) & 7
    if envelope > 4:
        raise ValueError("Reserved GeoPackage envelope code")
    offset = 8 + (0, 32, 48, 48, 64)[envelope]
    geometry = from_wkb(blob[offset:])
    if geometry.geom_type != "LineString" or geometry.has_z or geometry.has_m or geometry.is_empty:
        raise ValueError("Expected a nonempty 2D road LineString")
    if not geometry.is_valid:
        raise ValueError("Invalid source road geometry")
    return geometry


def parts(geometry):
    if geometry.geom_type in ("LineString", "Polygon"):
        if not geometry.is_empty:
            yield geometry
    elif hasattr(geometry, "geoms"):
        for part in geometry.geoms:
            yield from parts(part)


def read_roads(source, bounds, compression, lane_width, include_missing_lanes=False, centerlines_only=False,
               road_classes=None):
    selected_classes = ROAD_CLASSES if road_classes is None else set(road_classes)
    if not selected_classes or not selected_classes <= ROAD_CLASSES:
        raise ValueError("Road class selection must be a nonempty subset of vehicle ROAD_CLASSES")
    clip = box(*bounds)
    lon0 = (bounds[0] + bounds[2]) / 2
    lat0 = (bounds[1] + bounds[3]) / 2
    zone = int((lon0 + 180) // 6) + 1
    crs = 32600 + zone if lat0 >= 0 else 32700 + zone
    projection = Transformer.from_crs(4326, crs, always_xy=True)
    origin = projection.transform(lon0, lat0)
    records, unscaled, compressed, node_sets, ordered_nodes = [], [], [], [], []
    stats = Counter()
    with sqlite3.connect(source.as_uri() + "?mode=ro", uri=True) as connection:
        connection.execute("PRAGMA query_only=ON")
        columns = {row[1] for row in connection.execute("PRAGMA table_info(lines)")}
        if not {"id", "geom", "osm_id", "name", "highway", "other_tags"} <= columns:
            raise ValueError("Expected the GDAL OSM lines layer")
        placeholders = ",".join("?" for _ in selected_classes)
        lane_predicate = "" if include_missing_lanes else " AND instr(other_tags, '\"lanes\"') > 0"
        cursor = connection.execute(
            f"SELECT id, osm_id, name, highway, other_tags, geom FROM lines "
            f"WHERE highway IN ({placeholders}){lane_predicate} ORDER BY id",
            sorted(selected_classes),
        )
        while batch := cursor.fetchmany(256):
            for row_id, osm_id, name, highway, raw_tags, blob in batch:
                tags = dict(TAG_PATTERN.findall(raw_tags or ""))
                lanes_text = tags.get("lanes", "").strip()
                stats["source_candidates" if include_missing_lanes else "lane_tagged_source_candidates"] += 1
                valid_lanes = lanes_text.isascii() and lanes_text.isdigit() and int(lanes_text) > 0
                if not valid_lanes and not include_missing_lanes:
                    stats["invalid_or_nonpositive_lane_count"] += 1
                    continue
                lanes = int(lanes_text) if valid_lanes else None
                width_match = WIDTH_PATTERN.fullmatch(tags.get("width", ""))
                width = float(width_match.group(1)) if width_match else (
                    lanes * lane_width if lanes is not None and not centerlines_only else None
                )
                if width_match and (width_match.group(2) or "").lower() in {"ft", "feet", "foot", "'"}:
                    width *= 0.3048
                width_source = "explicit_width" if width_match else (
                    "lanes_times_lane_width" if width is not None else "unknown"
                )
                if width is not None and (not math.isfinite(width) or width <= 0):
                    if centerlines_only:
                        width = None
                        width_source = "invalid_explicit_width"
                    else:
                        raise ValueError(f"Nonpositive width on OSM way {osm_id}")
                if width is None and not centerlines_only:
                    raise ValueError(f"Unknown road width on OSM way {osm_id}; use --centerlines-only")
                source_line = gpkg_line(blob)
                clipped = source_line.intersection(clip)
                clipped_parts = [p for p in parts(clipped) if p.geom_type == "LineString" and p.length > 0]
                if not clipped_parts:
                    stats["outside_requested_bounds"] += 1
                    continue
                stats["retained_source_ways"] += 1
                stats["retained_ways_with_lane_count" if valid_lanes else "retained_ways_without_valid_lane_count"] += 1
                stats["ways_using_" + width_source] += 1
                if "width" in tags and not width_match:
                    stats["unparsed_explicit_width_tags"] += 1
                for part_index, line in enumerate(clipped_parts):
                    projected = translate(transform(line, projection.transform, interleaved=False), -origin[0], -origin[1])
                    reduced = scale(projected, xfact=1 / compression, yfact=1 / compression, origin=(0, 0))
                    if not projected.is_valid or not reduced.is_valid:
                        raise ValueError(f"Invalid projected road {osm_id}")
                    source_nodes = [(round(x, 8), round(y, 8)) for x, y in line.coords]
                    nodes = set(source_nodes)
                    node_sets.append(nodes)
                    ordered_nodes.append(source_nodes)
                    unscaled.append(projected)
                    compressed.append(reduced)
                    records.append({
                        "source_row_id": row_id, "osm_id": osm_id, "part": part_index,
                        "name": name, "highway": highway, "lanes": lanes,
                        "width_m": width, "width_source": width_source, "tags": tags,
                        "source_length_m": projected.length, "game_length_m": reduced.length,
                        "points_xz_m": [[x, -y] for x, y in reduced.coords],
                    })
            processed = stats["source_candidates"] if include_missing_lanes else stats["lane_tagged_source_candidates"]
            if include_missing_lanes and processed % 10240 == 0:
                print(f"Read/compressed {processed:,} source road candidates; "
                      f"{len(records):,} clipped road parts", flush=True)
    return records, unscaled, compressed, node_sets, stats, crs, origin, clip, projection, ordered_nodes


def connected_components(node_sets):
    parents = list(range(len(node_sets)))
    owners = {}
    def find(index):
        while parents[index] != index:
            parents[index] = parents[parents[index]]
            index = parents[index]
        return index
    for index, nodes in enumerate(node_sets):
        for node in nodes:
            previous = owners.setdefault(node, index)
            parents[find(index)] = find(previous)
    groups = defaultdict(list)
    for index in range(len(parents)):
        groups[find(index)].append(index)
    return sorted(groups.values(), key=lambda group: (-len(group), group[0]))


def connectivity(node_sets):
    sizes = [len(group) for group in connected_components(node_sets)]
    return {
        "method": "shared source polyline vertices, rounded to 8 decimal degrees; not proximity snapping",
        "component_count": len(sizes), "largest_components_in_road_parts": sizes[:10],
        "isolated_road_parts": sizes.count(1),
    }


def prune_dead_ends(records, unscaled, compressed, ordered_nodes):
    """Peel degree-one source vertices, retaining the undirected graph's 2-core."""
    incident = defaultdict(set)
    endpoints, road_edges = [], []
    repeated_segments = 0
    for vertices in ordered_nodes:
        edges = []
        local_edges = {}
        for a, b in zip(vertices, vertices[1:]):
            if a == b:
                edges.append(None)
                continue
            key = tuple(sorted((a, b)))
            if key in local_edges:
                edges.append(local_edges[key])
                repeated_segments += 1
                continue
            edge = len(endpoints)
            local_edges[key] = edge
            endpoints.append((a, b))
            incident[a].add(edge)
            incident[b].add(edge)
            edges.append(edge)
        road_edges.append(edges)
    active = set(range(len(endpoints)))
    leaves = deque(node for node, edges in incident.items() if len(edges) == 1)
    while leaves:
        node = leaves.popleft()
        if len(incident[node]) != 1:
            continue
        edge = next(iter(incident[node]))
        active.remove(edge)
        for endpoint in endpoints[edge]:
            incident[endpoint].remove(edge)
            if len(incident[endpoint]) == 1:
                leaves.append(endpoint)
    if not active:
        raise ValueError("Dead-end pruning left no cycle-containing road core")

    kept_records, kept_unscaled, kept_compressed, kept_nodes = [], [], [], []
    fully_removed = partially_trimmed = split_parts = 0
    for record, original, reduced, vertices, edges in zip(records, unscaled, compressed, ordered_nodes, road_edges):
        kept_edges = [edge is not None and edge in active for edge in edges]
        if not any(kept_edges):
            fully_removed += 1
            continue
        if any(edge is not None and edge not in active for edge in edges):
            partially_trimmed += 1
        runs = []
        start = None
        for index in range(len(edges) + 1):
            keep = index < len(edges) and (kept_edges[index] or edges[index] is None)
            if keep and start is None:
                start = index
            elif not keep and start is not None:
                if any(kept_edges[start:index]):
                    runs.append((start, index))
                start = None
        split_parts += max(0, len(runs) - 1)
        for fragment, (start, end) in enumerate(runs):
            full_part = start == 0 and end == len(edges)
            source_line = original if full_part else LineString(list(original.coords)[start:end + 1])
            game_line = reduced if full_part else LineString(list(reduced.coords)[start:end + 1])
            kept_record = dict(record)
            kept_record.update({
                "source_length_m": source_line.length,
                "game_length_m": game_line.length,
                "points_xz_m": [[x, -y] for x, y in game_line.coords],
                "pruned_fragment": fragment,
                "source_part_vertex_range": [start, end],
            })
            kept_records.append(kept_record)
            kept_unscaled.append(source_line)
            kept_compressed.append(game_line)
            kept_nodes.append(set(vertices[start:end + 1]))
    report = {
        "mode": "recursive_dead_end_pruning",
        "method": "undirected source-vertex multigraph; repeatedly remove degree-one vertices and their edges",
        "road_parts_before": len(records), "road_parts_after": len(kept_records),
        "fully_removed_road_parts": fully_removed, "partially_trimmed_road_parts": partially_trimmed,
        "additional_parts_from_splitting": split_parts,
        "source_segments_before": len(endpoints), "source_segments_removed": len(endpoints) - len(active),
        "repeated_segments_ignored_for_degree": repeated_segments,
        "remaining_degree_one_vertices": sum(len(edges) == 1 for edges in incident.values()),
        "removed_game_centerline_length_km": (sum(g.length for g in compressed) - sum(g.length for g in kept_compressed)) / 1000,
        "notes": [
            "Prunes all dangling branches, regardless of length or highway class, including extract-boundary stubs.",
            "Keeps loops and routes joining loops; separate parallel source edges remain separate.",
            "Repeated traversal of the same segment within one road part does not create a graph cycle.",
            "Geometric crossings and compressed overlaps do not create graph connections.",
            "This is undirected topology, not a driving graph respecting one-way or access restrictions.",
        ],
    }
    return kept_records, kept_unscaled, kept_compressed, kept_nodes, report


def road_layer(record):
    tags = record["tags"]
    return tags.get("layer", "0"), tags.get("bridge", "no"), tags.get("tunnel", "no")


def local_axis_angle(a, b, point):
    vectors = []
    for line in (a, b):
        distance = line.project(point)
        delta = min(0.5, line.length / 4)
        start = line.interpolate(max(0, distance - delta))
        end = line.interpolate(min(line.length, distance + delta))
        dx, dy = end.x - start.x, end.y - start.y
        length = math.hypot(dx, dy)
        if length <= 1e-12:
            return None
        vectors.append((dx / length, dy / length))
    dot = abs(sum(x * y for x, y in zip(*vectors)))
    return math.degrees(math.acos(min(1.0, dot)))


def audit(records, unscaled, compressed, node_sets, output, diagnostics=False,
          junction_category="shared_source_junction"):
    # Widths remain in game metres; only centerline positions are compressed.
    footprints = [g.buffer(r["width_m"] / 2, cap_style="flat", join_style="mitre", mitre_limit=2)
                  for g, r in zip(compressed, records)]
    baseline = [g.buffer(r["width_m"] / 2, cap_style="flat", join_style="mitre", mitre_limit=2)
                for g, r in zip(unscaled, records)]
    tree = STRtree(footprints)
    counts, involved = Counter(), defaultdict(set)
    kinds, crossing_counts = Counter(), Counter()
    highlights = []
    columns = ["road_a", "road_b", "osm_id_a", "osm_id_b", "category", "overlap_m2"]
    if diagnostics:
        columns.extend(["overlap_kind", "local_axis_angle_deg", "centerlines_intersect", "centerline_gap_m"])
    with (output / "overlaps.csv").open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=columns)
        writer.writeheader()
        for i, footprint in enumerate(footprints):
            for j in tree.query(footprint, predicate="intersects"):
                j = int(j)
                if j <= i:
                    continue
                overlap = footprint.intersection(footprints[j])
                if overlap.area <= 1e-6:
                    continue
                if node_sets[i] & node_sets[j]:
                    category = junction_category
                elif road_layer(records[i]) != road_layer(records[j]):
                    category = "grade_or_structure_sensitive"
                elif baseline[i].intersects(baseline[j]):
                    category = "already_overlapping_at_1_to_1"
                else:
                    category = "introduced_by_compression"
                    highlights.extend(p for p in parts(overlap) if p.geom_type == "Polygon")
                counts[category] += 1
                involved[category].update((i, j))
                row = {"road_a": i, "road_b": j, "osm_id_a": records[i]["osm_id"],
                       "osm_id_b": records[j]["osm_id"], "category": category,
                       "overlap_m2": round(overlap.area, 6)}
                if diagnostics:
                    intersects = compressed[i].intersects(compressed[j])
                    angle = local_axis_angle(compressed[i], compressed[j], overlap.centroid)
                    kind = "centerline_crossing" if intersects else (
                        "parallel_or_aligned_candidate" if angle is not None and angle <= 15 else "converging_or_complex"
                    )
                    row.update({"overlap_kind": kind,
                                "local_axis_angle_deg": round(angle, 3) if angle is not None else "",
                                "centerlines_intersect": intersects,
                                "centerline_gap_m": round(compressed[i].distance(compressed[j]), 6)})
                    if category == "introduced_by_compression":
                        kinds[kind] += 1
                    if intersects and category != junction_category:
                        crossing_counts[category] += 1
                writer.writerow(row)
            if (i + 1) % 2000 == 0:
                print(f"Overlap audit: {i + 1}/{len(footprints)} road parts", flush=True)
    return footprints, highlights, {
        "pairs_by_category": dict(counts),
        "road_parts_involved_by_category": {key: len(value) for key, value in involved.items()},
        "introduced_overlap_area_m2_pairwise_sum": sum(p.area for p in highlights),
        **({"introduced_pairs_by_kind": dict(kinds),
            "centerline_crossings_without_shared_source_vertex_by_category": dict(crossing_counts),
            "parallel_candidate_method": "nonintersecting centerlines, local undirected axes within 15 degrees at overlap centroid"}
           if diagnostics else {}),
        "notes": [
            "Shared junctions are categorized, not proven safe at their widened game widths.",
            "Grade/structure-sensitive overlaps need bridge/tunnel decisions; this audit has no elevations.",
            "Pairwise areas may overlap each other and are not a unique covered-area total.",
            "This is an audit between distinct road parts; folds within a single part are not tested.",
            "No junction repair, road merging, curve smoothing or artificial reconnection is performed.",
        ],
    }


def preview(compressed, records, highlights, game_bounds, output, compression,
            include_missing_lanes=False, overlap_audited=True, filename="overview.png", road_classes=None):
    fig, ax = plt.subplots(figsize=(14, 11), dpi=160)
    segments = [list(g.coords) for g in compressed]
    class_selection = road_classes is not None and set(road_classes) != ROAD_CLASSES
    families = [record["highway"].split("_")[0] for record in records]
    colors = [HIGHWAY_COLORS.get(family, "#405b78") for family in families] if class_selection else "#405b78"
    ax.add_collection(LineCollection(segments, colors=colors, linewidths=0.65 if class_selection else 0.45,
                                    rasterized=True))
    if class_selection:
        handles = [Line2D([0], [0], color=color, linewidth=2, label=f"{family} + links")
                   for family, color in HIGHWAY_COLORS.items() if family in families]
        if any(family not in HIGHWAY_COLORS for family in families):
            handles.append(Line2D([0], [0], color="#405b78", linewidth=2, label="other selected classes"))
        ax.legend(handles=handles, loc="upper right", framealpha=0.95)
    patches = [PlotPolygon(list(g.exterior.coords), closed=True) for g in highlights]
    ax.add_collection(PatchCollection(patches, facecolors="#e54646", edgecolors="none", alpha=0.6))
    ax.set_xlim(game_bounds[0], game_bounds[2])
    ax.set_ylim(game_bounds[1], game_bounds[3])
    ax.set_aspect("equal")
    ax.set_xlabel("Game metres east of area center")
    ax.set_ylabel("Game metres north of area center")
    label = "selected backbone roads" if class_selection else (
        "all vehicle roads" if include_missing_lanes else "lane-tagged roads"
    )
    detail = "red: introduced road-footprint overlaps" if overlap_audited else "source centerlines"
    ax.set_title(f"Bengaluru {label} · 1:{compression:g}\n"
                 f"{len(records):,} road parts · {detail}")
    ax.grid(alpha=0.15)
    fig.tight_layout()
    fig.savefig(output / filename)
    plt.close(fig)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("--bounds", nargs=4, type=float, default=[77.413, 12.857, 77.907, 13.236],
                        metavar=("WEST", "SOUTH", "EAST", "NORTH"))
    parser.add_argument("--compression", type=float, default=20.0)
    parser.add_argument("--lane-width", type=float, default=3.0, help="Game metres per lane when width is missing")
    parser.add_argument("--include-roads-without-lanes", action="store_true",
                        help="Read all vehicle road classes, regardless of lane metadata")
    parser.add_argument("--centerlines-only", action="store_true",
                        help="Export centerlines without width inference or footprint overlap auditing")
    parser.add_argument("--highway-classes", nargs="+", choices=sorted(ROAD_CLASSES), default=sorted(ROAD_CLASSES),
                        help="Select specific highway classes before compression and pruning")
    parser.add_argument("--keep-largest-component", action="store_true",
                        help="Keep only the largest group linked by shared source vertices, measured in road parts")
    parser.add_argument("--prune-dead-ends", action="store_true",
                        help="Recursively trim all dangling branches using shared source vertices")
    parser.add_argument("--output", type=Path, default=ROOT / ".stage_authoring/bengaluru/source")
    args = parser.parse_args()
    if not math.isfinite(args.compression) or args.compression < 1:
        parser.error("Compression must be finite and at least 1")
    if not math.isfinite(args.lane_width) or args.lane_width <= 0:
        parser.error("Lane width must be finite and positive")
    west, south, east, north = args.bounds
    if not (-180 <= west < east <= 180 and -90 <= south < north <= 90):
        parser.error("Bounds must be an ordered longitude/latitude rectangle")
    source = args.source.resolve(strict=True)
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    result = read_roads(source, args.bounds, args.compression, args.lane_width,
                        args.include_roads_without_lanes, args.centerlines_only, args.highway_classes)
    records, unscaled, compressed, nodes, stats, crs, origin, clip, projection, ordered_nodes = result
    del result
    if not records:
        raise SystemExit("No vehicle roads survived filtering and clipping")
    game_rectangle = scale(translate(transform(clip, projection.transform, interleaved=False), -origin[0], -origin[1]),
                           xfact=1 / args.compression, yfact=1 / args.compression, origin=(0, 0))
    game_bounds = game_rectangle.bounds
    before_pruning = {
        "road_parts": len(records),
        "source_ways": len({r["osm_id"] for r in records}),
        "game_centerline_length_km": sum(g.length for g in compressed) / 1000,
        "connectivity": connectivity(nodes),
    }
    if args.centerlines_only and args.prune_dead_ends:
        print("Saving the compressed network preview before pruning", flush=True)
        preview(compressed, records, [], game_bounds, output, args.compression,
                args.include_roads_without_lanes, False, "compressed_overview.png", args.highway_classes)
    component_filter = {"mode": "all_components"}
    if args.keep_largest_component:
        before = connectivity(nodes)
        keep = connected_components(nodes)[0]
        kept_indices = set(keep)
        component_filter = {
            "mode": "largest_component_by_road_part_count",
            "tie_break": "earliest source road part in input order",
            "connectivity_before": before,
            "road_parts_before": len(records),
            "removed_components": before["component_count"] - 1,
            "removed_road_parts": len(records) - len(keep),
            "removed_game_centerline_length_km": sum(
                line.length for index, line in enumerate(compressed) if index not in kept_indices
            ) / 1000,
        }
        records = [records[index] for index in keep]
        unscaled = [unscaled[index] for index in keep]
        compressed = [compressed[index] for index in keep]
        nodes = [nodes[index] for index in keep]
        ordered_nodes = [ordered_nodes[index] for index in keep]
        print(f"Kept largest component: {len(records):,} road parts; removed "
              f"{component_filter['removed_road_parts']:,} parts in "
              f"{component_filter['removed_components']:,} groups", flush=True)
    straggler_filter = {"mode": "none"}
    if args.prune_dead_ends:
        records, unscaled, compressed, nodes, straggler_filter = prune_dead_ends(
            records, unscaled, compressed, ordered_nodes
        )
        print(f"Pruned dead ends: {straggler_filter['fully_removed_road_parts']:,} parts removed; "
              f"{straggler_filter['partially_trimmed_road_parts']:,} trimmed; "
              f"{len(records):,} retained", flush=True)
        del ordered_nodes
    if args.centerlines_only:
        highlights = []
        overlap = {"status": "not_run", "reason": "centerline-only compression and topology pruning"}
    else:
        print(f"Retained {len(records):,} clipped road parts; auditing widths at 1:{args.compression:g}", flush=True)
        footprints, highlights, overlap = audit(records, unscaled, compressed, nodes, output)
    graph = connectivity(nodes)
    report = {
        "source": str(source), "source_size_bytes": source.stat().st_size,
        "attribution": "Map data (c) OpenStreetMap contributors; extract created by BBBike",
        "source_bounds_lon_lat": args.bounds, "projected_crs": f"EPSG:{crs}",
        "projected_origin_m": origin, "compression": args.compression,
        "game_axes": "X east, Y up, Z south; road elevations not authored",
        "default_lane_width_m": None if args.centerlines_only else args.lane_width,
        "geometry_mode": "centerlines_only" if args.centerlines_only else "constant_width_footprint_audit",
        "width_policy": "explicit widths retained as source metadata only; no inferred widths or authored road surfaces"
                        if args.centerlines_only else "explicit width or lane count times default lane width, preserved in game metres",
        "filter": ("selected vehicle road classes regardless of lane metadata" if args.include_roads_without_lanes
                   else "vehicle road classes with an explicit positive integer lanes tag; other roads omitted")
                  + ("; only the largest shared-source-vertex component retained" if args.keep_largest_component else "")
                  + ("; dangling branches recursively pruned" if args.prune_dead_ends else ""),
        "retained_highway_classes": dict(Counter(r["highway"] for r in records)),
        "selected_highway_classes": sorted(set(args.highway_classes)),
        "source_filter_stats": dict(stats), "road_parts": len(records),
        "source_filter_stats_scope": "after road selection and clipping, before component selection or branch pruning",
        "compressed_network_before_pruning": before_pruning,
        "retained_source_ways": len({r["osm_id"] for r in records}),
        "retained_road_parts_with_lane_count": sum(r["lanes"] is not None for r in records),
        "retained_road_parts_without_lane_count": sum(r["lanes"] is None for r in records),
        "component_filter": component_filter,
        "straggler_filter": straggler_filter,
        "source_centerline_length_km": sum(g.length for g in unscaled) / 1000,
        "game_centerline_length_km": sum(g.length for g in compressed) / 1000,
        "game_rectangle_bounds_east_north_m": game_bounds,
        "game_rectangle_size_km": [(game_bounds[2] - game_bounds[0]) / 1000,
                                   (game_bounds[3] - game_bounds[1]) / 1000],
        "game_rectangle_area_km2": game_rectangle.area / 1_000_000,
        "connectivity": graph, "overlaps": overlap,
        "tool_versions": {"shapely": shapely.__version__, "pyproj": pyproj.__version__, "matplotlib": matplotlib.__version__},
    }
    with (output / "roads.json").open("w") as handle:
        json.dump({"metadata": report, "roads": records}, handle, separators=(",", ":"))
    (output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    preview(compressed, records, highlights, game_bounds, output, args.compression,
            args.include_roads_without_lanes, not args.centerlines_only, road_classes=args.highway_classes)
    print(json.dumps({"road_parts": len(records), "connectivity": graph, "overlaps": overlap, "output": str(output)}, indent=2))


if __name__ == "__main__":
    main()
