"""Disposable read-only NFSv3 client-behavior probe; no phone or real files."""
import socket,struct,threading,time,json
from pathlib import Path
W=Path(__file__).resolve().parent
U=lambda n:struct.pack('!I',n)
Q=lambda n:struct.pack('!Q',n)
def opaque(b):return U(len(b))+b+b'\0'*((-len(b))%4)
class X:
 def __init__(self,b):self.b=b;self.i=0
 def u(self):n=struct.unpack_from('!I',self.b,self.i)[0];self.i+=4;return n
 def q(self):n=struct.unpack_from('!Q',self.b,self.i)[0];self.i+=8;return n
 def raw(self,n):b=self.b[self.i:self.i+n];self.i+=n;return b
 def op(self):n=self.u();b=self.raw(n);self.i+=(-n)%4;return b
 def fh(self):return int.from_bytes(self.op(),'big')
def fh(i):return opaque(Q(i))
children={1:[('ready.txt',2),('slow.txt',3),('slowdir',4)],4:[('child.txt',5)]}
def attr(i):
 directory=i in children
 return b''.join(map(U,[2 if directory else 1,0o555 if directory else 0o444,2 if directory else 1,502,20]))+Q(0 if directory else 4096)+Q(4096)+U(0)*2+Q(9002)+Q(i)+(U(1700000000)+U(0))*3
def post(i):return U(1)+attr(i)
def unavailable(proc,i):
 try:mode,deadline=(W/'control-host').read_text().split()
 except (OSError,ValueError):return False
 return {'read':6,'getattr':1,'readdir':17}.get(mode)==proc and i in (3,4) and time.time()<float(deadline)
loglock=threading.Lock()
def event(**v):
 with loglock:
  with (W/'server.jsonl').open('a') as f:f.write(json.dumps({'time':time.time(),**v})+'\n')
def nfs(proc,x):
 if proc==0:return b''
 i=x.fh()
 if i not in (1,2,3,4,5):return U(70)
 if unavailable(proc,i):
  event(proc=proc,inode=i,status=10008)
  return U(10008)+(b'' if proc==1 else post(i))
 if proc==1:return U(0)+attr(i)
 if proc==3:
  name=x.op().decode();found=dict(children.get(i,[])).get(name)
  return U(0)+fh(found)+post(found)+post(i) if found else U(2)+post(i)
 if proc==4:return U(0)+post(i)+U(x.u()&0x23)
 if proc==6:
  off=x.q();n=min(x.u(),max(0,4096-off));b=b'R'*n;event(proc=proc,inode=i,status=0)
  return U(0)+post(i)+U(n)+U(off+n>=4096)+opaque(b)
 if proc in (16,17):
  cookie=x.q();x.raw(8);x.u()
  if proc==17:x.u()
  result=U(0)+post(i)+b'\0'*8
  for c,(name,j) in enumerate(children.get(i,[]),1):
   if c<=cookie:continue
   result+=U(1)+Q(j)+opaque(name.encode())+Q(c)
   if proc==17:result+=post(j)+U(1)+fh(j)
  return result+U(0)+U(1)
 if proc==18:return U(0)+post(i)+Q(1024**3)*3+Q(10000)*3+U(0)
 if proc==19:return U(0)+post(i)+b''.join(U(n) for n in [32768,32768,4096,32768,32768,4096,32768])+Q(2**40)+U(0)+U(1)+U(8)
 if proc==20:return U(0)+post(i)+U(1)+U(255)+U(1)+U(1)+U(0)+U(1)
 event(unknown_proc=proc,inode=i)
 return U(30)+U(0)+post(i)
def recv(s,n):
 b=b''
 while len(b)<n:
  v=s.recv(n-len(b))
  if not v:raise EOFError
  b+=v
 return b
def client(s):
 try:
  while True:
   data=b''
   while True:
    marker=struct.unpack('!I',recv(s,4))[0];data+=recv(s,marker&0x7fffffff)
    if marker>>31:break
   x=X(data);xid=x.u();kind=x.u();rpc=x.u();prog=x.u();vers=x.u();proc=x.u();x.u();x.op();x.u();x.op()
   if prog==100005:
    if proc==1:x.op();body=U(0)+fh(1)+U(1)+U(1)
    else:body=b''
   elif prog==100003:body=nfs(proc,x)
   else:body=b''
   reply=U(xid)+U(1)+U(0)+U(0)+U(0)+U(0)+body
   s.sendall(U(0x80000000|len(reply))+reply)
 except EOFError:pass
 except Exception as e:event(error=repr(e))
 finally:s.close()
def serve(port):
 s=socket.socket();s.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1);s.bind(('127.0.0.1',port));s.listen()
 while True:
  c,a=s.accept();threading.Thread(target=client,args=(c,),daemon=True).start()
threading.Thread(target=serve,args=(12050,),daemon=True).start();serve(12049)
