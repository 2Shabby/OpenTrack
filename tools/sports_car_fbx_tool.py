#!/usr/bin/env python3
"""Inspect and repair the single SportsCar FBX used by the game.

Run through Blender:
    /opt/homebrew/bin/blender --background --python tools/sports_car_fbx_tool.py -- --inspect
    /opt/homebrew/bin/blender --background --python tools/sports_car_fbx_tool.py -- --write
"""

from __future__ import annotations

import argparse
import os
import sys
from pathlib import Path

import bpy
from mathutils import Matrix, Vector


REPO_ROOT = Path(__file__).resolve().parents[1]
SPORTS_CAR_FBX = REPO_ROOT / "assets" / "cars" / "fbx" / "SportsCar.fbx"
REPAIRED_FBX = SPORTS_CAR_FBX.with_name("SportsCar.repaired.fbx")
WHEEL_HUB_MATERIAL = "Grey"

ROLE_BY_SOURCE_NAME = {
    "frontleftwheel": "FrontLeftWheel",
    "frontrightwheel": "FrontRightWheel",
    "backwheels": "BackWheels",
    "cylinder.013": "FrontLeftWheel",
    "cylinder.014": "FrontRightWheel",
    "cylinder.004": "BackWheels",
}


def parse_args() -> argparse.Namespace:
    try:
        blender_separator = sys.argv.index("--")
        argv = sys.argv[blender_separator + 1 :]
    except ValueError:
        argv = []

    parser = argparse.ArgumentParser()
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--inspect", action="store_true")
    mode.add_argument("--write", action="store_true")
    parser.add_argument("--asset", type=Path, default=SPORTS_CAR_FBX)
    return parser.parse_args(argv)


def reset_scene() -> None:
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.object.delete()


def import_fbx(path: Path) -> None:
    if not path.exists():
        raise FileNotFoundError(path)
    bpy.ops.import_scene.fbx(filepath=str(path))


def mesh_objects() -> list[bpy.types.Object]:
    return sorted(
        [obj for obj in bpy.context.scene.objects if obj.type == "MESH"],
        key=lambda obj: obj.name,
    )


def object_bounds(obj: bpy.types.Object) -> tuple[Vector, Vector]:
    corners = [obj.matrix_world @ Vector(corner) for corner in obj.bound_box]
    minimum = Vector((min(corner[i] for corner in corners) for i in range(3)))
    maximum = Vector((max(corner[i] for corner in corners) for i in range(3)))
    return minimum, maximum


def role_from_source_name(obj: bpy.types.Object) -> str | None:
    for source in (obj.name, obj.data.name):
        normalized = source.lower()
        for source_name, role in ROLE_BY_SOURCE_NAME.items():
            if source_name in normalized:
                return role
    return None


def role_from_location(obj: bpy.types.Object) -> str | None:
    loc = obj.location
    if loc.y < -0.8 and loc.x > 0.3:
        return "FrontLeftWheel"
    if loc.y < -0.8 and loc.x < -0.3:
        return "FrontRightWheel"
    if loc.y > 0.8 and abs(loc.x) < 0.3:
        return "BackWheels"
    if loc.z > 80.0 and loc.x > 30.0:
        return "FrontLeftWheel"
    if loc.z > 80.0 and loc.x < -30.0:
        return "FrontRightWheel"
    if loc.z < -80.0 and abs(loc.x) < 30.0:
        return "BackWheels"
    if loc.z > 0.8 and loc.x > 0.3:
        return "FrontLeftWheel"
    if loc.z > 0.8 and loc.x < -0.3:
        return "FrontRightWheel"
    if loc.z < -0.8 and abs(loc.x) < 0.3:
        return "BackWheels"
    return None


def wheel_role(obj: bpy.types.Object) -> str | None:
    return role_from_source_name(obj) or role_from_location(obj)


def print_scene_report(asset_path: Path) -> None:
    print(f"SPORTS_CAR_FBX asset={asset_path}")
    print(f"SPORTS_CAR_FBX objects={len(bpy.context.scene.objects)} meshes={len(mesh_objects())}")

    for index, obj in enumerate(mesh_objects()):
        minimum, maximum = object_bounds(obj)
        dimensions = maximum - minimum
        role = wheel_role(obj) or "body"
        print(
            "SPORTS_CAR_FBX mesh"
            f" #{index:02d}"
            f" name={obj.name}"
            f" mesh={obj.data.name}"
            f" role={role}"
            f" loc=({obj.location.x:.3f},{obj.location.y:.3f},{obj.location.z:.3f})"
            f" scale=({obj.scale.x:.4f},{obj.scale.y:.4f},{obj.scale.z:.4f})"
            f" dims=({dimensions.x:.3f},{dimensions.y:.3f},{dimensions.z:.3f})"
        )
        hub = wheel_hub_report(obj)
        if hub is not None:
            print(
                "SPORTS_CAR_FBX wheel_hub"
                f" name={obj.name}"
                f" material={WHEEL_HUB_MATERIAL}"
                f" center=({hub.center.x:.3f},{hub.center.y:.3f},{hub.center.z:.3f})"
                f" normal=({hub.normal.x:.3f},{hub.normal.y:.3f},{hub.normal.z:.3f})"
            )

    for material in sorted(bpy.data.materials, key=lambda material: material.name):
        alpha = material.diffuse_color[3] if len(material.diffuse_color) >= 4 else 1.0
        blend = getattr(material, "blend_method", "<none>")
        node_alpha = principled_alpha(material)
        print(
            f"SPORTS_CAR_FBX material name={material.name}"
            f" alpha={alpha:.3f}"
            f" principled_alpha={node_alpha:.3f}"
            f" blend={blend}"
        )


