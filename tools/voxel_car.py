#!/usr/bin/env python3
"""Author the first car, or export a labelled MagicaVoxel car to Godot assets.

python3 tools/voxel_car.py --author-hatchback
python3 tools/voxel_car.py assets/cars/rally_hatchback.vox
python3 tools/voxel_car.py assets/cars/rally_hatchback.vox --check

The .vox is the editable source. Exporting never rewrites it. No dependencies.
"""
from __future__ import annotations

import argparse
import json
import math
import struct
from collections import defaultdict
from pathlib import Path
from palette import colors

ROOT = Path(__file__).resolve().parents[1]
DEFAULT = ROOT / "assets/cars/rally_hatchback.vox"
VOXEL_SIZE = 0.1
WHEELS = ("WheelFL", "WheelFR", "WheelRL", "WheelRR")
TRIKE_WHEELS = ("WheelFront", "WheelRL", "WheelRR")
LAMPS = ("TailLeft", "TailRight", "BrakeLeft", "BrakeRight", "ReverseLeft", "ReverseRight")
PART_ROLES = {"BodyPaint": "Paint", "BodyGlass": "Glass", "BodyTrim": "Trim", "BodyPlate": "Plate",
              **{n: "Wheel" for n in (*WHEELS, "WheelFront")},
              **{n: n.replace("Left", "").replace("Right", "") + "Lamp" for n in LAMPS}, "HeadLeft": "Headlamp", "HeadRight": "Headlamp"}
# Palette entries are colors only. Material roles belong to labelled source parts.


def chunk(tag, payload=b"", children=b""):
    return tag.encode() + struct.pack("<II", len(payload), len(children)) + payload + children


def string(value):
    data = value.encode()
    return struct.pack("<i", len(data)) + data


def dictionary(values):
    return struct.pack("<i", len(values)) + b"".join(string(k) + string(v) for k, v in values.items())


def author_hatchback(path):
    parts = {n: {} for n in PART_ROLES if n != "WheelFront"}
    axle_z = (-12.5, 11.5)
    for y in range(3, 15):
        for z in range(-20, 20):
            for x in range(-8, 8):
                cabin = y >= 10
                front, rear, width = -9 + max(0, y - 10), 18 - max(0, y - 11), 8 - max(0, y - 11) // 2
                if cabin and (z < front or z >= rear or x < -width or x >= width):
                    continue
                if abs(x + .5) > 5.5 and any((y + .5 - 3.5)**2 + (z + .5 - a)**2 < 4.5**2 for a in axle_z):
                    continue
                role, color = "BodyPaint", 20
                if y == 3 or (not cabin and z in (-20, 19) and y <= 5):
                    role, color = "BodyTrim", 25
                if not cabin and z == -20 and 6 <= y <= 8 and abs(x + .5) < 4:
                    role, color = "BodyTrim", 26
                if cabin and 10 <= y <= 13 and ((x in (-width, width-1) and z != 3) or z in (front, rear-1)):
                    role, color = "BodyGlass", 17
                parts[role][(x,y,z)] = color
    parts["BodyPlate"] = {(x,y,20):3 for x in range(-2,2) for y in range(6,8)}
    for sign in (-1,1):
        for y in range(10,12):
            for z in range(-5,-3):
                parts["BodyTrim"][(8 if sign > 0 else -9,y,z)] = 25
    for name, left, front in [("WheelFL",True,True),("WheelFR",False,True),("WheelRL",True,False),("WheelRR",False,False)]:
        center = axle_z[0 if front else 1]
        for x in range(-9,-7) if left else range(7,9):
            for y in range(7):
                for z in range(math.floor(center)-3,math.floor(center)+4):
                    radius2 = (y+.5-3.5)**2 + (z+.5-center)**2
                    if radius2 <= 3.5**2:
                        parts[name][(x,y,z)] = 21 if x == (-9 if left else 8) and radius2 < 1.8**2 else 26
    for side, span in [("Left",range(-7,-4)),("Right",range(4,7))]:
        for role, y, color in [("Tail",7,8),("Brake",8,9)]:
            parts[role+side] = {(x,y,20):color for x in span}
        parts["Reverse"+side] = {(x,y,20):20 for x in ([-4] if side == "Left" else [3]) for y in range(7,9)}
        parts["Head"+side] = {(x,y,-21):3 for x in (range(-7,-4) if side == "Left" else range(4,7)) for y in range(6,8)}
    write_source(path, parts, "RallyHatchback")


