#!/usr/bin/env python3
"""Translate approved physical-arm marks into repeatable source-centerline cuts."""
import hashlib
import json
from collections import defaultdict

import numpy as np
import shapely
from shapely.geometry import LineString
from shapely.ops import substring, unary_union
from shapely.strtree import STRtree

from audit_city_roads import connected_components, parts, road_layer
from node_city_intersections import coordinate_key, node_roads, validate_intersections
from repair_city_overlaps import lines_for, read_polygon
from review_city_pruning import GROUND, make_network, selected_edges
from review_city_surface_branches import rasterize, skeleton_records


def digest(roads):
    return hashlib.sha256(json.dumps(roads, sort_keys=True).encode()).hexdigest()


def cut_line(line, removed_tree, retained_tree, step=0.5):
    """Cut where the marked physical arm is closer than the retained network.

    Only the transition positions are sampled. Output keeps original vertices,
    and crossing positions are refined to 0.1 mm on the original polyline.
    """
    if not len(removed_tree.geometries) or not len(retained_tree.geometries):
        raise ValueError("A cut needs both removed and retained physical paths")

    def selected(distances):
        points = shapely.line_interpolate_point(line, distances)
        _, removed = removed_tree.query_nearest(points, return_distance=True, all_matches=False)
        _, retained = retained_tree.query_nearest(points, return_distance=True, all_matches=False)
        return (removed <= 8.0) & (removed + 0.25 < retained)

    positions = np.linspace(0, line.length, max(2, int(np.ceil(line.length / step)) + 1))
    states = selected(positions)
    transitions = np.flatnonzero(states[:-1] != states[1:])
    low, high = positions[transitions].copy(), positions[transitions + 1].copy()
    for _ in range(14):
        if not len(low):
            break
        middle = (low + high) / 2
        same = selected(middle) == states[transitions]
        low = np.where(same, middle, low)
        high = np.where(same, high, middle)
    cuts = [0, *((low + high) / 2).tolist(), line.length]
    keep, remove = [], []
    state = bool(states[0])
    for a, b in zip(cuts, cuts[1:]):
        if b - a > 1e-7:
            (remove if state else keep).append(substring(line, a, b))
        state = not state
    return keep, remove


def approved_plan(data, surfaces, branch_review, pruning):
    report = pruning['report']
    if digest(data['roads']) != report['roads_geometry_sha256']:
        raise ValueError("Pruning marks do not match current road geometry")
    if branch_review['surface_review']['surface_sha256'] != report['surface_sha256']:
        raise ValueError('Initial arm review and follow-up marks describe different surfaces')
    floor = unary_union([read_polygon(r) for key in ('road_bodies', 'junctions', 'medians')
                         for r in surfaces[key] if tuple(r['structure_profile']) == GROUND])
    cell = report['cell_size_m']
    print('Mapping approved marks onto original road polylines', flush=True)
    mask, minx, miny = rasterize(floor, cell)
    records, _ = skeleton_records(mask, minx, miny, cell)
    coordinates, edges, ground_edges, mapping = make_network(records, data['roads'], cell, floor)
    if mapping['unmatched_grade_attachments']:
        raise ValueError('Cannot prune with unmatched grade attachments')
    original = [b for b in branch_review['surface_branches'] if b['branch'] in report['initial_surface_arm_ids']]
    if len(original) != report['initial_removed_arms']:
        raise ValueError('An approved initial arm is missing from the current review')
    selected = selected_edges(original, ground_edges)
    grade_indexes = set()
    for mark in pruning['marks']:
        grade_indexes.update(mark['current_grade_segment_indexes'])
        for path in mark['paths_xz_m']:
            xy = [(x, -z) for x, z in path]
            for a, b in zip(xy, xy[1:]):
                key = tuple(sorted((coordinate_key(a), coordinate_key(b))))
                if key in ground_edges:
                    selected.add(ground_edges[key])
    removed = [LineString([coordinates[e['a']], coordinates[e['b']]]) for i, e in enumerate(edges)
               if e['role'] == 'ground' and i in selected]
    retained = [LineString([coordinates[e['a']], coordinates[e['b']]]) for i, e in enumerate(edges)
                if e['role'] == 'ground' and i not in selected]
    removed_tree, retained_tree = STRtree(removed), STRtree(retained)
    near_removed = unary_union(removed).buffer(8.1)
    cuts = []
    for index, (record, line) in enumerate(zip(data['roads'], lines_for(data['roads']))):
        if road_layer(record) != GROUND:
            pieces = [line] if index in grade_indexes else []
        elif line.intersects(near_removed):
            _, pieces = cut_line(line, removed_tree, retained_tree)
        else:
            pieces = []
        for piece in pieces:
            cuts.append({'structure_profile': list(road_layer(record)),
                         'points_xz_m': [[x, -y] for x, y in piece.coords]})
    if not cuts:
        raise ValueError('Approved marks produced no actual road cuts')
    return {'format': 'city-pruning-plan-v1', 'source_geometry_sha256': data['metadata']['source_geometry_sha256'],
            'approved_initial_arms': len(original), 'approved_follow_up_marks': len(pruning['marks']),
            'cuts': cuts}


