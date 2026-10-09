"""ENDE​SGA-32 authoring source and generated Godot palette resources."""
from pathlib import Path
import struct
import zlib
import argparse
import re

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "assets/endesga-32.pal"


def colors():
    lines = SOURCE.read_text().splitlines()
    if lines[:3] != ["JASC-PAL", "0100", "32"] or len(lines) != 35:
        raise ValueError("Expected exactly 32 JASC-PAL colors")
    result = [tuple(map(int, line.split())) for line in lines[3:]]
    if len(set(result)) != 32 or any(len(c) != 3 or any(v < 0 or v > 255 for v in c) for c in result):
        raise ValueError("Invalid ENDE​SGA-32 authoring palette")
    return result


def godot_color(index, alpha=1):
    return "Color(%s, %s)" % (", ".join(format(v / 255, ".10g") for v in colors()[index]), alpha)


def outputs():
    palette = colors()
    text = '[gd_resource type="Resource" load_steps=2 format=3]\n\n'
    text += '[ext_resource type="Script" path="res://scripts/authoring_palette.gd" id="1"]\n\n[resource]\nscript = ExtResource("1")\n'
    text += 'colors = PackedColorArray(' + ', '.join(', '.join(format(v / 255, '.10g') for v in (*c, 255)) for c in palette) + ')\n'
    def chunk(tag, data):
        return struct.pack('>I', len(data)) + tag + data + struct.pack('>I', zlib.crc32(tag + data))
    png = b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', 32, 1, 8, 2, 0, 0, 0))
    png += chunk(b'IDAT', zlib.compress(b'\0' + bytes(v for c in palette for v in c))) + chunk(b'IEND', b'')
    return {ROOT / 'resources/authoring_palette.tres': text.encode(), ROOT / 'assets/textures/palette.png': png}


def check_authoring_colors():
    allowed = colors()
    files = [ROOT / 'project.godot']
    for folder in ('scripts', 'scenes', 'materials', 'theme', 'tools'):
        files.extend(p for p in (ROOT / folder).rglob('*') if p.suffix in ('.gd', '.tres', '.tscn'))
    for path in files:
        for match in re.finditer(r'Color\(([\d.\s,]+)\)', path.read_text()):
            rgb = tuple(float(v) * 255 for v in match.group(1).split(',')[:3])
            if len(rgb) != 3 or not any(all(abs(a - b) < .001 for a, b in zip(rgb, c)) for c in allowed):
                raise ValueError(f'Color outside ENDE​SGA-32: {path.relative_to(ROOT)}: {match.group(0)}')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true', help='Validate authoring colors and generated palette freshness')
    args = parser.parse_args()
    for path, data in outputs().items():
        if args.check:
            if not path.exists() or path.read_bytes() != data:
                raise SystemExit(f'Stale palette asset: {path.relative_to(ROOT)}')
        else:
            path.write_bytes(data)
    check_authoring_colors()
    print('ENDE​SGA-32 authoring palette valid')
