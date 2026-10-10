#!/usr/bin/env python3
"""Review non-bridge road overlaps and group the red preview marks into regions."""
import argparse
import csv
import heapq
import json
import math
import textwrap
from collections import Counter, defaultdict
from pathlib import Path

from audit_city_roads import parts, road_layer
import matplotlib.pyplot as plt
from matplotlib.collections import LineCollection, PatchCollection
from matplotlib.path import Path as PlotPath
from matplotlib.patches import PathPatch
from shapely.geometry import GeometryCollection, LineString, Point
from shapely.geometry.polygon import orient
from shapely.ops import unary_union
from shapely.strtree import STRtree


COLORS = {"direct_junction_local": "#dfb33b", "connected_nearby_local": "#e59538",
          "junction_spill": "#cc4545", "corridor_parallel_overlap": "#cc4545",
          "corridor_converging_overlap": "#cc4545", "non_bridge_level_change": "#9167b3"}


def is_bridge(record):
    return str(record["tags"].get("bridge", "no")).lower() not in {"", "no", "false", "0"}


class JunctionContext:
    def __init__(self, records, radius):
        self.records, self.radius = records, radius
        self.coords, self.graph, self.cache = {}, defaultdict(list), {}
        for record in records:
            for node, (x, z) in zip(record["node_ids"], record["points_xz_m"]):
                point = (x, -z)
                if node in self.coords and self.coords[node] != point:
                    raise ValueError("Shared graph node has inconsistent positions")
                self.coords[node] = point
            if not is_bridge(record):
                a, b = record["node_ids"][0], record["node_ids"][-1]
                layer = road_layer(record)
                self.graph[a].append((b, record["game_length_m"], layer))
                self.graph[b].append((a, record["game_length_m"], layer))

    def reachable(self, start, layer):
        key = (start, layer)
        if key not in self.cache:
            distances, queue = {start: 0.0}, [(0.0, start)]
            while queue:
                distance, node = heapq.heappop(queue)
                if distance != distances[node]:
                    continue
                for other, length, edge_layer in self.graph[node]:
                    candidate = distance + length
                    if edge_layer == layer and candidate <= self.radius and candidate < distances.get(other, math.inf):
                        distances[other] = candidate
                        heapq.heappush(queue, (candidate, other))
            self.cache[key] = frozenset(distances)
        return self.cache[key]

    def nearby_nodes(self, index):
        road = self.records[index]
        layer = road_layer(road)
        return self.reachable(road["node_ids"][0], layer) | self.reachable(road["node_ids"][-1], layer)

    def mask(self, nodes, overlap):
        disks = [Point(self.coords[node]).buffer(self.radius, quad_segs=8) for node in sorted(nodes)
                 if Point(self.coords[node]).distance(overlap) <= self.radius]
        return unary_union(disks) if disks else GeometryCollection()


def classify_pair(a, b, overlap, angle, context):
    records = context.records
    if is_bridge(records[a]) or is_bridge(records[b]):
        return "bridge_excluded", overlap.area, GeometryCollection()
    if road_layer(records[a]) != road_layer(records[b]):
        return "non_bridge_level_change", overlap.area, overlap
    shared = set(records[a]["node_ids"]) & set(records[b]["node_ids"])
    direct = context.mask(shared, overlap)
    direct_spill = overlap.difference(direct)
    if shared and direct_spill.area <= 0.01:
        return "direct_junction_local", 0.0, GeometryCollection()
    common = context.nearby_nodes(a) & context.nearby_nodes(b)
    local = context.mask(common | shared, overlap)
    spill = overlap.difference(local)
    if common and spill.area <= 0.01:
        return "connected_nearby_local", 0.0, GeometryCollection()
    if overlap.area - spill.area > 0.01:
        return "junction_spill", spill.area, spill
    return ("corridor_parallel_overlap" if angle is not None and angle <= 15 else "corridor_converging_overlap",
            overlap.area, overlap)


