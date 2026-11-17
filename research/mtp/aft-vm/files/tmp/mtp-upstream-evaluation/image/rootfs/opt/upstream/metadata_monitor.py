from pathlib import Path
import json,sys,time
out=Path(sys.argv[1]);target=Path('/mnt/upstream/Internal storage')/sys.argv[2]
with (out/'guest-metadata.jsonl').open('w',buffering=1) as f:
 while not (out/'stop-monitor').exists():
  start=time.time();t=time.monotonic()
  try:
   names=list(target.iterdir());r={'time':start,'seconds':time.monotonic()-t,'entries':len(names)}
  except Exception as e:r={'time':start,'seconds':time.monotonic()-t,'error':str(e)}
  f.write(json.dumps(r)+'\n');time.sleep(.5)
