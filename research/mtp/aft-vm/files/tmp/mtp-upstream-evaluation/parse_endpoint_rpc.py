# Parse only captured RPC record headers; payload snaplen gaps are not reassembled.
from pathlib import Path
import struct,json,collections,sys
P=Path(sys.argv[1]);port=int(sys.argv[2]);R=Path(sys.argv[3]);R.mkdir(exist_ok=True)
b=P.read_bytes();formats={b'\xd4\xc3\xb2\xa1':('<',1e6),b'\xa1\xb2\xc3\xd4':('>',1e6),b'\x4d\x3c\xb2\xa1':('<',1e9),b'\xa1\xb2\x3c\x4d':('>',1e9)}
endian,resolution=formats[b[:4]];linktype=struct.unpack_from(endian+'I',b,20)[0];pos=24;calls={};replies={};packets=0
if linktype not in (0,1,12,101,108):raise ValueError('Unsupported capture link type '+str(linktype))
while pos+16<=len(b):
 sec,usec,cap,wire=struct.unpack_from(endian+'4I',b,pos);pos+=16
 if pos+cap>len(b):break
 p=b[pos:pos+cap];pos+=cap;packets+=1
 ip=14 if linktype==1 else (4 if linktype in (0,108) else 0)
 if len(p)<ip+20 or p[ip]>>4!=4 or p[ip+9]!=6:continue
 tcp=ip+(p[ip]&15)*4
 if len(p)<tcp+20:continue
 sport,dport=struct.unpack_from('>HH',p,tcp);h=tcp+(p[tcp+12]>>4)*4;data=p[h:];time=sec+usec/resolution
 for i in range(max(0,len(data)-31)):
  marker,xid,kind=struct.unpack_from('>3I',data,i)
  if not marker&0x80000000 or not 24<=marker&0x7fffffff<=1048576:continue
  if dport==port and kind==0:
   rpc,prog,ver,proc=struct.unpack_from('>4I',data,i+12)
   if (rpc,prog,ver)!=(2,100003,3) or proc>21:continue
   calls.setdefault(xid,{'time':time,'proc':proc,'sport':sport})
  elif sport==port and kind==1:
   rstat,vflav,vlen,accept=struct.unpack_from('>4I',data,i+12)
   if (rstat,vflav,vlen,accept)!=(0,0,0,0):continue
   replies.setdefault(xid,{'time':time,'status':struct.unpack_from('>I',data,i+28)[0]})
names={1:'getattr',3:'lookup',4:'access',6:'read',7:'write',16:'readdir',17:'readdirplus',21:'commit'}
pairs=[{'xid':xid,'op':names.get(x['proc'],str(x['proc'])),'start':x['time'],'end':replies[xid]['time'],'seconds':replies[xid]['time']-x['time'],'status':replies[xid]['status'],'client_port':x['sport']} for xid,x in calls.items() if xid in replies and replies[xid]['time']>=x['time']]
(R/'rpc-pairs.json').write_text(json.dumps(pairs))
summary={'method':'Captured record headers only; TCP payload gaps are not reassembled; unmatched calls are not classified as losses','capture_linktype':linktype,'port':port,'captured_packets':packets,'calls':len(calls),'paired':len(pairs),'op_counts':dict(collections.Counter(x['op'] for x in pairs)),'status_counts':dict(collections.Counter(x['status'] for x in pairs)),'longest_queries':sorted([x for x in pairs if x['op'] in ['getattr','read','readdirplus','readdir','access','lookup']],key=lambda x:x['seconds'],reverse=True)[:10],'longest_writes':sorted([x for x in pairs if x['op']=='write'],key=lambda x:x['seconds'],reverse=True)[:3]}
(R/'rpc-header-summary.json').write_text(json.dumps(summary,indent=2));print(json.dumps(summary,indent=2))
