from pathlib import Path
import subprocess,time,json,os
w=Path(__file__).resolve().parent;m=w/'mount';c=Path('/Users/jike/.anylinuxfs/mtp-cache/VMG1Omju/retry-probe/control')
options='vers=3,tcp,port=2049,mountport=32767,soft,intr,locallocks,timeo=600,retrans=100,deadtimeout=0,nfc,actimeo=0'
results=[]
for mode,seconds in [('read',35),('open',15),('getattr',15),('readdir',15),('read',75)]:
 subprocess.run(['diskutil','unmount',str(m)],check=True,stdout=subprocess.DEVNULL)
 subprocess.run(['mount','-t','nfs','-o',options,'127.0.0.1:/tmp/retry-probe/mnt',str(m)],check=True)
 t=time.time();c.write_text(f'{mode} {t+seconds}\n'); case=f'{mode}-{seconds}';print('START',case,t,flush=True)
 code='import os,time,json; from pathlib import Path; t=time.time(); p=Path('+repr(str(m))+'); '
 if mode=='getattr':code+='result=(p/"slow.txt").stat().st_size'
 elif mode=='readdir':code+='result=os.listdir(p/"slowdir")'
 elif mode=='open':code+='fd=os.open(p/"slow.txt",os.O_RDONLY); os.close(fd); result="opened"'
 else:code+='result=len((p/"slow.txt").read_bytes())'
 code+='; print(json.dumps({"elapsed":time.time()-t,"result":result}))'
 p=subprocess.Popen(['python3','-c',code],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
 samples=[]
 while p.poll() is None and time.time()-t<130:
  s=time.time()
  try:
   assert (m/'ready.txt').read_bytes()==b'R'*4096
   v=(m/'ready.txt').stat();err=None
  except Exception as e:err=str(e)
  stat=subprocess.run(['nfsstat','-m'],capture_output=True,text=True).stdout
  samples.append({'time':s,'ready_seconds':time.time()-s,'error':err,'nfs':stat})
  time.sleep(.5)
 c.write_text('none 0\n')
 try:out,err=p.communicate(timeout=40)
 except subprocess.TimeoutExpired:
  p.kill();out,err=p.communicate();err+=' probe deadline exceeded'
 result={'case':case,'start':t,'end':time.time(),'exit_code':p.returncode,'stdout':out,'stderr':err,'samples':samples}
 results.append(result);(w/'results.json').write_text(json.dumps(results,indent=2))
 print('RESULT',case,p.returncode,out.strip(),err.strip(),'max_ready',max(x['ready_seconds'] for x in samples),flush=True)
