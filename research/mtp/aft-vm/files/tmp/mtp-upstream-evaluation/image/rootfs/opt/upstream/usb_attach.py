from pathlib import Path
import socket,struct,time
def attach(bus):
 s=socket.socket(socket.AF_VSOCK,socket.SOCK_STREAM);s.connect((2,12701));s.sendall(struct.pack('!HHI',0x111,0x8003,0)+bus.encode().ljust(32,b'\0'))
 def read(n):
  b=b''
  while len(b)<n:
   d=s.recv(n-len(b))
   if not d:raise RuntimeError('USB import EOF')
   b+=d
  return b
 assert struct.unpack('!HHI',read(8))==(0x111,3,0)
 meta=read(312);busnum,devnum,speed=struct.unpack('!III',meta[288:300]);vh=Path('/sys/devices/platform/vhci_hcd.0')
 hub='hs' if speed<=3 else 'ss'
 port=next(int(x.split()[1]) for x in (vh/'status').read_text().splitlines()[1:] if x.split()[0]==hub and int(x.split()[2])==4)
 (vh/'attach').write_text(f'{port} {s.fileno()} {(busnum<<16)|devnum} {speed}');s.close()
 for _ in range(300):
  rows=(vh/'status').read_text().splitlines()[1:]
  if any(int(x.split()[1])==port and int(x.split()[2])==6 for x in rows):time.sleep(.5);return vh,port
  time.sleep(.05)
 raise RuntimeError('USB import timeout')

