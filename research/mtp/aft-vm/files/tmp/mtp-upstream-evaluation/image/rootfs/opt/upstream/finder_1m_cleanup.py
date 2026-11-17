from pathlib import Path
import json,sys
b=Path('/opt/upstream');out=b/'runs'/sys.argv[1];m=json.loads((b/'finder-1m-expected-final.json').read_text());verified=json.loads((out/'fresh-hashes.json').read_text());assert set(verified)==set(m['files']) and all(x['matches'] for x in verified.values())
root=Path('/mnt/upstream/Internal storage')/m['directory'];allowed=set(m['files'])|{'._'+n for n in m['files']}|{'.DS_Store'}
items=list(root.iterdir());assert all(p.is_file() and p.name in allowed for p in items),[p.name for p in items]
for p in items:p.unlink()
root.rmdir();result={'deleted':[p.name for p in items],'directory':m['directory'],'absent':not root.exists()};(out/'cleanup.json').write_text(json.dumps(result,indent=2));print(json.dumps(result))
