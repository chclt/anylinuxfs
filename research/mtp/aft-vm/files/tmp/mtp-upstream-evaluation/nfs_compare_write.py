from pathlib import Path
import hashlib,json,os,threading,time,subprocess,sys
b=Path(__file__).resolve().parent;run=sys.argv[1];m=json.loads((b/(run+'-manifest.json')).read_text());d=b/'mount'/'Internal storage'/m['directory'];stop=threading.Event();metadata=[];health=[]
def monitor():
 while not stop.is_set():
  t=time.monotonic();started=time.time()
  try:list(d.iterdir());metadata.append({'time':started,'seconds':time.monotonic()-t})
  except Exception as e:metadata.append({'time':started,'seconds':time.monotonic()-t,'error':str(e)})
  stop.wait(.5)
def nfs():
 while not stop.wait(1):
  try:
   p=subprocess.run(['nfsstat','-m'],capture_output=True,text=True,timeout=5);health.append({'time':time.time(),'not_responding':'not responding' in p.stdout})
  except Exception as e:health.append({'time':time.time(),'error':str(e)})
threads=[threading.Thread(target=f,daemon=True) for f in [monitor,nfs]]
for th in threads:th.start()
r={'start':time.time(),'bytes_accepted':0,'name':'large-CHEN.mp4'};t=time.monotonic()
def save():
 (b/(run+'-progress.json')).write_text(json.dumps(r,indent=2))
try:
 h=hashlib.sha256()
 with Path('/Users/jike/Downloads/CHEN_5min_MIP04_Master_H264.mp4').open('rb') as src,(d/r['name']).open('xb',buffering=0) as dst:
  while data:=src.read(1048576):
   h.update(data);v=memoryview(data)
   while v:n=dst.write(v);r['bytes_accepted']+=n;v=v[n:]
   r['elapsed_seconds']=time.monotonic()-t;save()
  r['source_sha256']=h.hexdigest();r['write_seconds']=time.monotonic()-t;save();sync=time.monotonic();os.fsync(dst.fileno());r['fsync_seconds']=time.monotonic()-sync;r['fsync_passed']=True
 r['total_seconds']=time.monotonic()-t;r['success']=True
except Exception as e:r.update(success=False,error=repr(e),total_seconds=time.monotonic()-t)
finally:
 stop.set()
 for th in threads:th.join(timeout=2)
 save();r['metadata_samples']=metadata;r['nfs_samples']=health;(b/(run+'-result.json')).write_text(json.dumps(r,indent=2));print(json.dumps({k:v for k,v in r.items() if k not in ['metadata_samples','nfs_samples']},indent=2),flush=True);print('metadata max',max((x['seconds'] for x in metadata),default=0),'metadata errors',sum('error' in x for x in metadata),'not responding samples',sum(x.get('not_responding',False) for x in health),flush=True)