def apply_plan(records, plan):
    grouped = defaultdict(list)
    for cut in plan['cuts']:
        grouped[tuple(cut['structure_profile'])].append(LineString([(x, -z) for x, z in cut['points_xz_m']]))
    masks = {profile: unary_union(lines).buffer(1e-7, cap_style='square') for profile, lines in grouped.items()}
    positions = {(road_layer(r), coordinate_key((x, -z))): node for r in records
                 for node, (x, z) in zip(r['node_ids'], r['points_xz_m'])}
    next_node = max(n for r in records for n in r['node_ids']) + 1
    clipped = []
    removed_length = 0
    for record, line in zip(records, lines_for(records)):
        profile = road_layer(record)
        geometry = line.difference(masks[profile]) if profile in masks else line
        for piece in parts(geometry):
            if piece.geom_type != 'LineString' or piece.length <= 1e-6:
                continue
            ids = []
            for xy in piece.coords:
                key = (profile, coordinate_key(xy))
                if key not in positions:
                    positions[key] = next_node
                    next_node += 1
                ids.append(positions[key])
            clipped.append({**record, 'osm_id': f'cut-{len(clipped):05d}', 'node_ids': ids,
                            'points_xz_m': [[x, -y] for x, y in piece.coords], 'game_length_m': piece.length})
        removed_length += line.length - geometry.length
    noded, checks = node_roads(clipped)
    # New dead-end chains and isolated groups created by actual cuts must not
    # survive merely because the metre-sampled diagnostic missed a source loop.
    active = set(range(len(noded)))
    tip_length = 0
    for _ in range(len(noded)):
        incident = defaultdict(list)
        for i in active:
            for n in (noded[i]['node_ids'][0], noded[i]['node_ids'][-1]):
                incident[n].append(i)
        tips = {values[0] for values in incident.values() if len(values) == 1}
        if not tips:
            break
        tip_length += sum(noded[i]['game_length_m'] for i in tips)
        active.difference_update(tips)
    survivors = [r for i, r in enumerate(noded) if i in active]
    if not survivors:
        raise ValueError('Approved cuts removed the entire road graph')
    groups = connected_components([{r['node_ids'][0], r['node_ids'][-1]} for r in survivors])
    groups.sort(key=lambda g: -sum(survivors[i]['game_length_m'] for i in g))
    kept = [survivors[i] for i in sorted(groups[0])]
    island_length = sum(r['game_length_m'] for r in survivors) - sum(r['game_length_m'] for r in kept)
    checks.update(validate_intersections(kept))
    return kept, {'approved_initial_arms': plan['approved_initial_arms'],
                  'approved_follow_up_marks': plan['approved_follow_up_marks'],
                  'explicit_cut_length_m': removed_length, 'new_dead_end_length_removed_m': tip_length,
                  'disconnected_groups_removed': len(groups) - 1, 'disconnected_length_removed_m': island_length,
                  'remaining_components': 1, 'input_segments': len(records), 'output_segments': len(kept),
                  'remaining_length_km': sum(r['game_length_m'] for r in kept) / 1000,
                  'intersection_checks': checks}
