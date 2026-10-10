"""Author source-preserving grade decks and ramps with explicit crossing heights."""
import math
from collections import defaultdict

import numpy as np
import shapely
from scipy.spatial import Delaunay
from shapely.geometry import LineString, MultiPoint, Point, Polygon, box
from shapely.ops import nearest_points, unary_union
from shapely.strtree import STRtree

from audit_city_roads import connected_components, parts, road_layer
from repair_city_overlaps import lines_for


GROUND = ('0', 'no', 'no')


def rank(row):
    if road_layer(row) == GROUND:
        return 0
    layer = int(row['tags'].get('layer', '0'))
    if layer < 0 or row['tags'].get('tunnel', 'no') not in ('no', '', '0'):
        return max(-2, min(-1, layer))
    return max(1, min(2, layer))


class HeightField:
    def __init__(self, anchors):
        self.xy = np.asarray(list(anchors), dtype=float)
        self.values = np.asarray([anchors[p][0] for p in anchors], dtype=float)
        self.triangulation = Delaunay(self.xy)
        self.hull = MultiPoint(self.xy).convex_hull

    def sample(self, points):
        points = np.asarray(points).reshape(-1, 2)
        indexes = self.triangulation.find_simplex(points, tol=1e-7)
        if np.any(indexes < 0):
            points = points.copy()
            for i in np.flatnonzero(indexes < 0):
                if self.hull.distance(Point(points[i])) > 1e-5:
                    raise ValueError('Bridge surface extends outside its height constraints')
                edge = np.asarray(nearest_points(self.hull, Point(points[i]))[0].coords[0])
                points[i] = edge + (self.xy.mean(axis=0) - edge) * 1e-9
            indexes = self.triangulation.find_simplex(points, tol=1e-6)
            if np.any(indexes < 0):
                raise ValueError('Cannot resolve bridge boundary height')
        transform = self.triangulation.transform[indexes]
        first = np.einsum('ijk,ik->ij', transform[:, :2], points - transform[:, 2])
        weights = np.column_stack([first, 1 - first.sum(axis=1)])
        return np.sum(weights * self.values[self.triangulation.simplices[indexes]], axis=1)


