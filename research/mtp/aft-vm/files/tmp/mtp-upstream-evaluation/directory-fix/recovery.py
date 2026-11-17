from pathlib import Path
import hashlib,json,shutil,time
b=Path(__file__).resolve().parent;m=json.loads((b/'expected.json').read_text());root=Path(m.pop('root'));rows=[]
def save(r):
 rows.append(r);(b/'recovery.json').write_text(json.dumps(rows,indent=2));print(r,flush=True)
def sha(p):
 h=hashlib.sha256()
 with p.open('rb') as f:
  for chunk in iter(lambda:f.read(1024*1024),b''):h.update(chunk)
 return h.hexdigest()
for name,expected in m.items():
 p=root/name;t=time.monotonic();actual={'size':p.stat().st_size,'sha256':sha(p)};save({'file':name,**actual,'matches':actual==expected,'seconds':time.monotonic()-t});assert actual==expected
partial=root/'disconnect-write.bin';size=partial.stat().st_size;assert size<=128*1024*1024
block=(b/'source-32MiB.bin').open('rb').read(1024*1024);matches=True;offset=0;first_mismatch=None
acknowledged=json.loads((b/'disconnect-progress.json').read_text())['offset']
with partial.open('rb') as f:
 while True:
  chunk=f.read(len(block))
  if not chunk:break
  if chunk!=block[:len(chunk)]:
   matches=False
   if first_mismatch is None:first_mismatch=offset+next(i for i,(a,c) in enumerate(zip(chunk,block)) if a!=c)
  offset+=len(chunk)
save({'file':partial.name,'size':size,'every_byte_matches_source_pattern':matches,'acknowledged_before_disconnect':acknowledged,'first_mismatch_offset':first_mismatch,'acknowledged_prefix_matches':size>=acknowledged and (first_mismatch is None or first_mismatch>=acknowledged)});assert size>=acknowledged and (first_mismatch is None or first_mismatch>=acknowledged)
new=root/'reconnected.bin';new.write_bytes(block*8);assert sha(new)==hashlib.sha256(block*8).hexdigest();save({'step':'new write after reconnect','bytes':8*len(block),'matches':True})
allowed=set(m)|{'disconnect-write.bin','reconnected.bin'}
for file in list(allowed):
 p=root/file
 if p.exists():p.unlink()
 sibling=p.with_name('._'+p.name)
 if sibling.exists():sibling.unlink()
(root/'sub').rmdir()
if (root/'._sub').exists():(root/'._sub').unlink()
root.rmdir();save({'step':'cleanup','root_absent':not root.exists()})
