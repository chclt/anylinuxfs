from pathlib import Path
import json,sys,subprocess
b=Path(__file__).resolve().parent;run=sys.argv[1];m=json.loads((b/(run+'-manifest.json')).read_text());(b/'mount'/'Internal storage'/m['directory']).mkdir()
r=b/'image/rootfs/opt/upstream/runs'/run
code="""from pathlib import Path
import subprocess,json,sys
p=Path(sys.argv[1]);log=(p/'capture.log').open('w')
a=subprocess.Popen(['tcpdump','-i','eth0','-s','512','-U','-w',str(p/'nfs.pcap'),'tcp port 2049'],stdout=log,stderr=log)
b=subprocess.Popen(['python3','/opt/upstream/metadata_monitor.py',str(p),sys.argv[2]],stdout=log,stderr=log)
(p/'observer-pids.json').write_text(json.dumps({'tcpdump':a.pid,'metadata':b.pid}));print('observers started',a.pid,b.pid)
"""
(r/'request.json').write_text(json.dumps({'argv':['python3','-c',code,'/opt/upstream/runs/'+run,m['directory']],'timeout':5}))
p=subprocess.run(['nfsstat','-m'],capture_output=True,text=True,check=True);(b/(run+'-mount-options.txt')).write_text(p.stdout);print(p.stdout)