def build_regions(pair_geometries, records):
    merged = unary_union([geometry for _, _, geometry in pair_geometries])
    geometries = sorted((p for p in parts(merged) if p.geom_type == "Polygon" and p.area > 1e-6),
                        key=lambda p: (-p.area, p.bounds))
    if not geometries:
        return [], []
    tree = STRtree(geometries)
    members = [set() for _ in geometries]
    for a, b, geometry in pair_geometries:
        for index in tree.query(geometry, predicate="intersects"):
            index = int(index)
            if geometry.intersection(geometries[index]).area > 1e-6:
                members[index].update((a, b))
    reports = []
    for index, (geometry, roads) in enumerate(zip(geometries, members), 1):
        corners = list(geometry.minimum_rotated_rectangle.exterior.coords)
        lengths = [math.dist(a, b) for a, b in zip(corners, corners[1:])]
        long, short = max(lengths), min(lengths)
        names = sorted({name for i in roads for name in records[i]["names"]})
        x, y = geometry.representative_point().coords[0]
        reports.append({"region": index, "unique_overlap_area_m2": geometry.area,
                        "bounds_east_north_m": list(geometry.bounds), "label_point_east_north_m": [x, y],
                        "long_extent_m": long, "short_extent_m": short,
                        "shape_hint": "extended_corridor" if long >= 30 and long >= short * 4 else "compact_or_complex",
                        "road_indexes": sorted(roads), "road_names": names,
                        "footprint_xz_m": {"exterior": [[x, -y] for x, y in geometry.exterior.coords],
                                           "holes": [[[x, -y] for x, y in ring.coords] for ring in geometry.interiors]}})
    return reports, geometries


def patches(geometries):
    result = []
    for geometry in geometries:
        for polygon in parts(geometry):
            if polygon.geom_type != "Polygon":
                continue
            polygon = orient(polygon, sign=1.0)
            vertices, codes = [], []
            for ring in [polygon.exterior, *polygon.interiors]:
                coords = list(ring.coords)
                vertices.extend(coords)
                codes.extend([PlotPath.MOVETO, *([PlotPath.LINETO] * (len(coords) - 2)), PlotPath.CLOSEPOLY])
            result.append(PathPatch(PlotPath(vertices, codes)))
    return result


def draw_map(records, lines, highlights, bounds, path, title, regions=None):
    fig, ax = plt.subplots(figsize=(14, 11), dpi=160)
    ax.add_collection(LineCollection([list(g.coords) for g, r in zip(lines, records) if not is_bridge(r)],
                                    colors="#77828a", linewidths=0.7, alpha=0.75))
    for category, geometries in highlights.items():
        ax.add_collection(PatchCollection(patches(geometries), facecolors=COLORS.get(category, "#cc4545"),
                                         edgecolors="none", alpha=0.8, rasterized=True))
    if regions:
        for region in regions[:8]:
            x, y = region["label_point_east_north_m"]
            ax.annotate(str(region["region"]), (x, y), xytext=(5, 5), textcoords="offset points",
                        fontsize=9, bbox={"facecolor": "white", "alpha": 0.9, "edgecolor": "#777"})
    ax.set_xlim(bounds[0], bounds[2])
    ax.set_ylim(bounds[1], bounds[3])
    ax.set_aspect("equal")
    ax.set_title(title)
    ax.set_xlabel("Game metres east of area center")
    ax.set_ylabel("Game metres north of area center")
    ax.grid(alpha=0.15)
    fig.tight_layout()
    fig.savefig(path)
    plt.close(fig)


