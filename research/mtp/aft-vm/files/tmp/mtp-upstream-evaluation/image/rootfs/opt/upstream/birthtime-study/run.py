from pathlib import Path
import sys,subprocess,json,os,time
sys.path.insert(0,'/opt/upstream')
from usb_attach import attach
b=Path('/opt/upstream');out=b/'runs/birthtime-protocol-01';out.mkdir();vh=None
os.environ['TZ']='/opt/upstream/time-study/host-zoneinfo';time.tzset()
try:
 src=b/'sources/aft-time-fix'
 args=['/usr/bin/g++','-std=gnu++11','-pthread','-I'+str(src),'-I'+str(src/'mtp/backend/posix'),'-I'+str(src/'mtp/backend/linux'),str(b/'birthtime-study/probe.cpp'),str(b/'build/aft-time-fix/libmtp-ng-static.a'),'-lmagic','-o',str(b/'birthtime-study/probe')]
 p=subprocess.run(args,capture_output=True,text=True,timeout=60);(out/'build.json').write_text(json.dumps({'returncode':p.returncode,'stderr':p.stderr}));p.check_returncode()
 vh,port=attach('2-1')
 with (out/'probe.log').open('w') as f:p=subprocess.run([str(b/'birthtime-study/probe')],stdout=f,stderr=f,timeout=60)
 (out/'result.json').write_text(json.dumps({'returncode':p.returncode}));p.check_returncode()
finally:
 if vh is not None:(vh/'detach').write_text(str(port))
