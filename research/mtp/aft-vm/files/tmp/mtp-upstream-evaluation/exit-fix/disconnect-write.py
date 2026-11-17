from pathlib import Path
import os,json,time
b=Path(__file__).resolve().parent;root=Path((b/'test-path.txt').read_text());source=b/'source-32MiB.bin';block=source.open('rb').read(1024*1024)
start=time.monotonic();events=[];total=0;fd=None
try:
 for cycle in range(16):
  if time.monotonic()-start>180:break
  fd=os.open(root/'disconnect-write.bin',os.O_WRONLY|os.O_CREAT|os.O_TRUNC,0o600)
  for n in range(128):
   begin=time.time();size=os.write(fd,block);total+=size
   if size!=len(block):raise RuntimeError('short write '+str(size))
   row={'time':time.time(),'cycle':cycle,'offset':(n+1)*len(block),'total_acknowledged':total,'seconds':time.time()-begin}
   events.append(row);(b/'disconnect-progress.json').write_text(json.dumps(row))
  os.close(fd);fd=None
 result={'unexpected_success':True,'seconds':time.monotonic()-start,'total_acknowledged':total}
except Exception as e:
 result={'error':str(e),'errno':getattr(e,'errno',None),'seconds':time.monotonic()-start,'total_acknowledged':total,'time':time.time()}
finally:
 if fd is not None:
  try:os.close(fd)
  except OSError as e:print('close:',e,flush=True)
 (b/'disconnect-result.json').write_text(json.dumps(result,indent=2));(b/'disconnect-writes.json').write_text(json.dumps(events));print(result,flush=True)
