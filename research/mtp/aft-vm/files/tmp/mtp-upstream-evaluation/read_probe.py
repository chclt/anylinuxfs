from pathlib import Path
import hashlib,json,os,sys,threading,time,subprocess
b=Path(__file__).resolve().parent;run,storage=sys.argv[1:];m=b/'mount'/storage/'ALFS-Finder-Test-20261112-140129';p=m/'CHEN_5min_MIP04_Master_H264.mp4';stop=threading.Event();samples=[];health=[]
def monitor():
 while not stop.is_set():
  t=time.monotonic()
  try:list(m.iterdir());samples.append({'seconds':time.monotonic()-t})
  except Exception as e:samples.append({'seconds':time.monotonic()-t,'error':str(e)})
  stop.wait(.5)
def nfs():
 while not stop.wait(1):
  p=subprocess.run(['nfsstat','-m'],capture_output=True,text=True,timeout=5);health.append({'time':time.time(),'not_responding':'not responding' in p.stdout})
threads=[threading.Thread(target=x,daemon=True) for x in [monitor,nfs]]
for th in threads:th.start()
t=time.monotonic();result={'run':run,'start':time.time()}
try:
 with p.open('rb') as f,(b/(run+'-readback.mp4')).open('wb') as out:
  result['open_seconds']=time.monotonic()-t;data=f.read(1048576);result['first_read_seconds']=time.monotonic()-t;out.write(data);h=hashlib.sha256(data);size=len(data)
  while data:=f.read(1048576):h.update(data);out.write(data);size+=len(data)
 result.update(seconds=time.monotonic()-t,bytes=size,sha256=h.hexdigest());result['matches']=result['sha256']=='54891e473609cfa199cdee4f6b989a73ad185a73bc664a123600af6a9a2a6cef'
except Exception as e:result.update(error=str(e),seconds=time.monotonic()-t)
finally:
 stop.set()
 for th in threads:th.join(timeout=2)
 result['metadata_samples']=samples;result['nfs_samples']=health;(b/(run+'-read.json')).write_text(json.dumps(result,indent=2));print(json.dumps({k:v for k,v in result.items() if k not in ['metadata_samples','nfs_samples']},indent=2));print('metadata max seconds',max((x['seconds'] for x in samples),default=0),'errors',sum('error' in x for x in samples),'NFS not responding samples',sum(x['not_responding'] for x in health))
