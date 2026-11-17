from pathlib import Path
import subprocess,json,time,os
from usb_attach import attach
p=Path('/opt/upstream');out=p/'runs/aft-adoption-ro-01';out.mkdir();vh=None
try:
 src=p/'sources/android-file-transfer-linux-712dce6ea71793f6f8454e8ce75a0fd4ea553226'
 args=['/usr/bin/g++','-std=gnu++11','-pthread','-I'+str(src),'-I'+str(src/'mtp/backend/posix'),'-I'+str(src/'mtp/backend/linux'),str(p/'adoption_probe.cpp'),str(p/'build/aft/libmtp-ng-static.a'),'-lmagic','-o',str(p/'adoption-probe')]
 q=subprocess.run(args,capture_output=True,text=True,timeout=60);(out/'build.json').write_text(json.dumps({'returncode':q.returncode,'stdout':q.stdout,'stderr':q.stderr}));q.check_returncode()
 vh,port=attach('2-1');t=time.monotonic()
 with (out/'probe.log').open('w') as f:q=subprocess.run([str(p/'adoption-probe')],stdout=f,stderr=f,timeout=100)
 (out/'result.json').write_text(json.dumps({'returncode':q.returncode,'seconds':time.monotonic()-t}));print((out/'result.json').read_text(),flush=True)
 # The device has closed its protocol session. Verify the unchanged FUSE driver with only TZ set.
 env=os.environ.copy();env['TZ']='CST-8';mount=Path('/mnt/upstream');mount.mkdir(exist_ok=True)
 with (out/'fuse.log').open('w') as log:
  front=subprocess.Popen([str(p/'bin/aft-mtp-mount'),'-f','-o','ro,allow_other',str(mount)],env=env,stdout=log,stderr=log)
  try:
   deadline=time.monotonic()+30
   while not os.path.ismount(mount):
    if front.poll() is not None:raise RuntimeError('FUSE exited')
    if time.monotonic()>deadline:raise TimeoutError('FUSE mount')
    time.sleep(.1)
   f=mount/'Internal storage/ALFS-Finder-Test-20261112-140129/CHEN_5min_MIP04_Master_H264.mp4'
   st=f.stat();(out/'timezone-result.json').write_text(json.dumps({'TZ':'CST-8','mtime':st.st_mtime,'size':st.st_size,'matches_phone_stat':st.st_mtime==1794470392}))
  finally:
   if os.path.ismount(mount):subprocess.run(['umount',str(mount)],check=True,timeout=15)
   front.wait(timeout=30)
   (out/'fuse-exit.json').write_text(json.dumps({'returncode':front.returncode}))
finally:
 if vh is not None:(vh/'detach').write_text(str(port))
