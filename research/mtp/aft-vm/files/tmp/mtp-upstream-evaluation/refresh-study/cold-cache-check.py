from pathlib import Path
import json,time,os,hashlib
base=Path('/Users/jike/Developer/chclt/anylinuxfs/tmp/mtp-upstream-evaluation')
run=base/'image/rootfs/opt/upstream/runs/aft-refresh-02'
root=base/'mount/Internal storage/alfs-refresh-regression-20261117'
expected='00495905420ae650521c75cee0c7bfe7ae8e235fc26ec14e4ce660659845823e'
result={'started':time.time(),'rounds':[]}
def rpc(code):
 response=run/'response.json'
 if response.exists():response.unlink()
 request=run/'request.json';request.write_text(json.dumps({'argv':['python3','-c',code],'timeout':60}))
 deadline=time.monotonic()+70
 while not response.exists():
  if time.monotonic()>deadline:raise TimeoutError('guest command')
  time.sleep(.1)
 r=json.loads(response.read_text())
 if r.get('returncode')!=0:raise RuntimeError(r)
 return r
try:
 initial=os.listdir(root/'pages');assert len(initial)==len(set(initial));(root/'payload.bin').stat()
 for n in range(2):
  rpc("import os; from pathlib import Path; os.sync(); Path('/proc/sys/vm/drop_caches').write_text('3'); print('caches dropped')")
  t=time.monotonic();names=os.listdir(root/'pages');assert set(names)==set(initial) and len(names)==len(initial)
  st=(root/'payload.bin').stat();assert st.st_size==32*1024*1024
  result['rounds'].append({'round':n,'seconds':time.monotonic()-t,'entries':len(names),'size':st.st_size})
 r=rpc("import hashlib,json; from pathlib import Path; p=Path('/mnt/upstream/Internal storage/alfs-refresh-regression-20261117/payload.bin'); print(json.dumps({'sha256':hashlib.sha256(p.read_bytes()).hexdigest()}))")
 actual=json.loads(r['stdout'])['sha256'];assert actual==expected;result['guest_sha256']=actual
 result['status']='passed'
except Exception as e:result['status']='failed';result['error']=repr(e);raise
finally:
 (base/'refresh-study/cold-cache-result.json').write_text(json.dumps(result,indent=2));print(json.dumps(result),flush=True)
