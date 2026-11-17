# Disposable full-capture TCP/RPC analysis. Original captures remain unchanged.
from pathlib import Path
import struct,json,collections,bisect,sys
p=Path(sys.argv[1]);port=int(sys.argv[2]);out=Path(sys.argv[3]);out.mkdir(exist_ok=True)
b=p.read_bytes();formats={b'\xd4\xc3\xb2\xa1':('<',1e6),b'\xa1\xb2\xc3\xd4':('>',1e6),b'\x4d\x3c\xb2\xa1':('<',1e9),b'\xa1\xb2\x3c\x4d':('>',1e9)}
endian,resolution=formats[b[:4]];link=struct.unpack_from(endian+'I',b,20)[0]
if link not in (0,1,12,101,108):raise ValueError(('unsupported link',link))
pos=24;flows={};packets=0;truncated=0
while pos+16<=len(b):
 sec,frac,cap,wire=struct.unpack_from(endian+'4I',b,pos);pos+=16
 if pos+cap>len(b):raise ValueError('Incomplete pcap packet')
 packet=b[pos:pos+cap];pos+=cap;packets+=1
 ip=14 if link==1 else (4 if link in (0,108) else 0)
 if len(packet)<ip+20 or packet[ip]>>4!=4 or packet[ip+9]!=6:continue
 tcp=ip+(packet[ip]&15)*4
 if len(packet)<tcp+20:continue
 sport,dport,seq=struct.unpack_from('>HHI',packet,tcp)
 if port not in (sport,dport):continue
 total=struct.unpack_from('>H',packet,ip+2)[0];head=tcp+(packet[tcp+12]>>4)*4;data=packet[head:ip+total];flags=packet[tcp+13]
 f=flows.setdefault((sport,dport),{'base':None,'segments':[]})
 if flags&2:
  new=(seq+1)&0xffffffff
  if f['base'] is not None and f['base']!=new:raise ValueError('TCP tuple reused within capture')
  f['base']=new
 if data:
  if len(data)<ip+total-head:truncated+=1
  f['segments'].append((seq,sec+frac/resolution,data))
del b
calls={};replies={};stats=[]
for key,f in flows.items():
 if not f['segments']:continue
 if f['base'] is None:raise ValueError(('Missing TCP handshake',key))
 segs=sorted([((seq-f['base'])&0xffffffff,t,d) for seq,t,d in f['segments']],key=lambda v:(v[0],v[1]));stream=bytearray();ends=[];times=[];gap=None
 for at,t,d in segs:
  if at>len(stream):gap={'expected':len(stream),'next':at};break
  skip=len(stream)-at
  if skip<len(d):stream.extend(d[skip:]);ends.append(len(stream));times.append(t)
 cur=0;records=0;incomplete=False
 while cur+4<=len(stream):
  start=cur;payload=bytearray()
  while True:
   if cur+4>len(stream):incomplete=True;break
   marker=struct.unpack_from('>I',stream,cur)[0];n=marker&0x7fffffff
   if n>4194304:raise ValueError(('Invalid RPC fragment length',key,cur,n))
   if cur+4+n>len(stream):incomplete=True;break
   payload.extend(stream[cur+4:cur+4+n]);cur+=4+n
   if marker&0x80000000:break
  if incomplete:break
  if len(payload)<24:raise ValueError(('Short RPC record',key,start))
  first=bisect.bisect_right(ends,start);last=bisect.bisect_right(ends,cur-1);xid,kind=struct.unpack_from('>II',payload,0)
  row={'xid':xid,'record_start':start,'record_end':cur,'arrival':times[first],'ready':max(times[first:last+1]),'client_port':key[0] if key[1]==port else key[1]};records+=1
  if kind==0:
   rpc,prog,ver,proc=struct.unpack_from('>4I',payload,8)
   if (rpc,prog,ver)!=(2,100003,3):raise ValueError(('Unexpected RPC program',rpc,prog,ver))
   row['proc']=proc
   credlen=struct.unpack_from('>I',payload,28)[0];at=32+((credlen+3)//4)*4;vlen=struct.unpack_from('>I',payload,at+4)[0];at+=8+((vlen+3)//4)*4
   if proc in (1,2,3,4,6,7,8,9,12,13,16,17,18,19,20,21):
    flen=struct.unpack_from('>I',payload,at)[0];row['fh']=payload[at+4:at+4+flen].hex();at+=4+((flen+3)//4)*4
    if proc in (3,8,9):
     nlen=struct.unpack_from('>I',payload,at)[0];name=bytes(payload[at+4:at+4+nlen])
     if name.startswith(b'ALFS-Upstream-') or name in (b'sample-128MiB.bin',b'._sample-128MiB.bin'):row['test_name']=name.decode()
    if proc==7:row['offset'],row['write_count']=struct.unpack_from('>QI',payload,at)
   calls.setdefault(xid,row)
  elif kind==1:
   rstat=struct.unpack_from('>I',payload,8)[0];row['rpc_reply_status']=rstat
   if rstat==0:
    vlen=struct.unpack_from('>I',payload,16)[0];at=20+((vlen+3)//4)*4;accept=struct.unpack_from('>I',payload,at)[0];row['rpc_accept_status']=accept
    if accept==0 and len(payload)>=at+8:
     row['nfs_status']=struct.unpack_from('>I',payload,at+4)[0];proc=calls.get(xid,{}).get('proc');cursor=at+8
     if row['nfs_status']==0 and proc in (3,8,9):
      present=True
      if proc in (8,9):present=bool(struct.unpack_from('>I',payload,cursor)[0]);cursor+=4
      if present:
       flen=struct.unpack_from('>I',payload,cursor)[0];row['object_fh']=payload[cursor+4:cursor+4+flen].hex()
   replies.setdefault(xid,row)
  else:raise ValueError(('Unexpected RPC message kind',kind))
 stats.append({'ports':key,'stream_bytes':len(stream),'records':records,'gap':gap,'incomplete_tail':incomplete})
names={1:'getattr',2:'setattr',3:'lookup',4:'access',6:'read',7:'write',8:'create',16:'readdir',17:'readdirplus',18:'fsstat',21:'commit'}
pairs=[{**r,'op':names.get(r['proc'],str(r['proc'])),'reply_arrival':replies[xid]['arrival'],'reply_ready':replies[xid]['ready'],'seconds':replies[xid]['ready']-r['ready'],'first_byte_seconds':replies[xid]['ready']-r['arrival'],'status':replies[xid].get('nfs_status'),'object_fh':replies[xid].get('object_fh')} for xid,r in calls.items() if xid in replies]
(out/'rpc-pairs.json').write_text(json.dumps(pairs));summary={'method':'TCP stream reassembly from SYN; complete RPC record parsing; timings use last request and reply byte','packets':packets,'truncated_data_packets':truncated,'flows':stats,'calls':len(calls),'replies':len(replies),'paired':len(pairs),'unmatched_calls':len(calls.keys()-replies.keys()),'status_counts':dict(collections.Counter(str(r['status']) for r in pairs)),'op_counts':dict(collections.Counter(r['op'] for r in pairs))}
(out/'summary.json').write_text(json.dumps(summary,indent=2));print(json.dumps(summary,indent=2))
