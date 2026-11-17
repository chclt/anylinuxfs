from pathlib import Path
import hashlib,json,os,shutil,subprocess,time
b=Path(__file__).resolve().parent;root=b.parent/'mount'/'Internal storage'/time.strftime('ALFS-AFT-Regression-%Y%m%d-%H%M%S');root.mkdir();(b/'test-path.txt').write_text(str(root))
records=[]
def save(label,**kw):
 r={'step':label,**kw};records.append(r);(b/'operations.json').write_text(json.dumps(records,indent=2));print(r,flush=True)
def sha(p):
 h=hashlib.sha256()
 with p.open('rb') as f:
  for v in iter(lambda:f.read(1024*1024),b''):h.update(v)
 return h.hexdigest()
source=b/'source-32MiB.bin'
with source.open('wb') as f:
 block=os.urandom(1024*1024)
 for i in range(32):f.write(block)
expected=sha(source);start=time.monotonic();shutil.copyfile(source,root/'complete.bin');assert sha(root/'complete.bin')==expected;save('32 MiB complete readback',seconds=time.monotonic()-start,sha256=expected)
(root/'sub').mkdir();s=root/'move.bin';s.write_bytes(b'cross directory payload\n'*10000);h=sha(s);t=root/'sub'/'中文 移动.bin';move_errno=0
try:s.rename(t)
except OSError as e:
 move_errno=e.errno
 if move_errno!=18:raise
 shutil.move(s,t)
assert not s.exists() and sha(t)==h;save('cross-directory move',rename_errno=move_errno,copy_delete_fallback=move_errno==18,sha256=h)
s=root/'overwrite.bin';s.write_bytes(b'old'*500000);shutil.copyfile(source,s);assert sha(s)==expected;s.write_bytes(b'short replacement');assert s.read_bytes()==b'short replacement';save('overwrite grow and shrink',passed=True)
video=Path('/Users/jike/Downloads/CHEN_5min_MIP04_Master_H264.mp4');dest=root/'cancelled.mp4';start=time.monotonic();p=subprocess.Popen(['/bin/cp',str(video),str(dest)],stdout=subprocess.DEVNULL,stderr=(b/'cancel-stderr.log').open('w'));deadline=start+20;seen=0
while p.poll() is None and time.monotonic()<deadline:
 try:seen=dest.stat().st_size
 except FileNotFoundError:pass
 if seen>=8*1024*1024:break
 time.sleep(.1)
was_running=p.poll() is None
if was_running:p.terminate()
rc=p.wait(timeout=30);size=dest.stat().st_size
with video.open('rb') as f:prefix=hashlib.sha256(f.read(size)).hexdigest()
actual=sha(dest);save('cancel command-line copy',sent_SIGTERM=was_running,exit=rc,seconds=time.monotonic()-start,partial_size=size,source_size=video.stat().st_size,prefix_matches=actual==prefix,sha256=actual)
assert was_running and rc!=0 and 0<size<video.stat().st_size and actual==prefix
manifest={'root':str(root),'complete.bin':{'size':source.stat().st_size,'sha256':expected},'sub/中文 移动.bin':{'size':t.stat().st_size,'sha256':h},'overwrite.bin':{'size':s.stat().st_size,'sha256':sha(s)},'cancelled.mp4':{'size':size,'sha256':actual}}
(b/'expected.json').write_text(json.dumps(manifest,indent=2));save('ready for disconnect',passed=True)
