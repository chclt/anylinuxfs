from pathlib import Path
import json,hashlib,time,sys
b=Path('/opt/upstream');out=b/'runs'/sys.argv[1];m=json.loads((b/'finder-1m-expected-final.json').read_text());root=Path('/mnt/upstream/Internal storage')/m['directory'];results={}
names=sorted(x.name for x in root.iterdir());(out/'fresh-directory.json').write_text(json.dumps(names));assert '待删除-测试.bin' not in names and 'overwrite copy.bin' not in names
for name,expected in m['files'].items():
 p=root/name;t=time.monotonic()
 with p.open('rb',buffering=0) as f:h=hashlib.file_digest(f,'sha256').hexdigest()
 actual={'size':p.stat().st_size,'sha256':h,'seconds':time.monotonic()-t};actual['matches']=actual['size']==expected['size'] and h==expected['sha256'];results[name]=actual
 (out/'fresh-hashes.json').write_text(json.dumps(results,indent=2));print(name,json.dumps(actual),flush=True)
 assert actual['matches'],name
print('ALL VERIFIED',flush=True)
