from pathlib import Path
import subprocess,json,time
from usb_attach import attach
p=Path('/opt/upstream');out=p/'runs/aft-metadata-protocol-03';out.mkdir();vh=None
try:
 src=p/'sources/android-file-transfer-linux-712dce6ea71793f6f8454e8ce75a0fd4ea553226'
 args=['/usr/bin/g++','-std=gnu++11','-pthread','-I'+str(src),'-I'+str(src/'mtp/backend/posix'),'-I'+str(src/'mtp/backend/linux'),str(p/'metadata_probe.cpp'),str(p/'build/aft/libmtp-ng-static.a'),'-lmagic','-o',str(p/'metadata-probe')]
 q=subprocess.run(args,capture_output=True,text=True,timeout=60);(out/'build.json').write_text(json.dumps({'returncode':q.returncode,'stdout':q.stdout,'stderr':q.stderr}));q.check_returncode()
 vh,port=attach('2-1');t=time.monotonic()
 with (out/'probe.log').open('w') as f:q=subprocess.run([str(p/'metadata-probe')],stdout=f,stderr=f,timeout=100)
 (out/'result.json').write_text(json.dumps({'returncode':q.returncode,'seconds':time.monotonic()-t}));print((out/'result.json').read_text(),flush=True)
finally:
 if vh is not None:(vh/'detach').write_text(str(port))