def principled_alpha(material: bpy.types.Material) -> float:
    if not material.use_nodes:
        return material.diffuse_color[3] if len(material.diffuse_color) >= 4 else 1.0

    for node in material.node_tree.nodes:
        if node.type != "BSDF_PRINCIPLED":
            continue
        if "Alpha" in node.inputs:
            return float(node.inputs["Alpha"].default_value)
    return 1.0


class WheelHubReport:
    def __init__(self, center: Vector, normal: Vector) -> None:
        self.center = center
        self.normal = normal


def wheel_hub_report(obj: bpy.types.Object) -> WheelHubReport | None:
    if wheel_role(obj) is None:
        return None

    hub_material_index = None
    for index, slot in enumerate(obj.material_slots):
        if slot.material is not None and slot.material.name == WHEEL_HUB_MATERIAL:
            hub_material_index = index
            break
    if hub_material_index is None:
        return None

    coords: list[Vector] = []
    normal = Vector()
    normal_area = 0.0
    normal_matrix = obj.matrix_world.to_3x3()
    for polygon in obj.data.polygons:
        if polygon.material_index != hub_material_index:
            continue
        coords.extend(obj.matrix_world @ obj.data.vertices[index].co for index in polygon.vertices)
        normal += (normal_matrix @ polygon.normal).normalized() * polygon.area
        normal_area += polygon.area

    if not coords or normal_area <= 0.0:
        return None

    return WheelHubReport(
        sum(coords, Vector()) / len(coords),
        (normal / normal_area).normalized(),
    )


def set_principled_alpha(material: bpy.types.Material, alpha: float) -> None:
    if not material.use_nodes:
        return
    for node in material.node_tree.nodes:
        if node.type != "BSDF_PRINCIPLED":
            continue
        if "Base Color" in node.inputs:
            base_color = node.inputs["Base Color"].default_value
            if len(base_color) >= 4:
                base_color[3] = alpha
        if "Alpha" in node.inputs:
            node.inputs["Alpha"].default_value = alpha


def set_enum_field(owner: object, field_name: str, value: str) -> None:
    if not hasattr(owner, field_name):
        return
    try:
        setattr(owner, field_name, value)
    except TypeError:
        print(
            f"SPORTS_CAR_FBX warning: could not set {owner}.{field_name} to {value}",
            file=sys.stderr,
        )


def make_materials_opaque() -> None:
    for material in bpy.data.materials:
        material.diffuse_color[3] = 1.0
        set_principled_alpha(material, 1.0)
        set_enum_field(material, "blend_method", "OPAQUE")
        if hasattr(material, "use_screen_refraction"):
            material.use_screen_refraction = False
        if hasattr(material, "show_transparent_back"):
            material.show_transparent_back = False


def rename_vehicle_objects() -> None:
    body_index = 0
    for obj in mesh_objects():
        role = wheel_role(obj)
        if role is None:
            suffix = "" if body_index == 0 else f"_{body_index:02d}"
            obj.name = f"SportsCar_Body{suffix}"
            obj.data.name = f"{obj.name}_Mesh"
            body_index += 1
            continue

        obj.name = f"SportsCar_{role}"
        obj.data.name = f"{obj.name}_Mesh"


def set_wheel_origins_to_bounds() -> None:
    for obj in mesh_objects():
        if wheel_role(obj) is None:
            continue
        bpy.ops.object.select_all(action="DESELECT")
        obj.select_set(True)
        bpy.context.view_layer.objects.active = obj
        bpy.ops.object.origin_set(type="ORIGIN_GEOMETRY", center="BOUNDS")


def repair_front_wheel_hub_sides() -> None:
    mirror_width_axis = Matrix.Scale(-1.0, 4, Vector((1.0, 0.0, 0.0)))
    for obj in mesh_objects():
        role = wheel_role(obj)
        if role not in {"FrontLeftWheel", "FrontRightWheel"}:
            continue
        if front_wheel_hub_points_outward(obj, role):
            continue

        obj.data.transform(mirror_width_axis)
        obj.data.flip_normals()
        obj.data.update()
        obj.data.name = f"{obj.name}_Mesh"


def front_wheel_hub_points_outward(obj: bpy.types.Object, role: str) -> bool:
    hub = wheel_hub_report(obj)
    if hub is None:
        return True

    if role == "FrontLeftWheel":
        return hub.center.x > obj.location.x and hub.normal.x > 0.0
    if role == "FrontRightWheel":
        return hub.center.x < obj.location.x and hub.normal.x < 0.0
    return True


def export_repaired_fbx(target: Path) -> None:
    if REPAIRED_FBX.exists():
        REPAIRED_FBX.unlink()

    bpy.ops.export_scene.fbx(
        filepath=str(REPAIRED_FBX),
        use_selection=False,
        object_types={"EMPTY", "MESH"},
        apply_unit_scale=True,
        bake_space_transform=False,
        add_leaf_bones=False,
    )
    os.replace(REPAIRED_FBX, target)


def main() -> None:
    args = parse_args()
    asset_path = args.asset.resolve()

    reset_scene()
    import_fbx(asset_path)
    print_scene_report(asset_path)

    if args.inspect:
        return

    make_materials_opaque()
    rename_vehicle_objects()
    set_wheel_origins_to_bounds()
    repair_front_wheel_hub_sides()
    export_repaired_fbx(asset_path)
    print(f"SPORTS_CAR_FBX wrote={asset_path}")


if __name__ == "__main__":
    main()
