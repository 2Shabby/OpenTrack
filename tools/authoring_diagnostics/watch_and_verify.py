import json
import subprocess
import time
from pathlib import Path
root=Path.cwd()
state=root/'.stage_authoring'
last=None
while True:
    p=json.loads((state/'progress.json').read_text())
    c=json.loads((root/'resources/stages/catalog.json').read_text())['stages']
    published=sum(bool(s.get('baked_scene_path')) for s in c)
    snapshot=(published,len(p['completed']),len(p['failed']),p['status'])
    if snapshot!=last:
        print('VERIFY_WATCH published=%d/50 processed=%d/50 failed=%d status=%s'%snapshot,flush=True)
        last=snapshot
    if p['status'] in ('complete','failed','stopped'):
        break
    time.sleep(5)
if p['status']!='complete' or len(p['completed'])!=50 or p['failed']:
    raise SystemExit('Batch did not complete successfully')
for name,script,args,token in [
    ('final-game','tools/validate_stages.gd',['--require-baked','--world'],'Saved stage validation: 0 failures; 50 stages; 50 baked worlds;'),
    ('final-worlds','tools/validate_baked_worlds.gd',[],'ALL_BAKED_WORLDS_VERIFIED 50 worlds · 0 failures'),
]:
    print('VERIFY_START '+name,flush=True)
    log=state/(name+'.log')
    with log.open('w') as f:
        code=subprocess.run(['/opt/homebrew/bin/godot','--headless','--path',str(root),'--script',script,'--']+args,stdout=f,stderr=subprocess.STDOUT).returncode
    output=log.read_text()
    print(output[-3000:],flush=True)
    if code!=0 or token not in output or 'SCRIPT ERROR:' in output or '\nERROR:' in output:
        raise SystemExit('Verification failed: '+str(log))
print('ALL_50_BAKED_AND_VERIFIED',flush=True)
