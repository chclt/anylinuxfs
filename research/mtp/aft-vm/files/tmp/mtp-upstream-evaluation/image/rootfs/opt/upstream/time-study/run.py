from pathlib import Path
import os,sys,subprocess,json,time
sys.path.insert(0,'/opt/upstream')
from usb_attach import attach
base=Path('/opt/upstream');out=base/'runs/mtime-study-01';out.mkdir()
os.environ['TZ']='/opt/upstream/time-study/host-zoneinfo';time.tzset()
src=base/'sources/aft-time-fix';lib=base/'build/aft-time-fix/libmtp-ng-static.a';vh=None
try:
 for name in ['parser-probe','mtime-probe']:
  args=['/usr/bin/g++','-std=gnu++11','-pthread','-I'+str(src),'-I'+str(src/'mtp/backend/posix'),'-I'+str(src/'mtp/backend/linux'),str(base/'time-study'/f'{name}.cpp'),str(lib),'-lmagic','-o',str(base/'time-study'/name)]
  p=subprocess.run(args,capture_output=True,text=True,timeout=60);(out/f'{name}-build.json').write_text(json.dumps({'returncode':p.returncode,'stdout':p.stdout,'stderr':p.stderr}));p.check_returncode()
 cases=['20261117T180307','20261117T100307Z','20261117T180307+0800','20261117T050307-0500','20261117T100307.5Z','20260715T120000','20260115T120000']
 records=[]
 for tz in ['/opt/upstream/time-study/host-zoneinfo','/opt/upstream/time-study/new-york-zoneinfo']:
  p=subprocess.run([str(base/'time-study/parser-probe'),tz,*cases],capture_output=True,text=True,check=True);records.append({'zone':tz,'stdout':p.stdout})
 (out/'parser-results.json').write_text(json.dumps(records))
 vh,port=attach('2-1')
 with (out/'probe.log').open('w') as f:p=subprocess.run([str(base/'time-study/mtime-probe')],stdout=f,stderr=f,timeout=100)
 (out/'result.json').write_text(json.dumps({'returncode':p.returncode}));print((out/'result.json').read_text(),flush=True)
finally:
 if vh is not None:(vh/'detach').write_text(str(port))
