from pathlib import Path
import subprocess,json,os,time
b=Path(__file__).resolve().parent;mount=b/'mount'
for size in [262144,1048576,65536]:
 run='aft-speed-large-'+str(size)
 if os.path.ismount(mount):raise RuntimeError('Unexpected existing mount')
 opts=f'rw,sync,tcp,vers=3,port=12049,mountport=12050,locallocks,soft,intr,timeo=100,retrans=3,dumbtimer,rwsize={size}'
 subprocess.run(['/sbin/mount_nfs','-o',opts,'127.0.0.1:/mnt/upstream',str(mount)],check=True,timeout=30)
 try:
  m=subprocess.run(['nfsstat','-m'],capture_output=True,text=True,check=True);(b/(run+'-mount-options.txt')).write_text(m.stdout)
  (mount/'Internal storage/ALFS-Upstream-Speed-20261117'/('large-'+str(size))).mkdir()
  print('START',run,flush=True)
  with (b/(run+'-client.log')).open('w') as log:p=subprocess.run(['python3','-u',str(b/'nfs_size_write.py'),run],stdout=log,stderr=log,timeout=180)
  p.check_returncode();d=json.loads((b/(run+'-result.json')).read_text());print(json.dumps({k:v for k,v in d.items() if k not in ['metadata_samples','nfs_samples']}),flush=True)
  if not d['success']:raise RuntimeError('Copy failed')
 finally:subprocess.run(['diskutil','unmount',str(mount)],check=True,timeout=30)
