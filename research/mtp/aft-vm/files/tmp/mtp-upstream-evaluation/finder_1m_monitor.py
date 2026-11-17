from pathlib import Path
import time,json,threading,subprocess,os
b=Path(__file__).resolve().parent;run='aft-finder-1m-01';p=b/'mount/Internal storage/ALFS-Upstream-Finder-1MiB-20261117';stop=b/(run+'-stop')
def listing():
 with (b/(run+'-host-metadata.jsonl')).open('w',buffering=1) as f:
  while not stop.exists():
   t=time.monotonic();r={'time':time.time()}
   try:r['names']=os.listdir(p)
   except Exception as e:r['error']=str(e)
   r['seconds']=time.monotonic()-t;f.write(json.dumps(r)+'\n');time.sleep(.5)
t=threading.Thread(target=listing,daemon=True);t.start()
with (b/(run+'-nfs.jsonl')).open('w',buffering=1) as f:
 while not stop.exists():
  r={'time':time.time()}
  try:
   q=subprocess.run(['nfsstat','-m'],capture_output=True,text=True,timeout=5);r['not_responding']='not responding' in q.stdout.lower();r['returncode']=q.returncode
  except Exception as e:r['error']=str(e)
  f.write(json.dumps(r)+'\n');time.sleep(1)
t.join(15)
