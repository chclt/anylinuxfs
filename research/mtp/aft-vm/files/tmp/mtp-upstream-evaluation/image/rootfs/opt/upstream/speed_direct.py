from pathlib import Path
import os,time,json,threading
out=Path('/opt/upstream/runs/aft-speed-01');d=Path('/mnt/upstream/Internal storage/ALFS-Upstream-Speed-20261117');d.mkdir();results=[]
for size in [32768,1048576]:
 name='guest-'+str(size)+'.bin';stop=threading.Event();samples=[]
 def monitor():
  while not stop.is_set():
   t=time.monotonic();r={'time':time.time()}
   try:os.listdir(d)
   except Exception as e:r['error']=str(e)
   r['seconds']=time.monotonic()-t;samples.append(r);stop.wait(.5)
 th=threading.Thread(target=monitor);th.start();t=time.monotonic();n=0
 with Path('/opt/upstream/speed-source-128.bin').open('rb',buffering=0) as src,(d/name).open('xb',buffering=0) as dst:
  while data:=src.read(size):
   v=memoryview(data)
   while v:k=dst.write(v);n+=k;v=v[k:]
  os.fsync(dst.fileno())
 elapsed=time.monotonic()-t;stop.set();th.join();r={'name':name,'application_block':size,'bytes':n,'seconds':elapsed,'MiB_s':n/1048576/elapsed,'metadata_samples':samples};results.append(r);(out/'direct-results.json').write_text(json.dumps(results));print(json.dumps({k:v for k,v in r.items() if k!='metadata_samples'}),flush=True)
