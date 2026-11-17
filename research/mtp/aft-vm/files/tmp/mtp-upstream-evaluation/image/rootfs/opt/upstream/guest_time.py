from pathlib import Path
import json,os,signal,socket,subprocess,sys,time,traceback,urllib.request
from usb_attach import attach
os.environ['TZ']='/opt/upstream/time-study/host-zoneinfo'
time.tzset()
os.environ['PATH']='/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin'
candidate,bus,mode,run=sys.argv[1:]
base=Path('/opt/upstream');out=base/'runs'/run;out.mkdir(parents=True,exist_ok=False)
mount=Path('/mnt/upstream');mount.mkdir(exist_ok=True)
front=None;server=None;vh=None;port=None
args={'jmtpfs':['-f','-d','-o',mode+',allow_other,noforget'], 'simple-mtpfs':['-f','-o',mode+',allow_other,noforget'], 'go-mtpfs':['-allow-other'], 'aft-mtp-mount':['-f','-o',mode+',allow_other']}[candidate]
def event(stage,**extra):
 data={'time':time.time(),'stage':stage,**extra};print(json.dumps(data),flush=True)
 (out/'state.next').write_text(json.dumps(data));(out/'state.next').replace(out/'state.json')
def command(argv,timeout=30):return subprocess.run(argv,capture_output=True,text=True,timeout=timeout)
try:
 scratch=out/'disk-tmp';scratch.mkdir()
 subprocess.run(['cp','-a','/tmp/.',str(scratch)],check=True)
 subprocess.run(['mount','--bind',str(scratch),'/tmp'],check=True)
 (out/'tmp-storage.txt').write_text(command(['df','-h','/tmp']).stdout)
 vh,port=attach(bus);event('usb_attached')
 deadline=time.monotonic()+5
 while not list(Path('/sys/bus/usb/devices').glob('*/idVendor')) and time.monotonic()<deadline:time.sleep(.1)
 usb=[]
 for dev in Path('/sys/bus/usb/devices').iterdir():
  if (dev/'idVendor').exists():usb.append({name:(dev/name).read_text().strip() for name in ['idVendor','idProduct','busnum','devnum','bNumInterfaces'] if (dev/name).exists()})
 (out/'usb-devices.json').write_text(json.dumps(usb))
 cmd=[str(base/'build/aft-time-fix/fuse/aft-mtp-mount'),*args,str(mount)];(out/'driver-command.json').write_text(json.dumps(cmd))
 log=(out/'driver.log').open('w');front=subprocess.Popen(cmd,stdout=log,stderr=log)
 deadline=time.monotonic()+45
 while not os.path.ismount(mount):
  if front.poll() is not None:raise RuntimeError('driver exited: '+str(front.returncode))
  if time.monotonic()>deadline:raise TimeoutError('driver mount readiness')
  time.sleep(.1)
 p=command(['python3','-c',"import os,json; print(json.dumps([{'name':p.name,'stat':list(p.stat())} for p in os.scandir('/mnt/upstream')]))"]);(out/'guest-root.txt').write_text(p.stdout+p.stderr)
 (out/'guest-root-result.json').write_text(json.dumps({'returncode':p.returncode,'stdout':p.stdout,'stderr':p.stderr}))
 if p.returncode:raise RuntimeError('guest root listing failed: '+str(p.returncode))
 # Same generic export options as --ignore-permissions, with the FUSE fsid.
 Path('/tmp/exports').write_text('"/mnt/upstream" *('+mode+',no_subtree_check,all_squash,anonuid=0,anongid=0,insecure,fsid=0)\n')
 env=os.environ.copy();env['NFS_SERVER_THREAD_COUNT']=os.environ.get('ALFS_EVAL_NFS_THREADS','1')
 (out/'nfs-threads-requested.txt').write_text(env['NFS_SERVER_THREAD_COUNT'])
 serverlog=(out/'nfs.log').open('w');server=subprocess.Popen(['/usr/local/bin/entrypoint.sh'],stdout=serverlog,stderr=serverlog,env=env)
 deadline=time.monotonic()+30
 while True:
  if server.poll() is not None:raise RuntimeError('NFS startup failed')
  try:
   with socket.create_connection(('127.0.0.1',2049),timeout=.5):break
  except OSError:
   if time.monotonic()>deadline:raise TimeoutError('NFS readiness')
   time.sleep(.2)
 # Match the validated 1 MiB NFS configuration before exposing it to macOS.
 subprocess.run(['rpc.nfsd','0'],check=True)
 Path('/proc/fs/nfsd/max_block_size').write_text('1048576')
 subprocess.run(['rpc.nfsd','4'],check=True)
 for local,remote in [(12049,2049),(12050,32767)]:
  data=json.dumps({'local':f'127.0.0.1:{local}','remote':f'192.168.127.2:{remote}'}).encode()
  req=urllib.request.Request('http://192.168.127.1/services/forwarder/expose',data=data,headers={'Content-Type':'application/json'})
  urllib.request.urlopen(req,timeout=10).read()
 (out/'exports.txt').write_text(command(['exportfs','-v']).stdout)
 event('ready',candidate=candidate,mode=mode)
 while not (out/'finish').exists():
  if front.poll() is not None:raise RuntimeError('driver exited during test')
  request=out/'request.json'
  if request.exists():
   spec=json.loads(request.read_text());request.unlink()
   try:
    t=time.monotonic();p=command(spec['argv'],spec.get('timeout',30));result={'returncode':p.returncode,'stdout':p.stdout,'stderr':p.stderr,'seconds':time.monotonic()-t}
   except Exception as e:result={'error':str(e)}
   (out/'response.json').write_text(json.dumps(result))
  time.sleep(.2)
 server.send_signal(signal.SIGTERM);server.wait(timeout=15);server=None
 p=command(['umount',str(mount)])
 if p.returncode:raise RuntimeError('FUSE unmount failed: '+p.stderr)
 front.wait(timeout=30);event('complete',driver_exit=front.returncode)
except Exception as e:
 event('failed',error=str(e));traceback.print_exc()
finally:
 if server is not None and server.poll() is None:
  server.terminate()
  try:server.wait(timeout=10)
  except subprocess.TimeoutExpired:server.kill()
 if os.path.ismount(mount):subprocess.run(['umount','-l',str(mount)],timeout=10)
 if front is not None and front.poll() is None:
  front.terminate()
  try:front.wait(timeout=15)
  except subprocess.TimeoutExpired:front.kill();front.wait()
 if vh is not None:
  try:(vh/'detach').write_text(str(port))
  except OSError as e:print('USB detach:',e,flush=True)
