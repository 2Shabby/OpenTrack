#!/usr/bin/env python3
"""Bake the approved city layout into bounded native Godot road geometry."""
import argparse
import hashlib
import json
import math
import shutil
import subprocess
import tempfile
from collections import Counter, defaultdict
from pathlib import Path

import numpy as np
import shapely
from shapely.geometry import Point, box
from shapely.ops import unary_union
from shapely.strtree import STRtree

from audit_city_roads import ROOT, parts, road_layer
from repair_city_overlaps import lines_for, read_polygon, surface_partition
from review_city_overlaps import is_bridge
from author_city_bridges import bake as bake_bridges


GROUND = ('0', 'no', 'no')
TOOLS = Path(__file__).resolve().parent


def cross_sections(records, policy):
    output = []
    for row in records:
        lanes = policy['bridge_lanes_per_direction'] if is_bridge(row) else (
            policy['motorway_lanes_per_direction'] if row['highway'] == 'motorway' else policy['road_lanes_per_direction'])
        lane, median = policy['lane_width_m'], policy['median_width_m']
        output.append({**row, 'lanes_per_direction': lanes, 'lane_width_m': lane, 'median_width_m': median,
                       'carriageway_width_m': lane * lanes, 'width_m': 2 * lane * lanes + median,
                       'lane_center_offsets_from_median_m': {'left': [median / 2 + (i + .5) * lane for i in range(lanes)],
                                                            'right': [median / 2 + (i + .5) * lane for i in range(lanes)]}})
    return output


class MeshWriter:
    def __init__(self, path, chunk):
        self.file = path.open('wb')
        self.chunk = chunk
        self.meshes = []
        self.triangles = Counter()
        self.omitted_area = 0.
        self.pending = defaultdict(list)

    def add(self, faces, kind, origin, collision=True):
        faces = np.asarray(faces, dtype=np.float32).reshape(-1, 3, 3)
        if not len(faces):
            return
        normal = np.cross(faces[:, 1] - faces[:, 0], faces[:, 2] - faces[:, 0])
        valid = np.linalg.norm(normal, axis=1) > 1e-8
        self.omitted_area += float(np.linalg.norm(normal[~valid], axis=1).sum()) / 2
        faces = faces[valid]
        if not len(faces):
            return
        if not np.isfinite(faces).all():
            raise ValueError('Non-finite mesh positions')
        self.pending[kind, tuple(origin), collision].append(faces)
        self.triangles[kind] += len(faces)

    def polygons(self, geometry, kind, height=0., collision=True):
        polygons = [p for p in parts(geometry) if p.geom_type == 'Polygon' and p.area > 1e-8]
        if not polygons:
            return
        tree = STRtree(polygons)
        bounds = geometry.bounds
        for ix in range(math.floor(bounds[0] / self.chunk), math.floor(bounds[2] / self.chunk) + 1):
            for iy in range(math.floor(bounds[1] / self.chunk), math.floor(bounds[3] / self.chunk) + 1):
                tile = box(ix * self.chunk, iy * self.chunk, (ix + 1) * self.chunk, (iy + 1) * self.chunk)
                indexes = tree.query(tile, predicate='intersects')
                if not len(indexes):
                    continue
                clipped = unary_union([polygons[int(i)].intersection(tile) for i in indexes])
                origin = [(ix + .5) * self.chunk, 0., -(iy + .5) * self.chunk]
                triangles = []
                for polygon in parts(clipped):
                    if polygon.geom_type != 'Polygon' or polygon.area < 1e-8:
                        continue
                    cells = shapely.constrained_delaunay_triangles(polygon)
                    if abs(sum(t.area for t in cells.geoms) - polygon.area) > 1e-5:
                        raise ValueError('Triangulation changes road coverage')
                    for t in cells.geoms:
                        xy = list(t.exterior.coords)[:3]
                        if not t.exterior.is_ccw:
                            xy.reverse()
                        triangles.extend([[x - origin[0], height, -y - origin[2]] for x, y in xy])
                self.add(triangles, kind, origin, collision)

    def close(self):
        for (kind, origin, collision), values in sorted(self.pending.items()):
            faces = np.concatenate(values)
            offset = self.file.tell()
            raw = faces.astype('<f4').tobytes()
            self.file.write(raw)
            self.meshes.append({'kind': kind, 'origin': list(origin), 'offset': offset, 'bytes': len(raw),
                                'triangles': len(faces), 'collision': collision})
        self.file.close()
        self.pending.clear()


def lane_markings(records, allowed, policy):
    patches = []
    for row, line in zip(records, lines_for(records)):
        if road_layer(row) != GROUND:
            continue
        for lane in range(1, row['lanes_per_direction']):
            offset = row['median_width_m'] / 2 + lane * row['lane_width_m']
            for sign in (-1, 1):
                parallel = line.offset_curve(sign * offset, join_style='mitre')
                for piece in parts(parallel):
                    if piece.geom_type != 'LineString':
                        continue
                    for distance in np.arange(0, piece.length, 6):
                        end = min(distance + 3, piece.length)
                        if end - distance < .15:
                            continue
                        from shapely.ops import substring
                        patches.append(substring(piece, distance, end).buffer(.075, cap_style='flat'))
    return unary_union(patches).intersection(allowed) if patches else Point().buffer(0)


