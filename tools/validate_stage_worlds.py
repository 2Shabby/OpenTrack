#!/usr/bin/env python3
"""Validate each complete saved world in a fresh Godot process, then aggregate."""
import argparse
from concurrent.futures import ThreadPoolExecutor, as_completed
import json
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
STATE = ROOT / '.stage_authoring'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--jobs', type=int, default=1)
    parser.add_argument('--godot', default=shutil.which('godot'))
    args = parser.parse_args()
    if args.jobs < 1 or not args.godot:
        parser.error('Godot and positive job count are required')
    STATE.mkdir(exist_ok=True)
    (STATE / '.gdignore').touch()
    entries = json.loads((ROOT / 'resources/stages/catalog.json').read_text())['stages']
    report = {'worlds': [], 'failures': 0}

    def validate(entry):
        sid = entry['id']
        log_path = STATE / ('verify-' + sid + '.log')
        result_path = STATE / ('verified-' + sid + '.json')
        result_path.unlink(missing_ok=True)
        with log_path.open('w') as log:
            code = subprocess.run([args.godot, '--headless', '--path', str(ROOT),
                                   '--script', 'tools/validate_baked_worlds.gd', '--',
                                   '--stage=' + sid], stdout=log, stderr=subprocess.STDOUT).returncode
        output = log_path.read_text()
        token = 'ALL_BAKED_WORLDS_VERIFIED 1 worlds · 0 failures'
        if code or token not in output or 'SCRIPT ERROR:' in output or '\nERROR:' in output or not result_path.exists():
            return {'id': sid, 'failures': 1, 'exit_code': code, 'log': str(log_path)}, None
        data = json.loads(result_path.read_text())
        return data['worlds'][0], data['engine']

    with ThreadPoolExecutor(max_workers=args.jobs) as workers:
        tasks = [workers.submit(validate, entry) for entry in entries]
        for future in as_completed(tasks):
            world, engine = future.result()
            report['worlds'].append(world)
            report['failures'] += world['failures']
            if engine:
                report['engine'] = engine
            print(f"WORLD_VERIFIED {len(report['worlds'])}/{len(entries)} {world['id']} · {world['failures']} failures", flush=True)
            temporary = STATE / 'world_validation.pending.json'
            temporary.write_text(json.dumps(report, indent=2) + '\n')
            temporary.replace(STATE / 'world_validation.json')
    print(f"ALL_BAKED_WORLDS_VERIFIED {len(report['worlds'])} worlds · {report['failures']} failures", flush=True)
    raise SystemExit(1 if report['failures'] else 0)


if __name__ == '__main__':
    main()