def draw_details(records, lines, regions, geometries, path, title="Largest red-marked conflict regions"):
    if not regions:
        return
    fig, axes = plt.subplots(2, 3, figsize=(18, 12), dpi=150, layout="constrained")
    for ax, region, geometry in zip(axes.flat, regions[:6], geometries[:6]):
        minx, miny, maxx, maxy = geometry.bounds
        padding = max(12, max(maxx - minx, maxy - miny) * 0.08)
        roads = [i for i, line in enumerate(lines) if not is_bridge(records[i]) and
                 line.bounds[0] <= maxx + padding and line.bounds[2] >= minx - padding and
                 line.bounds[1] <= maxy + padding and line.bounds[3] >= miny - padding]
        surfaces = [lines[i].buffer(records[i]["width_m"] / 2, cap_style="flat", join_style="mitre", mitre_limit=2) for i in roads]
        ax.add_collection(PatchCollection(patches(surfaces), facecolors="#c7cbd0", edgecolors="none", alpha=0.65))
        ax.add_collection(PatchCollection(patches([geometry]), facecolors="#cf3f3f", edgecolors="none", alpha=0.85))
        ax.add_collection(LineCollection([list(lines[i].coords) for i in roads], colors="#4f677c", linewidths=0.7))
        span = max(maxx - minx, maxy - miny) + padding * 2
        center_x, center_y = (minx + maxx) / 2, (miny + maxy) / 2
        ax.set_xlim(center_x - span / 2, center_x + span / 2)
        ax.set_ylim(center_y - span / 2, center_y + span / 2)
        ax.set_aspect("equal")
        names = ", ".join(region["road_names"][:2]) or "Unnamed corridors"
        ax.set_title(f"Region {region['region']} · {region['unique_overlap_area_m2']:.0f} m²\n{textwrap.fill(names, 48)}", fontsize=10)
        ax.grid(alpha=0.2)
    for ax in list(axes.flat)[min(6, len(regions)):]:
        ax.set_visible(False)
    fig.suptitle(title + " · bridge overlaps excluded\nGrey: 13 m road footprints · blue: median centerlines · red: overlap", fontsize=14)
    fig.savefig(path)
    plt.close(fig)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("folder", type=Path, help="Dual-carriageway layout directory")
    parser.add_argument("--junction-radius", type=float, default=13.0, help="Game metres, diagnostic junction neighborhood")
    args = parser.parse_args()
    if not math.isfinite(args.junction_radius) or args.junction_radius <= 0:
        parser.error("Junction radius must be finite and positive")
    folder = args.folder.resolve(strict=True)
    data = json.loads((folder / "roads.json").read_text())
    if data["metadata"]["format"] != "dual-carriageway-layout-v1":
        raise ValueError("Expected a dual-carriageway layout export")
    records = data["roads"]
    lines = []
    for r in records:
        coords = r["points_xz_m"]
        if len(coords) != len(r["node_ids"]) or len(coords) < 2 or any(not all(math.isfinite(v) for v in p) for p in coords):
            raise ValueError("Invalid road coordinates or graph nodes")
        line = LineString([(x, -z) for x, z in coords])
        if not line.is_valid or line.length <= 0 or not math.isclose(line.length, r["game_length_m"], abs_tol=1e-7):
            raise ValueError("Invalid corridor length")
        if not math.isfinite(r["width_m"]) or r["width_m"] <= 0:
            raise ValueError("Invalid road width")
        lines.append(line)
    footprints = [line.buffer(r["width_m"] / 2, cap_style="flat", join_style="mitre", mitre_limit=2)
                  for line, r in zip(lines, records)]
    context = JunctionContext(records, args.junction_radius)
    rows, counts, source_counts, marked_counts = [], Counter(), Counter(), Counter()
    marked_pairs, residual_pairs = [], []
    highlights = defaultdict(list)
    with (folder / "overlaps.csv").open() as handle:
        for row in csv.DictReader(handle):
            a, b = int(row["road_a"]), int(row["road_b"])
            if not 0 <= a < b < len(records) or row["osm_id_a"] != records[a]["osm_id"] or row["osm_id_b"] != records[b]["osm_id"]:
                raise ValueError("Overlap CSV indexes differ from the layout")
            source_counts[row["category"]] += 1
            marked = row["category"] == "introduced_by_compression"
            overlap = footprints[a].intersection(footprints[b])
            if not math.isclose(overlap.area, float(row["overlap_m2"]), abs_tol=0.01):
                raise ValueError("Overlap geometry differs from the layout audit")
            angle = float(row["local_axis_angle_deg"]) if row["local_axis_angle_deg"] else None
            category, residual_area, residual = classify_pair(a, b, overlap, angle, context)
            counts[category] += 1
            if marked:
                marked_counts[category] += 1
            if category != "bridge_excluded":
                rows.append({"road_a": a, "road_b": b, "corridor_a": records[a]["osm_id"],
                             "corridor_b": records[b]["osm_id"], "original_category": row["category"],
                             "red_marked": marked, "review_category": category,
                             "overlap_m2": round(overlap.area, 6), "outside_local_neighborhood_m2": round(residual_area, 6),
                             "names_a": "; ".join(records[a]["names"]), "names_b": "; ".join(records[b]["names"])})
                highlights[category].append(overlap)
                if marked:
                    marked_pairs.append((a, b, overlap))
                if not residual.is_empty and category != "non_bridge_level_change":
                    residual_pairs.append((a, b, residual))
            if sum(source_counts.values()) % 5000 == 0:
                print(f"Reviewed {sum(source_counts.values()):,} overlap pairs", flush=True)
    if source_counts != Counter(data["metadata"]["overlaps"]["pairs_by_category"]):
        raise ValueError("Overlap CSV is incomplete")
    regions, region_geometries = build_regions(marked_pairs, records)
    residual_regions, residual_geometries = build_regions(residual_pairs, records)
    report = {"scope": "all overlapping road pairs except those involving any bridge-tagged corridor",
              "marked_scope": "red areas from the layout preview: introduced_by_compression pairs",
              "junction_radius_m": args.junction_radius, "pair_counts": dict(counts), "red_marked_pair_counts": dict(marked_counts),
              "non_bridge_overlap_pairs": len(rows), "red_marked_non_bridge_pairs": len(marked_pairs),
              "red_marked_regions": len(regions), "red_marked_unique_area_m2": sum(g.area for g in region_geometries),
              "outside_junction_regions": len(residual_regions),
              "outside_junction_unique_area_m2": sum(r["unique_overlap_area_m2"] for r in residual_regions),
              "notes": [f"Local junction labels describe geometry within a {args.junction_radius:g}m neighborhood; they do not certify safe lane or median connections.",
                        "Nearby local overlaps can be separate road sections connected by short links, rather than disconnected roads.",
                        "Non-bridge level/tunnel differences remain separate review items; they are not automatic planar merge candidates.",
                        "Regions union overlapping polygons, so region counts and areas avoid pair-count duplication.",
                        "Region areas measure unique 2D projected overlap, not stacked 3D surfaces or a merged routing graph.",
                        "Shape hints are geometric descriptions, not road topology or automatic repair decisions.",
                        "This review keeps all existing layout geometry unchanged and does not generate junction meshes."]}
    output = folder / "overlap-review"
    output.mkdir(exist_ok=True)
    with (output / "pairs.csv").open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=["road_a", "road_b", "corridor_a", "corridor_b", "original_category",
                                                   "red_marked", "review_category", "overlap_m2",
                                                   "outside_local_neighborhood_m2", "names_a", "names_b"])
        writer.writeheader()
        writer.writerows(rows)
    for name, values in (("marked_regions", regions), ("outside_junction_regions", residual_regions)):
        (output / f"{name}.json").write_text(json.dumps({"axes": "X east, Z south, game metres", "regions": values}, separators=(",", ":")))
    (output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    bounds = data["metadata"]["game_rectangle_bounds_east_north_m"]
    draw_map(records, lines, highlights, bounds, output / "overview.png",
             "Non-bridge overlaps · yellow: direct junction · orange: nearby connected sections\nRed: corridor overlap/spill · purple: different level/tunnel; local labels need junction design")
    draw_map(records, lines, {"marked": region_geometries}, bounds, output / "marked_overlaps.png",
             f"Red-marked non-bridge overlaps · {len(regions):,} regions · {report['red_marked_unique_area_m2']:,.0f} m² unique area", regions)
    draw_details(records, lines, regions, region_geometries, output / "marked_details.png")
    draw_map(records, lines, {"spill": residual_geometries}, bounds, output / "outside_junctions.png",
             f"Non-bridge corridor conflicts outside local junction neighborhoods\n{len(residual_regions):,} regions · {report['outside_junction_unique_area_m2']:,.0f} m² unique area", residual_regions)
    draw_details(records, lines, residual_regions, residual_geometries, output / "outside_junction_details.png",
                 "Largest corridor conflicts outside junction neighborhoods")
    print(json.dumps({key: report[key] for key in ("pair_counts", "red_marked_pair_counts", "red_marked_regions",
                                                  "red_marked_unique_area_m2", "outside_junction_regions", "outside_junction_unique_area_m2")}, indent=2))
    print(json.dumps({"largest_marked_regions": [{key: r[key] for key in ("region", "unique_overlap_area_m2", "long_extent_m", "shape_hint", "road_names")}
                                                for r in regions[:3]]}, indent=2))


if __name__ == "__main__":
    main()