def choose_spawn(records, floor):
    for row, line in sorted(zip(records, lines_for(records)), key=lambda v: -v[1].length):
        if road_layer(row) != GROUND or line.length < 30:
            continue
        for distance in np.arange(8, line.length - 8, 4):
            p, q = line.interpolate(distance), line.interpolate(distance + 1)
            tangent = np.asarray(q.coords[0]) - np.asarray(p.coords[0])
            tangent /= np.linalg.norm(tangent)
            left = np.asarray([-tangent[1], tangent[0]])
            xy = np.asarray(p.coords[0]) + left * 2.
            if floor.covers(Point(xy).buffer(2.5)):
                return {'position': [float(xy[0]), 0., float(-xy[1])],
                        'forward': [float(tangent[0]), 0., float(-tangent[1])]}
    raise ValueError('No supported driving spawn')


def ground_geometry(records, recipe, writer):
    ground = [r for r in records if road_layer(r) == GROUND]
    bodies, junctions, medians, stats = surface_partition(ground, {**recipe['overlap_repair'], 'ensure_centerline_coverage': True})
    road = unary_union([read_polygon(r) for r in bodies + junctions])
    median = unary_union([read_polygon(r) for r in medians])
    floor = road.union(median)
    writer.polygons(road, 'asphalt')
    writer.polygons(median, 'median')
    markings = lane_markings(ground, unary_union([read_polygon(r) for r in bodies]), recipe['game_track'])
    writer.polygons(markings, 'paint', .015, False)
    probes = []
    for row, line in zip(ground, lines_for(ground)):
        if line.length < 2:
            continue
        p = line.interpolate(.5, normalized=True)
        probes.append([p.x, 0., -p.y])
    return floor, choose_spawn(ground, floor), probes, stats


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--city', type=Path, default=ROOT / '.stage_authoring/bengaluru')
    parser.add_argument('--output', type=Path, default=ROOT / 'assets/city/bengaluru/city.scn')
    parser.add_argument('--roads-only', action='store_true')
    args = parser.parse_args()
    city = args.city.resolve(strict=True)
    recipe = json.loads((TOOLS / 'bengaluru_backbone.json').read_text())
    raw = (city / 'current/roads.json').read_bytes()
    data = json.loads(raw)
    records = cross_sections(data['roads'], recipe['game_track'])
    with tempfile.TemporaryDirectory(prefix='.bake-', dir=city) as temporary:
        work = Path(temporary)
        writer = MeshWriter(work / 'track.bin', recipe['game_track']['collision_chunk_m'])
        print('Baking welded ground roads, motorway widths and lane dividers', flush=True)
        floor, spawn, probes, stats = ground_geometry(records, recipe, writer)
        structure_stats = {}
        bridge_spawn = None
        if not args.roads_only:
            print('Authoring bridge decks, exact-position ramps and underpasses', flush=True)
            structure_probes, structure_stats, bridge_spawn = bake_bridges(records, recipe['game_track'], writer)
            probes.extend(structure_probes)
        writer.close()
        manifest = {'format': 'city-track-v1', 'source_sha256': hashlib.sha256(raw).hexdigest(),
                    'policy': recipe['game_track'], 'spawn': spawn, 'probes': probes, 'meshes': writer.meshes,
                    'stats': {'triangles': dict(writer.triangles), 'chunks': len(writer.meshes), 'ground': stats,
                              'roads_only': args.roads_only, 'omitted_area_m2': writer.omitted_area}}
        manifest['stats']['structures'] = structure_stats
        manifest['bridge_spawn'] = bridge_spawn
        (work / 'track.json').write_text(json.dumps(manifest, separators=(',', ':')))
        args.output.parent.mkdir(parents=True, exist_ok=True)
        godot = shutil.which('godot')
        if not godot:
            raise ValueError('Godot is required to bake the native scene')
        result = subprocess.run([godot, '--headless', '--path', str(ROOT), '--script', str(TOOLS / 'bake_city_scene.gd'), '--',
                                 '--input=' + str(work), '--output=' + str(args.output.resolve())],
                                stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, timeout=900)
        print(result.stdout, end='', flush=True)
        # Godot can exit zero after a script parse error; require the save marker.
        if result.returncode or 'CITY_BAKED triangles ' not in result.stdout:
            raise RuntimeError('Native Godot bake did not save a verified city scene')
        for name in ('track.json', 'track.bin'):
            (work / name).replace(city / 'current' / name)
    print('Baked city scene:', args.output, flush=True)


if __name__ == '__main__':
    main()
