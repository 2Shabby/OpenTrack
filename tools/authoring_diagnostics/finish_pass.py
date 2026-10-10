import json
from pathlib import Path
import subprocess
import time
root = Path.cwd()
state = root / '.stage_authoring'
initial = json.loads((state / 'progress.json').read_text())
expected_pid = initial['pid']
while True:
    progress = json.loads((state / 'progress.json').read_text())
    if progress['pid'] != expected_pid:
        raise SystemExit('Bake manager changed; finalizer stopped')
    if progress['status'] != 'running':
        break
    time.sleep(5)
if progress['status'] == 'stopped':
    raise SystemExit('Bake was stopped; finalizer stopped')
print('UPGRADE_SUCCESSFUL_GEOMETRY_PROVENANCE', flush=True)
with (state / 'fingerprint-upgrade.log').open('w') as log:
    code = subprocess.run(['/opt/homebrew/bin/godot', '--headless', '--path', str(root), '--script', 'tools/authoring_diagnostics/upgrade_fingerprints.gd'], stdout=log, stderr=subprocess.STDOUT).returncode
output = (state / 'fingerprint-upgrade.log').read_text()
print(output, flush=True)
if code or 'PROVENANCE_UPGRADED' not in output or 'ERROR:' in output:
    raise SystemExit('Provenance upgrade failed')
print('FINAL_RESUME_ALL_50', flush=True)
with (state / 'resume.log').open('w') as log:
    code = subprocess.run(['python3', '-u', 'tools/bake_stage_worlds.py', '--jobs', '2', '--large-stage-km', '18'], stdout=log, stderr=subprocess.STDOUT).returncode
if code:
    raise SystemExit('Resume failed; inspect progress.json')
print('VERIFY_ALL_50', flush=True)
code = subprocess.run(['python3', '-u', 'tools/authoring_diagnostics/watch_and_verify.py']).returncode
raise SystemExit(code)
