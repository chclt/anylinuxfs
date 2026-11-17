from pathlib import Path
import json,sys
b=Path(__file__).resolve().parent;run=sys.argv[1];r=b/'image/rootfs/opt/upstream/runs'/run;(r/'stop-monitor').touch()
code="import pathlib,json,os,signal,sys; p=pathlib.Path(sys.argv[1]); os.kill(json.loads((p/'observer-pids.json').read_text())['tcpdump'],signal.SIGINT); print('capture stopped')"
(r/'request.json').write_text(json.dumps({'argv':['python3','-c',code,'/opt/upstream/runs/'+run],'timeout':5}))