def layout(records, policy):
    lines = lines_for(records)
    levels = [rank(r) for r in records]
    grades = [i for i, level in enumerate(levels) if level]
    ground_nodes = {n for r, level in zip(records, levels) if not level for n in r['node_ids']}
    groups = connected_components([set(records[i]['node_ids']) for i in grades])
    owner = {grades[i]: g for g, values in enumerate(groups) for i in values}
    positions = {n: (x, -z) for r in records for n, (x, z) in zip(r['node_ids'], r['points_xz_m'])}
    ground_interfaces = [Point(positions[n]) for n in ground_nodes
                         if any(n in records[i]['node_ids'] for i in grades)]
    interface_tree = STRtree(ground_interfaces)
    tree = STRtree(lines)
    crossings, forced = [], defaultdict(list)
    for a in grades:
        for b in tree.query(lines[a], predicate='intersects'):
            b = int(b)
            if levels[a] == levels[b] or (levels[b] and b <= a):
                continue
            shared_nodes = set(records[a]['node_ids']) & set(records[b]['node_ids'])
            intersection = lines[a].intersection(lines[b])
            points = [intersection] if intersection.geom_type == 'Point' else list(getattr(intersection, 'geoms', []))
            for p in points:
                if p.geom_type != 'Point':
                    continue
                if any(p.distance(Point(positions[n])) < 1e-5 for n in shared_nodes):
                    continue
                if len(ground_interfaces) and p.distance(ground_interfaces[int(interface_tree.nearest(p))]) < 1e-5:
                    continue
                crossings.append((a, b, p))
                if levels[a]: forced[a].append(lines[a].project(p))
                if levels[b]: forced[b].append(lines[b].project(p))
    crossing_groups = {owner[i] for pair in crossings for i in pair[:2] if i in owner}
    spacing = policy['bridge_clearance_m'] + policy['deck_thickness_m'] + 1.2
    group_scale = {g: (1. if g in crossing_groups else min(1., sum(lines[grades[i]].length for i in values) / 60))
                   for g, values in enumerate(groups)}
    node_targets = defaultdict(list)
    for i in grades:
        for n in records[i]['node_ids']:
            node_targets[n].append(levels[i] * spacing * group_scale[owner[i]])
    node_heights = {n: (0. if n in ground_nodes else min(values, key=abs)) for n, values in node_targets.items()}
    surfaces, anchors, profiles = defaultdict(list), defaultdict(dict), {}
    maximum_grade, steep = 0., []

    def anchor(level, xy, h, priority):
        key = tuple(round(float(v), 6) for v in xy)
        previous = anchors[level].get(key)
        if previous is None or priority > previous[1]:
            anchors[level][key] = (float(h), priority)
        elif priority == previous[1]:
            anchors[level][key] = ((previous[0] + float(h)) / 2, priority)

    for i in grades:
        row, line, level = records[i], lines[i], levels[i]
        xy = np.asarray(line.coords)
        distances = np.concatenate([[0.], np.cumsum(np.linalg.norm(np.diff(xy, axis=0), axis=1))])
        controls = {float(d): node_heights[n] for d, n in zip(distances, row['node_ids'])}
        target = level * spacing * group_scale[owner[i]]
        # Even a sub-metre source culvert stays continuous; only genuine road
        # crossings force the full vehicle-clearance height.
        for d in (min(25., line.length / 3), line.length / 2, max(line.length - 25., line.length * 2 / 3)):
            if 1e-8 < d < line.length - 1e-8:
                controls.setdefault(d, target)
        for d in forced[i]:
            controls[d] = level * spacing
        keys = sorted(controls)
        heights = [controls[k] for k in keys]
        positions_s = sorted(set(keys + np.linspace(0, line.length, max(2, math.ceil(line.length / policy['mesh_step_m']) + 1)).tolist()))
        values = np.interp(positions_s, keys, heights)
        slopes = [abs((heights[k + 1] - heights[k]) / (keys[k + 1] - keys[k])) for k in range(len(keys) - 1) if keys[k + 1] - keys[k] > 1e-8]
        slope = max(slopes, default=0)
        maximum_grade = max(maximum_grade, slope)
        if slope > policy['maximum_ramp_grade']:
            steep.append({'segment': i, 'maximum_grade': slope, 'length_m': line.length, 'names': row['names']})
        stations = []
        for d, h in zip(positions_s, values):
            point = np.asarray(line.interpolate(d).coords[0])
            delta = min(.01, line.length / 10)
            direction = np.asarray(line.interpolate(min(line.length, d + delta)).coords[0]) - np.asarray(line.interpolate(max(0, d - delta)).coords[0])
            if np.linalg.norm(direction) < 1e-9:
                direction = xy[-1] - xy[0]
            if np.linalg.norm(direction) < 1e-9:
                direction = xy[1] - xy[0]
            direction /= np.linalg.norm(direction)
            normal = np.asarray([-direction[1], direction[0]])
            width = row['width_m']
            taper = min(20., line.length / 2)
            for endpoint, distance in ((row['node_ids'][0], d), (row['node_ids'][-1], line.length - d)):
                if endpoint in ground_nodes:
                    ground_width = max(r['width_m'] for r in records if endpoint in r['node_ids'] and rank(r) == 0)
                    width = max(width, row['width_m'] + (ground_width - row['width_m']) * max(0., 1 - distance / taper))
            left, right = point + normal * width / 2, point - normal * width / 2
            priority = 4 if d in (0., line.length) else (3 if d in forced[i] else 1)
            for p in (point, left, right): anchor(level, p, h, priority)
            stations.append((left, right))
        quads = []
        for (la, ra), (lb, rb) in zip(stations, stations[1:]):
            polygon = shapely.make_valid(Polygon([la, ra, rb, lb]))
            quads.extend(p for p in parts(polygon) if p.geom_type == 'Polygon' and p.area > 1e-8)
        footprint = unary_union(quads)
        if line.difference(footprint.buffer(1e-6)).length > 1e-4:
            # Sharp compressed returns can fold a loft: include straight
            # source intervals without moving the source centerline.
            extra = [LineString([a, b]).buffer(row['width_m'] / 2, cap_style='flat') for a, b in zip(xy, xy[1:]) if not np.array_equal(a, b)]
            footprint = footprint.union(unary_union(extra))
        for polygon in parts(footprint):
            if polygon.geom_type != 'Polygon':continue
            for ring in (polygon.exterior, *polygon.interiors):
                for p in ring.coords:
                    h = np.interp(line.project(Point(p)), keys, heights)
                    anchor(level, p, h, 0)
        surfaces[level].append(footprint)
        profiles[i] = {'distances': keys, 'heights': heights}
    fields = {level: HeightField(values) for level, values in anchors.items()}
    floor = {level: unary_union(values) for level, values in surfaces.items()}
    # Check the actual continuous height fields, not just nominal source tags.
    clearance = []
    for a, b, point in crossings:
        ha = float(fields[levels[a]].sample([point.coords[0]])[0]) if levels[a] else 0.
        hb = float(fields[levels[b]].sample([point.coords[0]])[0]) if levels[b] else 0.
        gap = abs(ha - hb) - policy['deck_thickness_m']
        if gap < policy['bridge_clearance_m'] - .05:
            raise ValueError(f'Bridge crossing lacks clearance: {a}/{b}: {gap:.3f}m')
        clearance.append({'point': [point.x, max(ha, hb), -point.y], 'under': [point.x, min(ha, hb), -point.y], 'clearance_m': gap})
    return floor, fields, profiles, {'grade_segments': len(grades), 'bridge_source_groups': len(groups),
                                     'crossings_checked': len(clearance), 'minimum_clearance_m': min((c['clearance_m'] for c in clearance), default=None),
                                     'maximum_ramp_grade': maximum_grade, 'steep_ramps': steep,
                                     'crossings': clearance, 'horizontal_centerlines_preserved': True}


