from pathlib import Path
import subprocess,json,time
from usb_attach import attach
out=Path('/opt/upstream/runs/aft-metadata-cli-01');out.mkdir();vh=None;port=None
try:
 vh,port=attach('2-1')
 commands=['ls /Android/data/org.videolan.vlc/files/medialib','test-property-list /Android/data/org.videolan.vlc/files/medialib','properties /ALFS-Finder-Test-20261112-140129/CHEN_5min_MIP04_Master_H264.mp4']
 (out/'commands.json').write_text(json.dumps(commands));t=time.monotonic()
 with (out/'cli.log').open('w') as log:
  p=subprocess.run(['/opt/upstream/build/aft/cli/aft-mtp-cli','-v',*commands],stdout=log,stderr=log,timeout=100)
 (out/'result.json').write_text(json.dumps({'returncode':p.returncode,'seconds':time.monotonic()-t}));print((out/'result.json').read_text(),flush=True)
finally:
 if vh is not None:(vh/'detach').write_text(str(port))
