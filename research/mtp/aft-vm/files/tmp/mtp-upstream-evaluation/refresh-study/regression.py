from pathlib import Path
import os,time,json,hashlib
base=Path('/Users/jike/Developer/chclt/anylinuxfs/tmp/mtp-upstream-evaluation')
root=base/'mount/Internal storage/alfs-refresh-regression-20261117'
out=base/'refresh-study/regression.json'
r={'started':time.time(),'directory':str(root),'steps':[]}
def step(name,**data):
 r['steps'].append({'step':name,'time':time.time(),**data});out.write_text(json.dumps(r,indent=2));print(name,data,flush=True)
try:
 root.mkdir()
 payload=os.urandom(32*1024*1024);expected=hashlib.sha256(payload).hexdigest()
 t=time.monotonic()
 with (root/'payload.bin').open('xb') as f:f.write(payload);f.flush();os.fsync(f.fileno())
 step('write_32MiB',seconds=time.monotonic()-t,sha256=expected)
 t=time.monotonic();actual=hashlib.sha256((root/'payload.bin').read_bytes()).hexdigest();assert actual==expected
 step('read_32MiB',seconds=time.monotonic()-t,sha256=actual,note='host readback; guest fresh read follows separately')
 pages=root/'pages';pages.mkdir();names=[];t=time.monotonic()
 for i in range(240):
  name=f'item-{i:04d}-'+('x'*155)+'.txt';names.append(name);(pages/name).touch(exist_ok=False)
 step('create_pagination_files',count=len(names),seconds=time.monotonic()-t)
 for n in range(3):
  t=time.monotonic();got=os.listdir(pages);assert len(got)==len(names) and set(got)==set(names)
  step('list_pagination_files',iteration=n,count=len(got),seconds=time.monotonic()-t)
  time.sleep(4)
 (root/'payload.bin').rename(root/'renamed.bin')
 with (root/'renamed.bin').open('r+b') as f:f.truncate(8*1024*1024);f.flush();os.fsync(f.fileno())
 assert (root/'renamed.bin').stat().st_size==8*1024*1024
 step('rename_truncate',sha256=hashlib.sha256(payload[:8*1024*1024]).hexdigest())
 step('ready_for_guest_verification')
except Exception as e:
 step('failed',error=repr(e));raise
