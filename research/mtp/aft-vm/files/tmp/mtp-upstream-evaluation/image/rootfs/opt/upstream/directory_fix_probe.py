from pathlib import Path
import json,os,subprocess,sys,time
from usb_attach import attach
b=Path('/opt/upstream');out=b/'runs/directory-fix-02';out.mkdir(exist_ok=False)
mount=Path('/mnt/upstream');mount.mkdir(exist_ok=True)
results=[];vh=None
try:
 vh,port=attach(sys.argv[1])
 for label,driver in [('baseline',b/'bin/aft-mtp-mount'),('fixed',b/'build/aft-directory-fix/fuse/aft-mtp-mount')]:
  with (out/(label+'.log')).open('w') as log:
   front=subprocess.Popen([str(driver),'-f','-o','ro,allow_other',str(mount)],stdout=log,stderr=log)
   try:
    deadline=time.monotonic()+30
    while not os.path.ismount(mount):
     if front.poll() is not None:raise RuntimeError(label+' mount exited')
     if time.monotonic()>deadline:raise TimeoutError(label+' mount readiness')
     time.sleep(.1)
    target=mount/'Internal storage/Android/data/org.videolan.vlc/files/medialib'
    for attempt in range(3):
     t=time.monotonic()
     try:row={'entries':os.listdir(target),'errno':0}
     except OSError as e:row={'errno':e.errno,'error':str(e)}
     row.update(driver=label,attempt=attempt+1,seconds=time.monotonic()-t);results.append(row);print(json.dumps(row),flush=True)
    names=os.listdir(mount/'Internal storage');print(json.dumps({'driver':label,'healthy_storage_entries':len(names)}),flush=True)
   finally:
    if os.path.ismount(mount):subprocess.run(['umount',str(mount)],check=True,timeout=15)
    front.wait(timeout=30)
    print(json.dumps({'driver':label,'exit':front.returncode}),flush=True)
 (out/'results.json').write_text(json.dumps(results,indent=2))
 assert [r['errno'] for r in results[:3]]==[5,0,0],results
 assert all(r['errno']==5 for r in results[3:]),results
 print('DIRECTORY FIX PASS',flush=True)
finally:
 if vh is not None:(vh/'detach').write_text(str(port))