def author_rickshaw(path):
    parts = {n: {} for n in PART_ROLES if n not in ("WheelFL", "WheelFR")}
    def box(name, color, xs, ys, zs):
        for x in xs:
            for y in ys:
                for z in zs:
                    parts[name][(x, y, z)] = color
    # All dimensions are integer 10 cm cells; Godot faces -Z.
    box("BodyTrim", 26, range(-6, 6), range(4, 6), range(-9, 14))
    box("BodyPaint", 12, range(-6, 6), range(6, 9), range(5, 14))
    box("BodyPaint", 12, range(-6, 6), range(9, 14), range(12, 14))
    for side in (-6, 5):
        box("BodyPaint", 12, [side], range(6, 11), range(5, 12))
        box("BodyTrim", 26, [side], range(11, 18), [12])
        box("BodyPaint", 12, [side], range(10, 18), [-8])
        box("BodyTrim", 26, [side], [17], range(-7, 12))
    # Narrow nose, framed windshield and a dark fabric canopy.
    box("BodyPaint", 12, range(-4, 4), range(6, 11), range(-13, -8))
    box("BodyGlass", 17, range(-5, 5), range(11, 17), [-9])
    box("BodyPaint", 12, range(-6, 6), [10, 17], [-9])
    box("BodyTrim", 26, range(-6, 6), range(18, 20), range(-9, 14))
    # Passenger bench, driver saddle and handlebar remain separate from paint.
    box("BodyTrim", 25, range(-5, 5), range(9, 11), range(7, 11))
    box("BodyTrim", 25, range(-5, 5), range(11, 14), [11])
    box("BodyTrim", 25, range(-2, 2), range(9, 11), range(-3, 1))
    box("BodyTrim", 21, range(-3, 3), [11], [-6])
    box("BodyTrim", 21, [-1, 0], range(6, 11), [-11])
    # Short front fender and fork; no hidden fourth wheel.
    box("BodyPaint", 12, range(-2, 2), [6], range(-15, -8))
    for side in (-2, 1):
        box("BodyTrim", 21, [side], range(3, 6), [-12])
    for name, xs, hub_z in [("WheelFront", range(-1, 1), -11.5),
                            ("WheelRL", range(-7, -5), 8.5),
                            ("WheelRR", range(5, 7), 8.5)]:
        for x in xs:
            for y in range(6):
                for z in range(math.floor(hub_z) - 2, math.floor(hub_z) + 4):
                    radius2 = (y + .5 - 3)**2 + (z + .5 - hub_z)**2
                    if radius2 <= 3**2:
                        parts[name][(x, y, z)] = 21 if radius2 < 1.5**2 else 26
    # Rear wheel arches clear the full suspension travel.
    for name in ("BodyPaint", "BodyTrim"):
        for cell in list(parts[name]):
            x, y, z = cell
            if abs(x + .5) >= 4.5 and (y + .5 - 3)**2 + (z + .5 - 8.5)**2 < 4**2:
                del parts[name][cell]
    for side, xs in [("Left", range(-5, -3)), ("Right", range(3, 5))]:
        box("Tail" + side, 8, xs, [8], [14])
        box("Brake" + side, 9, xs, [9], [14])
        box("Reverse" + side, 20, xs, [7], [14])
        box("Head" + side, 20, [-3 if side == "Left" else 2], [8], [-14])
    box("BodyPlate", 11, range(-2, 2), [7, 8], [14])
    write_source(path, parts, "AutoRickshaw")


