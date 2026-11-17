from pathlib import Path
import json,sys
base=Path('/opt/upstream');out=base/'runs'/sys.argv[1]
expected=json.loads((base/'speed-expected.json').read_text())
verified=json.loads((out/'speed-verified.json').read_text())
assert {x['path']: {'size':x['size'],'sha256':x['sha256']} for x in verified}==expected
root=Path('/mnt/upstream/Internal storage/ALFS-Upstream-Speed-20261117')
allowed_files=set(expected)
allowed_dirs={str(Path(x).parent) for x in expected if str(Path(x).parent)!='.'}
for rel in list(expected)+list(allowed_dirs):
 p=Path(rel);allowed_files.add(str(p.with_name('._'+p.name)))
items=list(root.rglob('*'))
for p in items:
 rel=str(p.relative_to(root))
 if p.is_dir():assert rel in allowed_dirs,('unexpected directory',rel)
 else:assert rel in allowed_files,('unexpected file',rel)
for p in items:
 if not p.is_dir():p.unlink()
for p in sorted((p for p in items if p.is_dir()),key=lambda x:len(x.parts),reverse=True):p.rmdir()
root.rmdir()
(out/'speed-cleanup.json').write_text(json.dumps({'removed_root':str(root),'removed_entries':[str(p.relative_to(root)) for p in items],'root_absent':not root.exists()},indent=2))
print('CLEANUP COMPLETE',flush=True)