def bake(records, policy, writer):
    floors, fields, profiles, report = layout(records, policy)
    probes = []
    chunk = writer.chunk
    thickness = policy['deck_thickness_m']
    for level, floor in floors.items():
        field = fields[level]
        polygons = [p for p in parts(floor) if p.geom_type == 'Polygon']
        tree = STRtree(polygons)
        median = unary_union([line.buffer(row['median_width_m'] / 2, cap_style='flat')
                              for row, line in zip(records, lines_for(records)) if rank(row) == level]).intersection(floor)
        for ids in field.triangulation.simplices:
            xy = field.xy[ids]
            triangle = Polygon(xy)
            if triangle.area < 1e-9:continue
            indexes = tree.query(triangle, predicate='intersects')
            if not len(indexes):continue
            clipped = unary_union([polygons[int(i)].intersection(triangle) for i in indexes])
            for polygon in parts(clipped):
                if polygon.geom_type != 'Polygon' or polygon.area < 1e-8:continue
                bounds = polygon.bounds
                for ix in range(math.floor(bounds[0] / chunk), math.floor(bounds[2] / chunk) + 1):
                    for iy in range(math.floor(bounds[1] / chunk), math.floor(bounds[3] / chunk) + 1):
                        cell = polygon.intersection(box(ix * chunk, iy * chunk, (ix + 1) * chunk, (iy + 1) * chunk))
                        origin = [(ix + .5) * chunk, 0., -(iy + .5) * chunk]
                        for piece in parts(cell):
                            if piece.geom_type != 'Polygon' or piece.area < 1e-8:continue
                            faces, undersides = [], []
                            for t in shapely.constrained_delaunay_triangles(piece).geoms:
                                pts = list(t.exterior.coords)[:3]
                                if not t.exterior.is_ccw:pts.reverse()
                                heights = field.sample(pts)
                                face = [[p[0] - origin[0], float(h), -p[1] - origin[2]] for p, h in zip(pts, heights)]
                                faces.extend(face)
                                undersides.extend([[p[0], p[1] - thickness, p[2]] for p in reversed(face)])
                            writer.add(faces, 'bridge' if level > 0 else 'structure', origin)
                            writer.add(undersides, 'bridge' if level > 0 else 'structure', origin)
                            paint = piece.intersection(median)
                            for patch in parts(paint):
                                if patch.geom_type != 'Polygon' or patch.area < 1e-8:continue
                                for t in shapely.constrained_delaunay_triangles(patch).geoms:
                                    pts = list(t.exterior.coords)[:3]
                                    if not t.exterior.is_ccw:pts.reverse()
                                    heights = field.sample(pts) + .015
                                    writer.add([[p[0] - origin[0], float(h), -p[1] - origin[2]] for p, h in zip(pts, heights)], 'median', origin, False)
        for polygon in polygons:
            for ring in (polygon.exterior, *polygon.interiors):
                coords = list(ring.coords)
                for a, b in zip(coords, coords[1:]):
                    edge = LineString([a, b])
                    steps = max(1, math.ceil(edge.length / policy['mesh_step_m']))
                    distances = list(np.linspace(0, edge.length, steps + 1))
                    for axis in (0, 1):
                        low, high = sorted((a[axis], b[axis]))
                        for position in np.arange((math.floor(low / chunk) + 1) * chunk, high, chunk):
                            distances.append(float((position - a[axis]) / (b[axis] - a[axis]) * edge.length))
                    xy = [edge.interpolate(d).coords[0] for d in sorted(set(distances))]
                    heights = field.sample(xy)
                    for k in range(len(xy) - 1):
                        p, q = xy[k], xy[k + 1]
                        origin = [(math.floor((p[0] + q[0]) / 2 / chunk) + .5) * chunk, 0., -(math.floor((p[1] + q[1]) / 2 / chunk) + .5) * chunk]
                        pa = [p[0] - origin[0], float(heights[k]), -p[1] - origin[2]]
                        pb = [q[0] - origin[0], float(heights[k + 1]), -q[1] - origin[2]]
                        qa, qb = [pa[0], pa[1] - thickness, pa[2]], [pb[0], pb[1] - thickness, pb[2]]
                        writer.add([pa, pb, qb, pa, qb, qa], 'bridge' if level > 0 else 'structure', origin)
        for row, line in zip(records, lines_for(records)):
            if rank(row) != level:continue
            points = [line.interpolate(d) for d in (0., line.length / 2, line.length)]
            heights = field.sample([p.coords[0] for p in points])
            probes.extend([[p.x, float(h), -p.y] for p, h in zip(points, heights)])
    return probes, report, driving_spawn(records, floors, fields)


def driving_spawn(records, floors, fields):
    candidates = sorted(zip(records, lines_for(records)), key=lambda item: -item[1].length)
    for row, line in candidates:
        level = rank(row)
        if level <= 0 or line.length < 25:
            continue
        for distance in np.arange(8, line.length - 12, 3):
            p, q = line.interpolate(distance), line.interpolate(distance + 1)
            direction = np.asarray(q.coords[0]) - np.asarray(p.coords[0])
            direction /= np.linalg.norm(direction)
            normal = np.asarray([-direction[1], direction[0]])
            xy = np.asarray(p.coords[0]) + normal * 2
            points = [xy + direction * d for d in (-2, 0, 2, 6, 10)]
            if not all(floors[level].covers(Point(v).buffer(1.1)) for v in points):continue
            h = fields[level].sample(points)
            if np.ptp(h) > .08 or h[1] < 2:continue
            return {'position': [float(xy[0]), float(h[1]), float(-xy[1])],
                    'forward': [float(direction[0]), 0., float(-direction[1])]}
    raise ValueError('No level bridge test spawn is available')
