from pathlib import Path
import json,hashlib,time,sys
base=Path('/opt/upstream');out=base/'runs'/sys.argv[1]
root=Path('/mnt/upstream/Internal storage/ALFS-Upstream-Speed-20261117')
expected=json.loads((base/'speed-expected.json').read_text());results=[]
for rel,meta in expected.items():
 p=root/rel;t=time.monotonic()
 with p.open('rb',buffering=0) as f:h=hashlib.file_digest(f,'sha256').hexdigest()
 actual={'path':rel,'size':p.stat().st_size,'sha256':h,'seconds':time.monotonic()-t}
 results.append(actual);(out/'speed-verify-progress.json').write_text(json.dumps(results,indent=2));print(json.dumps(actual),flush=True)
 if actual['size']!=meta['size'] or h!=meta['sha256']:raise RuntimeError('Readback mismatch: '+rel)
(out/'speed-verified.json').write_text(json.dumps(results,indent=2))
print('ALL VERIFIED',len(results),flush=True)
