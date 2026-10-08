"""Asset-pipeline checks: python3 -m unittest tools.test_voxel_car"""
import json
import struct
import tempfile
import unittest
from pathlib import Path

from tools import voxel_car


class VoxelCarTests(unittest.TestCase):
    def test_authored_source_round_trip(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "hatchback.vox"
            voxel_car.author_hatchback(source)
            self.assertEqual(source.read_bytes(), voxel_car.DEFAULT.read_bytes())
            parts, _, notes = voxel_car.read_vox(source)
            self.assertEqual(len(parts), 13)
            self.assertEqual(notes[0], "Paint")
            for name in voxel_car.WHEELS:
                self.assertGreater(len(parts[name]), 100)

    def test_committed_exports_are_reproducible_and_oriented(self):
        outputs, report = voxel_car.export_assets(voxel_car.DEFAULT)
        for path, data in outputs.items():
            self.assertEqual(path.read_bytes(), data, f"Stale generated file: {path}")
        data = outputs[voxel_car.DEFAULT.with_suffix(".glb")]
        length = struct.unpack_from("<I", data, 12)[0]
        gltf = json.loads(data[20:20 + length])
        mounts = {node["name"]: node["translation"] for node in gltf["nodes"][1:]}
        self.assertEqual(mounts["WheelFL"], [-0.8, 0.325, -1.2750000000000001])
        self.assertEqual(mounts["WheelFR"][0], 0.8)
        self.assertAlmostEqual(mounts["WheelRL"][2] - mounts["WheelFL"][2], 2.45)
        self.assertEqual(report["wheels"]["WheelRR"], (0.325, 0.2))
        self.assertLess(report["quads"], 1000)
        materials = {material["name"]: material for material in gltf["materials"]}
        self.assertLess(max(materials["Glass"]["pbrMetallicRoughness"]["baseColorFactor"][:3]), 0.1)
        self.assertLess(materials["BrakeLamp"]["pbrMetallicRoughness"]["baseColorFactor"][1], 0.02)

    def test_mesher_removes_internal_material_boundaries(self):
        cells = {(x, y, z): 1 if x < 2 else 2 for x in range(4) for y in range(3) for z in range(2)}
        quads = list(voxel_car.exposed_quads(cells))
        # Different colors split exposed faces, without adding an internal wall.
        surface_area = 0
        for _, points, _ in quads:
            a = [points[1][i] - points[0][i] for i in range(3)]
            b = [points[3][i] - points[0][i] for i in range(3)]
            cross = [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0]]
            surface_area += sum(c*c for c in cross) ** 0.5
        self.assertAlmostEqual(surface_area, 2*(4*3 + 3*2 + 4*2)*voxel_car.VOXEL_SIZE**2)

    def test_missing_part_label_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "broken.vox"
            source.write_bytes(voxel_car.DEFAULT.read_bytes().replace(b"Body", b"Roof"))
            with self.assertRaisesRegex(ValueError, "Missing labelled car parts: Body"):
                voxel_car.read_vox(source)

    def test_palette_rgb_edits_preserve_semantic_roles(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "paint.vox"
            data = bytearray(voxel_car.DEFAULT.read_bytes())
            palette_start = data.index(b"RGBA") + 12
            data[palette_start:palette_start + 4] = bytes((120, 120, 120, 255))
            source.write_bytes(data)
            _, palette, notes = voxel_car.read_vox(source)
            self.assertEqual(palette[0], (120, 120, 120, 255))
            self.assertEqual(notes[0], "Paint")


if __name__ == "__main__":
    unittest.main()
