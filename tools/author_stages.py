#!/usr/bin/env python3
"""Author complete rally2gpx-compatible GPX routes; no network or runtime dependencies.

python3 tools/author_stages.py
Godot then bakes these recipes with tools/bake_stages.gd into native .res assets.
"""
import argparse
import bisect
import hashlib
import json
import math
from pathlib import Path
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
SPACING = 2.0


def cumulative(points):
    result = [0.0]
    for a, b in zip(points, points[1:]):
        result.append(result[-1] + math.dist(a, b))
    return result


def sample(points, distances, at):
    i = min(len(points) - 2, max(0, bisect.bisect_right(distances, at) - 1))
    t = (at - distances[i]) / (distances[i + 1] - distances[i])
    return tuple(a + (b - a) * t for a, b in zip(points[i], points[i + 1]))


def read_gpx(path):
    root = ET.parse(path).getroot()
    # Never join disjoint segments or laps silently.
    segments = root.findall('.//{*}trkseg')
    if len(segments) == 1:
        nodes = segments[0].findall('{*}trkpt')
    elif not segments:
        routes = root.findall('{*}rte')
        if len(routes) != 1:
            raise ValueError('Expected one complete track segment or route')
        nodes = routes[0].findall('{*}rtept')
    else:
        raise ValueError('Multiple track segments require explicit authoring')
    gps = [(float(p.attrib['lon']), float(p.attrib['lat'])) for p in nodes]
    if len(gps) < 2 or any(not (-180 <= lon <= 180 and -90 <= lat <= 90) for lon, lat in gps):
        raise ValueError('Invalid geographic coordinates')
    lon0, lat0 = gps[0]
    points = []
    for lon, lat in gps:
        point = (math.radians(lon - lon0) * 6371008.8 * math.cos(math.radians(lat0)),
                 math.radians(lat - lat0) * 6371008.8)
        if not points or math.dist(point, points[-1]) > 0.05:
            points.append(point)
    if len(points) < 2:
        raise ValueError('Route has no length')
    return points


def author(entry):
    path = ROOT / entry['gpx']
    original = read_gpx(path)
    distance = cumulative(original)
    length = distance[-1]
    if length < 500 or length > 60000:
        raise ValueError('Unsupported complete route length')
    count = math.ceil(length / SPACING)
    # Full endpoints, no scaling, trimming, reversal or repeated laps. A small
    # 4 m triangular filter removes GPS kinks without flattening rally bends.
    points = [sample(original, distance, i * length / count) for i in range(count + 1)]
    for _ in range(2):
        points = [points[0]] + [tuple((points[i-1][axis] + 2*points[i][axis] + points[i+1][axis])/4
                                     for axis in range(2)) for i in range(1, count)] + [points[-1]]
    distances = cumulative(points)
    headings = []
    for i in range(count + 1):
        a, b = points[max(0, i-1)], points[min(count, i+1)]
        yaw = math.atan2(b[0]-a[0], b[1]-a[1])
        if headings:
            yaw = headings[-1] + (yaw-headings[-1]+math.pi) % (2*math.pi)-math.pi
        headings.append(yaw)
    initial = headings[0]
    c, s = math.cos(initial), math.sin(initial)
    points = [(x*c-z*s, x*s+z*c) for x, z in points]
    headings = [h-initial for h in headings]
    curvature = [(headings[i+1]-headings[i])/(distances[i+1]-distances[i]) for i in range(count)]
    minimum_radius = 1 / max(abs(k) for k in curvature)
    # Existing procedural stages stay 12 m wide. Real rally roads use a saved
    # 6 m width, shared by rendering, collision, shoulders, spawn and finish.
    width = entry.get('road_width', 6.0)
    if minimum_radius < width / 2 + 0.5:
        raise ValueError(f'Inner road edge folds (radius {minimum_radius:.1f} m)')
    buckets = {}
    clearance = width + 2.0
    for i, (x, z) in enumerate(points):
        cell = (math.floor(x / clearance), math.floor(z / clearance))
        for dx in (-1, 0, 1):
            for dz in (-1, 0, 1):
                for j in buckets.get((cell[0]+dx, cell[1]+dz), []):
                    if distances[i]-distances[j] > 40 and math.dist(points[i], points[j]) < clearance:
                        raise ValueError(f'Nonlocal road/shoulder overlap at {distances[i]:.0f} m')
        buckets.setdefault(cell, []).append(i)
    labels = [0 if abs(k) < 0.0015 else (1 if k > 0 else -1) for k in curvature]
    # Remove very brief kinks so pacenotes describe useful contiguous bends.
    def runs(values):
        start = 0
        for i in range(1, len(values)+1):
            if i == len(values) or values[i] != values[start]:
                yield start, i, values[start]
                start = i
    for first, last, direction in list(runs(labels)):
        if direction and (distances[last]-distances[first] < 12 or abs(headings[last]-headings[first]) < 0.045):
            labels[first:last] = [0] * (last-first)
    features = []
    radii = [14, 22, 35, 55, 90, 140]
    for first, last, direction in runs(labels):
        radius = 1 / max(abs(k) for k in curvature[first:last]) if direction else 0
        grade = min(range(6), key=lambda j: abs(math.log(radius/radii[j])))+1 if direction else 0
        features.append({'kind': 'corner' if direction else 'straight', 'first_station': first,
                         'last_station': last, 'direction': 'Right' if direction > 0 else 'Left',
                         'grade': grade, 'angle': headings[last]-headings[first],
                         'surface': entry['surface'], 'start_m': distances[first],
                         'end_m': distances[last], 'length_m': distances[last]-distances[first]})
    if abs(distances[-1]-length) / length > 0.01:
        raise ValueError('Smoothing changed route length by over 1%')
    seed = int(hashlib.sha256(entry['id'].encode()).hexdigest()[:8], 16) & 0x7fffffff
    return dict(entry, centers=[[round(x, 5), 0, round(z, 5)] for x, z in points],
                headings=headings, distances=distances, features=features, seed=seed,
                source_length_m=length, length_m=distances[-1], road_width=width,
                minimum_radius_m=minimum_radius, source_sha256=hashlib.sha256(path.read_bytes()).hexdigest())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--manifest', type=Path, default=ROOT/'assets/tracks/selection.json')
    parser.add_argument('--output', type=Path, default=ROOT/'assets/tracks/recipes.json')
    args = parser.parse_args()
    entries = json.loads(args.manifest.read_text())['stages']
    recipes = [author(entry) for entry in entries]
    args.output.write_text(json.dumps({'version': 1, 'stages': recipes}, ensure_ascii=False, separators=(',', ':'))+'\n')
    print(f'Authored {len(recipes)} complete routes, {sum(r["length_m"] for r in recipes)/1000:.1f} km')


if __name__ == '__main__':
    main()