def write_source(path, parts, root_name):
    vox_parts = {name:{(x,-z-1,y):c for (x,y,z),c in cells.items()} for name,cells in parts.items()}
    children = b""
    nodes = chunk("nTRN",struct.pack("<i",0)+dictionary({"_name":root_name,"_opentrack":"car-v2","_voxel_size":"0.1"})+struct.pack("<iiii",1,-1,-1,1)+dictionary({"_t":"0 0 0"}))
    nodes += chunk("nGRP",struct.pack("<i",1)+dictionary({})+struct.pack("<i",len(parts))+b"".join(struct.pack("<i",2+i*2) for i in range(len(parts))))
    for i,(name,cells) in enumerate(vox_parts.items()):
        minimum = tuple(min(p[a] for p in cells) for a in range(3))
        size = tuple(max(p[a] for p in cells)-minimum[a]+1 for a in range(3))
        children += chunk("SIZE",struct.pack("<iii",*size))
        children += chunk("XYZI",struct.pack("<i",len(cells))+b"".join(bytes((*[p[a]-minimum[a] for a in range(3)],color)) for p,color in sorted(cells.items())))
        translation = " ".join(str(minimum[a]+size[a]//2) for a in range(3))
        node_id = 2+i*2
        nodes += chunk("nTRN",struct.pack("<i",node_id)+dictionary({"_name":name,"_role":PART_ROLES[name]})+struct.pack("<iiii",node_id+1,-1,-1,1)+dictionary({"_t":translation}))
        nodes += chunk("nSHP",struct.pack("<i",node_id+1)+dictionary({})+struct.pack("<ii",1,i)+dictionary({}))
    palette = colors()
    children += chunk("RGBA",b"".join(bytes((*palette[i % 32],255)) for i in range(256)))
    children += chunk("NOTE",struct.pack("<i",32)+b"".join(string("#%02x%02x%02x" % c) for c in palette)) + nodes
    path.parent.mkdir(parents=True,exist_ok=True)
    path.write_bytes(b"VOX "+struct.pack("<i",150)+chunk("MAIN",children=children))


class Reader:
    def __init__(self, data):
        self.data, self.offset = data, 0

    def integer(self):
        value = struct.unpack_from("<i", self.data, self.offset)[0]
        self.offset += 4
        return value

    def string(self):
        length = self.integer()
        if length < 0 or self.offset + length > len(self.data):
            raise ValueError("Invalid VOX string length")
        value = self.data[self.offset:self.offset + length].decode()
        self.offset += length
        return value

    def dictionary(self):
        return {self.string(): self.string() for _ in range(self.integer())}


def read_vox(path):
    data = path.read_bytes()
    if data[:4] != b"VOX " or struct.unpack_from("<i", data, 4)[0] not in (150, 200):
        raise ValueError("Expected a MagicaVoxel 150/200 source")
    models, transforms, groups, shapes, notes, palette = [], {}, {}, {}, [], []
    size = None
    def visit(start, end):
        nonlocal size, notes, palette
        while start < end:
            tag, length, nested = struct.unpack_from("<4sII", data, start)
            finish = start + 12 + length
            if finish + nested > end:
                raise ValueError("Truncated VOX chunk")
            payload = data[start + 12:finish]
            reader = Reader(payload)
            if tag == b"SIZE":
                size = struct.unpack("<iii", payload)
            elif tag == b"XYZI":
                count = reader.integer()
                if size is None or len(payload) != 4 + count * 4:
                    raise ValueError("Invalid voxel model")
                cells = {}
                for i in range(count):
                    x, y, z, color = payload[4 + i * 4:8 + i * 4]
                    if color == 0 or any(p >= bound for p, bound in zip((x, y, z), size)):
                        raise ValueError("Voxel outside model or empty palette slot")
                    cells[(x, y, z)] = color
                models.append((size, cells))
                size = None
            elif tag == b"RGBA":
                palette = [tuple(payload[i:i+4]) for i in range(0, 1024, 4)]
            elif tag == b"NOTE":
                notes = [reader.string() for _ in range(reader.integer())]
            elif tag == b"nTRN":
                node, attrs = reader.integer(), reader.dictionary()
                child = reader.integer()
                reader.integer()
                reader.integer()
                if reader.integer() != 1:
                    raise ValueError("Car source must have one static transform frame")
                transforms[node] = (attrs, child, reader.dictionary())
            elif tag == b"nGRP":
                node = reader.integer()
                reader.dictionary()
                groups[node] = [reader.integer() for _ in range(reader.integer())]
            elif tag == b"nSHP":
                node = reader.integer()
                reader.dictionary()
                if reader.integer() != 1:
                    raise ValueError("Car part must reference one static voxel model")
                shapes[node] = reader.integer()
            if nested:
                visit(finish, finish + nested)
            start = finish + nested
    visit(8, len(data))
    if len(palette) != 256 or palette[:32] != [(*c,255) for c in colors()]:
        raise ValueError("Car source must embed the canonical ENDE​SGA-32 palette")
    if transforms.get(0, ({},))[0].get("_opentrack") != "car-v2" or transforms[0][0].get("_voxel_size") != "0.1":
        raise ValueError("Car source requires car-v2 metadata and 10 cm voxels")
    parts, roles = {}, {}
    def walk(node, offset=(0, 0, 0), rotation=((1,0,0),(0,1,0),(0,0,1)), name="", role="", ancestry=()):
        if node in ancestry:
            raise ValueError("Cyclic VOX scene graph")
        ancestry += (node,)
        if node in transforms:
            attrs, child, frame = transforms[node]
            if attrs.get("_hidden") == "1":
                return
            translation = tuple(map(int, frame.get("_t", "0 0 0").split()))
            code = int(frame.get("_r", 4))
            axes = (code & 3, (code >> 2) & 3)
            if max(axes) > 2 or axes[0] == axes[1]:
                raise ValueError("Invalid VOX rotation")
            axes += (3 - sum(axes),)
            local = tuple(tuple((-1 if code & (1 << (4 + row)) else 1) if col == axes[row] else 0 for col in range(3)) for row in range(3))
            next_rotation = tuple(tuple(sum(rotation[r][k] * local[k][c] for k in range(3)) for c in range(3)) for r in range(3))
            next_offset = tuple(offset[r] + sum(rotation[r][k] * translation[k] for k in range(3)) for r in range(3))
            walk(child, next_offset, next_rotation, attrs.get("_name", name), attrs.get("_role", role), ancestry)
        elif node in groups:
            for child in groups[node]:
                walk(child, offset, rotation, name, role, ancestry)
        elif node in shapes:
            if not name or name in parts:
                raise ValueError("Car parts require unique scene names")
            size, cells = models[shapes[node]]
            result = {}
            for point, color in cells.items():
                # Transform voxel centres; negative axes must also rotate cell extents.
                center = [point[a] + 0.5 - size[a] // 2 for a in range(3)]
                world = tuple(round(offset[r] + sum(rotation[r][k] * center[k] for k in range(3)) - 0.5) for r in range(3))
                result[world] = color
            parts[name] = result
            roles[name] = role
        else:
            raise ValueError("Missing VOX scene node")
    walk(0)
    wheel_names = TRIKE_WHEELS if "WheelFront" in parts else WHEELS
    required = (set(PART_ROLES) - set((*WHEELS, "WheelFront"))) | set(wheel_names)
    if any(n in parts for n in set((*WHEELS, "WheelFront")) - set(wheel_names)):
        raise ValueError("Use either FL/FR/RL/RR or Front/RL/RR wheel bindings")
    if not required.issubset(parts):
        raise ValueError("Missing labelled car parts: " + ", ".join(sorted(required - parts.keys())))
    for name, cells in parts.items():
        if not cells or roles[name] not in set(PART_ROLES.values()) or any(c > 32 for c in cells.values()):
            raise ValueError("Empty part, unknown part role or color outside ENDE​SGA-32")
        if name in PART_ROLES and roles[name] != PART_ROLES[name]:
            raise ValueError("Incorrect role for labelled part: " + name)
    return parts, palette, roles


def godot_point(point):
    return (point[0] * VOXEL_SIZE, point[2] * VOXEL_SIZE, -point[1] * VOXEL_SIZE)


def exposed_quads(cells):
    planes = defaultdict(dict)
    for point, color in cells.items():
        for axis in range(3):
            u, v = (axis + 1) % 3, (axis + 2) % 3
            for sign in (-1, 1):
                neighbor = list(point)
                neighbor[axis] += sign
                if tuple(neighbor) not in cells:
                    planes[(axis, sign, point[axis] + (sign > 0))][(point[u], point[v])] = color
    for (axis, sign, plane), mask in sorted(planes.items()):
        u, v = (axis + 1) % 3, (axis + 2) % 3
        while mask:
            a, b = min(mask, key=lambda p: (p[1], p[0]))
            color = mask[(a, b)]
            width = 1
            while mask.get((a + width, b)) == color:
                width += 1
            height = 1
            while all(mask.get((x, b + height)) == color for x in range(a, a + width)):
                height += 1
            points = []
            for du, dv in [(0,0), (width,0), (width,height), (0,height)]:
                p = [0, 0, 0]
                p[axis], p[u], p[v] = plane, a + du, b + dv
                points.append(godot_point(p))
            if sign < 0:
                points.reverse()
            normal = [0, 0, 0]
            normal[axis] = sign
            yield color, points, godot_point(normal)
            for y in range(b, b + height):
                for x in range(a, a + width):
                    del mask[(x, y)]


def export_assets(path):
    parts, palette, roles = read_vox(path)
    wheel_names = TRIKE_WHEELS if "WheelFront" in parts else WHEELS
    def material_key(name, color):
        role = roles[name]
        return role, 0 if role == "Paint" else color
    def linear_rgb(color):
        return [c / 255 / 12.92 if c / 255 <= 0.04045 else ((c / 255 + 0.055) / 1.055) ** 2.4 for c in palette[color - 1][:3]]
    material_keys = sorted({material_key(name,c) for name,cells in parts.items() for c in cells.values()})
    gltf = {"asset": {"version": "2.0", "generator": "OpenTrack voxel_car.py"}, "scene": 0,
            "scenes": [{"nodes": [0]}], "nodes": [{"name": "CarModel", "children": []}],
            "meshes": [], "materials": [], "bufferViews": [], "accessors": [], "buffers": []}
    binary = bytearray()
    for role, color in material_keys:
        # Paint uses one palette albedo; natural lighting supplies tonal shading.
        albedo = [1, 1, 1] if role == "Paint" else linear_rgb(color)
        gltf["materials"].append({"name": role, "pbrMetallicRoughness": {"baseColorFactor": [*albedo,1], "metallicFactor": 0.4 if color == 21 else 0, "roughnessFactor": 0.3 if role == "Glass" else 0.8}})
    def accessor(values, kind, components, bounds=False):
        while len(binary) % 4:
            binary.append(0)
        index = len(gltf["accessors"])
        start = len(binary)
        flat = [n for value in values for n in value] if components > 1 else values
        binary.extend(struct.pack("<" + ("I" if kind == 5125 else "f") * len(flat), *flat))
        gltf["bufferViews"].append({"buffer": 0, "byteOffset": start, "byteLength": len(binary) - start})
        spec = {"bufferView": len(gltf["bufferViews"]) - 1, "componentType": kind, "count": len(values), "type": {1:"SCALAR",3:"VEC3",4:"VEC4"}[components]}
        if bounds:
            spec["min"] = [min(p[a] for p in values) for a in range(components)]
            spec["max"] = [max(p[a] for p in values) for a in range(components)]
        gltf["accessors"].append(spec)
        return index
    wheel_sizes = {}
    quads = 0
    for name, cells in parts.items():
        buckets = defaultdict(lambda: {"positions": [], "normals": [], "colors": [], "indices": []})
        low = tuple(min(p[a] for p in cells) for a in range(3))
        high = tuple(max(p[a] for p in cells) + 1 for a in range(3))
        pivot = godot_point(tuple((low[a] + high[a]) / 2 for a in range(3))) if name in wheel_names else (0,0,0)
        if name in wheel_names:
            wheel_sizes[name] = (max(high[1] - low[1], high[2] - low[2]) * VOXEL_SIZE / 2, (high[0] - low[0]) * VOXEL_SIZE)
        for color, points, normal in exposed_quads(cells):
            key = material_key(name,color)
            bucket = buckets[key]
            base = len(bucket["positions"])
            bucket["positions"].extend(tuple(p[a] - pivot[a] for a in range(3)) for p in points)
            magnitude = math.sqrt(sum(n*n for n in normal))
            bucket["normals"].extend([tuple(n / magnitude for n in normal)] * 4)
            bucket["indices"].extend([base, base+1, base+2, base, base+2, base+3])
            quads += 1
        primitives = []
        for key, bucket in sorted(buckets.items()):
            attributes = {"POSITION": accessor(bucket["positions"],5126,3,True), "NORMAL": accessor(bucket["normals"],5126,3)}
            primitives.append({"attributes": attributes, "indices": accessor(bucket["indices"],5125,1), "material": material_keys.index(key)})
        gltf["meshes"].append({"name": name, "primitives": primitives})
        gltf["nodes"][0]["children"].append(len(gltf["nodes"]))
        gltf["nodes"].append({"name": name, "mesh": len(gltf["meshes"]) - 1, "translation": pivot})
    gltf["buffers"] = [{"byteLength": len(binary)}]
    json_data = json.dumps(gltf, separators=(",", ":"), sort_keys=True).encode()
    json_data += b" " * (-len(json_data) % 4)
    binary += b"\0" * (-len(binary) % 4)
    glb = struct.pack("<III", 0x46546C67, 2, 12 + 8 + len(json_data) + 8 + len(binary)) + struct.pack("<II",len(json_data),0x4E4F534A) + json_data + struct.pack("<II",len(binary),0x004E4942) + binary
    glb_path = path.with_suffix(".glb")
    visual_path = ROOT / "scenes/cars" / (path.stem + "_visual.tscn")
    scene = '[gd_scene load_steps=3 format=3]\n\n[ext_resource type="Script" path="res://scripts/car_visual.gd" id="1"]\n'
    scene += f'[ext_resource type="PackedScene" path="res://{glb_path.relative_to(ROOT).as_posix()}" id="2"]\n\n'
    bindings = {"wheels": wheel_names, "tail_lamps": ("TailLeft", "TailRight"), "brake_lamps": ("BrakeLeft", "BrakeRight"), "reverse_lamps": ("ReverseLeft", "ReverseRight")}
    scene += '[node name="Visual" type="Node3D" node_paths=PackedStringArray(' + ', '.join(json.dumps(n) for n in bindings) + ')]\nscript = ExtResource("1")\n'
    for key, names in bindings.items():
        scene += key + ' = Array[NodePath]([' + ', '.join(f'NodePath("Model/CarModel/{n}")' for n in names) + '])\n'
    scene += 'wheel_radii = PackedFloat32Array(' + ', '.join(str(round(wheel_sizes[n][0],6)) for n in wheel_names) + ')\n'
    scene += 'wheel_widths = PackedFloat32Array(' + ', '.join(str(round(wheel_sizes[n][1],6)) for n in wheel_names) + ')\n\n[node name="Model" parent="." instance=ExtResource("2")]\n'
    return {glb_path: glb, visual_path: scene.encode()}, {"parts": len(parts), "voxels": sum(map(len,parts.values())), "quads": quads, "wheels": wheel_sizes}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", nargs="?", type=Path, default=DEFAULT)
    parser.add_argument("--author-hatchback", action="store_true")
    parser.add_argument("--author-rickshaw", action="store_true")
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if args.author_hatchback or args.author_rickshaw:
        if args.check:
            parser.error("--check cannot author a source")
        if args.author_hatchback and args.author_rickshaw:
            parser.error("Choose one vehicle to author")
        if args.author_rickshaw:
            if args.source == DEFAULT:
                args.source = ROOT / "assets/cars/auto_rickshaw.vox"
            author_rickshaw(args.source)
        else:
            author_hatchback(args.source)
    outputs, report = export_assets(args.source.resolve())
    for path, data in outputs.items():
        if args.check:
            if not path.exists() or path.read_bytes() != data:
                raise SystemExit(f"Stale generated asset: {path.relative_to(ROOT)}")
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
    print(json.dumps(report, sort_keys=True))


if __name__ == "__main__":
    main()
