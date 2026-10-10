#!/usr/bin/env python3
"""Build one current city layout and its review without retaining intermediate exports."""
import argparse
import hashlib
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

from audit_city_roads import ROOT
from repair_city_overlaps import draw_repair, surface_partition, write_obj
from review_city_backbone import geometry_digest
from node_city_intersections import dangling_branches, draw_review
from prune_city_roads import apply_plan, approved_plan, digest


TOOLS = Path(__file__).resolve().parent


def run(script, *args):
    subprocess.run([sys.executable, str(TOOLS / script), *(str(a) for a in args)], check=True, timeout=900)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--city", type=Path, default=ROOT / ".stage_authoring/bengaluru")
    parser.add_argument("--from-source", action="store_true", help="Rebuild alignment and repairs from the locked reference")
    parser.add_argument("--remove-candidates", action="store_true", help="Apply all currently approved initial and follow-up pruning marks")
    args = parser.parse_args()
    city = args.city.resolve(strict=True)
    source = city / "source"
    recipe_path = TOOLS / "bengaluru_backbone.json"
    recipe = json.loads(recipe_path.read_text())
    reference = json.loads((source / "roads.json").read_text())
    if len(reference["roads"]) != recipe["expected_road_parts"] or geometry_digest(reference["roads"]) != recipe["geometry_sha256"]:
        raise ValueError("Source selection differs from the locked recipe")
    plan_path = source / "pruning.json"
    plan = json.loads(plan_path.read_text()) if plan_path.exists() else None
    if args.remove_candidates:
        folder = city / "current"
        surface_bytes = (folder / "surfaces.json").read_bytes()
        marks = json.loads((city / "review/pruning.json").read_text())
        if hashlib.sha256(surface_bytes).hexdigest() != marks["report"]["surface_sha256"]:
            raise ValueError("Pruning marks do not match the current surface")
        existing = json.loads((folder / "roads.json").read_text())
        if digest(existing["roads"]) != marks["report"]["roads_geometry_sha256"]:
            raise ValueError("Pruning marks do not match the current roads")
        if not marks["report"]["initial_removed_arms"] and not marks["marks"]:
            print("No marked pruning candidates remain; current layout preserved", flush=True)
            return
        new_plan = approved_plan(existing, json.loads(surface_bytes),
                                 json.loads((city / "review/dangling.json").read_text()), marks)
        if plan:
            new_plan["cuts"] = plan["cuts"] + new_plan["cuts"]
            for key in ("approved_initial_arms", "approved_follow_up_marks"):
                new_plan[key] += plan[key]
        plan = new_plan
    if plan and plan["source_geometry_sha256"] != recipe["geometry_sha256"]:
        raise ValueError("Pruning plan belongs to a different source selection")
    with tempfile.TemporaryDirectory(prefix=".build-", dir=city) as temporary:
        work = Path(temporary)
        seed_path = source / "layout_seed.json"
        if args.from_source:
            reference_dir = work / "reference"
            reference_dir.mkdir()
            for name in ("roads.json", "report.json", "overlaps.csv"):
                shutil.copy2(source / name, reference_dir / name)
            run("align_city_carriageways.py", reference_dir, "--recipe", recipe_path)
            run("repair_city_overlaps.py", reference_dir / "dual-carriageway", "--recipe", recipe_path)
            seed_path = reference_dir / "dual-carriageway/repaired/roads.json"
        seed = json.loads(seed_path.read_text())
        if seed["metadata"]["source_geometry_sha256"] != recipe["geometry_sha256"]:
            raise ValueError("Layout seed is not derived from the locked reference")
        current, review = work / "current", work / "review"
        run("node_city_intersections.py", seed_path, "--output", current, "--review", review)
        data = json.loads((current / "roads.json").read_text())
        metadata = data["metadata"]
        if plan:
            print("Applying approved road cuts and cleaning resulting disconnected/dead-end segments", flush=True)
            data["roads"], metadata["pruning"] = apply_plan(data["roads"], plan)
            metadata["corridors"] = len(data["roads"])
            metadata["layout_centerline_length_km"] = metadata["pruning"]["remaining_length_km"]
            metadata["intersection_splitting"] = metadata["pruning"]["intersection_checks"]
            metadata["connected_components"] = 1
            metadata["notes"] = ["Approved physical-arm cuts are applied to original centerlines before surface and mesh generation.",
                                  "New source-graph dead-end chains and disconnected groups are removed after the cuts.",
                                  "Same-profile crossings remain explicitly split; grade crossings retain source attachment semantics."]
            branches, graph, coordinates = dangling_branches(data["roads"])
            (review / "dangling.json").write_text(json.dumps({"axes": "X east, Z south", "branches": branches}, separators=(",", ":")))
            (review / "junctions.json").write_text(json.dumps({"junctions": [{"node": n, "point_xz_m": [coordinates[n][0], -coordinates[n][1]], "segment_indexes": values}
                                                                               for n, values in sorted(graph.items()) if len(values) >= 3]}, separators=(",", ":")))
            draw_review(data["roads"], branches, graph, coordinates, metadata["game_rectangle_bounds_east_north_m"], review)
        print("Building the shared road surface and ground meshes", flush=True)
        bodies, junctions, medians, surface_stats = surface_partition(data["roads"] if plan else seed["roads"], recipe["overlap_repair"])
        surface = {"axes": "X east, Z south; no elevations", "road_bodies": bodies, "junctions": junctions, "medians": medians}
        surface_path = current / "surfaces.json"
        surface_path.write_text(json.dumps(surface, separators=(",", ":")))
        metadata["repair"]["surface_partition"] = surface_stats
        metadata["repair"]["mesh_export_checks"] = {}
        for name, values, key in (("ground_road_surface", bodies + junctions, "ground_road_surface_triangles"),
                                  ("ground_medians", medians, "ground_median_triangles")):
            checks = {}
            metadata["repair"][key] = write_obj([r for r in values if r["structure_profile"] == ["0", "no", "no"]], current / f"{name}.obj", checks)
            metadata["repair"]["mesh_export_checks"][name] = checks
        metadata["surface_geometry_sha256"] = hashlib.sha256(surface_path.read_bytes()).hexdigest()
        (current / "roads.json").write_text(json.dumps(data, separators=(",", ":")))
        (current / "report.json").write_text(json.dumps(metadata, indent=2) + "\n")
        draw_repair(data["roads"], bodies, junctions, medians,
                    metadata["game_rectangle_bounds_east_north_m"], review / "overview.png")
        run("review_city_surface_branches.py", current, "--review", review)
        review_data = json.loads((review / "dangling.json").read_text())
        metadata["surface_branch_review"] = review_data["surface_review"]
        run("review_city_pruning.py", current, "--review", review)
        metadata["pruning_simulation"] = json.loads((review / "pruning.json").read_text())["report"]
        (current / "roads.json").write_text(json.dumps(data, separators=(",", ":")))
        (current / "report.json").write_text(json.dumps(metadata, indent=2) + "\n")
        if args.from_source:
            shutil.copy2(seed_path, work / "layout_seed.json")
        replaced = []
        try:
            for name in ("current", "review"):
                target = city / name
                if target.exists():
                    target.rename(work / f"old_{name}")
                (work / name).rename(target)
                replaced.append(name)
            if args.from_source:
                (work / "layout_seed.json").replace(source / "layout_seed.json")
            if args.remove_candidates:
                (work / "pruning.json").write_text(json.dumps(plan, separators=(",", ":")))
                (work / "pruning.json").replace(plan_path)
        except BaseException:
            for name in reversed(replaced):
                shutil.rmtree(city / name)
            for name in ("current", "review"):
                previous = work / f"old_{name}"
                if previous.exists():
                    previous.rename(city / name)
            raise
    print(f"Current layout: {city / 'current'}\nReview: {city / 'review'}", flush=True)


if __name__ == "__main__":
    main()
