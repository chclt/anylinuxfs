from pathlib import Path
import subprocess,sys,json,time
from usb_attach import attach
out=Path('/opt/upstream/runs/refresh-protocol-01');out.mkdir(exist_ok=False);vh=None
try:
 vh,port=attach(sys.argv[1])
 with (out/'protocol.log').open('w') as f:
  p=subprocess.run(['/opt/upstream/refresh-probe',str(out/'finish')],stdout=f,stderr=f,timeout=1000)
 (out/'result.json').write_text(json.dumps({'exit':p.returncode,'finished':time.time()}));print((out/'result.json').read_text(),flush=True)
finally:
 if vh is not None:(vh/'detach').write_text(str(port))
