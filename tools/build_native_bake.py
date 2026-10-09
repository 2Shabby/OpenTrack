#!/usr/bin/env python3
"""Build the optional offline terrain accelerator using local godot-cpp bindings."""
from pathlib import Path
import os
import subprocess

root = Path(__file__).resolve().parents[1]
if not (root/'extensions/godot-cpp/CMakeLists.txt').exists():
    raise SystemExit('Local extensions/godot-cpp bindings are required. The baker also works without this optional accelerator.')
state = root/'.stage_authoring'
state.mkdir(exist_ok=True)
(state/'.gdignore').touch()
build = state/'native/build'
subprocess.run(['cmake', '-S', str(root/'tools/native_bake'), '-B', str(build), '-DCMAKE_BUILD_TYPE=Release'], check=True)
subprocess.run(['cmake', '--build', str(build), '-j'+str(min(6, os.cpu_count() or 1))], check=True)
# The game never loads this descriptor; only the offline authoring script does.
(root/'.stage_authoring/native/accelerator.gdextension').write_text('''[configuration]
entry_symbol = "native_bake_init"
compatibility_minimum = "4.7"
[libraries]
macos.release.arm64 = "res://.stage_authoring/native/libnative_bake.dylib"
macos.debug.arm64 = "res://.stage_authoring/native/libnative_bake.dylib"
''')
