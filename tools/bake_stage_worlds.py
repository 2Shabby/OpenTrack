#!/usr/bin/env python3
"""Resume the one-off saved-world bake, isolating each stage in its own process.

python3 tools/bake_stage_worlds.py             # skip unchanged completed stages
python3 tools/bake_stage_worlds.py --jobs 2    # parallel offline workers
python3 tools/bake_stage_worlds.py --force     # rebuild every world
python3 tools/bake_stage_worlds.py --stage ID  # author a specific stage
Progress and logs: .stage_authoring/
"""
import argparse
from datetime import datetime, timezone
import fcntl
import json
import os
from pathlib import Path
import selectors
import shutil
import signal
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]
STATE = ROOT / '.stage_authoring'


def write_progress(data):
    data['updated_at'] = datetime.now(timezone.utc).isoformat()
    temporary = STATE / 'progress.pending.json'
    temporary.write_text(json.dumps(data, indent=2)+'\n')
    temporary.replace(STATE / 'progress.json')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--force', action='store_true')
    parser.add_argument('--stage')
    parser.add_argument('--jobs', type=int, default=1)
    parser.add_argument('--large-stage-km', type=float, default=20.0, help='Use at most two workers above this length to bound source-grid memory')
    parser.add_argument('--godot', default=shutil.which('godot'))
    args = parser.parse_args()
    if not args.godot or args.jobs < 1:
        parser.error('Godot executable and positive job count are required')
    STATE.mkdir(exist_ok=True)
    (STATE / '.gdignore').touch()
    with (STATE/'lock').open('w') as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            parser.error('A stage authoring pass is already running')
        # A killed publisher may leave its tiny directory mutex behind.
        catalog_lock = STATE/'catalog.lock'
        if catalog_lock.exists():
            pid_file = catalog_lock/'pid'
            try:
                os.kill(int(pid_file.read_text()), 0)
            except (ProcessLookupError, ValueError, FileNotFoundError):
                shutil.rmtree(catalog_lock)
            else:
                parser.error('Another world is publishing the catalog')
        entries = json.loads((ROOT/'resources/stages/catalog.json').read_text())['stages']
        entries.sort(key=lambda entry: entry['length_m'])
        if args.stage:
            entries = [entry for entry in entries if entry['id'] == args.stage]
            if not entries:
                parser.error('Unknown stage ID')
        state = {'pid': os.getpid(), 'status': 'running', 'total': len(entries), 'completed': [], 'failed': [], 'current': None,
                 'jobs': args.jobs, 'active': {}, 'started_at': datetime.now(timezone.utc).isoformat()}
        active = {}
        selector = selectors.DefaultSelector()

        def stop(signum, _frame):
            for job in active.values():
                job['process'].terminate()
            for job in active.values():
                try:
                    job['process'].wait(timeout=15)
                except subprocess.TimeoutExpired:
                    job['process'].kill()
                    job['process'].wait()
                job['log'].close()
            state['status'] = 'stopped'
            state['active'] = {}
            write_progress(state)
            raise SystemExit(128+signum)

        signal.signal(signal.SIGTERM, stop)
        signal.signal(signal.SIGINT, stop)
        write_progress(state)
        next_index = 0

        def consume(job, line):
            job['log'].write(line)
            job['log'].flush()
            sid = job['id']
            if 'SCRIPT ERROR:' in line or line.startswith('ERROR:'):
                job['errors'] = True
            if line.startswith(('WORLD_BAKED '+sid, 'WORLD_ALREADY_BAKED '+sid)):
                job['published'] = True
            if any(token in line for token in ('Terrain sampled chunk', 'WORLD_', 'Terrain surface refinement')):
                state['active'][sid]['progress'] = line.strip()
                state['current'] = sid
                state['current_progress'] = line.strip()
                write_progress(state)

        while next_index < len(entries) or active:
            while next_index < len(entries) and len(active) < (min(args.jobs, 2) if entries[next_index]["length_m"] > args.large_stage_km * 1000 else args.jobs):
                entry = entries[next_index]
                next_index += 1
                sid = entry['id']
                command = [args.godot, '--headless', '--path', str(ROOT), '--script', 'tools/bake_stage_world.gd', '--', '--stage='+sid]
                if args.force:
                    command.append('--force')
                print(f'Authoring {sid} ({next_index}/{len(entries)})', flush=True)
                process = subprocess.Popen(command, cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
                job = {'id': sid, 'process': process, 'log': (STATE/(sid+'.log')).open('w'), 'published': False, 'errors': False, 'start': time.monotonic()}
                active[sid] = job
                state['active'][sid] = {'pid': process.pid, 'progress': ''}
                state['current'] = sid
                selector.register(process.stdout, selectors.EVENT_READ, job)
                write_progress(state)
            for key, _event in selector.select(timeout=0.5):
                job = key.data
                line = key.fileobj.readline()
                if line:
                    consume(job, line)
            for sid, job in list(active.items()):
                code = job['process'].poll()
                if code is None:
                    continue
                for line in job['process'].stdout:
                    consume(job, line)
                selector.unregister(job['process'].stdout)
                job['process'].stdout.close()
                job['log'].close()
                elapsed = round(time.monotonic()-job['start'], 1)
                if code == 0 and job['published'] and not job['errors']:
                    state['completed'].append(sid)
                    print(f'Saved {sid} in {elapsed}s', flush=True)
                else:
                    state['failed'].append({'id': sid, 'exit_code': code, 'published': job['published'], 'errors': job['errors'], 'log': str(STATE/(sid+'.log'))})
                    print(f'Failed {sid}; see its log (exit {code})', flush=True)
                del active[sid]
                del state['active'][sid]
                write_progress(state)
        selector.close()
        state['status'] = 'failed' if state['failed'] else 'complete'
        state['current'] = None
        write_progress(state)
        print(f'Authoring pass: {len(state["completed"])} saved, {len(state["failed"])} failed', flush=True)
        return 1 if state['failed'] else 0


if __name__ == '__main__':
    raise SystemExit(main())
